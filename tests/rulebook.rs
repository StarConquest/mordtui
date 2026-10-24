use clap::Parser;
use mordtui::{cli::Cli, engine, model::*, rules::editor::*, store::Store};
use serde_json::Value;
use std::collections::BTreeMap;
use tempfile::TempDir;
fn command(store: &mut Store, args: &[&str]) -> anyhow::Result<Value> {
    engine::execute(
        store,
        &Cli::try_parse_from(std::iter::once("mordheim").chain(args.iter().copied()))?,
    )
}
fn save(store: &mut Store, d: Definition) -> anyhow::Result<Value> {
    command(store, &["rulebook", "save", &serde_json::to_string(&d)?])
}
struct Keys {
    profile: String,
    warrior: String,
    list: String,
    item: String,
    ability: String,
    gear: String,
}
fn draft() -> (TempDir, Store, Keys) {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let schema = command(&mut store, &["rulebook", "schema"]).unwrap();
    assert!(store.generation.is_empty());
    let Definition::Profile(mut p) = serde_json::from_value(schema["profile"].clone()).unwrap()
    else {
        panic!()
    };
    p.name = "Custom rules".into();
    p.description = "Campaign-specific injuries and rewards.".into();
    let profile = save(&mut store, Definition::Profile(p)).unwrap()["profile_id"]
        .as_str()
        .unwrap()
        .to_string();
    let list = save(
        &mut store,
        Definition::EquipmentList(ListDefinition {
            list: EquipmentList {
                profile_id: profile.clone(),
                name: "Weapons".into(),
                ..EquipmentList::default()
            },
            ..ListDefinition::default()
        }),
    )
    .unwrap()["record_id"]
        .as_str()
        .unwrap()
        .to_string();
    let item = save(
        &mut store,
        Definition::Equipment(ItemDefinition {
            item: Item {
                profile_id: profile.clone(),
                name: "Sword".into(),
                category: "melee".into(),
                cost: 10,
                ..Item::default()
            },
            lists: vec![list.clone()],
        }),
    )
    .unwrap()["record_id"]
        .as_str()
        .unwrap()
        .to_string();
    let ability = save(
        &mut store,
        Definition::Ability(AbilityDefinition {
            ability: Ability {
                profile_id: profile.clone(),
                name: "Drill".into(),
                kind: "combat".into(),
                description: "A custom skill.".into(),
                ..Ability::default()
            },
            ..AbilityDefinition::default()
        }),
    )
    .unwrap()["record_id"]
        .as_str()
        .unwrap()
        .to_string();
    let Definition::Warrior(mut t) = serde_json::from_value(schema["warrior"].clone()).unwrap()
    else {
        panic!()
    };
    t.warrior.profile_id = profile.clone();
    t.warrior.name = "Marshal".into();
    t.warrior.leader = Some(true);
    t.warrior.free_dagger = Some(false);
    t.equipment_list = Some(list.clone());
    t.abilities = vec![ability.clone()];
    t.starting_abilities = vec![ability.clone()];
    t.caps = BTreeMap::from([("WS".into(), 6)]);
    let warrior = save(&mut store, Definition::Warrior(t)).unwrap()["record_id"]
        .as_str()
        .unwrap()
        .to_string();
    let gear = save(
        &mut store,
        Definition::StartingGear(StartingGearDefinition {
            id: String::new(),
            profile: profile.clone(),
            warrior: warrior.clone(),
            item: item.clone(),
            quantity: 2,
        }),
    )
    .unwrap()["record_id"]
        .as_str()
        .unwrap()
        .to_string();
    save(
        &mut store,
        Definition::Progression(ProgressionDefinition {
            profile: profile.clone(),
            role: "hero".into(),
            thresholds: vec![2, 4, 8],
        }),
    )
    .unwrap();
    (
        dir,
        store,
        Keys {
            profile,
            warrior,
            list,
            item,
            ability,
            gear,
        },
    )
}

#[test]
fn profile_copy_remaps_every_dependency_and_can_be_edited_or_removed_independently() {
    let (_dir, mut store, k) = draft();
    let original = store.db.clone();
    let generation = store.generation.clone();
    let result = command(
        &mut store,
        &[
            "rulebook",
            "clone",
            &k.profile,
            "Independent",
            "--version",
            "2.0",
        ],
    )
    .unwrap();
    let p = result["profile_id"].as_str().unwrap();
    let t = store
        .db
        .types
        .iter()
        .find(|t| t.profile_id == p)
        .unwrap()
        .clone();
    let i = store.db.items.iter().find(|i| i.profile_id == p).unwrap();
    let a = store
        .db
        .abilities
        .iter()
        .find(|a| a.profile_id == p)
        .unwrap();
    assert_ne!(t.id, k.warrior);
    assert_ne!(i.id, k.item);
    assert_ne!(a.id, k.ability);
    let l = store.db.equipment_list_for(&t.id).unwrap().clone();
    assert_ne!(l.id, k.list);
    assert!(
        store
            .db
            .equipment_list_items
            .iter()
            .any(|r| r.list_id == l.id && r.item_id == i.id)
    );
    assert!(
        store
            .db
            .permissions
            .iter()
            .any(|r| r.type_id == t.id && r.target_id == a.id && r.kind == "starting-ability")
    );
    assert!(store.db.permissions.iter().any(|r| r.type_id == t.id
        && r.target_id == i.id
        && r.kind == "starting-item"
        && r.quantity == 2));
    assert!(
        store
            .db
            .caps
            .iter()
            .any(|c| c.type_id == t.id && c.maximum == 6)
    );
    assert_eq!(
        store
            .db
            .thresholds
            .iter()
            .filter(|r| r.profile_id == p)
            .count(),
        3
    );
    let p = p.to_string();
    let mut changed = t.clone();
    changed.cost = 99;
    save(
        &mut store,
        Definition::Warrior(Box::new(WarriorDefinition {
            warrior: changed,
            equipment_list: Some(l.id.clone()),
            ..WarriorDefinition::default()
        })),
    )
    .unwrap();
    assert_eq!(store.db.typ(&k.warrior), original.typ(&k.warrior));
    assert_eq!(store.historical(&generation).unwrap(), original);
    command(&mut store, &["rulebook", "delete", "profile", &p]).unwrap();
    assert!(!store.db.types.iter().any(|t| t.profile_id == p));
    assert!(!store.db.equipment_lists.iter().any(|l| l.profile_id == p));
    store.db.validate().unwrap();
}

#[test]
fn invalid_edits_and_deleting_referenced_definitions_leave_the_store_unchanged() {
    let (_dir, mut store, k) = draft();
    command(
        &mut store,
        &[
            "rulebook",
            "create-warband",
            &k.profile,
            "Campaign",
            "Warband",
            "--new-campaign",
        ],
    )
    .unwrap();
    command(&mut store, &["recruit", "Marshal", "Mara"]).unwrap();
    let original = store.db.clone();
    let generation = store.generation.clone();
    let mut t = store.db.typ(&k.warrior).clone();
    t.role = "henchman".into();
    assert!(
        save(
            &mut store,
            Definition::Warrior(Box::new(WarriorDefinition {
                warrior: t,
                equipment_list: Some(k.list.clone()),
                ..WarriorDefinition::default()
            }))
        )
        .is_err()
    );
    let mut item = store
        .db
        .items
        .iter()
        .find(|i| i.id == k.item)
        .unwrap()
        .clone();
    item.cost = -1;
    assert!(
        save(
            &mut store,
            Definition::Equipment(ItemDefinition {
                item,
                lists: vec![k.list.clone()]
            })
        )
        .is_err()
    );
    assert!(
        save(
            &mut store,
            Definition::Progression(ProgressionDefinition {
                profile: k.profile.clone(),
                role: "hero".into(),
                thresholds: vec![2, 2]
            })
        )
        .is_err()
    );
    for (kind, id) in [
        ("profile", &k.profile),
        ("warrior", &k.warrior),
        ("equipment", &k.item),
        ("ability", &k.ability),
        ("equipment-list", &k.list),
    ] {
        assert!(command(&mut store, &["rulebook", "delete", kind, id]).is_err());
        assert_eq!(store.db, original);
    }
    assert_eq!(store.generation, generation);
    assert_eq!(store.db.gold(&store.db.warbands[0].id), 500);
    assert_eq!(store.db.owned(&store.db.warbands[0].id, &k.item), 2);
    let json = serde_json::json!({"kind":"profile","data":{"id":k.profile,"unrecognized":true}})
        .to_string();
    assert!(command(&mut store, &["rulebook", "save", &json]).is_err());
    assert_eq!(store.db, original);
}

#[test]
fn definition_references_cannot_cross_profiles_and_existing_ids_are_stable() {
    let (dir, mut store, k) = draft();
    let mut p = store.db.profiles[0].clone();
    p.id.clear();
    p.name = "Other".into();
    let other = save(&mut store, Definition::Profile(p)).unwrap()["profile_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = store.db.clone();
    let mut i = store.db.items[0].clone();
    i.profile_id = other.clone();
    assert!(
        save(
            &mut store,
            Definition::Equipment(ItemDefinition {
                item: i,
                lists: vec![]
            })
        )
        .is_err()
    );
    assert!(
        save(
            &mut store,
            Definition::StartingGear(StartingGearDefinition {
                id: String::new(),
                profile: other,
                warrior: k.warrior.clone(),
                item: k.item.clone(),
                quantity: 1
            })
        )
        .is_err()
    );
    assert_eq!(store.db, before);
    let mut ability = store.db.abilities[0].clone();
    ability.description = "Updated effect".into();
    save(
        &mut store,
        Definition::Ability(AbilityDefinition {
            ability: ability.clone(),
            types: vec![k.warrior.clone()],
            starting_types: vec![k.warrior.clone()],
        }),
    )
    .unwrap();
    assert_eq!(
        store
            .db
            .abilities
            .iter()
            .find(|a| a.id == k.ability)
            .unwrap()
            .description,
        "Updated effect"
    );
    command(
        &mut store,
        &["rulebook", "delete", "starting-gear", &k.gear],
    )
    .unwrap();
    assert!(!store.db.permissions.iter().any(|p| p.id == k.gear));
    let saved = store.db.clone();
    drop(store);
    assert_eq!(Store::open(dir.path()).unwrap().db, saved);
}

#[test]
fn starting_equipment_is_granted_per_model_and_reinforcements_preserve_group_loadouts() {
    let (_dir, mut store, k) = draft();
    let mut t = store.db.typ(&k.warrior).clone();
    t.id.clear();
    t.name = "Soldiers".into();
    t.role = "henchman".into();
    t.group_max = 10;
    t.leader = Some(false);
    let kind = save(
        &mut store,
        Definition::Warrior(Box::new(WarriorDefinition {
            warrior: t,
            equipment_list: Some(k.list.clone()),
            ..WarriorDefinition::default()
        })),
    )
    .unwrap()["record_id"]
        .as_str()
        .unwrap()
        .to_string();
    let gear = save(
        &mut store,
        Definition::StartingGear(StartingGearDefinition {
            id: String::new(),
            profile: k.profile.clone(),
            warrior: kind.clone(),
            item: k.item.clone(),
            quantity: 1,
        }),
    )
    .unwrap()["record_id"]
        .as_str()
        .unwrap()
        .to_string();
    command(
        &mut store,
        &[
            "rulebook",
            "create-warband",
            &k.profile,
            "Campaign",
            "Band",
            "--new-campaign",
        ],
    )
    .unwrap();
    command(
        &mut store,
        &["recruit", "Soldiers", "Blades", "--count", "2"],
    )
    .unwrap();
    let unit = store.db.units[0].id.clone();
    let wb = store.db.warbands[0].id.clone();
    command(
        &mut store,
        &["recruit", "Soldiers", "Blades", "--group", &unit],
    )
    .unwrap();
    assert_eq!(store.db.owned(&wb, &k.item), 3);
    save(
        &mut store,
        Definition::StartingGear(StartingGearDefinition {
            id: gear.clone(),
            profile: k.profile.clone(),
            warrior: kind.clone(),
            item: k.item.clone(),
            quantity: 2,
        }),
    )
    .unwrap();
    command(
        &mut store,
        &["recruit", "Soldiers", "Blades", "--group", &unit],
    )
    .unwrap();
    assert_eq!(store.db.owned(&wb, &k.item), 5);
    assert_eq!(store.db.assigned(&wb, &k.item), 4);
    save(
        &mut store,
        Definition::StartingGear(StartingGearDefinition {
            id: gear,
            profile: k.profile.clone(),
            warrior: kind,
            item: k.item.clone(),
            quantity: 0,
        }),
    )
    .unwrap();
    command(
        &mut store,
        &["recruit", "Soldiers", "Blades", "--group", &unit],
    )
    .unwrap();
    assert_eq!(store.db.assigned(&wb, &k.item), 5);
    let before = store.db.clone();
    let error = command(
        &mut store,
        &["recruit", "Soldiers", "Blades", "--group", &unit],
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("replacement"), "{error}");
    assert_eq!(store.db, before);
    assert_eq!(
        store
            .db
            .ledger
            .iter()
            .filter(|l| l.category == "find" && l.item_id == k.item)
            .map(|l| l.quantity)
            .sum::<u32>(),
        5
    );
}
