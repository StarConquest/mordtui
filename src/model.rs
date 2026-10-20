use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
mod equipment;

pub const SCHEMA: u32 = 1;
pub const STATS: [&str; 9] = ["M", "WS", "BS", "S", "T", "W", "I", "A", "Ld"];
pub const STEPS: [&str; 10] = [
    "injuries",
    "experience",
    "exploration",
    "wyrdstone",
    "veterans",
    "rare",
    "dramatis",
    "recruitment",
    "equipment",
    "rating",
];
pub fn id() -> String {
    Uuid::new_v4().to_string()
}
macro_rules! row { ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct $name { $(pub $field: $ty),* }
}; }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Campaign {
    pub id: String,
    pub name: String,
    pub notes: String,
    #[serde(default)]
    pub skip_dramatis_personae: bool,
}
row!(Warband {
    id: String,
    campaign_id: String,
    profile_id: String,
    name: String,
    leader_id: String
});
row!(Profile {
    id: String,
    schema_version: u32,
    version: String,
    name: String,
    faction: String,
    source: String,
    classification: String,
    starting_gold: i64,
    bonus_gold: i64,
    max_members: u32,
    max_heroes: u32,
    allow_debt: bool,
    initial_advance_cost: i64
});
row!(WarriorType {
    id: String,
    profile_id: String,
    name: String,
    role: String,
    cost: i64,
    upkeep: i64,
    starting_xp: u32,
    gains_xp: bool,
    limit: u32,
    group_max: u32,
    rating: u32,
    counts_roster: bool,
    counts_income: bool,
    counts_battle: bool,
    m: String,
    ws: String,
    bs: String,
    s: String,
    t: String,
    w: String,
    i: String,
    a: String,
    ld: String
});
row!(Item {
    id: String,
    profile_id: String,
    name: String,
    cost: i64,
    category: String,
    rarity: u32,
    description: String
});
row!(Permission {
    id: String,
    type_id: String,
    target_id: String,
    kind: String
});
row!(EquipmentList {
    id: String,
    profile_id: String,
    name: String,
    description: String
});
row!(EquipmentListItem {
    id: String,
    list_id: String,
    item_id: String
});
row!(EquipmentListAssignment {
    id: String,
    entity_id: String,
    list_id: String
});
row!(Ability {
    id: String,
    profile_id: String,
    name: String,
    kind: String,
    description: String,
    source: String
});
row!(Cap {
    id: String,
    type_id: String,
    characteristic: String,
    maximum: i16
});
row!(Threshold {
    id: String,
    profile_id: String,
    role: String,
    xp: u32
});
row!(Unit {
    id: String,
    warband_id: String,
    type_id: String,
    name: String,
    role: String,
    starting_xp: u32,
    earned_xp: u32
});
row!(Member {
    id: String,
    unit_id: String,
    name: String,
    status: String,
    missed_games: u32
});
row!(Advance {
    id: String,
    unit_id: String,
    threshold: u32,
    characteristic: String,
    delta: i16,
    ability_id: String,
    dice_id: String,
    reason: String
});
row!(Effect {
    id: String,
    member_id: String,
    characteristic: String,
    delta: i16,
    description: String,
    condition: String,
    battle_id: String
});
row!(Holding {
    id: String,
    warband_id: String,
    item_id: String,
    quantity: u32
});
row!(Allocation {
    id: String,
    member_id: String,
    item_id: String,
    quantity: u32
});
row!(Ledger {
    id: String,
    warband_id: String,
    category: String,
    item_id: String,
    member_id: String,
    quantity: u32,
    unit_price: i64,
    gold: i64,
    date: String,
    battle_id: String,
    phase: String,
    notes: String
});
row!(Note {
    id: String,
    entity_id: String,
    text: String,
    date: String
});
row!(Battle {
    id: String,
    warband_id: String,
    name: String,
    date: String,
    scenario: String,
    opponent: String,
    opponent_rating: u32,
    rating: u32,
    status: String,
    result: String,
    pre_snapshot: String,
    post_snapshot: String,
    veteran_budget: u32
});
row!(Participant {
    id: String,
    battle_id: String,
    member_id: String,
    wounds: i16,
    condition: String,
    kills: u32,
    objectives: u32,
    injury_resolved: bool,
    attribution: String
});
row!(Step {
    id: String,
    battle_id: String,
    position: u32,
    name: String,
    status: String,
    reason: String
});
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub id: String,
    pub battle_id: String,
    pub member_id: String,
    pub kind: String,
    pub allowance: u32,
    pub used: u32,
    #[serde(default)]
    pub item_id: String,
    #[serde(default)]
    pub outcome: String,
    #[serde(default)]
    pub purchased: bool,
}
row!(Resolution {
    id: String,
    battle_id: String,
    key: String,
    description: String,
    dice_id: String
});
row!(Dice {
    id: String,
    notation: String,
    results: String,
    modifiers: i16,
    rerolls: String,
    table_version: String,
    choice: String,
    source: String
});
row!(Snapshot {
    id: String,
    name: String,
    generation: String,
    warband_id: String
});
row!(Event {
    id: String,
    sequence: u64,
    recorded_at: String,
    date: String,
    reason: String,
    action: String,
    related_ids: String,
    generation: String,
    parent_generation: String,
    provenance: String
});

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Db {
    pub campaigns: Vec<Campaign>,
    pub warbands: Vec<Warband>,
    pub profiles: Vec<Profile>,
    pub types: Vec<WarriorType>,
    pub items: Vec<Item>,
    pub permissions: Vec<Permission>,
    #[serde(default)]
    pub equipment_lists: Vec<EquipmentList>,
    #[serde(default)]
    pub equipment_list_items: Vec<EquipmentListItem>,
    #[serde(default)]
    pub equipment_list_assignments: Vec<EquipmentListAssignment>,
    pub abilities: Vec<Ability>,
    pub caps: Vec<Cap>,
    pub thresholds: Vec<Threshold>,
    pub units: Vec<Unit>,
    pub members: Vec<Member>,
    pub advances: Vec<Advance>,
    pub effects: Vec<Effect>,
    pub holdings: Vec<Holding>,
    pub allocations: Vec<Allocation>,
    pub ledger: Vec<Ledger>,
    pub notes: Vec<Note>,
    pub battles: Vec<Battle>,
    pub participants: Vec<Participant>,
    pub steps: Vec<Step>,
    pub searches: Vec<Search>,
    pub resolutions: Vec<Resolution>,
    pub dice: Vec<Dice>,
    pub snapshots: Vec<Snapshot>,
    pub events: Vec<Event>,
}
pub trait Named {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
}
macro_rules! named { ($($t:ty),*) => { $(impl Named for $t { fn id(&self)->&str { &self.id } fn name(&self)->&str { &self.name } })* }; }
named!(
    Campaign,
    Warband,
    Profile,
    WarriorType,
    Item,
    EquipmentList,
    Ability,
    Unit,
    Member,
    Battle,
    Snapshot
);
pub fn resolve<'a, T: Named>(rows: &'a [T], query: &str) -> Result<&'a T> {
    if let Some(r) = rows.iter().find(|r| r.id() == query) {
        return Ok(r);
    }
    let found: Vec<_> = rows
        .iter()
        .filter(|r| {
            r.name().eq_ignore_ascii_case(query) || (query.len() >= 8 && r.id().starts_with(query))
        })
        .collect();
    match found.as_slice() {
        [r] => Ok(r),
        [] => bail!("No match for {query:?}"),
        _ => bail!("Ambiguous name {query:?}; use a stable ID"),
    }
}
pub fn active(m: &Member) -> bool {
    !["dead", "dismissed", "retired"].contains(&m.status.as_str())
}
impl WarriorType {
    pub fn stat(&self, s: &str) -> &str {
        match s {
            "M" => &self.m,
            "WS" => &self.ws,
            "BS" => &self.bs,
            "S" => &self.s,
            "T" => &self.t,
            "W" => &self.w,
            "I" => &self.i,
            "A" => &self.a,
            "Ld" => &self.ld,
            _ => "0",
        }
    }
}
impl Db {
    /// Kill credits use the existing battle-resolution ledger; older totals may
    /// contain additional unnamed credits without a ledger entry.
    pub fn battle_kills<'a>(
        &'a self,
        p: &Participant,
    ) -> impl DoubleEndedIterator<Item = &'a Resolution> + 'a {
        let prefix = format!("kill:{}:", p.id);
        let battle = p.battle_id.clone();
        self.resolutions
            .iter()
            .filter(move |r| r.battle_id == battle && r.key.starts_with(&prefix))
    }
    pub fn skips_dramatis(&self, warband: &str) -> bool {
        self.warbands
            .iter()
            .find(|w| w.id == warband)
            .is_some_and(|w| {
                self.campaigns
                    .iter()
                    .any(|c| c.id == w.campaign_id && c.skip_dramatis_personae)
            })
    }
    pub fn rare_search_eligible(&self, battle: &str, m: &Member) -> bool {
        m.status == "active"
            && self.unit(&m.unit_id).role == "hero"
            && self.participants.iter().any(|p| {
                p.battle_id == battle && p.member_id == m.id && p.condition != "out-of-action"
            })
    }
    pub fn rare_search_remaining(&self, battle: &str, member: &str) -> u32 {
        u32::from(!self.searches.iter().any(|s| {
            s.battle_id == battle && s.member_id == member && s.kind == "rare" && s.used > 0
        }))
    }
    pub fn unit(&self, id: &str) -> &Unit {
        self.units
            .iter()
            .find(|x| x.id == id)
            .expect("validated unit")
    }
    pub fn typ(&self, id: &str) -> &WarriorType {
        self.types
            .iter()
            .find(|x| x.id == id)
            .expect("validated type")
    }
    pub fn profile(&self, id: &str) -> &Profile {
        self.profiles
            .iter()
            .find(|x| x.id == id)
            .expect("validated profile")
    }
    pub fn member_unit(&self, m: &Member) -> &Unit {
        self.unit(&m.unit_id)
    }
    pub fn gold(&self, wb: &str) -> i64 {
        self.ledger
            .iter()
            .filter(|x| x.warband_id == wb)
            .map(|x| x.gold as i128)
            .sum::<i128>() as i64
    }
    pub fn owned(&self, wb: &str, item: &str) -> u32 {
        self.holdings
            .iter()
            .find(|x| x.warband_id == wb && x.item_id == item)
            .map_or(0, |x| x.quantity)
    }
    pub fn assigned(&self, wb: &str, item: &str) -> u32 {
        self.allocations
            .iter()
            .filter(|a| {
                a.item_id == item
                    && self
                        .members
                        .iter()
                        .any(|m| m.id == a.member_id && self.unit(&m.unit_id).warband_id == wb)
            })
            .map(|a| a.quantity)
            .sum()
    }
    pub fn members_of<'a>(&'a self, wb: &'a str) -> impl Iterator<Item = &'a Member> + 'a {
        self.members
            .iter()
            .filter(move |m| self.unit(&m.unit_id).warband_id == wb)
    }
    pub fn rating(&self, wb: &str) -> u32 {
        self.members_of(wb)
            .filter(|m| active(m))
            .map(|m| {
                let u = self.unit(&m.unit_id);
                self.typ(&u.type_id).rating + u.starting_xp + u.earned_xp
            })
            .sum()
    }
    pub fn pending(&self, u: &Unit) -> Vec<u32> {
        if !self.members.iter().any(|m| m.unit_id == u.id && active(m)) {
            return Vec::new();
        }
        if self
            .advances
            .iter()
            .any(|a| a.unit_id == u.id && a.reason == "pending-promotion")
        {
            return vec![0];
        }

        let profile = &self
            .warbands
            .iter()
            .find(|w| w.id == u.warband_id)
            .unwrap()
            .profile_id;
        let mut pending: Vec<u32> = self
            .thresholds
            .iter()
            .filter(|t| {
                t.profile_id == *profile
                    && t.role == u.role
                    && t.xp > u.starting_xp
                    && t.xp <= u.starting_xp + u.earned_xp
                    && !self
                        .advances
                        .iter()
                        .any(|a| a.unit_id == u.id && a.threshold == t.xp)
            })
            .map(|t| t.xp)
            .collect();
        pending.sort_unstable();
        pending
    }
    pub fn stat(&self, m: &Member, s: &str, battle: Option<&str>) -> String {
        let u = self.unit(&m.unit_id);
        let raw_base = self.typ(&u.type_id).stat(s);
        let resolved = self.effects.iter().rev().find(|e| {
            e.member_id == m.id
                && e.characteristic == s
                && e.description == "Resolved base characteristic"
                && (e.battle_id.is_empty() || Some(e.battle_id.as_str()) == battle)
        });
        let resolved_value = resolved.map(|e| e.delta.to_string());
        let base = resolved_value.as_deref().unwrap_or(raw_base);
        let delta: i64 = self
            .advances
            .iter()
            .filter(|a| a.unit_id == u.id && a.characteristic == s)
            .map(|a| a.delta as i64)
            .sum::<i64>()
            + self
                .effects
                .iter()
                .filter(|e| {
                    e.member_id == m.id
                        && e.description != "Resolved base characteristic"
                        && e.characteristic == s
                        && e.condition.is_empty()
                        && (e.battle_id.is_empty() || Some(e.battle_id.as_str()) == battle)
                })
                .map(|e| e.delta as i64)
                .sum::<i64>();
        match base.parse::<i16>() {
            Ok(b) => (b as i64 + delta).to_string(),
            Err(_) => {
                if delta == 0 {
                    base.to_string()
                } else {
                    format!("{base}{delta:+}")
                }
            }
        }
    }
    pub fn validate(&self) -> Result<()> {
        self.validate_equipment_lists()?;
        let mut ids = std::collections::HashSet::new();
        macro_rules! unique { ($($f:ident),*) => {$(for x in &self.$f {ensure!(!x.id.is_empty()&&ids.insert(x.id.clone()),"Duplicate or empty ID in {}: {}",stringify!($f),x.id);})*}; }
        unique!(
            campaigns,
            warbands,
            profiles,
            types,
            items,
            permissions,
            equipment_lists,
            equipment_list_items,
            equipment_list_assignments,
            abilities,
            caps,
            thresholds,
            units,
            members,
            advances,
            effects,
            holdings,
            allocations,
            ledger,
            notes,
            battles,
            participants,
            steps,
            searches,
            resolutions,
            dice,
            snapshots,
            events
        );
        for p in &self.profiles {
            ensure!(
                p.schema_version == SCHEMA,
                "Unsupported profile schema {}",
                p.schema_version
            );
            ensure!(
                p.starting_gold >= 0 && p.bonus_gold >= 0 && p.max_members > 0 && p.max_heroes > 0,
                "Invalid profile {}",
                p.id
            );
        }
        for t in &self.types {
            ensure!(
                self.profiles.iter().any(|p| p.id == t.profile_id),
                "Unknown profile for type {}",
                t.id
            );
            ensure!(
                ["hero", "henchman", "hired", "dramatis"].contains(&t.role.as_str())
                    && t.group_max > 0
                    && t.cost >= 0
                    && t.upkeep >= 0,
                "Invalid type {}",
                t.id
            );
            for s in STATS {
                validate_expression(t.stat(s))?;
            }
        }
        for i in &self.items {
            ensure!(
                self.profiles.iter().any(|p| p.id == i.profile_id) && i.cost >= 0,
                "Invalid item {}",
                i.id
            );
        }
        for a in &self.abilities {
            ensure!(
                self.profiles.iter().any(|p| p.id == a.profile_id),
                "Unknown ability profile {}",
                a.id
            );
        }
        for p in &self.permissions {
            ensure!(
                self.types.iter().any(|t| t.id == p.type_id)
                    || self.units.iter().any(|u| u.id == p.type_id),
                "Unknown permission type {}",
                p.type_id
            );
            ensure!(
                match p.kind.as_str() {
                    "item" => self.items.iter().any(|i| i.id == p.target_id),
                    "ability" | "starting-ability" =>
                        self.abilities.iter().any(|i| i.id == p.target_id),
                    _ => false,
                },
                "Invalid permission {}",
                p.id
            );
        }
        for c in &self.caps {
            ensure!(
                self.types.iter().any(|t| t.id == c.type_id)
                    && STATS.contains(&c.characteristic.as_str())
                    && c.maximum > 0,
                "Invalid cap {}",
                c.id
            );
        }
        for t in &self.thresholds {
            ensure!(
                self.profiles.iter().any(|p| p.id == t.profile_id) && t.xp > 0,
                "Invalid threshold {}",
                t.id
            );
            ensure!(
                self.thresholds
                    .iter()
                    .filter(|a| a.profile_id == t.profile_id && a.role == t.role && a.xp == t.xp)
                    .count()
                    == 1,
                "Duplicate threshold"
            );
        }
        for w in &self.warbands {
            let gold: i128 = self
                .ledger
                .iter()
                .filter(|l| l.warband_id == w.id)
                .map(|l| l.gold as i128)
                .sum();
            ensure!(
                gold >= i64::MIN as i128 && gold <= i64::MAX as i128,
                "Treasury total overflow"
            );
            ensure!(
                self.campaigns.iter().any(|c| c.id == w.campaign_id)
                    && self.profiles.iter().any(|p| p.id == w.profile_id),
                "Broken warband references {}",
                w.id
            );
            ensure!(
                w.leader_id.is_empty()
                    || self
                        .members_of(&w.id)
                        .any(|m| m.id == w.leader_id && active(m)),
                "Invalid leader"
            );
            ensure!(
                self.profile(&w.profile_id).allow_debt || self.gold(&w.id) >= 0,
                "Insufficient gold for {}",
                w.name
            );
        }
        for u in &self.units {
            ensure!(
                u.starting_xp
                    .checked_add(u.earned_xp)
                    .is_some_and(|xp| xp <= 1_000_000),
                "Invalid or excessive experience"
            );
            ensure!(
                ["hero", "henchman", "hired", "dramatis"].contains(&u.role.as_str()),
                "Invalid unit role"
            );
            let w = self
                .warbands
                .iter()
                .find(|w| w.id == u.warband_id)
                .ok_or_else(|| anyhow::anyhow!("Unknown unit warband"))?;
            ensure!(
                self.types
                    .iter()
                    .any(|t| t.id == u.type_id && t.profile_id == w.profile_id),
                "Unknown/mismatched unit type"
            );
        }
        for m in &self.members {
            ensure!(
                self.units.iter().any(|u| u.id == m.unit_id),
                "Unknown member unit"
            );
            ensure!(
                [
                    "active",
                    "dead",
                    "dismissed",
                    "retired",
                    "captured",
                    "recovering"
                ]
                .contains(&m.status.as_str()),
                "Invalid member status"
            );
        }
        for a in &self.advances {
            ensure!(
                self.units.iter().any(|u| u.id == a.unit_id),
                "Unknown advance unit"
            );
            ensure!(
                a.characteristic.is_empty() || STATS.contains(&a.characteristic.as_str()),
                "Invalid advance characteristic"
            );
            ensure!(
                a.ability_id.is_empty() || self.abilities.iter().any(|b| b.id == a.ability_id),
                "Unknown ability"
            );
            ensure!(
                a.dice_id.is_empty() || self.dice.iter().any(|d| d.id == a.dice_id),
                "Unknown advance dice"
            );
            if a.threshold > 0 {
                ensure!(
                    self.advances
                        .iter()
                        .filter(|b| b.unit_id == a.unit_id && b.threshold == a.threshold)
                        .count()
                        == 1,
                    "Duplicate advance resolution"
                );
            }
        }
        for e in &self.effects {
            ensure!(
                self.members.iter().any(|m| m.id == e.member_id)
                    && (e.characteristic.is_empty() || STATS.contains(&e.characteristic.as_str())),
                "Invalid effect"
            );
        }
        for h in &self.holdings {
            ensure!(
                self.warbands.iter().any(|w| w.id == h.warband_id)
                    && self.items.iter().any(|i| i.id == h.item_id
                        && self
                            .warbands
                            .iter()
                            .any(|w| w.id == h.warband_id && w.profile_id == i.profile_id)),
                "Invalid holding references"
            );
            ensure!(
                self.holdings
                    .iter()
                    .filter(|x| x.warband_id == h.warband_id && x.item_id == h.item_id)
                    .count()
                    == 1,
                "Duplicate holding"
            );
            ensure!(
                self.assigned(&h.warband_id, &h.item_id) <= h.quantity,
                "Overallocated item {}",
                h.item_id
            );
        }
        for a in &self.allocations {
            let m = self
                .members
                .iter()
                .find(|m| m.id == a.member_id)
                .ok_or_else(|| anyhow::anyhow!("Unknown allocation member"))?;
            ensure!(
                active(m)
                    && a.quantity > 0
                    && a.quantity <= self.owned(&self.unit(&m.unit_id).warband_id, &a.item_id),
                "Invalid allocation {}",
                a.id
            );
            ensure!(
                self.allocations
                    .iter()
                    .filter(|b| b.member_id == a.member_id && b.item_id == a.item_id)
                    .count()
                    == 1,
                "Duplicate allocation"
            );
        }
        for l in &self.ledger {
            ensure!(
                self.warbands.iter().any(|w| w.id == l.warband_id),
                "Unknown ledger warband"
            );
            ensure!(
                l.item_id.is_empty() || self.items.iter().any(|i| i.id == l.item_id),
                "Unknown ledger item"
            );
        }
        for b in &self.battles {
            ensure!(
                self.warbands.iter().any(|w| w.id == b.warband_id),
                "Unknown battle warband"
            );
            ensure!(
                [
                    "prepared",
                    "in-progress",
                    "post-battle",
                    "complete",
                    "abandoned"
                ]
                .contains(&b.status.as_str()),
                "Invalid battle state"
            );
        }
        for p in &self.participants {
            ensure!(
                self.battles.iter().any(|b| b.id == p.battle_id)
                    && self.members.iter().any(|m| m.id == p.member_id),
                "Invalid participant"
            );
        }
        for s in &self.steps {
            ensure!(
                self.battles.iter().any(|b| b.id == s.battle_id)
                    && ["pending", "in-progress", "completed", "skipped"]
                        .contains(&s.status.as_str()),
                "Invalid step"
            );
        }
        for s in &self.searches {
            ensure!(
                s.used <= s.allowance
                    && self.battles.iter().any(|b| b.id == s.battle_id)
                    && self.members.iter().any(|m| m.id == s.member_id),
                "Invalid search"
            );
            ensure!(
                s.item_id.is_empty()
                    || self.items.iter().any(|i| i.id == s.item_id && i.rarity > 0),
                "Invalid rare-search item"
            );
            ensure!(
                ["", "found", "failed"].contains(&s.outcome.as_str())
                    && (!s.purchased || s.used > 0 && s.outcome != "failed"),
                "Invalid search outcome"
            );
        }
        for r in &self.resolutions {
            ensure!(
                self.battles.iter().any(|b| b.id == r.battle_id),
                "Unknown resolution battle"
            );
            ensure!(
                self.resolutions
                    .iter()
                    .filter(|q| q.battle_id == r.battle_id && q.key == r.key)
                    .count()
                    == 1,
                "Duplicate resolution key"
            );
            if let Some(rest) = r.key.strip_prefix("kill:") {
                let (participant, credit) = rest
                    .split_once(':')
                    .ok_or_else(|| anyhow::anyhow!("Invalid kill record key"))?;
                ensure!(
                    !credit.is_empty()
                        && self
                            .participants
                            .iter()
                            .any(|p| p.id == participant && p.battle_id == r.battle_id),
                    "Unknown kill participant"
                );
            }
        }
        for p in &self.participants {
            ensure!(
                self.battle_kills(p).count() <= p.kills as usize,
                "Kill records exceed participant total"
            );
        }
        for e in &self.effects {
            ensure!(
                e.battle_id.is_empty() || self.battles.iter().any(|b| b.id == e.battle_id),
                "Unknown effect battle"
            );
        }
        for l in &self.ledger {
            ensure!(
                l.member_id.is_empty() || self.members.iter().any(|m| m.id == l.member_id),
                "Unknown ledger member"
            );
            ensure!(
                l.battle_id.is_empty() || self.battles.iter().any(|b| b.id == l.battle_id),
                "Unknown ledger battle"
            );
        }
        for w in &self.warbands {
            ensure!(
                self.battles
                    .iter()
                    .filter(|b| b.warband_id == w.id
                        && !["complete", "abandoned"].contains(&b.status.as_str()))
                    .count()
                    <= 1,
                "Multiple active battles"
            );
        }
        for p in &self.participants {
            let b = self.battles.iter().find(|b| b.id == p.battle_id).unwrap();
            let m = self.members.iter().find(|m| m.id == p.member_id).unwrap();
            ensure!(
                self.unit(&m.unit_id).warband_id == b.warband_id,
                "Participant belongs to another warband"
            );
            ensure!(
                self.participants
                    .iter()
                    .filter(|x| x.battle_id == p.battle_id && x.member_id == p.member_id)
                    .count()
                    == 1,
                "Duplicate battle participant"
            );
        }
        for s in &self.steps {
            ensure!(
                self.steps
                    .iter()
                    .filter(|a| a.battle_id == s.battle_id
                        && (a.position == s.position || a.name == s.name))
                    .count()
                    == 1,
                "Duplicate battle step"
            );
        }
        for r in &self.resolutions {
            ensure!(
                r.dice_id.is_empty() || self.dice.iter().any(|d| d.id == r.dice_id),
                "Unknown resolution dice"
            );
        }
        for d in &self.dice {
            validate_expression(&d.notation)?;
            ensure!(
                ["physical", "tool"].contains(&d.source.as_str()),
                "Invalid dice source"
            );
        }
        for (index, e) in self.events.iter().enumerate() {
            ensure!(e.sequence == index as u64 + 1, "Invalid event sequence");
        }
        Ok(())
    }
}
pub fn validate_expression(s: &str) -> Result<()> {
    if let Ok(n) = s.parse::<i16>() {
        ensure!(n >= 0, "Negative characteristic");
        return Ok(());
    }
    let s = s.to_ascii_uppercase();
    let (n, rest) = s.split_once('D').ok_or_else(|| {
        anyhow::anyhow!("Invalid random characteristic {s}; use e.g. D6 or 2D6+1")
    })?;
    let count = if n.is_empty() { 1 } else { n.parse::<u32>()? };
    let sides = rest.split(['+', '-']).next().unwrap().parse::<u32>()?;
    ensure!(
        count > 0 && count <= 100 && sides > 0 && sides <= 1000,
        "Invalid dice expression {s}"
    );
    if let Some(pos) = rest.find(['+', '-']) {
        rest[pos..].parse::<i16>()?;
    }
    Ok(())
}
