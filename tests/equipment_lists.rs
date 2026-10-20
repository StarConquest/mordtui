use clap::Parser;
use mordtui::{cli::Cli, engine, model::*, store::Store};
use tempfile::TempDir;

fn command(store: &mut Store, args: &[&str]) -> anyhow::Result<serde_json::Value> {
    engine::execute(
        store,
        &Cli::try_parse_from(std::iter::once("mordheim").chain(args.iter().copied()))?,
    )
}
fn example() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    command(
        &mut store,
        &["import", "examples/burgomeisters.json", "--apply"],
    )
    .unwrap();
    (dir, store)
}

#[test]
fn legacy_access_converts_deterministically_to_shared_lists_and_keeps_unit_exceptions() {
    let mut original: Db =
        serde_json::from_str(include_str!("../examples/burgomeisters.json")).unwrap();
    let mut extra = original.items[0].clone();
    extra.id = "personal-weapon".into();
    extra.name = "Personal weapon".into();
    original.items.push(extra);
    original.permissions.push(Permission {
        id: "personal-permission".into(),
        type_id: "arrows-one-unit".into(),
        target_id: "personal-weapon".into(),
        kind: "item".into(),
    });
    let mut db = original.clone();
    db.normalize_equipment_lists().unwrap();
    assert!(db.permissions.iter().all(|p| p.kind != "item"));
    assert_eq!(
        db.equipment_list_for("hans").unwrap().id,
        db.equipment_list_for("champion-one").unwrap().id
    );
    for m in &original.members {
        let u = original.unit(&m.unit_id);
        for item in &original.items {
            let allowed = original.permissions.iter().any(|p| {
                p.kind == "item"
                    && (p.type_id == u.type_id || p.type_id == u.id)
                    && p.target_id == item.id
            });
            assert_eq!(
                db.equipment_allowed(&m.id, &item.id),
                allowed,
                "{} / {}",
                m.name,
                item.name
            );
        }
    }
    assert!(db.equipment_allowed("arrows-one-1", "personal-weapon"));
    assert!(!db.equipment_allowed("arrows-two-1", "personal-weapon"));
    assert_eq!(db.allocations, original.allocations);
    assert_eq!(db.ledger, original.ledger);
    let canonical = db.clone();
    db.normalize_equipment_lists().unwrap();
    assert_eq!(canonical, db);
    original.normalize_equipment_lists().unwrap();
    assert_eq!(original, canonical);
    db.validate().unwrap();
}

#[test]
fn one_item_can_belong_to_several_lists_shared_by_multiple_types_and_future_recruits() {
    let (dir, mut store) = example();
    command(
        &mut store,
        &[
            "equipment-list",
            "create",
            "Heroes",
            "--type",
            "Captain,Champion",
            "--item",
            "Dagger",
        ],
    )
    .unwrap();
    command(
        &mut store,
        &[
            "equipment-list",
            "create",
            "Warriors",
            "--type",
            "Marksmen,Warrior",
            "--item",
            "Dagger",
        ],
    )
    .unwrap();
    command(
        &mut store,
        &[
            "add-equipment",
            "Two-handed weapon",
            "--list",
            "Heroes,Warriors",
            "--category",
            "equipment",
        ],
    )
    .unwrap();
    let item = store
        .db
        .items
        .iter()
        .find(|i| i.name == "Two-handed weapon")
        .unwrap()
        .id
        .clone();
    assert_eq!(
        store
            .db
            .equipment_list_items
            .iter()
            .filter(|r| r.item_id == item)
            .count(),
        2
    );
    command(&mut store, &["find", &item, "4"]).unwrap();
    for unit in ["hans-unit", "champion-one-unit", "arrows-one-unit"] {
        command(&mut store, &["equip", unit, &item]).unwrap();
    }
    assert_eq!(store.db.assigned("example-warband", &item), 4);
    let before = store.db.clone();
    let error = command(&mut store, &["equip", "swordsmen-unit", &item])
        .unwrap_err()
        .to_string();
    assert!(error.contains("equipment list"), "{error}");
    assert_eq!(store.db, before);
    command(
        &mut store,
        &["recruit", "Warrior", "New warrior", "--price", "0"],
    )
    .unwrap();
    let recruit = store
        .db
        .members
        .iter()
        .find(|m| m.name.starts_with("New warrior"))
        .unwrap();
    assert_eq!(
        store.db.equipment_list_for(&recruit.id).unwrap().name,
        "Warriors"
    );
    assert!(store.db.equipment_allowed(&recruit.id, &item));
    assert_eq!(
        engine::roster(&store.db, "example-warband")[0]["equipment_list"]["name"],
        "Heroes"
    );
    let saved = store.db.clone();
    drop(store);
    assert_eq!(Store::open(dir.path()).unwrap().db, saved);
}

#[test]
fn individual_and_group_overrides_apply_in_order_and_group_equip_checks_every_model() {
    let (_dir, mut store) = example();
    let original = store
        .db
        .equipment_list_for("arrows-one-1")
        .unwrap()
        .id
        .clone();
    command(&mut store, &["equipment-list", "create", "Restricted"]).unwrap();
    command(
        &mut store,
        &["equipment-list", "assign", "Restricted", "arrows-one-unit"],
    )
    .unwrap();
    command(
        &mut store,
        &["equipment-list", "assign", &original, "arrows-one-1"],
    )
    .unwrap();
    assert!(
        store
            .db
            .equipment_allowed("arrows-one-1", "burgomeisters-bow")
    );
    assert!(
        !store
            .db
            .equipment_allowed("arrows-one-2", "burgomeisters-bow")
    );
    let before = store.db.clone();
    assert!(command(&mut store, &["equip", "arrows-one-unit", "Bow"]).is_err());
    assert_eq!(store.db, before);
    command(
        &mut store,
        &["equipment-list", "assign", &original, "arrows-one-unit"],
    )
    .unwrap();
    command(
        &mut store,
        &[
            "equipment-list",
            "assign",
            "Restricted",
            "burgomeisters-marksmen",
        ],
    )
    .unwrap();
    assert!(
        store
            .db
            .equipment_allowed("arrows-one-2", "burgomeisters-bow")
    );
    assert!(
        !store
            .db
            .equipment_allowed("arrows-two-1", "burgomeisters-bow")
    );
    command(
        &mut store,
        &["equipment-list", "assign", "inherit", "arrows-one-unit"],
    )
    .unwrap();
    assert!(
        store
            .db
            .equipment_allowed("arrows-one-1", "burgomeisters-bow")
    );
    assert!(
        !store
            .db
            .equipment_allowed("arrows-one-2", "burgomeisters-bow")
    );
    command(
        &mut store,
        &["equipment-list", "assign", "inherit", "arrows-one-1"],
    )
    .unwrap();
    assert!(
        !store
            .db
            .equipment_allowed("arrows-one-1", "burgomeisters-bow")
    );
    assert_eq!(store.db.allocations, before.allocations); // Rule changes retain current loadouts.
}

#[test]
fn editing_item_memberships_is_atomic_scoped_and_does_not_remove_current_equipment() {
    let (_dir, mut store) = example();
    let mut db = store.db.clone();
    let mut profile = db.profiles[0].clone();
    profile.id = "foreign".into();
    profile.name = "Foreign rules".into();
    db.profiles.push(profile);
    db.equipment_lists.push(EquipmentList {
        id: "foreign-list".into(),
        profile_id: "foreign".into(),
        name: "Foreign list".into(),
        description: String::new(),
    });
    store
        .commit(db, "Foreign list fixture", "2026-10-20", "", "manual")
        .unwrap();
    let foreign = store
        .db
        .equipment_lists
        .iter()
        .find(|l| l.profile_id == "foreign")
        .unwrap()
        .id
        .clone();
    let own = store.db.equipment_list_for("hans").unwrap().id.clone();
    let before = store.db.clone();
    for args in [
        vec![
            "add-equipment",
            "Bad item",
            "--list",
            &own,
            "--list",
            &foreign,
        ],
        vec!["equipment-list", "assign", &foreign, "hans"],
        vec!["equipment-list", "item", "Sword", "--list", &foreign],
        vec![
            "equipment-list",
            "assign",
            "inherit",
            "burgomeisters-captain",
        ],
    ] {
        assert!(command(&mut store, &args).is_err(), "{args:?}");
        assert_eq!(store.db, before);
    }
    command(
        &mut store,
        &[
            "equipment-list",
            "item",
            "Sword",
            "--list",
            &own,
            "--list",
            &own,
        ],
    )
    .unwrap();
    assert_eq!(
        store
            .db
            .equipment_list_items
            .iter()
            .filter(|r| r.item_id == "burgomeisters-sword")
            .count(),
        1
    );
    assert!(store.db.equipment_allowed("hans", "burgomeisters-sword"));
    assert!(
        store
            .db
            .equipment_allowed("champion-one", "burgomeisters-sword")
    );
    assert!(
        !store
            .db
            .equipment_allowed("swordsmen", "burgomeisters-sword")
    );
    assert_eq!(store.db.allocations, before.allocations);
    assert_eq!(store.db.holdings, before.holdings);
    command(&mut store, &["equipment-list", "item", "Sword"]).unwrap();
    assert!(!store.db.equipment_allowed("hans", "burgomeisters-sword"));
    assert_eq!(store.db.allocations, before.allocations);
    let lists = command(&mut store, &["list", "equipment-lists"]).unwrap();
    assert!(
        lists
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["list"]["profile_id"] == "burgomeisters")
    );
    let mut invalid = store.db.clone();
    invalid
        .equipment_list_assignments
        .push(EquipmentListAssignment {
            id: id(),
            entity_id: "hans".into(),
            list_id: foreign,
        });
    assert!(invalid.validate().is_err());
}
