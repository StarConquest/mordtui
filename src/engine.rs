use crate::{
    cli::*,
    model::*,
    rules,
    store::{Bundle, Store, read_tables},
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{fs, path::Path};
mod equipment;

#[derive(Clone)]
pub struct ContextData {
    pub wb: String,
    pub date: String,
    pub reason: String,
    pub override_rules: bool,
    pub phase: String,
}
impl ContextData {
    pub fn check(&self, valid: bool, message: &str) -> Result<()> {
        ensure!(
            valid || self.override_rules,
            "{message}; use --override --reason to record a house-rule exception"
        );
        Ok(())
    }
}
pub fn selected(db: &Db, cli: &Cli) -> Result<String> {
    let candidates: Vec<_> = db
        .warbands
        .iter()
        .filter(|w| {
            cli.campaign.as_ref().is_none_or(|q| {
                db.campaigns.iter().any(|c| {
                    c.id == w.campaign_id && (c.id == *q || c.name.eq_ignore_ascii_case(q))
                })
            })
        })
        .cloned()
        .collect();
    if let Some(q) = &cli.warband {
        return Ok(resolve(&candidates, q)?.id.clone());
    }
    if candidates.len() == 1 {
        return Ok(candidates[0].id.clone());
    }
    if let Some(n) = db.notes.iter().rev().find(|n| n.entity_id == "selection")
        && candidates.iter().any(|w| w.id == n.text)
    {
        return Ok(n.text.clone());
    }
    bail!(
        "Select a warband with --warband NAME/ID or `select NAME`. Use `list warbands` to see choices"
    )
}
pub fn context(db: &Db, cli: &Cli, needs_wb: bool) -> Result<ContextData> {
    ensure!(
        !cli.override_rules || !cli.reason.trim().is_empty(),
        "--override requires --reason"
    );
    let date = cli
        .date
        .clone()
        .unwrap_or_else(|| chrono::Local::now().date_naive().to_string());
    chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").context("--date must be YYYY-MM-DD")?;
    Ok(ContextData {
        wb: if needs_wb {
            selected(db, cli)?
        } else {
            String::new()
        },
        date,
        reason: cli.reason.clone(),
        override_rules: cli.override_rules,
        phase: String::new(),
    })
}
fn unit(db: &Db, wb: &str, q: &str) -> Result<Unit> {
    let rows: Vec<_> = db
        .units
        .iter()
        .filter(|u| u.warband_id == wb)
        .cloned()
        .collect();
    Ok(resolve(&rows, q)?.clone())
}
fn member(db: &Db, wb: &str, q: &str) -> Result<Member> {
    let rows: Vec<_> = db.members_of(wb).cloned().collect();
    Ok(resolve(&rows, q)?.clone())
}
fn item(db: &Db, wb: &str, q: &str) -> Result<Item> {
    let profile = &db.warbands.iter().find(|w| w.id == wb).unwrap().profile_id;
    let rows: Vec<_> = db
        .items
        .iter()
        .filter(|i| i.profile_id == *profile)
        .cloned()
        .collect();
    Ok(resolve(&rows, q)?.clone())
}
fn wb<'a>(db: &'a Db, id: &str) -> &'a Warband {
    db.warbands.iter().find(|w| w.id == id).unwrap()
}
fn current_battle(db: &Db, wb: &str) -> Result<Battle> {
    db.battles
        .iter()
        .rev()
        .find(|b| b.warband_id == wb && !["complete", "abandoned"].contains(&b.status.as_str()))
        .cloned()
        .context("No active battle")
}
fn stage(db: &Db, bid: &str) -> String {
    db.steps
        .iter()
        .filter(|s| s.battle_id == bid)
        .find(|s| !["completed", "skipped"].contains(&s.status.as_str()))
        .map_or(String::new(), |s| s.name.clone())
}
fn gate(db: &Db, c: &mut ContextData, expected: &[&str]) -> Result<()> {
    if let Ok(b) = current_battle(db, &c.wb) {
        ensure!(
            b.status == "post-battle",
            "Campaign changes are locked during a prepared/active battle"
        );
        let available = db.steps.iter().find(|s| {
            s.battle_id == b.id
                && expected.contains(&s.name.as_str())
                && !matches!(s.status.as_str(), "completed" | "skipped")
        });
        c.check(
            available.is_some(),
            &format!(
                "Action belongs to stages {}; current stage is {}",
                expected.join(", "),
                stage(db, &b.id)
            ),
        )?;
        c.phase = available
            .map(|s| s.name.clone())
            .unwrap_or_else(|| expected.first().unwrap_or(&"").to_string());
    }
    Ok(())
}
fn ledger(
    db: &mut Db,
    c: &ContextData,
    category: &str,
    refs: (&str, &str),
    quantity: u32,
    price: i64,
    gold: i64,
) {
    let b = current_battle(db, &c.wb).ok();
    let phase = if !c.phase.is_empty() {
        c.phase.clone()
    } else {
        b.as_ref()
            .map_or_else(|| "setup".into(), |b| stage(db, &b.id))
    };
    db.ledger.push(Ledger {
        id: id(),
        warband_id: c.wb.clone(),
        category: category.into(),
        item_id: refs.0.into(),
        member_id: refs.1.into(),
        quantity,
        unit_price: price,
        gold,
        date: c.date.clone(),
        battle_id: b.map_or(String::new(), |b| b.id),
        phase,
        notes: c.reason.clone(),
    });
}
fn holding(db: &mut Db, wb: &str, item: &str, delta: i64) -> Result<()> {
    let old = db.owned(wb, item) as i64;
    let next = old.checked_add(delta).context("Quantity overflow")?;
    ensure!(
        next >= 0 && next <= u32::MAX as i64,
        "Insufficient inventory"
    );
    if let Some(h) = db
        .holdings
        .iter_mut()
        .find(|h| h.warband_id == wb && h.item_id == item)
    {
        h.quantity = next as u32;
    } else {
        db.holdings.push(Holding {
            id: id(),
            warband_id: wb.into(),
            item_id: item.into(),
            quantity: next as u32,
        });
    }
    Ok(())
}
fn note(db: &mut Db, entity: &str, text: &str, c: &ContextData) {
    db.notes.push(Note {
        id: id(),
        entity_id: entity.into(),
        text: text.into(),
        date: c.date.clone(),
    });
}
fn ensure_new_key(db: &Db, bid: &str, key: &str) -> Result<()> {
    ensure!(
        !db.resolutions
            .iter()
            .any(|r| r.battle_id == bid && r.key == key),
        "Outcome {key:?} already recorded"
    );
    Ok(())
}
fn resolution(db: &mut Db, bid: &str, key: &str, description: &str, dice: &str) -> Result<()> {
    key_check_dice(db, dice)?;
    ensure_new_key(db, bid, key)?;
    db.resolutions.push(Resolution {
        id: id(),
        battle_id: bid.into(),
        key: key.into(),
        description: description.into(),
        dice_id: dice.into(),
    });
    Ok(())
}
fn key_check_dice(db: &Db, dice: &str) -> Result<()> {
    ensure!(
        dice.is_empty() || db.dice.iter().any(|d| d.id == dice),
        "Unknown dice ID"
    );
    Ok(())
}
fn amount(q: u32, p: i64) -> Result<i64> {
    ensure!(p >= 0, "Price must be nonnegative");
    p.checked_mul(q as i64).context("Price overflow")
}
fn check_budget(db: &Db, wb: &str, gold: i64) -> Result<()> {
    ensure!(
        db.profile(&self::wb(db, wb).profile_id).allow_debt
            || db.gold(wb).checked_add(gold).context("Gold overflow")? >= 0,
        "Insufficient gold"
    );
    Ok(())
}
fn buy(
    db: &mut Db,
    c: &ContextData,
    it: &Item,
    qty: u32,
    price: i64,
    category: &str,
) -> Result<()> {
    ensure!(qty > 0, "Quantity must be positive");
    let cost = amount(qty, price)?;
    check_budget(db, &c.wb, -cost)?;
    holding(db, &c.wb, &it.id, qty as i64)?;
    ledger(db, c, category, (&it.id, ""), qty, price, -cost);
    Ok(())
}
fn member_depart(
    db: &mut Db,
    c: &ContextData,
    m: &Member,
    status: &str,
    disposition: &str,
) -> Result<()> {
    ensure!(active(m), "Member already departed");
    ensure!(
        ["dead", "dismissed", "retired"].contains(&status),
        "Use dead, dismissed or retired"
    );
    ensure!(
        ["stash", "lose"].contains(&disposition),
        "Equipment disposition must be stash or lose"
    );
    c.check(
        status != "dead" || disposition == "lose",
        "Death normally loses all carried equipment",
    )?;
    let allocations: Vec<_> = db
        .allocations
        .iter()
        .filter(|a| a.member_id == m.id)
        .cloned()
        .collect();
    db.allocations.retain(|a| a.member_id != m.id);
    for a in allocations {
        if disposition == "lose" {
            holding(db, &c.wb, &a.item_id, -(a.quantity as i64))?;
            ledger(db, c, "loss", (&a.item_id, &m.id), a.quantity, 0, 0);
        }
    }
    db.members.iter_mut().find(|x| x.id == m.id).unwrap().status = status.into();
    if wb(db, &c.wb).leader_id == m.id {
        db.warbands
            .iter_mut()
            .find(|w| w.id == c.wb)
            .unwrap()
            .leader_id
            .clear();
        note(
            db,
            &c.wb,
            "Leader departed; select a successor with status MEMBER active --leader",
            c,
        );
    }
    Ok(())
}

pub fn summary(db: &Db, wb_id: &str) -> Value {
    let w = wb(db, wb_id);
    let ms: Vec<_> = db.members_of(wb_id).filter(|m| active(m)).collect();
    let pending: usize = db
        .units
        .iter()
        .filter(|u| u.warband_id == wb_id)
        .map(|u| db.pending(u).len())
        .sum();
    json!({"warband":w,"profile":db.profile(&w.profile_id),"members":ms.len(),"roster_count":ms.iter().filter(|m|db.typ(&db.unit(&m.unit_id).type_id).counts_roster).count(),"income_count":ms.iter().filter(|m|db.typ(&db.unit(&m.unit_id).type_id).counts_income).count(),"battle_count":ms.iter().filter(|m|db.typ(&db.unit(&m.unit_id).type_id).counts_battle && m.status=="active" && m.missed_games==0).count(),"available":ms.iter().filter(|m|m.status=="active"&&m.missed_games==0).count(),"heroes":ms.iter().filter(|m|db.unit(&m.unit_id).role=="hero").count(),"rating":db.rating(wb_id),"gold":db.gold(wb_id),"wyrdstone":db.items.iter().find(|i|i.profile_id==w.profile_id&&i.name=="Wyrdstone").map_or(0,|i|db.owned(wb_id,&i.id)),"upkeep":ms.iter().map(|m|db.typ(&db.unit(&m.unit_id).type_id).upkeep).sum::<i64>(),"pending_advances":pending,"leader_missing":w.leader_id.is_empty(),"active_battle":current_battle(db,wb_id).ok()})
}
pub fn roster(db: &Db, wb: &str) -> Value {
    json!(db.members_of(wb).map(|m|{let u=db.unit(&m.unit_id);let stats:serde_json::Map<String,Value>=STATS.iter().map(|s|(s.to_string(),json!(db.stat(m,s,None)))).collect();json!({"id":m.id,"name":m.name,"unit":u,"status":m.status,"equipment_list":db.equipment_list_for(&m.id),"missed_games":m.missed_games,"stats":stats,"rating":if active(m){db.typ(&u.type_id).rating+u.starting_xp+u.earned_xp}else{0},"pending_advances":db.pending(u),"equipment":db.allocations.iter().filter(|a|a.member_id==m.id).collect::<Vec<_>>(),"advances":db.advances.iter().filter(|a|a.unit_id==u.id).collect::<Vec<_>>(),"effects":db.effects.iter().filter(|e|e.member_id==m.id).collect::<Vec<_>>()})}).collect::<Vec<_>>())
}
pub fn inventory(db: &Db, wb: &str) -> Value {
    json!(db.items.iter().filter(|i|i.profile_id==self::wb(db,wb).profile_id).map(|i|json!({"id":i.id,"name":i.name,"owned":db.owned(wb,&i.id),"assigned":db.assigned(wb,&i.id),"stash":db.owned(wb,&i.id)-db.assigned(wb,&i.id),"catalog_price":i.cost,"rarity":i.rarity,"description":i.description})).collect::<Vec<_>>())
}
pub fn list(db: &Db, cli: &Cli, what: &str, filter: Option<&str>) -> Result<Value> {
    let global = match what {
        "campaigns" => Some(json!(db.campaigns)),
        "warbands" => Some(json!(db.warbands)),
        "rules" => Some(json!(db.profiles)),
        "history" => Some(json!(db.events)),
        "snapshots" => Some(json!(db.snapshots)),
        "dice" => Some(json!(db.dice)),
        _ => None,
    };
    let mut value = if let Some(v) = global {
        v
    } else {
        let w = selected(db, cli)?;
        match what {
            "summary" => summary(db, &w),
            "roster" | "heroes" | "heros" | "henchmen" => {
                let v = roster(db, &w);
                if what == "roster" {
                    v
                } else {
                    json!(
                        v.as_array()
                            .unwrap()
                            .iter()
                            .filter(|r| r["unit"]["role"]
                                == if what == "henchmen" {
                                    "henchman"
                                } else {
                                    "hero"
                                })
                            .collect::<Vec<_>>()
                    )
                }
            }
            "equipment" | "inventory" => inventory(db, &w),
            "equipment-lists" => equipment::list_view(db, &w),
            "types" => json!(
                db.types
                    .iter()
                    .filter(|t| t.profile_id == wb(db, &w).profile_id)
                    .collect::<Vec<_>>()
            ),
            "abilities" | "skills" => json!(
                db.abilities
                    .iter()
                    .filter(|a| a.profile_id == wb(db, &w).profile_id)
                    .collect::<Vec<_>>()
            ),
            "ledger" => json!(
                db.ledger
                    .iter()
                    .filter(|l| l.warband_id == w)
                    .collect::<Vec<_>>()
            ),
            "notes" => json!(
                db.notes
                    .iter()
                    .filter(|n| n.entity_id == w
                        || db.members_of(&w).any(|m| m.id == n.entity_id)
                        || db
                            .units
                            .iter()
                            .any(|u| u.warband_id == w && u.id == n.entity_id)
                        || db
                            .battles
                            .iter()
                            .any(|b| b.warband_id == w && b.id == n.entity_id))
                    .collect::<Vec<_>>()
            ),
            "battles" => json!(
                db.battles
                    .iter()
                    .filter(|b| b.warband_id == w)
                    .collect::<Vec<_>>()
            ),
            "battle" => {
                let b = current_battle(db, &w)?;
                let p: Vec<_> = db
                    .participants
                    .iter()
                    .filter(|p| p.battle_id == b.id)
                    .collect();
                let ooa = p.iter().filter(|p| p.condition == "out-of-action").count();
                json!({"battle":b,"participants":p,"steps":db.steps.iter().filter(|s|s.battle_id==b.id).collect::<Vec<_>>(),"searches":db.searches.iter().filter(|s|s.battle_id==b.id).collect::<Vec<_>>(),"resolutions":db.resolutions.iter().filter(|s|s.battle_id==b.id).collect::<Vec<_>>(),"rout_threshold":p.len().div_ceil(4),"out_of_action":ooa})
            }
            _ => bail!("Unknown list {what:?}; see list --help"),
        }
    };
    if let (Some(f), Some(rows)) = (filter, value.as_array_mut()) {
        let f = f.to_lowercase();
        rows.retain(|r| r.to_string().to_lowercase().contains(&f));
    }
    Ok(value)
}

pub fn view(store: &Store, cli: &Cli, what: &str, filter: Option<&str>) -> Result<Value> {
    let mut value = list(&store.db, cli, what, filter)?;
    if what == "battle" {
        let w = selected(&store.db, cli)?;
        let b = current_battle(&store.db, &w)?;
        let snap = store
            .db
            .snapshots
            .iter()
            .find(|s| s.id == b.pre_snapshot)
            .context("Missing pre-battle snapshot")?;
        let frozen = store.historical(&snap.generation)?;
        value["reference_roster"] = roster(&frozen, &w);
        value["battle_stats"] = json!(
            store
                .db
                .participants
                .iter()
                .filter(|p| p.battle_id == b.id)
                .map(|p| {
                    let m = store
                        .db
                        .members
                        .iter()
                        .find(|m| m.id == p.member_id)
                        .unwrap();
                    let stats: serde_json::Map<String, Value> = STATS
                        .iter()
                        .map(|s| (s.to_string(), json!(store.db.stat(m, s, Some(&b.id)))))
                        .collect();
                    json!({"member_id":m.id,"name":m.name,"stats":stats,"state":p})
                })
                .collect::<Vec<_>>()
        );
    }
    Ok(value)
}

pub fn execute(store: &mut Store, cli: &Cli) -> Result<Value> {
    let cmd = cli.command.as_ref().context("No command")?;
    let needs_wb = !matches!(
        cmd,
        Command::Init { .. }
            | Command::Campaign { .. }
            | Command::Warband { .. }
            | Command::Select { .. }
            | Command::Rules { .. }
            | Command::List { .. }
            | Command::Validate
            | Command::Import { .. }
            | Command::Recover { .. }
            | Command::Roll { .. }
    );
    let mut c = context(&store.db, cli, needs_wb)?;
    let mut db = store.db.clone();
    let mut output = json!({"ok":true});
    match cmd {
        Command::List { what, filter } => return view(store, cli, what, filter.as_deref()),
        Command::Validate => {
            db.validate()?;
            return Ok(json!({"valid":true,"schema":SCHEMA,"generation":store.generation}));
        }
        Command::Rules { folder, install } => {
            let rules = rules::load(folder)?;
            if !install {
                return Ok(
                    json!({"valid":true,"profiles":rules.profiles,"types":rules.types.len(),"items":rules.items.len()}),
                );
            }
            rules::merge(&mut db, rules)?;
        }
        Command::AddEquipment {
            name,
            cost,
            category,
            rarity,
            description,
            lists,
            allowed,
        } => {
            let profile = wb(&db, &c.wb).profile_id.clone();
            ensure!(
                !name.trim().is_empty() && *cost >= 0,
                "Enter a name and nonnegative catalog cost"
            );
            ensure!(
                !db.items
                    .iter()
                    .any(|i| i.profile_id == profile && i.name.eq_ignore_ascii_case(name.trim())),
                "Equipment already exists in this catalog"
            );
            ensure!(
                ["melee", "missile", "armour", "equipment", "consumable"]
                    .contains(&category.as_str()),
                "Choose melee, missile, armour, equipment or consumable"
            );
            ensure!(
                lists.is_empty() || allowed.is_empty(),
                "Choose equipment lists rather than mixing lists and legacy warrior targets"
            );
            let targets = if allowed.is_empty() {
                equipment::resolve_lists(&db, &c.wb, lists, true)?
            } else {
                let mut lists = catalog_targets(&db, &c.wb, allowed)?
                    .iter()
                    .filter_map(|entity| db.equipment_list_for(entity).map(|l| l.id.clone()))
                    .collect::<Vec<_>>();
                lists.sort();
                lists.dedup();
                lists
            };
            ensure!(!targets.is_empty(), "Choose at least one equipment list");
            let item = Item {
                id: id(),
                profile_id: profile,
                name: name.trim().into(),
                cost: *cost,
                category: category.clone(),
                rarity: *rarity,
                description: description.clone(),
            };
            for target in targets {
                db.add_to_equipment_list(&target, &item.id);
            }
            output = json!({"item":item});
            db.items.push(item);
        }
        Command::EquipmentList(command) => output = equipment::execute(&mut db, &mut c, command)?,
        Command::AddAbility {
            name,
            kind,
            description,
            allowed,
        } => {
            let profile = wb(&db, &c.wb).profile_id.clone();
            ensure!(
                !name.trim().is_empty() && !kind.trim().is_empty(),
                "Enter a skill name and category"
            );
            ensure!(
                !db.abilities
                    .iter()
                    .any(|a| a.profile_id == profile && a.name.eq_ignore_ascii_case(name.trim())),
                "Skill already exists in this catalog"
            );
            let targets = catalog_targets(&db, &c.wb, allowed)?;
            let ability = Ability {
                id: id(),
                profile_id: profile,
                name: name.trim().into(),
                kind: kind.clone(),
                description: description.clone(),
                source: "Player-defined catalog extension".into(),
            };
            for target in targets {
                db.permissions.push(Permission {
                    id: id(),
                    type_id: target,
                    target_id: ability.id.clone(),
                    kind: "ability".into(),
                });
            }
            output = json!({"ability":ability});
            db.abilities.push(ability);
        }
        Command::GrantAbility { unit: q, ability } => {
            gate(&db, &mut c, &["experience", "recruitment"])?;
            let u = unit(&db, &c.wb, q)?;
            let (_, ability_id) = advance_choice(&db, &c, &u, None, Some(ability))?;
            db.advances.push(Advance {
                id: id(),
                unit_id: u.id,
                ability_id,
                reason: format!("Recorded skill: {}", c.reason),
                ..Advance::default()
            });
        }
        Command::Init {
            rules: folder,
            profile,
            campaign,
            name,
        } => {
            rules::merge(&mut db, rules::load(folder)?)?;
            let p = resolve(&db.profiles, profile)?.clone();
            let camp = Campaign {
                id: id(),
                name: campaign.clone(),
                notes: String::new(),
                skip_dramatis_personae: false,
            };
            let w = Warband {
                id: id(),
                campaign_id: camp.id.clone(),
                profile_id: p.id.clone(),
                name: name.clone(),
                leader_id: String::new(),
            };
            db.campaigns.push(camp);
            db.warbands.push(w.clone());
            let local = ContextData {
                wb: w.id.clone(),
                ..c.clone()
            };
            ledger(
                &mut db,
                &local,
                "funding",
                ("", ""),
                1,
                p.starting_gold,
                p.starting_gold,
            );
            if p.bonus_gold != 0 {
                ledger(
                    &mut db,
                    &local,
                    "faction-bonus",
                    ("", ""),
                    1,
                    p.bonus_gold,
                    p.bonus_gold,
                );
            }
            note(&mut db, "selection", &w.id, &local);
            output = summary(&db, &w.id);
        }
        Command::Campaign { name } => {
            db.campaigns.push(Campaign {
                id: id(),
                name: name.clone(),
                notes: String::new(),
                skip_dramatis_personae: false,
            });
        }
        Command::CampaignRules {
            skip_dramatis_personae,
        } => {
            let campaign = wb(&db, &c.wb).campaign_id.clone();
            db.campaigns
                .iter_mut()
                .find(|r| r.id == campaign)
                .unwrap()
                .skip_dramatis_personae = *skip_dramatis_personae;
            let battles: Vec<_> = db
                .battles
                .iter()
                .filter(|b| {
                    !matches!(b.status.as_str(), "complete" | "abandoned")
                        && db
                            .warbands
                            .iter()
                            .any(|w| w.id == b.warband_id && w.campaign_id == campaign)
                })
                .map(|b| b.id.clone())
                .collect();
            for s in db
                .steps
                .iter_mut()
                .filter(|s| s.name == "dramatis" && battles.contains(&s.battle_id))
            {
                if *skip_dramatis_personae && matches!(s.status.as_str(), "pending" | "in-progress")
                {
                    s.status = "skipped".into();
                    s.reason = "Campaign rule: skip Dramatis Personae".into();
                } else if !skip_dramatis_personae
                    && s.status == "skipped"
                    && s.reason == "Campaign rule: skip Dramatis Personae"
                {
                    s.status = "pending".into();
                    s.reason.clear();
                }
            }
        }
        Command::Warband { name, profile } => {
            let p = resolve(&db.profiles, profile)?.clone();
            let campaign = if let Some(q) = &cli.campaign {
                resolve(&db.campaigns, q)?
            } else {
                ensure!(db.campaigns.len() == 1, "Use --campaign");
                &db.campaigns[0]
            };
            let w = Warband {
                id: id(),
                campaign_id: campaign.id.clone(),
                profile_id: p.id,
                name: name.clone(),
                leader_id: String::new(),
            };
            let local = ContextData {
                wb: w.id.clone(),
                ..c.clone()
            };
            db.warbands.push(w.clone());
            ledger(
                &mut db,
                &local,
                "funding",
                ("", ""),
                1,
                p.starting_gold,
                p.starting_gold,
            );
            ledger(
                &mut db,
                &local,
                "faction-bonus",
                ("", ""),
                1,
                p.bonus_gold,
                p.bonus_gold,
            );
            output = json!(w);
        }
        Command::Select { warband } => {
            let w = resolve(&db.warbands, warband)?.clone();
            note(&mut db, "selection", &w.id, &c);
            output = json!(w);
        }
        Command::Show { entity } => return show(&db, &c.wb, entity),
        Command::Recruit {
            kind,
            name,
            count,
            group,
            price,
        } => {
            ensure!(*count > 0 && *count <= 1000, "Count must be 1..1000");
            let candidates: Vec<_> = db
                .types
                .iter()
                .filter(|t| t.profile_id == wb(&db, &c.wb).profile_id)
                .cloned()
                .collect();
            let t = resolve(&candidates, kind)?.clone();
            gate(
                &db,
                &mut c,
                if t.role == "dramatis" {
                    &["dramatis", "recruitment"]
                } else {
                    &["recruitment"]
                },
            )?;
            let p = db.profile(&t.profile_id).clone();
            let existing: Vec<_> = db.members_of(&c.wb).filter(|m| active(m)).collect();
            c.check(
                existing
                    .iter()
                    .filter(|m| db.typ(&db.unit(&m.unit_id).type_id).counts_roster)
                    .count() as u32
                    + if t.counts_roster { *count } else { 0 }
                    <= p.max_members,
                "Roster limit exceeded",
            )?;
            c.check(
                t.limit == 0
                    || existing
                        .iter()
                        .filter(|m| db.unit(&m.unit_id).type_id == t.id)
                        .count() as u32
                        + count
                        <= t.limit,
                "Warrior type limit exceeded",
            )?;
            c.check(
                t.role != "hero"
                    || existing
                        .iter()
                        .filter(|m| db.unit(&m.unit_id).role == "hero")
                        .count() as u32
                        + count
                        <= p.max_heroes,
                "Hero limit exceeded",
            )?;
            c.check(
                t.role == "henchman" || *count == 1,
                "Individuals must be hired one at a time",
            )?;
            let u = if let Some(q) = group {
                let u = unit(&db, &c.wb, q)?;
                ensure!(
                    u.type_id == t.id && u.role == "henchman",
                    "Replacement must match a henchman group"
                );
                u
            } else {
                Unit {
                    id: id(),
                    warband_id: c.wb.clone(),
                    type_id: t.id.clone(),
                    name: name.clone(),
                    role: t.role.clone(),
                    starting_xp: t.starting_xp,
                    earned_xp: 0,
                }
            };
            let group_members: Vec<_> = db
                .members
                .iter()
                .filter(|m| m.unit_id == u.id && active(m))
                .cloned()
                .collect();
            c.check(
                group_members.len() as u32 + count <= t.group_max,
                "Group capacity exceeded",
            )?;
            let xp = u.starting_xp + u.earned_xp;
            let veteran = if group.is_some() { xp * count } else { 0 };
            if veteran > 0 {
                let b = current_battle(&db, &c.wb)?;
                c.check(
                    b.veteran_budget >= veteran,
                    "Insufficient available veteran experience",
                )?;
                db.battles
                    .iter_mut()
                    .find(|x| x.id == b.id)
                    .unwrap()
                    .veteran_budget = b.veteran_budget.saturating_sub(veteran);
            }
            let cost = price.unwrap_or(t.cost + if group.is_some() { 2 * xp as i64 } else { 0 });
            let total = amount(*count, cost)?;
            check_budget(&db, &c.wb, -total)?;
            let loadout = group_members
                .first()
                .map(|m| {
                    db.allocations
                        .iter()
                        .filter(|a| a.member_id == m.id)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for a in &loadout {
                if db
                    .items
                    .iter()
                    .any(|i| i.id == a.item_id && i.name == "Dagger")
                {
                    continue;
                }
                ensure!(
                    db.owned(&c.wb, &a.item_id) - db.assigned(&c.wb, &a.item_id)
                        >= a.quantity * count,
                    "Not enough replacement equipment in stash"
                );
            }
            if group.is_none() {
                db.units.push(u.clone());
                let innate: Vec<_> = db
                    .permissions
                    .iter()
                    .filter(|p| p.type_id == t.id && p.kind == "starting-ability")
                    .cloned()
                    .collect();
                for perm in innate {
                    db.advances.push(Advance {
                        id: id(),
                        unit_id: u.id.clone(),
                        ability_id: perm.target_id,
                        reason: "Starting ability from pinned rules".into(),
                        ..Advance::default()
                    });
                }
            }
            let dagger = db
                .items
                .iter()
                .find(|i| i.profile_id == t.profile_id && i.name == "Dagger")
                .cloned();
            for n in 0..*count {
                let m = Member {
                    id: id(),
                    unit_id: u.id.clone(),
                    name: if t.role == "henchman" {
                        format!("{name} {}", group_members.len() + n as usize + 1)
                    } else {
                        name.clone()
                    },
                    status: "active".into(),
                    missed_games: 0,
                };
                if let Some(d) = &dagger {
                    holding(&mut db, &c.wb, &d.id, 1)?;
                    ledger(&mut db, &c, "find", (&d.id, &m.id), 1, 0, 0);
                    if !loadout.iter().any(|a| a.item_id == d.id) {
                        db.allocations.push(Allocation {
                            id: id(),
                            member_id: m.id.clone(),
                            item_id: d.id.clone(),
                            quantity: 1,
                        });
                    }
                }
                for a in &loadout {
                    db.allocations.push(Allocation {
                        id: id(),
                        member_id: m.id.clone(),
                        item_id: a.item_id.clone(),
                        quantity: a.quantity,
                    });
                }
                if t.name == "Captain" && wb(&db, &c.wb).leader_id.is_empty() {
                    db.warbands
                        .iter_mut()
                        .find(|w| w.id == c.wb)
                        .unwrap()
                        .leader_id = m.id.clone();
                }
                db.members.push(m);
            }
            ledger(&mut db, &c, "hire", ("", ""), *count, cost, -total);
            output = roster(&db, &c.wb);
        }
        Command::Remove {
            member: q,
            status,
            equipment,
        } => {
            gate(&db, &mut c, &["injuries", "recruitment"])?;
            let m = member(&db, &c.wb, q)?;
            member_depart(&mut db, &c, &m, status, equipment)?;
        }
        Command::Rename { entity, name } => {
            let id = entity_id(&db, &c.wb, entity)?;
            if let Some(x) = db.members.iter_mut().find(|x| x.id == id) {
                x.name = name.clone();
            } else if let Some(x) = db.units.iter_mut().find(|x| x.id == id) {
                x.name = name.clone();
            } else if let Some(x) = db.battles.iter_mut().find(|x| x.id == id) {
                x.name = name.clone();
            } else if let Some(x) = db.warbands.iter_mut().find(|x| x.id == id) {
                x.name = name.clone();
            } else {
                bail!("Entity cannot be renamed");
            }
        }
        Command::Xp { unit: q, amount } => {
            gate(&db, &mut c, &["experience"])?;
            let u = unit(&db, &c.wb, q)?;
            ensure!(
                db.typ(&u.type_id).gains_xp,
                "This type does not gain experience"
            );
            ensure!(*amount > 0, "XP must be positive");
            db.units
                .iter_mut()
                .find(|x| x.id == u.id)
                .unwrap()
                .earned_xp = u.earned_xp.checked_add(*amount).context("XP overflow")?;
            output = json!({"pending":db.pending(db.unit(&u.id))});
        }
        Command::Advance {
            unit: q,
            stat,
            ability,
            dice,
            initial,
        } => {
            gate(&db, &mut c, &["experience"])?;
            let u = unit(&db, &c.wb, q)?;
            let threshold = if *initial {
                c.check(
                    !db.battles.iter().any(|b| b.warband_id == c.wb),
                    "Initial advances are only for setup",
                )?;
                0
            } else {
                *db.pending(&u).first().context("No pending advance")?
            };
            let (s, a) = advance_choice(&db, &c, &u, stat.as_deref(), ability.as_deref())?;
            let dice_id = dice.as_deref().unwrap_or("");
            key_check_dice(&db, dice_id)?;
            if *initial {
                let cost = db.profile(&wb(&db, &c.wb).profile_id).initial_advance_cost;
                c.check(
                    cost > 0 || db.profile(&wb(&db, &c.wb).profile_id).classification == "house",
                    "Initial advances are not enabled by this profile",
                )?;
                check_budget(&db, &c.wb, -cost)?;
                ledger(&mut db, &c, "initial-advance", ("", ""), 1, cost, -cost);
            }
            if !*initial && threshold == 0 {
                db.advances
                    .retain(|a| !(a.unit_id == u.id && a.reason == "pending-promotion"));
            }
            db.advances.push(Advance {
                id: id(),
                unit_id: u.id,
                threshold,
                characteristic: s,
                delta: if stat.is_some() { 1 } else { 0 },
                ability_id: a,
                dice_id: dice_id.into(),
                reason: c.reason.clone(),
            });
        }
        Command::Promote {
            member: q,
            abilities,
            dice,
        } => {
            gate(&db, &mut c, &["experience"])?;
            let m = member(&db, &c.wb, q)?;
            let u = db.unit(&m.unit_id).clone();
            ensure!(
                u.role == "henchman" && active(&m),
                "Select an active henchman"
            );
            let threshold = *db.pending(&u).first().context("No pending advance")?;
            let p = db.profile(&wb(&db, &c.wb).profile_id);
            c.check(
                db.members_of(&c.wb)
                    .filter(|m| active(m) && db.unit(&m.unit_id).role == "hero")
                    .count()
                    < (p.max_heroes as usize),
                "No Hero slot available",
            )?;
            key_check_dice(&db, dice.as_deref().unwrap_or(""))?;
            let old_advances: Vec<_> = db
                .advances
                .iter()
                .filter(|a| a.unit_id == u.id)
                .cloned()
                .collect();
            let new = Unit {
                id: id(),
                warband_id: c.wb.clone(),
                type_id: u.type_id.clone(),
                name: m.name.clone(),
                role: "hero".into(),
                starting_xp: u.starting_xp + u.earned_xp,
                earned_xp: 0,
            };
            for a in old_advances {
                db.advances.push(Advance {
                    id: id(),
                    unit_id: new.id.clone(),
                    threshold: 0,
                    ..a
                });
            }
            db.units.push(new.clone());
            if let Some(list) = db
                .equipment_list_assignments
                .iter()
                .find(|a| a.entity_id == u.id)
                .map(|a| a.list_id.clone())
            {
                db.assign_equipment_list(&new.id, &list);
            }
            db.members
                .iter_mut()
                .find(|x| x.id == m.id)
                .unwrap()
                .unit_id = new.id.clone();
            note(
                &mut db,
                &new.id,
                &format!(
                    "Promoted from {} at {} XP; skill access: {}. Immediate Hero advance pending.",
                    u.id,
                    new.starting_xp,
                    abilities.join(",")
                ),
                &c,
            );
            db.advances.push(Advance {
                id: id(),
                unit_id: new.id.clone(),
                reason: "pending-promotion".into(),
                ..Advance::default()
            });
            let available: Vec<_> = db
                .abilities
                .iter()
                .filter(|a| a.profile_id == wb(&db, &c.wb).profile_id)
                .cloned()
                .collect();
            for selected in abilities {
                for a in available.iter().filter(|a| {
                    a.kind.eq_ignore_ascii_case(selected)
                        || a.name.eq_ignore_ascii_case(selected)
                        || a.id == *selected
                }) {
                    db.permissions.push(Permission {
                        id: id(),
                        type_id: new.id.clone(),
                        target_id: a.id.clone(),
                        kind: "ability".into(),
                    });
                }
            }
            if !db.members.iter().any(|m| m.unit_id == u.id && active(m)) {
                db.advances.push(Advance {
                    id: id(),
                    unit_id: u.id,
                    threshold,
                    reason: "Sole member promoted".into(),
                    ..Advance::default()
                });
            }
        }
        Command::Effect {
            member: q,
            description,
            stat,
            delta,
            condition,
            battle,
        } => {
            gate(&db, &mut c, &["injuries"])?;
            let m = member(&db, &c.wb, q)?;
            let bid = if let Some(q) = battle {
                resolve(&db.battles, q)?.id.clone()
            } else {
                String::new()
            };
            let s = stat.clone().unwrap_or_default();
            ensure!(
                s.is_empty() || STATS.contains(&s.as_str()),
                "Unknown characteristic"
            );
            db.effects.push(Effect {
                id: id(),
                member_id: m.id,
                characteristic: s,
                delta: *delta,
                description: description.clone(),
                condition: condition.clone(),
                battle_id: bid,
            });
        }
        Command::Stat {
            member: q,
            characteristic,
            value,
            battle,
            dice,
        } => {
            let m = member(&db, &c.wb, q)?;
            ensure!(
                STATS.contains(&characteristic.as_str()) && *value >= 0 && *value <= 1000,
                "Invalid characteristic/value"
            );
            key_check_dice(&db, dice.as_deref().unwrap_or(""))?;
            c.check(
                db.typ(&db.unit(&m.unit_id).type_id)
                    .stat(characteristic)
                    .parse::<i16>()
                    .is_err(),
                "Changing a fixed base characteristic requires an override",
            )?;
            let bid = if *battle {
                let b = current_battle(&db, &c.wb)?;
                ensure!(
                    ["prepared", "in-progress"].contains(&b.status.as_str()),
                    "Resolve battle characteristics before post-battle"
                );
                b.id
            } else {
                gate(&db, &mut c, &["injuries"])?;
                String::new()
            };
            db.effects.push(Effect {
                id: id(),
                member_id: m.id.clone(),
                characteristic: characteristic.clone(),
                delta: *value,
                description: "Resolved base characteristic".into(),
                condition: String::new(),
                battle_id: bid,
            });
            note(
                &mut db,
                &m.id,
                &format!(
                    "Resolved {}={} (dice {}): {}",
                    characteristic,
                    value,
                    dice.as_deref().unwrap_or("manual"),
                    c.reason
                ),
                &c,
            );
        }
        Command::Status {
            member: q,
            status,
            missed,
            leader,
        } => {
            gate(&db, &mut c, &["injuries", "recruitment"])?;
            ensure!(
                ["active", "captured", "recovering"].contains(&status.as_str()),
                "Use remove for permanent departures"
            );
            let m = member(&db, &c.wb, q)?;
            ensure!(active(&m), "Restore/undo a departure before reactivation");
            let x = db.members.iter_mut().find(|x| x.id == m.id).unwrap();
            x.status = status.clone();
            x.missed_games = *missed;
            if *leader {
                ensure!(
                    db.unit(&m.unit_id).role == "hero" && status == "active",
                    "Leader must be an active Hero"
                );
                db.warbands
                    .iter_mut()
                    .find(|w| w.id == c.wb)
                    .unwrap()
                    .leader_id = m.id;
            }
        }
        Command::Buy {
            item: q,
            quantity,
            price,
        } => {
            let it = item(&db, &c.wb, q)?;
            gate(
                &db,
                &mut c,
                if it.rarity > 0 {
                    &["rare"]
                } else {
                    &["recruitment"]
                },
            )?;
            let mut search = None;
            if it.rarity > 0 && current_battle(&db, &c.wb).is_ok() {
                let b = current_battle(&db, &c.wb)?;
                ensure!(
                    *quantity == 1,
                    "One rare item may be purchased per successful search"
                );
                search = db.searches.iter().position(|s| {
                    s.battle_id == b.id
                        && s.kind == "rare"
                        && s.used > 0
                        && !s.purchased
                        && ((s.outcome == "found" && s.item_id == it.id)
                            || (s.outcome.is_empty() && s.item_id.is_empty()))
                });
                ensure!(
                    search.is_some(),
                    "Record a successful search for this rare item first"
                );
            }
            let price = price.unwrap_or(it.cost);
            buy(&mut db, &c, &it, *quantity, price, "purchase")?;
            if let Some(i) = search {
                db.searches[i].purchased = true;
            }
        }
        Command::Find { item: q, quantity } => {
            gate(&db, &mut c, &["exploration", "recruitment"])?;
            let it = item(&db, &c.wb, q)?;
            buy(&mut db, &c, &it, *quantity, 0, "find")?;
        }
        Command::Lose { item: q, quantity } => {
            gate(&db, &mut c, &["injuries", "exploration", "equipment"])?;
            let it = item(&db, &c.wb, q)?;
            ensure!(
                *quantity > 0 && db.owned(&c.wb, &it.id) - db.assigned(&c.wb, &it.id) >= *quantity,
                "Loss requires sufficient unassigned items; unequip first"
            );
            holding(&mut db, &c.wb, &it.id, -(*quantity as i64))?;
            ledger(&mut db, &c, "loss", (&it.id, ""), *quantity, 0, 0);
        }
        Command::Sell {
            item: q,
            quantity,
            price,
        } => {
            gate(&db, &mut c, &["wyrdstone", "recruitment"])?;
            let it = item(&db, &c.wb, q)?;
            ensure!(
                it.name != "Wyrdstone",
                "Use wyrdstone SHARDS GOLD to enforce sale allowance"
            );
            ensure!(
                *quantity > 0 && db.owned(&c.wb, &it.id) - db.assigned(&c.wb, &it.id) >= *quantity,
                "Insufficient unassigned inventory"
            );
            let gold = amount(*quantity, *price)?;
            holding(&mut db, &c.wb, &it.id, -(*quantity as i64))?;
            ledger(&mut db, &c, "sale", (&it.id, ""), *quantity, *price, gold);
        }
        Command::Equip {
            unit: q,
            item: iq,
            quantity,
        } => {
            gate(&db, &mut c, &["equipment"])?;
            ensure!(*quantity > 0, "Quantity must be positive");
            let u = unit(&db, &c.wb, q)?;
            let it = item(&db, &c.wb, iq)?;
            c.check(
                db.members
                    .iter()
                    .filter(|m| m.unit_id == u.id && active(m))
                    .all(|m| db.equipment_allowed(&m.id, &it.id)),
                "Equipment is not on every member's assigned equipment list",
            )?;
            let ms: Vec<_> = db
                .members
                .iter()
                .filter(|m| m.unit_id == u.id && active(m))
                .cloned()
                .collect();
            ensure!(!ms.is_empty(), "No active members");
            db.allocations
                .retain(|a| !(a.item_id == it.id && ms.iter().any(|m| m.id == a.member_id)));
            for m in ms {
                db.allocations.push(Allocation {
                    id: id(),
                    member_id: m.id.clone(),
                    item_id: it.id.clone(),
                    quantity: *quantity,
                });
                for (category, limit) in [("melee", 2), ("missile", 2), ("armour", 1)] {
                    let count: u32 = db
                        .allocations
                        .iter()
                        .filter(|a| {
                            a.member_id == m.id
                                && db.items.iter().any(|i| {
                                    i.id == a.item_id
                                        && i.category == category
                                        && i.name != "Dagger"
                                })
                        })
                        .map(|a| a.quantity)
                        .sum();
                    c.check(count <= limit, "Equipment loadout limit exceeded")?;
                }
            }
            ensure!(
                db.assigned(&c.wb, &it.id) <= db.owned(&c.wb, &it.id),
                "Insufficient items for every member of the group"
            );
        }
        Command::Unequip { unit: q, item: iq } => {
            gate(&db, &mut c, &["equipment", "injuries"])?;
            let u = unit(&db, &c.wb, q)?;
            let it = item(&db, &c.wb, iq)?;
            let members: Vec<_> = db
                .members
                .iter()
                .filter(|m| m.unit_id == u.id)
                .map(|m| m.id.clone())
                .collect();
            db.allocations
                .retain(|a| !(members.contains(&a.member_id) && a.item_id == it.id));
        }
        Command::Gold { amount, category } => {
            gate(&db, &mut c, &["exploration", "recruitment", "rare"])?;
            check_budget(&db, &c.wb, *amount)?;
            ledger(
                &mut db,
                &c,
                category,
                ("", ""),
                1,
                amount.checked_abs().context("Amount out of range")?,
                *amount,
            );
        }
        Command::Wyrdstone { shards, gold } => {
            gate(&db, &mut c, &["wyrdstone"])?;
            ensure!(*gold >= 0, "Gold received must be nonnegative");
            let it = item(&db, &c.wb, "Wyrdstone")?;
            ensure!(
                db.owned(&c.wb, &it.id) - db.assigned(&c.wb, &it.id) >= *shards,
                "Insufficient unassigned wyrdstone"
            );
            if let Ok(b) = current_battle(&db, &c.wb) {
                resolution(
                    &mut db,
                    &b.id,
                    "wyrdstone-sale",
                    &format!("{shards} shards for {gold} gold; {}", c.reason),
                    "",
                )?;
            }
            if *shards > 0 {
                holding(&mut db, &c.wb, &it.id, -(*shards as i64))?;
            }
            ledger(
                &mut db,
                &c,
                "wyrdstone-sale",
                (&it.id, ""),
                *shards,
                0,
                *gold,
            );
        }
        Command::Note {
            entity,
            text,
            replace,
        } => {
            let target = entity_id(&db, &c.wb, entity)?;
            ensure!(!text.trim().is_empty(), "Note cannot be empty");
            if let Some(id) = replace {
                let old = db
                    .notes
                    .iter()
                    .find(|n| n.id == *id && n.entity_id == target)
                    .context("Unknown note to replace")?
                    .clone();
                note(
                    &mut db,
                    &target,
                    &format!("Supersedes note {}: {}", old.id, text),
                    &c,
                );
            } else {
                note(&mut db, &target, text, &c);
            }
        }
        Command::Roll {
            notation,
            results,
            rerolls,
            table,
            choice,
        } => {
            validate_expression(notation)?;
            let (n, s, modifier) = parse_dice(notation)?;
            let values = if results.is_empty() {
                use rand::Rng;
                let mut rng = rand::rng();
                (0..n).map(|_| rng.random_range(1..=s)).collect::<Vec<_>>()
            } else {
                ensure!(
                    results.len() == n as usize && results.iter().all(|r| *r >= 1 && *r <= s),
                    "Results don't match dice notation"
                );
                results.clone()
            };
            let d = Dice {
                id: id(),
                notation: notation.clone(),
                results: values
                    .iter()
                    .map(|r| r.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
                modifiers: modifier,
                rerolls: rerolls.clone(),
                table_version: table.clone(),
                choice: choice.clone(),
                source: if results.is_empty() {
                    "tool"
                } else {
                    "physical"
                }
                .into(),
            };
            output = json!({"total":values.iter().sum::<u32>() as i32+modifier as i32,"dice":d});
            db.dice.push(d);
        }
        Command::Battle(b) => {
            output = battle(&mut db, &c, b, &store.generation)?;
        }
        Command::RareSearch {
            member: q,
            item: iq,
            found,
        } => {
            gate(&db, &mut c, &["rare"])?;
            let b = current_battle(&db, &c.wb)?;
            let m = member(&db, &c.wb, q)?;
            ensure!(
                db.rare_search_eligible(&b.id, &m),
                "Only a participating Hero not taken out of action may search for rare items"
            );
            ensure!(
                db.rare_search_remaining(&b.id, &m.id) > 0,
                "This Hero has already used their one rare-item search"
            );
            let it = item(&db, &c.wb, iq)?;
            ensure!(
                it.rarity > 0,
                "Choose a rare item; common items require no search"
            );
            if let Some(s) = db
                .searches
                .iter_mut()
                .find(|s| s.battle_id == b.id && s.member_id == m.id && s.kind == "rare")
            {
                s.allowance = 1;
                s.used = 1;
                s.item_id = it.id;
                s.outcome = if *found { "found" } else { "failed" }.into();
                s.purchased = false;
            } else {
                db.searches.push(Search {
                    id: id(),
                    battle_id: b.id,
                    member_id: m.id,
                    kind: "rare".into(),
                    allowance: 1,
                    used: 1,
                    item_id: it.id,
                    outcome: if *found { "found" } else { "failed" }.into(),
                    purchased: false,
                });
            }
        }
        Command::Search {
            member: q,
            kind,
            allowance,
            use_attempt,
        } => {
            let b = current_battle(&db, &c.wb)?;
            gate(
                &db,
                &mut c,
                match kind.as_str() {
                    "rare" => &["rare"],
                    "dramatis" => &["dramatis"],
                    _ => &["exploration", "rare", "dramatis"],
                },
            )?;
            let m = member(&db, &c.wb, q)?;
            ensure!(
                m.status == "active" && db.unit(&m.unit_id).role == "hero",
                "Searcher must be an active Hero"
            );
            if kind == "rare" {
                ensure!(
                    db.rare_search_eligible(&b.id, &m),
                    "Hero was taken out of action or did not participate"
                );
                ensure!(
                    allowance.is_none_or(|n| n <= 1),
                    "Each eligible Hero has one rare-item search"
                );
                if *use_attempt {
                    ensure!(
                        db.rare_search_remaining(&b.id, &m.id) > 0,
                        "This Hero has already searched"
                    );
                }
            }
            if let Some(n) = allowance {
                if let Some(s) = db
                    .searches
                    .iter_mut()
                    .find(|s| s.battle_id == b.id && s.member_id == m.id && s.kind == *kind)
                {
                    s.allowance = *n;
                } else {
                    db.searches.push(Search {
                        id: id(),
                        battle_id: b.id.clone(),
                        member_id: m.id.clone(),
                        kind: kind.clone(),
                        allowance: *n,
                        used: 0,
                        ..Search::default()
                    });
                }
            }
            if *use_attempt {
                let s = db
                    .searches
                    .iter_mut()
                    .find(|s| s.battle_id == b.id && s.member_id == m.id && s.kind == *kind)
                    .context("Set an allowance first")?;
                ensure!(s.used < s.allowance, "No search attempts left");
                s.used += 1;
            }
        }
        Command::Veterans { experience } => {
            gate(&db, &mut c, &["veterans"])?;
            let b = current_battle(&db, &c.wb)?;
            resolution(
                &mut db,
                &b.id,
                "veterans",
                &format!("Available veteran experience: {experience}"),
                "",
            )?;
            db.battles
                .iter_mut()
                .find(|r| r.id == b.id)
                .unwrap()
                .veteran_budget = *experience;
        }
        Command::Resolve {
            key,
            description,
            gold,
            shards,
            dice,
        } => {
            ensure!(
                !key.starts_with("kill:"),
                "Kill credits must use battle kill"
            );
            gate(
                &db,
                &mut c,
                &[
                    "injuries",
                    "experience",
                    "exploration",
                    "veterans",
                    "rare",
                    "dramatis",
                    "rating",
                ],
            )?;
            let b = current_battle(&db, &c.wb)?;
            resolution(
                &mut db,
                &b.id,
                key,
                description,
                dice.as_deref().unwrap_or(""),
            )?;
            if *gold != 0 {
                check_budget(&db, &c.wb, *gold)?;
                ledger(
                    &mut db,
                    &c,
                    "reward",
                    ("", ""),
                    1,
                    gold.checked_abs().context("Amount out of range")?,
                    *gold,
                );
            }
            if *shards > 0 {
                let it = item(&db, &c.wb, "Wyrdstone")?;
                holding(&mut db, &c.wb, &it.id, *shards as i64)?;
                ledger(&mut db, &c, "find", (&it.id, ""), *shards, 0, 0);
            }
            if key == "veterans" {
                let d = db
                    .dice
                    .iter()
                    .find(|d| Some(&d.id) == dice.as_ref())
                    .context("Veteran availability requires --dice ID")?;
                let n = d
                    .results
                    .split(',')
                    .map(|r| r.parse::<u32>().unwrap_or(0))
                    .sum::<u32>();
                db.battles
                    .iter_mut()
                    .find(|x| x.id == b.id)
                    .unwrap()
                    .veteran_budget = n;
            }
        }
        Command::Step { name, skip, begin } => {
            let b = current_battle(&db, &c.wb)?;
            ensure!(b.status == "post-battle", "Not in post-battle");
            ensure!(
                db.steps.iter().any(|s| s.battle_id == b.id
                    && s.name == *name
                    && !matches!(s.status.as_str(), "completed" | "skipped")),
                "Stage {name} is already resolved or unavailable"
            );
            if !skip && !begin {
                validate_step(&db, &c, &b, name)?;
            }
            if !skip
                && !begin
                && name == "experience"
                && !db
                    .resolutions
                    .iter()
                    .any(|r| r.battle_id == b.id && r.key == "experience")
            {
                resolution(
                    &mut db,
                    &b.id,
                    "experience",
                    "Player confirmed XP and plus-ups by completing this stage",
                    "",
                )?;
            }
            let s = db
                .steps
                .iter_mut()
                .find(|s| s.battle_id == b.id && s.name == *name)
                .unwrap();
            s.status = if *begin {
                "in-progress"
            } else if *skip {
                "skipped"
            } else {
                "completed"
            }
            .into();
            s.reason = c.reason.clone();
        }
        Command::Upkeep {
            member: q,
            waive,
            depart,
        } => {
            gate(&db, &mut c, &["recruitment"])?;
            let b = current_battle(&db, &c.wb)?;
            let m = member(&db, &c.wb, q)?;
            let t = db.typ(&db.unit(&m.unit_id).type_id).clone();
            ensure!(t.upkeep > 0, "No upkeep for this member");
            resolution(
                &mut db,
                &b.id,
                &format!("upkeep-{}", m.id),
                &format!("waive={waive}, depart={depart}"),
                "",
            )?;
            if *depart {
                member_depart(&mut db, &c, &m, "dismissed", "lose")?;
            } else if !waive {
                check_budget(&db, &c.wb, -t.upkeep)?;
                ledger(&mut db, &c, "upkeep", ("", &m.id), 1, t.upkeep, -t.upkeep);
            }
        }
        Command::Snapshot { name } => {
            db.snapshots.push(Snapshot {
                id: id(),
                name: name.clone(),
                generation: store.generation.clone(),
                warband_id: c.wb.clone(),
            });
        }
        Command::Diff { left, right } => {
            let l = resolve(&db.snapshots, left)?;
            let a = store.historical(&l.generation)?;
            let b = if let Some(r) = right {
                store.historical(&resolve(&db.snapshots, r)?.generation)?
            } else {
                db.clone()
            };
            ensure!(
                a.warbands.iter().any(|w| w.id == c.wb) && b.warbands.iter().any(|w| w.id == c.wb),
                "Snapshot lacks selected warband"
            );
            return Ok(
                json!({"left":{"summary":summary(&a,&c.wb),"roster":roster(&a,&c.wb),"inventory":inventory(&a,&c.wb)},"right":{"summary":summary(&b,&c.wb),"roster":roster(&b,&c.wb),"inventory":inventory(&b,&c.wb)}}),
            );
        }
        Command::Restore { snapshot } => {
            ensure!(!c.reason.is_empty(), "Restoration needs --reason");
            let s = resolve(&db.snapshots, snapshot)?;
            let mut old = store.historical(&s.generation)?;
            old.snapshots = db.snapshots.clone();
            old.events = db.events.clone();
            db = old;
        }
        Command::Undo => {
            ensure!(!c.reason.is_empty(), "Undo needs --reason");
            let e = db.events.last().context("No action to undo")?;
            let mut old = store.historical(&e.parent_generation)?;
            old.snapshots.extend(
                db.snapshots
                    .iter()
                    .filter(|s| !old.snapshots.iter().any(|a| a.id == s.id))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            old.events = db.events.clone();
            db = old;
        }
        Command::Import {
            file,
            apply,
            reconcile,
        } => {
            let mut imported = if file.is_dir() {
                read_tables(file)?
            } else {
                serde_json::from_slice::<Db>(&fs::read(file)?)?
            };
            imported.normalize_equipment_lists()?;
            imported.validate()?;
            ensure!(
                *reconcile || db.warbands.is_empty(),
                "Opening-state import requires an empty database; use --reconcile with a copied export for explicit reconciliation"
            );
            if !apply {
                return Ok(
                    json!({"preview":true,"warbands":imported.warbands.iter().map(|w|json!({"summary":summary(&imported,&w.id),"roster":roster(&imported,&w.id),"inventory":inventory(&imported,&w.id)})).collect::<Vec<_>>(),"notes":imported.notes}),
                );
            }
            let previous = db.clone();
            db = imported;
            if *reconcile {
                ensure!(!c.reason.is_empty(), "Reconciliation requires --reason");
                ensure!(
                    db.events == previous.events && db.snapshots == previous.snapshots,
                    "Reconciliation cannot alter history/snapshot metadata; edit domain tables only"
                );
                for b in db
                    .battles
                    .iter()
                    .filter(|b| !["complete", "abandoned"].contains(&b.status.as_str()))
                {
                    ensure!(
                        previous.battles.iter().any(|p| p == b),
                        "Reconcile active battle changes through battle commands"
                    );
                }
            } else {
                ensure!(
                    db.events.is_empty() && db.snapshots.is_empty(),
                    "Use recover for historical bundles; opening-state import must not contain history"
                );
            }
        }
        Command::Export { file, format } => {
            export(store, &c.wb, file, format)?;
            return Ok(json!({"exported":file,"format":format}));
        }
        Command::Recover { file } => {
            store.restore_bundle(serde_json::from_slice::<Bundle>(&fs::read(file)?)?)?;
            return Ok(json!({"recovered":true,"generation":store.generation}));
        }
        Command::MigrateRules { profile, apply } => {
            gate(&db, &mut c, &[])?;
            let p = resolve(&db.profiles, profile)?.clone();
            let next = migrate_profile(&db, &c, &p.id)?;
            if !apply {
                return Ok(
                    json!({"preview":true,"before":summary(&db,&c.wb),"after":summary(&next,&c.wb),"before_roster":roster(&db,&c.wb),"after_roster":roster(&next,&c.wb),"inventory":inventory(&next,&c.wb)}),
                );
            }
            ensure!(!c.reason.is_empty(), "Migration requires --reason");
            db = next;
        }
        Command::Ai { prompt, send } => return crate::ai::advise(&db, &c.wb, prompt, *send),
        Command::Tui => bail!("TUI must be opened by the terminal frontend"),
    }
    db.validate()?;
    let action = if c.override_rules {
        format!("RULES OVERRIDE: {cmd:?}")
    } else {
        format!("{cmd:?}")
    };
    store.commit(
        db,
        &action,
        &c.date,
        &c.reason,
        if c.reason.to_lowercase().starts_with("ai-assisted") {
            "ai-assisted"
        } else {
            "manual"
        },
    )?;
    Ok(output)
}
fn catalog_targets(db: &Db, warband: &str, selected: &[String]) -> Result<Vec<String>> {
    let profile = &wb(db, warband).profile_id;
    let types: Vec<_> = db
        .types
        .iter()
        .filter(|t| &t.profile_id == profile)
        .cloned()
        .collect();
    let units: Vec<_> = db
        .units
        .iter()
        .filter(|u| u.warband_id == warband)
        .cloned()
        .collect();
    let mut targets = vec![];
    for name in selected {
        if name == "all" || name == "heroes" {
            targets.extend(
                types
                    .iter()
                    .filter(|t| name == "all" || t.role != "henchman")
                    .map(|t| t.id.clone()),
            );
            if name == "heroes" {
                targets.extend(
                    units
                        .iter()
                        .filter(|u| u.role == "hero")
                        .map(|u| u.id.clone()),
                );
            }
        } else if let Ok(t) = resolve(&types, name) {
            targets.push(t.id.clone());
        } else {
            targets.push(resolve(&units, name)?.id.clone());
        }
    }
    targets.sort();
    targets.dedup();
    ensure!(
        !targets.is_empty(),
        "Choose at least one eligible warrior type or unit"
    );
    Ok(targets)
}
fn advance_choice(
    db: &Db,
    c: &ContextData,
    u: &Unit,
    stat: Option<&str>,
    ability: Option<&str>,
) -> Result<(String, String)> {
    ensure!(
        stat.is_some() != ability.is_some(),
        "Choose exactly one of --stat or --ability"
    );
    if let Some(s) = stat {
        ensure!(
            STATS.contains(&s),
            "Characteristic must be one of {}",
            STATS.join(", ")
        );
        c.check(
            if u.role == "henchman" {
                ["I", "S", "BS", "WS", "A", "Ld"].contains(&s)
            } else {
                s != "M"
            },
            "Characteristic is not on this role's advance table",
        )?;
        if u.role == "henchman" {
            c.check(
                !db.advances
                    .iter()
                    .any(|a| a.unit_id == u.id && a.characteristic == s),
                "Henchman characteristic already advanced",
            )?;
        }
        for m in db.members.iter().filter(|m| m.unit_id == u.id && active(m)) {
            let base = db.stat(m, s, None).parse::<i16>().context(
                "Random characteristic: resolve manually with an effect and documented override",
            )?;
            let cap = db
                .caps
                .iter()
                .find(|a| a.type_id == u.type_id && a.characteristic == s)
                .map_or(i16::MAX, |a| a.maximum);
            c.check(base < cap, "Characteristic at maximum")?;
        }
        Ok((s.into(), String::new()))
    } else {
        c.check(
            u.role != "henchman",
            "Henchmen must be promoted before learning skills",
        )?;
        let candidates: Vec<_> = db
            .abilities
            .iter()
            .filter(|a| a.profile_id == wb(db, &c.wb).profile_id)
            .cloned()
            .collect();
        let a = resolve(&candidates, ability.unwrap())?;
        c.check(
            db.permissions.iter().any(|p| {
                (p.type_id == u.type_id || p.type_id == u.id)
                    && p.kind == "ability"
                    && p.target_id == a.id
            }),
            "Ability not available to this warrior",
        )?;
        ensure!(
            !db.advances
                .iter()
                .any(|b| b.unit_id == u.id && b.ability_id == a.id),
            "Ability already learned"
        );
        Ok((String::new(), a.id.clone()))
    }
}
fn entity_id(db: &Db, w: &str, q: &str) -> Result<String> {
    if q == "warband" || q == w {
        return Ok(w.into());
    }
    let mut all: Vec<(String, String)> = db
        .members_of(w)
        .map(|m| (m.id.clone(), m.name.clone()))
        .collect();
    all.extend(
        db.units
            .iter()
            .filter(|u| u.warband_id == w)
            .map(|u| (u.id.clone(), u.name.clone())),
    );
    all.extend(
        db.battles
            .iter()
            .filter(|b| b.warband_id == w)
            .map(|b| (b.id.clone(), b.name.clone())),
    );
    if let Some((id, _)) = all.iter().find(|(id, _)| id == q) {
        return Ok(id.clone());
    }
    let matching: Vec<_> = db
        .members_of(w)
        .filter(|m| m.name.eq_ignore_ascii_case(q))
        .collect();
    if matching.len() == 1
        && !db.units.iter().any(|u| {
            u.warband_id == w && u.name.eq_ignore_ascii_case(q) && u.id != matching[0].unit_id
        })
    {
        return Ok(matching[0].id.clone());
    }
    let ids: std::collections::BTreeSet<_> = all
        .iter()
        .filter(|(_, name)| name.eq_ignore_ascii_case(q))
        .map(|(id, _)| id.clone())
        .collect();
    ensure!(
        ids.len() == 1,
        "Unknown or ambiguous entity {q:?}; use an ID"
    );
    Ok(ids.into_iter().next().unwrap())
}
fn show(db: &Db, w: &str, q: &str) -> Result<Value> {
    let id = entity_id(db, w, q)?;
    if id == w {
        return Ok(summary(db, w));
    }
    if let Some(m) = db.members.iter().find(|m| m.id == id) {
        let u = db.unit(&m.unit_id);
        return Ok(
            json!({"member":m,"unit":u,"base":db.typ(&u.type_id),"details":roster(db,w).as_array().unwrap().iter().find(|r|r["id"]==id),"notes":db.notes.iter().filter(|n|n.entity_id==id||n.entity_id==u.id).collect::<Vec<_>>()}),
        );
    }
    if let Some(u) = db.units.iter().find(|u| u.id == id) {
        return Ok(
            json!({"unit":u,"members":db.members.iter().filter(|m|m.unit_id==id).collect::<Vec<_>>(),"advances":db.advances.iter().filter(|a|a.unit_id==id).collect::<Vec<_>>(),"pending":db.pending(u),"notes":db.notes.iter().filter(|n|n.entity_id==id).collect::<Vec<_>>()}),
        );
    }
    Ok(json!(db.battles.iter().find(|b| b.id == id)))
}
fn parse_dice(expr: &str) -> Result<(u32, u32, i16)> {
    let expr = expr.to_ascii_uppercase();
    let (n, r) = expr
        .split_once('D')
        .context("Use dice notation such as D6 or 2D6+1")?;
    let n = if n.is_empty() { 1 } else { n.parse()? };
    let pos = r.find(['+', '-']);
    let (s, m) = if let Some(p) = pos {
        (r[..p].parse()?, r[p..].parse()?)
    } else {
        (r.parse()?, 0)
    };
    Ok((n, s, m))
}
fn validate_step(db: &Db, c: &ContextData, b: &Battle, name: &str) -> Result<()> {
    match name {
        "injuries" => {
            ensure!(
                !db.participants.iter().any(|p| p.battle_id == b.id
                    && p.condition == "out-of-action"
                    && !p.injury_resolved),
                "Resolve all out-of-action injuries first"
            );
            c.check(
                !wb(db, &c.wb).leader_id.is_empty(),
                "Choose a successor before continuing",
            )?;
        }
        "experience" => {
            ensure!(
                !db.units
                    .iter()
                    .any(|u| u.warband_id == c.wb && !db.pending(u).is_empty()),
                "Resolve pending advances first"
            );
        }
        "exploration" => ensure!(
            db.resolutions
                .iter()
                .any(|r| r.battle_id == b.id && r.key == "exploration"),
            "Use resolve exploration DESCRIPTION with optional --dice, --gold, --shards"
        ),
        "veterans" => ensure!(
            db.resolutions
                .iter()
                .any(|r| r.battle_id == b.id && r.key == "veterans"),
            "Resolve veteran availability or explicitly skip"
        ),
        "recruitment" => {
            for m in db
                .members_of(&c.wb)
                .filter(|m| active(m) && db.typ(&db.unit(&m.unit_id).type_id).upkeep > 0)
            {
                ensure!(
                    db.resolutions
                        .iter()
                        .any(|r| r.battle_id == b.id && r.key == format!("upkeep-{}", m.id)),
                    "Resolve upkeep for {}",
                    m.name
                );
            }
        }
        "equipment" => {
            for u in db
                .units
                .iter()
                .filter(|u| u.warband_id == c.wb && u.role == "henchman")
            {
                let ms: Vec<_> = db
                    .members
                    .iter()
                    .filter(|m| m.unit_id == u.id && active(m))
                    .collect();
                let mut first = None;
                for m in ms {
                    let mut gear: Vec<_> = db
                        .allocations
                        .iter()
                        .filter(|a| a.member_id == m.id)
                        .map(|a| (&a.item_id, a.quantity))
                        .collect();
                    gear.sort();
                    if let Some(ref f) = first {
                        ensure!(*f == gear, "Group {} has unequal equipment", u.name);
                    } else {
                        first = Some(gear);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}
fn battle(db: &mut Db, c: &ContextData, cmd: &BattleCommand, generation: &str) -> Result<Value> {
    match cmd {
        BattleCommand::Prepare {
            name,
            scenario,
            opponent,
            rating,
        } => {
            ensure!(
                current_battle(db, &c.wb).is_err(),
                "A battle is already active"
            );
            c.check(
                db.members_of(&c.wb).filter(|m| active(m)).count() >= 3,
                "At least three members required",
            )?;
            c.check(!wb(db, &c.wb).leader_id.is_empty(), "A leader is required")?;
            ensure!(
                !db.units
                    .iter()
                    .any(|u| u.warband_id == c.wb && !db.pending(u).is_empty()),
                "Resolve pending advances first"
            );
            let snap = Snapshot {
                id: id(),
                name: format!("Before {name}"),
                generation: generation.into(),
                warband_id: c.wb.clone(),
            };
            let b = Battle {
                id: id(),
                warband_id: c.wb.clone(),
                name: name.clone(),
                date: c.date.clone(),
                scenario: scenario.clone(),
                opponent: opponent.clone(),
                opponent_rating: *rating,
                rating: db.rating(&c.wb),
                status: "prepared".into(),
                pre_snapshot: snap.id.clone(),
                ..Battle::default()
            };
            let ms: Vec<_> = db
                .members_of(&c.wb)
                .filter(|m| {
                    m.status == "active"
                        && m.missed_games == 0
                        && db.typ(&db.unit(&m.unit_id).type_id).counts_battle
                })
                .cloned()
                .collect();
            for m in ms {
                let wounds = db.stat(&m, "W", None).parse().unwrap_or(1);
                db.participants.push(Participant {
                    id: id(),
                    battle_id: b.id.clone(),
                    member_id: m.id,
                    wounds,
                    condition: "standing".into(),
                    ..Participant::default()
                });
            }
            for (i, name) in STEPS.iter().enumerate() {
                let skip = *name == "dramatis" && db.skips_dramatis(&c.wb);
                db.steps.push(Step {
                    id: id(),
                    battle_id: b.id.clone(),
                    position: i as u32,
                    name: name.to_string(),
                    status: if skip { "skipped" } else { "pending" }.into(),
                    reason: if skip {
                        "Campaign rule: skip Dramatis Personae"
                    } else {
                        ""
                    }
                    .into(),
                });
            }
            db.snapshots.push(snap);
            db.battles.push(b.clone());
            return Ok(json!(b));
        }
        BattleCommand::Start => {
            let b = current_battle(db, &c.wb)?;
            ensure!(b.status == "prepared", "Battle must be prepared");
            db.battles.iter_mut().find(|x| x.id == b.id).unwrap().status = "in-progress".into();
        }
        BattleCommand::Record {
            member: q,
            condition,
            wounds,
            kills,
            objectives,
            attribution,
        } => {
            let b = current_battle(db, &c.wb)?;
            ensure!(b.status == "in-progress", "Battle must be in progress");
            let m = member(db, &c.wb, q)?;
            if let Some(n) = kills {
                let p = db
                    .participants
                    .iter()
                    .find(|p| p.battle_id == b.id && p.member_id == m.id)
                    .context("Member is not participating")?;
                ensure!(
                    *n as usize >= db.battle_kills(p).count(),
                    "Remove individual recorded kill credits before lowering this total (Battle: J)"
                );
            }
            let p = db
                .participants
                .iter_mut()
                .find(|p| p.battle_id == b.id && p.member_id == m.id)
                .context("Member is not participating")?;
            if let Some(s) = condition {
                ensure!(
                    [
                        "standing",
                        "knocked-down",
                        "stunned",
                        "out-of-action",
                        "routed"
                    ]
                    .contains(&s.as_str()),
                    "Invalid condition"
                );
                p.condition = s.clone();
            }
            if let Some(n) = wounds {
                ensure!(*n >= 0, "Current wounds cannot be negative");
                p.wounds = *n;
            }
            if let Some(n) = kills {
                p.kills = *n;
            }
            if let Some(n) = objectives {
                p.objectives = *n;
            }
            // Keep old attribution fields readable in historical generations.
            // New free-form battle observations are ordinary model notes.
            if !attribution.trim().is_empty() {
                note(db, &m.id, &format!("{}: {}", b.name, attribution.trim()), c);
            }
        }
        BattleCommand::Kill {
            member: q,
            victim,
            remove,
        } => {
            let b = current_battle(db, &c.wb)?;
            ensure!(b.status == "in-progress", "Battle must be in progress");
            let m = member(db, &c.wb, q)?;
            let p = db
                .participants
                .iter()
                .find(|p| p.battle_id == b.id && p.member_id == m.id)
                .context("Member is not participating")?
                .clone();
            let total = if let Some(record) = remove {
                ensure!(
                    victim.trim().is_empty(),
                    "Choose either a victim to credit or a kill to remove"
                );
                ensure!(p.kills > 0, "No credited kills to remove");
                if record == "unspecified" {
                    ensure!(
                        p.kills as usize > db.battle_kills(&p).count(),
                        "No unspecified kills to remove"
                    );
                } else {
                    let kill = db
                        .battle_kills(&p)
                        .find(|r| r.id == *record)
                        .context("Kill record does not belong to this participant in this battle")?
                        .id
                        .clone();
                    db.resolutions.retain(|r| r.id != kill);
                }
                p.kills - 1
            } else {
                let total = p.kills.checked_add(1).context("Kill total overflow")?;
                resolution(
                    db,
                    &b.id,
                    &format!("kill:{}:{}", p.id, id()),
                    victim.trim(),
                    "",
                )?;
                total
            };
            db.participants
                .iter_mut()
                .find(|row| row.id == p.id)
                .unwrap()
                .kills = total;
        }
        BattleCommand::Consume {
            member: q,
            item: iq,
            quantity,
        } => {
            let b = current_battle(db, &c.wb)?;
            ensure!(b.status == "in-progress", "Battle must be in progress");
            ensure!(*quantity > 0, "Quantity must be positive");
            let m = member(db, &c.wb, q)?;
            ensure!(
                db.participants
                    .iter()
                    .any(|p| p.battle_id == b.id && p.member_id == m.id),
                "Not a participant"
            );
            let it = item(db, &c.wb, iq)?;
            let a = db
                .allocations
                .iter_mut()
                .find(|a| a.member_id == m.id && a.item_id == it.id)
                .context("Item not equipped")?;
            ensure!(a.quantity >= *quantity, "Insufficient equipped items");
            a.quantity -= quantity;
            db.allocations.retain(|a| a.quantity > 0);
            holding(db, &c.wb, &it.id, -(*quantity as i64))?;
            ledger(db, c, "consumption", (&it.id, &m.id), *quantity, 0, 0);
        }
        BattleCommand::End { result } => {
            let b = current_battle(db, &c.wb)?;
            ensure!(b.status == "in-progress", "Battle must be in progress");
            let x = db.battles.iter_mut().find(|x| x.id == b.id).unwrap();
            x.status = "post-battle".into();
            x.result = result.clone();
            let relevant: Vec<_> = db.members_of(&c.wb).map(|m| m.id.clone()).collect();
            for m in db.members.iter_mut().filter(|m| relevant.contains(&m.id)) {
                if m.missed_games > 0 {
                    m.missed_games -= 1;
                    if m.missed_games == 0 && m.status == "recovering" {
                        m.status = "active".into();
                    }
                }
            }
        }
        BattleCommand::Injury {
            member: q,
            outcome,
            dice,
            stat,
            delta,
            missed,
        } => {
            let b = current_battle(db, &c.wb)?;
            ensure!(
                b.status == "post-battle" && stage(db, &b.id) == "injuries",
                "Resolve injuries before later stages; undo dependent changes to amend resolved injuries"
            );
            let m = member(db, &c.wb, q)?;
            let p = db
                .participants
                .iter()
                .find(|p| p.battle_id == b.id && p.member_id == m.id)
                .context("Not a participant")?;
            ensure!(
                p.condition == "out-of-action" && !p.injury_resolved,
                "No unresolved injury for this member"
            );
            if outcome == "dead" {
                member_depart(db, c, &m, "dead", "lose")?;
            } else if outcome == "captured" {
                db.members.iter_mut().find(|x| x.id == m.id).unwrap().status = "captured".into();
            } else if *missed > 0 {
                let x = db.members.iter_mut().find(|x| x.id == m.id).unwrap();
                x.status = "recovering".into();
                x.missed_games = *missed;
            } else {
                ensure!(!outcome.trim().is_empty(), "Describe the injury outcome");
            }
            if let Some(stat) = stat {
                ensure!(STATS.contains(&stat.as_str()), "Unknown characteristic");
                db.effects.push(Effect {
                    id: id(),
                    member_id: m.id.clone(),
                    characteristic: stat.clone(),
                    delta: *delta,
                    description: outcome.clone(),
                    condition: String::new(),
                    battle_id: String::new(),
                });
            }
            db.participants
                .iter_mut()
                .find(|p| p.battle_id == b.id && p.member_id == m.id)
                .unwrap()
                .injury_resolved = true;
            resolution(
                db,
                &b.id,
                &format!("injury-{}", m.id),
                outcome,
                dice.as_deref().unwrap_or(""),
            )?;
        }
        BattleCommand::Complete => {
            let b = current_battle(db, &c.wb)?;
            ensure!(b.status == "post-battle", "Battle not in post-battle");
            ensure!(
                stage(db, &b.id).is_empty(),
                "Resolve or skip stage {}",
                stage(db, &b.id)
            );
            let snap = Snapshot {
                id: id(),
                name: format!("After {}", b.name),
                generation: "__next__".into(),
                warband_id: c.wb.clone(),
            };
            let x = db.battles.iter_mut().find(|x| x.id == b.id).unwrap();
            x.status = "complete".into();
            x.post_snapshot = snap.id.clone();
            db.snapshots.push(snap);
        }
        BattleCommand::Abandon { disposition } => {
            ensure!(
                !disposition.trim().is_empty() && !c.reason.is_empty(),
                "Describe disposition of consumed items and pending outcomes and supply --reason"
            );
            let b = current_battle(db, &c.wb)?;
            note(db, &b.id, disposition, c);
            db.battles.iter_mut().find(|x| x.id == b.id).unwrap().status = "abandoned".into();
        }
        BattleCommand::Awards => {
            let b = current_battle(db, &c.wb)?;
            let difference = b.opponent_rating.saturating_sub(b.rating);
            let underdog = match difference {
                0..=50 => 0,
                51..=75 => 1,
                76..=100 => 2,
                101..=150 => 3,
                151..=300 => 4,
                _ => 5,
            };
            let proposals:Vec<_>=db.units.iter().filter(|u|u.warband_id==c.wb && db.typ(&u.type_id).gains_xp).filter_map(|u|{
                let participants:Vec<_>=db.participants.iter().filter(|p|p.battle_id==b.id && db.members.iter().any(|m|m.id==p.member_id && m.unit_id==u.id)).collect();
                if participants.is_empty(){return None;}
                let survived=participants.iter().any(|p|db.members.iter().any(|m|m.id==p.member_id && active(m)));
                let survival=u32::from(survived);let leader_win=u32::from(b.result.eq_ignore_ascii_case("win") && participants.iter().any(|p|p.member_id==wb(db,&c.wb).leader_id));
                Some(json!({"unit_id":u.id,"name":u.name,"survival":survival,"underdog":if survived{underdog}else{0},"leader_victory":leader_win,"suggested_core_xp":survival+if survived{underdog}else{0}+leader_win,"kills_to_confirm":participants.iter().map(|p|p.kills).sum::<u32>(),"objectives_to_confirm":participants.iter().map(|p|p.objectives).sum::<u32>(),"reason":"Core survival, underdog and leader victory; confirm scenario-specific rules and kill attribution before awarding."}))
            }).collect();
            return Ok(
                json!({"preview":true,"scenario":b.scenario,"underdog_bonus":underdog,"proposals":proposals,"instructions":"Award XP with xp UNIT AMOUNT; group awards are collective, never per model. Confirm scenario-specific eligibility, survival, leader victory, kills and objectives. Complete the experience stage to confirm the allocation.","participants":db.participants.iter().filter(|p|p.battle_id==b.id).collect::<Vec<_>>()}),
            );
        }
    }
    Ok(json!({"ok":true}))
}
fn export(store: &Store, w: &str, file: &Path, format: &str) -> Result<()> {
    ensure!(!file.exists(), "Output already exists; choose another path");
    let db = &store.db;
    let data = match format {
        "json" => serde_json::to_vec_pretty(db)?,
        "bundle" => serde_json::to_vec_pretty(&store.bundle()?)?,
        "markdown" => {
            let mut t = format!(
                "# {}\n\nCampaign: {}\nRules: {} ({})\nGeneration: {}\nGold: {} | Rating: {}\n\n",
                wb(db, w).name,
                db.campaigns
                    .iter()
                    .find(|c| c.id == wb(db, w).campaign_id)
                    .unwrap()
                    .name,
                db.profile(&wb(db, w).profile_id).name,
                db.profile(&wb(db, w).profile_id).version,
                store.generation,
                db.gold(w),
                db.rating(w)
            );
            for m in db.members_of(w) {
                let u = db.unit(&m.unit_id);
                t.push_str(&format!(
                    "- {} [{}], {}: XP {}, {}; {}\n",
                    m.name,
                    m.id,
                    m.status,
                    u.starting_xp + u.earned_xp,
                    STATS
                        .iter()
                        .map(|s| format!("{} {}", s, db.stat(m, s, None)))
                        .collect::<Vec<_>>()
                        .join(" / "),
                    db.allocations
                        .iter()
                        .filter(|a| a.member_id == m.id)
                        .map(|a| format!(
                            "{} x{}",
                            db.items.iter().find(|i| i.id == a.item_id).unwrap().name,
                            a.quantity
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            t.push_str(&format!(
                "\nPending: {}\n",
                summary(db, w)["pending_advances"]
            ));
            t.into_bytes()
        }
        "ledger" => {
            let mut out = csv::Writer::from_writer(Vec::new());
            for l in db.ledger.iter().filter(|l| l.warband_id == w) {
                out.serialize(l)?;
            }
            out.into_inner()?
        }
        "inventory" => {
            let mut out = csv::Writer::from_writer(Vec::new());
            out.write_record([
                "id",
                "item",
                "owned",
                "assigned",
                "stash",
                "generation",
                "profile",
            ])?;
            for i in db
                .items
                .iter()
                .filter(|i| i.profile_id == wb(db, w).profile_id)
            {
                let owned = db.owned(w, &i.id);
                let assigned = db.assigned(w, &i.id);
                out.write_record([
                    i.id.clone(),
                    i.name.clone(),
                    owned.to_string(),
                    assigned.to_string(),
                    (owned - assigned).to_string(),
                    store.generation.clone(),
                    i.profile_id.clone(),
                ])?;
            }
            out.into_inner()?
        }
        "csv" => {
            crate::store::write_tables(file, db)?;
            return Ok(());
        }
        _ => bail!("Format must be markdown, ledger, inventory, json, csv or bundle"),
    };
    fs::write(file, data)?;
    Ok(())
}

fn migrate_profile(db: &Db, c: &ContextData, target: &str) -> Result<Db> {
    let mut next = db.clone();
    let mut types = std::collections::HashMap::new();
    let mut items = std::collections::HashMap::new();
    let mut abilities = std::collections::HashMap::new();
    for t in db
        .types
        .iter()
        .filter(|t| t.profile_id == wb(db, &c.wb).profile_id)
    {
        let rows: Vec<_> = db
            .types
            .iter()
            .filter(|n| n.profile_id == target)
            .cloned()
            .collect();
        if let Ok(n) = resolve(&rows, &t.name) {
            types.insert(t.id.clone(), n.id.clone());
        }
    }
    for t in db
        .items
        .iter()
        .filter(|t| t.profile_id == wb(db, &c.wb).profile_id)
    {
        let rows: Vec<_> = db
            .items
            .iter()
            .filter(|n| n.profile_id == target)
            .cloned()
            .collect();
        if let Ok(n) = resolve(&rows, &t.name) {
            items.insert(t.id.clone(), n.id.clone());
        }
    }
    for t in db
        .abilities
        .iter()
        .filter(|t| t.profile_id == wb(db, &c.wb).profile_id)
    {
        let rows: Vec<_> = db
            .abilities
            .iter()
            .filter(|n| n.profile_id == target)
            .cloned()
            .collect();
        if let Ok(n) = resolve(&rows, &t.name) {
            abilities.insert(t.id.clone(), n.id.clone());
        }
    }
    let units: Vec<_> = db
        .units
        .iter()
        .filter(|u| u.warband_id == c.wb)
        .map(|u| u.id.clone())
        .collect();
    let members: Vec<_> = db.members_of(&c.wb).map(|m| m.id.clone()).collect();
    for u in next.units.iter_mut().filter(|u| units.contains(&u.id)) {
        u.type_id = types
            .get(&u.type_id)
            .context("New profile lacks a required warrior type")?
            .clone();
    }
    for h in next.holdings.iter_mut().filter(|h| h.warband_id == c.wb) {
        h.item_id = items
            .get(&h.item_id)
            .context("New profile lacks a held item")?
            .clone();
    }
    for a in next
        .allocations
        .iter_mut()
        .filter(|a| members.contains(&a.member_id))
    {
        a.item_id = items
            .get(&a.item_id)
            .context("New profile lacks an equipped item")?
            .clone();
    }
    for a in next
        .advances
        .iter_mut()
        .filter(|a| units.contains(&a.unit_id) && !a.ability_id.is_empty())
    {
        a.ability_id = abilities
            .get(&a.ability_id)
            .context("New profile lacks a learned ability")?
            .clone();
    }
    for p in next
        .permissions
        .iter_mut()
        .filter(|p| units.contains(&p.type_id))
    {
        p.target_id = abilities
            .get(&p.target_id)
            .context("New profile lacks a granted ability")?
            .clone();
    }
    next.warbands
        .iter_mut()
        .find(|w| w.id == c.wb)
        .unwrap()
        .profile_id = target.into();
    equipment::migrate_overrides(&mut next, db, &c.wb, target, &items)?;
    for m in next.members_of(&c.wb).filter(|m| active(m)) {
        for stat in STATS {
            let u = next.unit(&m.unit_id);
            if let (Ok(value), Some(cap)) = (
                next.stat(m, stat, None).parse::<i16>(),
                next.caps
                    .iter()
                    .find(|a| a.type_id == u.type_id && a.characteristic == stat),
            ) {
                c.check(
                    value <= cap.maximum,
                    "New rules put a characteristic above its cap",
                )?;
            }
        }
    }
    next.validate()?;
    Ok(next)
}
