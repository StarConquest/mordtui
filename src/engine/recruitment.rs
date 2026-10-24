//! Shared reinforcement calculation for the engine and the TUI's cost review.
use super::*;
use std::collections::BTreeMap;

pub(crate) struct Gear {
    pub item: Item,
    pub per_model: u32,
    pub included: u32,
    pub from_stash: u32,
    pub purchase: u32,
    pub cost: i64,
}
pub(crate) struct Quote {
    pub warrior: WarriorType,
    pub group: Option<Unit>,
    pub members: Vec<Member>,
    pub starting: BTreeMap<String, u32>,
    pub loadout: BTreeMap<String, u32>,
    pub gear: Vec<Gear>,
    pub hire_each: i64,
    pub hire_total: i64,
    pub equipment_total: i64,
    pub total: i64,
    pub veteran_required: u32,
    pub veteran_battle: Option<Battle>,
}

/// The current post-battle allowance remains usable after completion, until the next battle.
pub(crate) fn veteran_battle<'a>(db: &'a Db, w: &str) -> Option<&'a Battle> {
    db.battles
        .iter()
        .rev()
        .find(|b| b.warband_id == w && matches!(b.status.as_str(), "post-battle" | "complete"))
}
/// Older saves recorded the original roll in the outcome, but only the remainder on Battle.
pub(crate) fn veteran_spent(db: &Db, b: &Battle) -> u32 {
    let legacy_total = db
        .resolutions
        .iter()
        .find(|r| r.battle_id == b.id && r.key == "veterans")
        .and_then(|r| {
            r.description
                .strip_prefix("Available veteran experience: ")
                .and_then(|n| n.parse::<u32>().ok())
                .or_else(|| {
                    db.dice.iter().find(|d| d.id == r.dice_id).and_then(|d| {
                        d.results
                            .split(',')
                            .try_fold(0u32, |sum, n| sum.checked_add(n.parse().ok()?))
                    })
                })
        });
    b.veteran_spent
        .max(legacy_total.map_or(0, |total| total.saturating_sub(b.veteran_budget)))
}
pub(crate) fn quote(
    db: &Db,
    w: &str,
    kind: &str,
    group: Option<&str>,
    count: u32,
    price: Option<i64>,
) -> Result<Quote> {
    ensure!(count > 0 && count <= 1000, "Count must be 1..1000");
    let candidates: Vec<_> = db
        .types
        .iter()
        .filter(|t| t.profile_id == wb(db, w).profile_id)
        .cloned()
        .collect();
    let warrior = resolve(&candidates, kind)?.clone();
    let group = group.map(|q| unit(db, w, q)).transpose()?;
    if let Some(u) = &group {
        ensure!(
            u.type_id == warrior.id && u.role == "henchman",
            "Replacement must match a henchman group"
        );
    }
    let members: Vec<_> = db
        .members
        .iter()
        .filter(|m| group.as_ref().is_some_and(|u| m.unit_id == u.id) && active(m))
        .cloned()
        .collect();
    let member_loadout = |id: &str| {
        db.allocations
            .iter()
            .filter(|a| a.member_id == id)
            .map(|a| (a.item_id.clone(), a.quantity))
            .collect::<BTreeMap<_, _>>()
    };
    let loadout = members
        .first()
        .map(|m| member_loadout(&m.id))
        .unwrap_or_default();
    ensure!(
        members.iter().all(|m| member_loadout(&m.id) == loadout),
        "Group has unequal equipment; match existing members' loadouts before reinforcing"
    );
    let xp = group
        .as_ref()
        .map(|u| {
            u.starting_xp
                .checked_add(u.earned_xp)
                .context("Group XP overflow")
        })
        .transpose()?
        .unwrap_or(0);
    let veteran_required = xp.checked_mul(count).context("Veteran XP overflow")?;
    let hire_each = match price {
        Some(price) => price,
        None => warrior
            .cost
            .checked_add(2 * i64::from(xp))
            .context("Hire cost overflow")?,
    };
    let hire_total = amount(count, hire_each)?;
    let mut starting: BTreeMap<_, _> = db
        .permissions
        .iter()
        .filter(|p| p.type_id == warrior.id && p.kind == "starting-item")
        .map(|p| (p.target_id.clone(), p.quantity))
        .collect();
    if warrior.free_dagger.unwrap_or(true)
        && let Some(dagger) = db
            .items
            .iter()
            .find(|i| i.profile_id == warrior.profile_id && i.name == "Dagger")
    {
        starting.entry(dagger.id.clone()).or_insert(1);
    }
    let mut gear = vec![];
    let mut equipment_total = 0i64;
    for (id, per_model) in &loadout {
        let item = db
            .items
            .iter()
            .find(|i| i.id == *id)
            .context("Missing group equipment definition")?
            .clone();
        let eligible = db.equipment_allowed(&group.as_ref().unwrap().id, id);
        ensure!(
            eligible,
            "{} is not available on the group's assigned equipment list; update its list before reinforcing",
            item.name
        );
        let included = (*per_model)
            .min(starting.get(id).copied().unwrap_or(0))
            .checked_mul(count)
            .context("Equipment quantity overflow")?;
        let required = per_model
            .checked_mul(count)
            .context("Equipment quantity overflow")?;
        let from_stash =
            (required - included).min(db.owned(w, id).saturating_sub(db.assigned(w, id)));
        let purchase = required - included - from_stash;
        ensure!(
            purchase == 0 || item.rarity == 0 && item.category != "resource",
            "{} needs {purchase} more copies in stash. Rare or unavailable equipment cannot be automatically purchased; acquire it separately first",
            item.name
        );
        let cost = amount(purchase, item.cost)?;
        equipment_total = equipment_total
            .checked_add(cost)
            .context("Equipment cost overflow")?;
        gear.push(Gear {
            item,
            per_model: *per_model,
            included,
            from_stash,
            purchase,
            cost,
        });
    }
    let total = hire_total
        .checked_add(equipment_total)
        .context("Recruitment total overflow")?;
    Ok(Quote {
        warrior,
        group,
        members,
        starting,
        loadout,
        gear,
        hire_each,
        hire_total,
        equipment_total,
        total,
        veteran_required,
        veteran_battle: veteran_battle(db, w).cloned().map(|mut b| {
            b.veteran_spent = veteran_spent(db, &b);
            b
        }),
    })
}
impl Quote {
    pub(crate) fn describe(&self, db: &Db, w: &str, count: u32) -> String {
        let mut lines = vec![format!(
            "Hire: {count} × {} gc = {} gc (includes veteran premium unless an actual price was entered).",
            self.hire_each, self.hire_total
        )];
        if self.veteran_required > 0 {
            lines.push(match &self.veteran_battle {
                Some(b) => format!("Veteran XP: {} required; {} remaining from {}. V on Warband records/corrects the total rolled, including after completion.", self.veteran_required,b.veteran_budget,b.name),
                None => format!("Veteran XP: {} required. No resolved battle has an allowance; record the battle and its veteran XP first.",self.veteran_required),
            });
        }
        for g in &self.gear {
            lines.push(format!(
                "{}: {} per new model; {} included, {} from stash, buy {} × {} gc = {} gc.",
                g.item.name, g.per_model, g.included, g.from_stash, g.purchase, g.item.cost, g.cost
            ));
        }
        lines.push(format!("Equipment purchases: {} gc. Total: {} gc. Treasury: {} → {} gc. Identical equipment is issued to the new members.",self.equipment_total,self.total,db.gold(w),db.gold(w).saturating_sub(self.total)));
        lines.join("\n")
    }
}
