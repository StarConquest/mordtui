//! Audited authoring commands shared by the graphical editor and AI proposals.
use crate::{cli::RulebookCommand, model::*};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Definition {
    Profile(Profile),
    Warrior(Box<WarriorDefinition>),
    Equipment(ItemDefinition),
    EquipmentList(ListDefinition),
    Ability(AbilityDefinition),
    Progression(ProgressionDefinition),
    StartingGear(StartingGearDefinition),
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarriorDefinition {
    pub warrior: WarriorType,
    pub equipment_list: Option<String>,
    pub abilities: Vec<String>,
    pub starting_abilities: Vec<String>,
    pub caps: BTreeMap<String, i16>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDefinition {
    pub item: Item,
    pub lists: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListDefinition {
    pub list: EquipmentList,
    pub items: Vec<String>,
    pub types: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityDefinition {
    pub ability: Ability,
    pub types: Vec<String>,
    pub starting_types: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressionDefinition {
    pub profile: String,
    pub role: String,
    pub thresholds: Vec<u32>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartingGearDefinition {
    pub id: String,
    pub profile: String,
    pub warrior: String,
    pub item: String,
    pub quantity: u32,
}

fn profile<'a>(db: &'a Db, id: &str) -> Result<&'a Profile> {
    db.profiles
        .iter()
        .find(|p| p.id == id)
        .context("Choose an existing rules profile")
}
fn edit_id<T: Named>(rows: &[T], current: &str) -> Result<String> {
    if current.is_empty() {
        Ok(id())
    } else {
        ensure!(
            rows.iter().any(|r| r.id() == current),
            "The definition no longer exists; create a new entry instead"
        );
        Ok(current.into())
    }
}
fn owner_matches(old: Option<&str>, next: &str) -> Result<()> {
    ensure!(
        old.is_none_or(|old| old == next),
        "A definition cannot move between profiles; copy the profile instead"
    );
    Ok(())
}
fn type_ids(db: &Db, p: &str, ids: &[String]) -> Result<()> {
    ensure!(
        ids.iter()
            .all(|id| db.types.iter().any(|t| t.id == *id && t.profile_id == p)),
        "A selected warrior type belongs to another profile or no longer exists"
    );
    Ok(())
}
fn ability_ids(db: &Db, p: &str, ids: &[String]) -> Result<()> {
    ensure!(
        ids.iter().all(|id| db
            .abilities
            .iter()
            .any(|a| a.id == *id && a.profile_id == p)),
        "A selected skill belongs to another profile or no longer exists"
    );
    Ok(())
}
fn add_permission(db: &mut Db, owner: &str, target: &str, kind: &str) {
    if !db
        .permissions
        .iter()
        .any(|p| p.type_id == owner && p.target_id == target && p.kind == kind)
    {
        db.permissions.push(Permission {
            id: id(),
            type_id: owner.into(),
            target_id: target.into(),
            kind: kind.into(),
            quantity: 1,
        });
    }
}
fn empty_list(db: &mut Db, t: &WarriorType) {
    let key = id();
    db.equipment_lists.push(EquipmentList {
        id: key.clone(),
        profile_id: t.profile_id.clone(),
        name: format!("{} equipment ({})", t.name, &key[..8]),
        description: "Empty default list; add permitted equipment in the Rules editor.".into(),
    });
    db.assign_equipment_list(&t.id, &key);
}

pub fn save(db: &mut Db, definition: Definition) -> Result<Value> {
    let result = match definition {
        Definition::Profile(mut p) => {
            ensure!(
                p.initial_advance_cost >= 0,
                "Initial advance cost cannot be negative"
            );
            ensure!(
                !p.name.trim().is_empty() && !p.version.trim().is_empty(),
                "Enter a profile name and version"
            );
            ensure!(
                ["published", "optional", "house"].contains(&p.classification.as_str()),
                "Choose published, optional or house rules"
            );
            ensure!(
                !db.profiles.iter().any(|r| r.id != p.id
                    && r.name.eq_ignore_ascii_case(p.name.trim())
                    && r.version == p.version),
                "This profile name/version already exists"
            );
            p.id = edit_id(&db.profiles, &p.id)?;
            p.name = p.name.trim().into();
            db.profiles.retain(|r| r.id != p.id);
            db.profiles.push(p.clone());
            json!({"profile_id":p.id,"record_id":p.id})
        }
        Definition::Warrior(mut input) => {
            let t = &mut input.warrior;
            if t.id.is_empty() {
                t.leader.get_or_insert(false);
                t.free_dagger.get_or_insert(false);
            }
            ensure!(
                t.starting_xp <= 1_000_000,
                "Starting experience cannot exceed 1000000"
            );
            profile(db, &t.profile_id)?;
            ensure!(!t.name.trim().is_empty(), "Enter a warrior type name");
            ensure!(
                !db.types.iter().any(|r| r.id != t.id
                    && r.profile_id == t.profile_id
                    && r.name.eq_ignore_ascii_case(t.name.trim())),
                "Warrior type name already exists"
            );
            let old = db.types.iter().find(|r| r.id == t.id);
            owner_matches(old.map(|r| r.profile_id.as_str()), &t.profile_id)?;
            ensure!(
                old.is_none_or(|r| r.role == t.role) || !db.units.iter().any(|u| u.type_id == t.id),
                "A recruited warrior type cannot change role; copy the profile or create another type"
            );
            ability_ids(db, &t.profile_id, &input.abilities)?;
            ability_ids(db, &t.profile_id, &input.starting_abilities)?;
            if let Some(list) = &input.equipment_list {
                ensure!(
                    db.equipment_lists
                        .iter()
                        .any(|l| l.id == *list && l.profile_id == t.profile_id),
                    "Choose an equipment list in this profile"
                );
            }
            ensure!(
                input
                    .caps
                    .iter()
                    .all(|(s, n)| STATS.contains(&s.as_str()) && *n > 0),
                "Caps require a characteristic and positive maximum"
            );
            t.id = edit_id(&db.types, &t.id)?;
            t.name = t.name.trim().into();
            db.types.retain(|r| r.id != t.id);
            db.types.push(t.clone());
            if let Some(list) = &input.equipment_list {
                db.assign_equipment_list(&t.id, list);
            } else if db.equipment_list_for(&t.id).is_none() {
                empty_list(db, t);
            }
            db.permissions.retain(|r| {
                r.type_id != t.id || !["ability", "starting-ability"].contains(&r.kind.as_str())
            });
            for a in input.abilities {
                add_permission(db, &t.id, &a, "ability");
            }
            for a in input.starting_abilities {
                add_permission(db, &t.id, &a, "starting-ability");
            }
            db.caps.retain(|c| c.type_id != t.id);
            for (characteristic, maximum) in input.caps {
                db.caps.push(Cap {
                    id: id(),
                    type_id: t.id.clone(),
                    characteristic,
                    maximum,
                });
            }
            json!({"profile_id":t.profile_id,"record_id":t.id})
        }
        Definition::Equipment(mut input) => {
            let item = &mut input.item;
            profile(db, &item.profile_id)?;
            owner_matches(
                db.items
                    .iter()
                    .find(|r| r.id == item.id)
                    .map(|r| r.profile_id.as_str()),
                &item.profile_id,
            )?;
            ensure!(
                !item.name.trim().is_empty()
                    && ["melee", "missile", "armour", "equipment", "consumable"]
                        .contains(&item.category.as_str()),
                "Enter an item name and valid category"
            );
            ensure!(
                !db.items.iter().any(|r| r.id != item.id
                    && r.profile_id == item.profile_id
                    && r.name.eq_ignore_ascii_case(item.name.trim())),
                "Equipment name already exists"
            );
            ensure!(
                input.lists.iter().all(|id| db
                    .equipment_lists
                    .iter()
                    .any(|l| l.id == *id && l.profile_id == item.profile_id)),
                "Choose equipment lists in this profile"
            );
            item.id = edit_id(&db.items, &item.id)?;
            item.name = item.name.trim().into();
            db.items.retain(|r| r.id != item.id);
            db.items.push(item.clone());
            db.equipment_list_items.retain(|r| r.item_id != item.id);
            for list in input.lists {
                db.add_to_equipment_list(&list, &item.id);
            }
            json!({"profile_id":item.profile_id,"record_id":item.id})
        }
        Definition::EquipmentList(mut input) => {
            let list = &mut input.list;
            profile(db, &list.profile_id)?;
            owner_matches(
                db.equipment_lists
                    .iter()
                    .find(|r| r.id == list.id)
                    .map(|r| r.profile_id.as_str()),
                &list.profile_id,
            )?;
            type_ids(db, &list.profile_id, &input.types)?;
            ensure!(
                input.items.iter().all(|id| db
                    .items
                    .iter()
                    .any(|i| i.id == *id && i.profile_id == list.profile_id)),
                "Choose equipment in this profile"
            );
            list.id = edit_id(&db.equipment_lists, &list.id)?;
            let detached: Vec<_> = db
                .types
                .iter()
                .filter(|t| {
                    !input.types.contains(&t.id)
                        && db
                            .equipment_list_assignments
                            .iter()
                            .any(|a| a.entity_id == t.id && a.list_id == list.id)
                })
                .cloned()
                .collect();
            db.equipment_lists.retain(|r| r.id != list.id);
            db.equipment_lists.push(list.clone());
            db.equipment_list_items.retain(|r| r.list_id != list.id);
            for item in input.items {
                db.add_to_equipment_list(&list.id, &item);
            }
            for t in input.types {
                db.assign_equipment_list(&t, &list.id);
            }
            for t in detached {
                empty_list(db, &t);
            }
            json!({"profile_id":list.profile_id,"record_id":list.id})
        }
        Definition::Ability(mut input) => {
            let a = &mut input.ability;
            profile(db, &a.profile_id)?;
            owner_matches(
                db.abilities
                    .iter()
                    .find(|r| r.id == a.id)
                    .map(|r| r.profile_id.as_str()),
                &a.profile_id,
            )?;
            ensure!(
                !a.name.trim().is_empty() && !a.kind.trim().is_empty(),
                "Enter a skill name and category"
            );
            ensure!(
                !db.abilities.iter().any(|r| r.id != a.id
                    && r.profile_id == a.profile_id
                    && r.name.eq_ignore_ascii_case(a.name.trim())),
                "Skill name already exists"
            );
            type_ids(db, &a.profile_id, &input.types)?;
            type_ids(db, &a.profile_id, &input.starting_types)?;
            a.id = edit_id(&db.abilities, &a.id)?;
            a.name = a.name.trim().into();
            db.abilities.retain(|r| r.id != a.id);
            db.abilities.push(a.clone());
            db.permissions
                .retain(|r| r.target_id != a.id || !db.types.iter().any(|t| t.id == r.type_id));
            for t in input.types {
                add_permission(db, &t, &a.id, "ability");
            }
            for t in input.starting_types {
                add_permission(db, &t, &a.id, "starting-ability");
            }
            json!({"profile_id":a.profile_id,"record_id":a.id})
        }
        Definition::StartingGear(input) => {
            profile(db, &input.profile)?;
            type_ids(db, &input.profile, std::slice::from_ref(&input.warrior))?;
            ensure!(
                input.quantity <= 1_000_000,
                "Starting equipment quantity is too large"
            );
            ensure!(
                db.items
                    .iter()
                    .any(|i| i.id == input.item && i.profile_id == input.profile),
                "Choose starting equipment in this profile"
            );
            if !input.id.is_empty() {
                let old = db
                    .permissions
                    .iter()
                    .find(|p| p.id == input.id && p.kind == "starting-item")
                    .context("Starting equipment rule no longer exists")?;
                ensure!(
                    db.equipment_entity_profile(&old.type_id) == Some(input.profile.as_str()),
                    "Starting equipment cannot move between profiles"
                );
            }
            let existing = db.permissions.iter().find(|p| {
                p.kind == "starting-item" && p.type_id == input.warrior && p.target_id == input.item
            });
            ensure!(
                input.id.is_empty() || existing.is_none_or(|p| p.id == input.id),
                "This type/item pair is already defined; edit its existing entry"
            );
            let key = if input.id.is_empty() {
                existing.map(|p| p.id.clone()).unwrap_or_else(id)
            } else {
                input.id
            };
            db.permissions.retain(|p| p.id != key);
            if input.quantity > 0 {
                db.permissions.push(Permission {
                    id: key.clone(),
                    type_id: input.warrior,
                    target_id: input.item,
                    kind: "starting-item".into(),
                    quantity: input.quantity,
                });
            }
            json!({"profile_id":input.profile,"record_id":key})
        }
        Definition::Progression(input) => {
            profile(db, &input.profile)?;
            ensure!(
                ["hero", "henchman", "hired", "dramatis"].contains(&input.role.as_str()),
                "Choose a valid progression role"
            );
            let unique: BTreeSet<_> = input.thresholds.iter().collect();
            ensure!(
                unique.len() == input.thresholds.len()
                    && unique.iter().all(|n| **n > 0 && **n <= 1_000_000),
                "XP thresholds must be unique positive values up to 1000000"
            );
            db.thresholds
                .retain(|t| t.profile_id != input.profile || t.role != input.role);
            for xp in input.thresholds {
                db.thresholds.push(Threshold {
                    id: id(),
                    profile_id: input.profile.clone(),
                    role: input.role.clone(),
                    xp,
                });
            }
            json!({"profile_id":input.profile,"record_id":input.role})
        }
    };
    db.normalize_equipment_lists()?;
    db.validate()?;
    Ok(result)
}

fn remove(db: &mut Db, kind: &str, key: &str) -> Result<Value> {
    let p = match kind {
        "profile" => {
            profile(db, key)?;
            ensure!(
                !db.warbands.iter().any(|w| w.profile_id == key),
                "This profile is used by a warband; retain it for history or copy it"
            );
            let types: Vec<_> = db
                .types
                .iter()
                .filter(|t| t.profile_id == key)
                .map(|t| t.id.clone())
                .collect();
            let lists: Vec<_> = db
                .equipment_lists
                .iter()
                .filter(|l| l.profile_id == key)
                .map(|l| l.id.clone())
                .collect();
            db.profiles.retain(|p| p.id != key);
            db.types.retain(|t| t.profile_id != key);
            db.items.retain(|i| i.profile_id != key);
            db.abilities.retain(|a| a.profile_id != key);
            db.caps.retain(|c| !types.contains(&c.type_id));
            db.permissions.retain(|p| !types.contains(&p.type_id));
            db.thresholds.retain(|t| t.profile_id != key);
            db.equipment_lists.retain(|l| l.profile_id != key);
            db.equipment_list_items
                .retain(|r| !lists.contains(&r.list_id));
            db.equipment_list_assignments
                .retain(|a| !lists.contains(&a.list_id));
            String::new()
        }
        "warrior" => {
            let t = db
                .types
                .iter()
                .find(|t| t.id == key)
                .context("Warrior type no longer exists")?
                .clone();
            ensure!(
                !db.units.iter().any(|u| u.type_id == key),
                "This warrior type has recruited models and must be retained"
            );
            db.types.retain(|t| t.id != key);
            db.caps.retain(|c| c.type_id != key);
            db.permissions.retain(|p| p.type_id != key);
            db.equipment_list_assignments.retain(|a| a.entity_id != key);
            t.profile_id
        }
        "equipment" => {
            let i = db
                .items
                .iter()
                .find(|i| i.id == key)
                .context("Item no longer exists")?
                .clone();
            ensure!(
                !db.permissions
                    .iter()
                    .any(|p| p.kind == "starting-item" && p.target_id == key),
                "Remove this item's starting equipment rules before deleting it"
            );
            ensure!(
                !db.holdings.iter().any(|h| h.item_id == key)
                    && !db.ledger.iter().any(|l| l.item_id == key)
                    && !db.searches.iter().any(|s| s.item_id == key),
                "This item is used by inventory, transactions or searches and must be retained"
            );
            db.items.retain(|i| i.id != key);
            db.equipment_list_items.retain(|r| r.item_id != key);
            i.profile_id
        }
        "equipment-list" => {
            let l = db
                .equipment_lists
                .iter()
                .find(|l| l.id == key)
                .context("List no longer exists")?
                .clone();
            ensure!(
                !db.equipment_list_assignments
                    .iter()
                    .any(|a| a.list_id == key),
                "Assign the types/models to another list before removing this list"
            );
            db.equipment_lists.retain(|l| l.id != key);
            db.equipment_list_items.retain(|r| r.list_id != key);
            l.profile_id
        }
        "ability" => {
            let a = db
                .abilities
                .iter()
                .find(|a| a.id == key)
                .context("Skill no longer exists")?
                .clone();
            ensure!(
                !db.advances.iter().any(|r| r.ability_id == key),
                "This skill has been learned and must be retained"
            );
            db.abilities.retain(|a| a.id != key);
            db.permissions.retain(|p| p.target_id != key);
            a.profile_id
        }
        "starting-gear" => {
            let row = db
                .permissions
                .iter()
                .find(|p| p.id == key && p.kind == "starting-item")
                .context("Starting gear rule no longer exists")?
                .clone();
            let profile = db
                .equipment_entity_profile(&row.type_id)
                .context("Missing starting gear type")?
                .to_string();
            db.permissions.retain(|p| p.id != key);
            profile
        }
        _ => bail!(
            "Choose profile, warrior, equipment, equipment-list or ability; edit progression to clear thresholds"
        ),
    };
    db.validate()?;
    Ok(json!({"profile_id":p,"removed":key}))
}

pub fn clone_profile(db: &mut Db, source: &str, name: &str, version: &str) -> Result<Value> {
    let source = resolve(&db.profiles, source)?.clone();
    let mut p = source.clone();
    p.id.clear();
    p.name = name.into();
    p.version = version.into();
    let result = save(db, Definition::Profile(p))?;
    let p = result["profile_id"].as_str().unwrap().to_string();
    let mut ids = BTreeMap::new();
    let mut copy = db.clone();
    macro_rules! copy_rows {
        ($field:ident) => {
            for row in copy.$field.iter_mut().filter(|r| r.profile_id == source.id) {
                let old = row.id.clone();
                row.id = id();
                ids.insert(old, row.id.clone());
                row.profile_id = p.clone();
                db.$field.push(row.clone());
            }
        };
    }
    copy_rows!(types);
    copy_rows!(items);
    copy_rows!(abilities);
    copy_rows!(equipment_lists);
    copy_rows!(thresholds);
    for c in &copy.caps {
        if let Some(t) = ids.get(&c.type_id) {
            db.caps.push(Cap {
                id: id(),
                type_id: t.clone(),
                ..c.clone()
            });
        }
    }
    for perm in &copy.permissions {
        if let (Some(owner), Some(target)) = (ids.get(&perm.type_id), ids.get(&perm.target_id)) {
            if perm.kind == "starting-item" {
                db.permissions.push(Permission {
                    id: id(),
                    type_id: owner.clone(),
                    target_id: target.clone(),
                    kind: perm.kind.clone(),
                    quantity: perm.quantity,
                });
            } else {
                add_permission(db, owner, target, &perm.kind);
            }
        }
    }
    for row in &copy.equipment_list_items {
        if let (Some(list), Some(item)) = (ids.get(&row.list_id), ids.get(&row.item_id)) {
            db.add_to_equipment_list(list, item);
        }
    }
    for row in &copy.equipment_list_assignments {
        if let (Some(owner), Some(list)) = (ids.get(&row.entity_id), ids.get(&row.list_id)) {
            db.assign_equipment_list(owner, list);
        }
    }
    db.normalize_equipment_lists()?;
    db.validate()?;
    Ok(result)
}

pub fn execute(db: &mut Db, command: &RulebookCommand, date: &str, reason: &str) -> Result<Value> {
    match command {
        RulebookCommand::Schema => Ok(templates()),
        RulebookCommand::Save { definition } => save(
            db,
            serde_json::from_str(definition)
                .context("Invalid rule definition; see rulebook schema")?,
        ),
        RulebookCommand::Delete { kind, id } => remove(db, kind, id),
        RulebookCommand::Clone {
            profile,
            name,
            version,
        } => clone_profile(db, profile, name, version),
        RulebookCommand::CreateWarband {
            profile: query,
            campaign,
            name,
            new_campaign,
        } => {
            let p = resolve(&db.profiles, query)?.clone();
            ensure!(
                !name.trim().is_empty() && !campaign.trim().is_empty(),
                "Enter warband and campaign names"
            );
            ensure!(
                db.types.iter().any(|t| t.profile_id == p.id),
                "Define at least one warrior type before creating a warband"
            );
            let camp = if *new_campaign {
                ensure!(
                    !db.campaigns
                        .iter()
                        .any(|c| c.name.eq_ignore_ascii_case(campaign.trim())),
                    "Campaign already exists; select it instead"
                );
                let camp = Campaign {
                    id: id(),
                    name: campaign.trim().into(),
                    ..Campaign::default()
                };
                db.campaigns.push(camp.clone());
                camp
            } else {
                resolve(&db.campaigns, campaign)?.clone()
            };
            ensure!(
                !db.warbands
                    .iter()
                    .any(|w| w.campaign_id == camp.id && w.name.eq_ignore_ascii_case(name.trim())),
                "Warband name already exists in this campaign"
            );
            let w = Warband {
                id: id(),
                campaign_id: camp.id,
                profile_id: p.id.clone(),
                name: name.trim().into(),
                leader_id: String::new(),
            };
            db.warbands.push(w.clone());
            for (category, gold) in [
                ("funding", p.starting_gold),
                ("faction-bonus", p.bonus_gold),
            ] {
                if category == "funding" || gold != 0 {
                    db.ledger.push(Ledger {
                        id: id(),
                        warband_id: w.id.clone(),
                        category: category.into(),
                        quantity: 1,
                        unit_price: gold,
                        gold,
                        date: date.into(),
                        phase: "setup".into(),
                        notes: reason.into(),
                        ..Ledger::default()
                    });
                }
            }
            db.notes.push(Note {
                id: id(),
                entity_id: "selection".into(),
                text: w.id.clone(),
                date: date.into(),
            });
            db.validate()?;
            Ok(json!({"profile_id":p.id,"warband_id":w.id}))
        }
    }
}

pub fn templates() -> Value {
    json!({"instructions":"Save {kind,data}; blank id creates, existing id edits. Use IDs from the selected profile for references. Descriptions record manually adjudicated rules.",
        "profile": Definition::Profile(Profile { schema_version: SCHEMA, version: "1.0".into(), name: "New profile".into(), classification: "house".into(), starting_gold: 500, max_members: 15, max_heroes: 6, ..Profile::default() }),
        "warrior": Definition::Warrior(Box::new(WarriorDefinition { warrior: WarriorType { name: "Warrior".into(), role: "hero".into(), group_max: 1, gains_xp: true, counts_roster: true, counts_income: true, counts_battle: true, m:"4".into(), ws:"3".into(), bs:"3".into(), s:"3".into(), t:"3".into(), w:"1".into(), i:"3".into(), a:"1".into(), ld:"7".into(), leader: Some(false), ..WarriorType::default() }, ..WarriorDefinition::default() })),
        "equipment": Definition::Equipment(ItemDefinition { item: Item { category:"equipment".into(), ..Item::default() }, ..ItemDefinition::default() }),
        "equipment-list": Definition::EquipmentList(ListDefinition::default()),
        "ability": Definition::Ability(AbilityDefinition::default()),
        "progression": Definition::Progression(ProgressionDefinition { role:"hero".into(), ..ProgressionDefinition::default() }),
        "starting-gear": Definition::StartingGear(StartingGearDefinition {quantity:1,..StartingGearDefinition::default()})})
}
