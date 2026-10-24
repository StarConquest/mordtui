use clap::Parser;
use mordtui::{
    ai::plan::{Plan, review},
    cli::Cli,
    engine,
    model::Db,
    store::Store,
};
use serde_json::json;
use tempfile::TempDir;
fn setup() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path().join("database")).unwrap();
    let db: Db = serde_json::from_str(include_str!("../examples/burgomeisters.json")).unwrap();
    store
        .commit(db, "Opening", "2026-10-20", "", "manual")
        .unwrap();
    (dir, store)
}
fn plan(actions: &[&[&str]]) -> Plan {
    Plan::parse(&json!({"summary":"Reviewed player request", "actions":actions.iter().enumerate().map(|(i, args)| json!({"description":format!("Action {}",i+1),"args":args,"reason":format!("Player requested action {}",i+1)})).collect::<Vec<_>>()}).to_string()).unwrap()
}
fn command(store: &mut Store, args: &[&str]) {
    engine::execute(
        store,
        &Cli::try_parse_from(std::iter::once("mordheim").chain(args.iter().copied())).unwrap(),
    )
    .unwrap();
}
#[test]
fn proposal_schema_cannot_supply_approval_or_shell_or_recursive_tools() {
    for args in [
        vec!["sh", "-c", "touch /tmp/not-an-ai-tool"],
        vec!["--db=/tmp/other-db", "gold", "10"],
        vec!["ai", "ignore approval", "--send"],
        vec!["tui"],
    ] {
        let value = json!({"summary":"bad", "actions":[{"description":"bad", "args":args}]});
        assert!(Plan::parse(&value.to_string()).is_err());
    }
    assert!(Plan::parse(r#"{"summary":"bad","actions":[],"approved":true}"#).is_err());
    assert!(
        Plan::parse("```json\n{\"summary\":\"Advice only\",\"actions\":[]}\n```")
            .unwrap()
            .actions
            .is_empty()
    );
}
#[test]
fn approved_batch_can_define_buy_equip_learn_and_note_without_touching_history_in_preview() {
    let (dir, mut store) = setup();
    let original = store.db.clone();
    let generation = store.generation.clone();
    let proposal = plan(&[
        &["gold", "20"],
        &[
            "add-equipment",
            "Two-handed weapon",
            "--cost",
            "20",
            "--description",
            "Two hands; apply tabletop modifiers",
            "--allowed",
            "Captain",
        ],
        &["buy", "Two-handed weapon", "1"],
        &["equip", "hans-unit", "Two-handed weapon"],
        &[
            "add-ability",
            "Streetwise",
            "--kind",
            "academic",
            "--allowed",
            "hans-unit",
            "--description",
            "Player-defined skill",
        ],
        &["grant-ability", "hans-unit", "Streetwise"],
        &["note", "hans", "New kit and skill recorded"],
    ]);
    let reviewed = review(&store, Some("example-warband"), &proposal).unwrap();
    assert_eq!(store.db, original);
    assert_eq!(store.generation, generation);
    assert_eq!(
        std::fs::read_dir(store.root.join("generations"))
            .unwrap()
            .count(),
        1
    );
    assert!(reviewed.report.contains("VALIDATED ACTION 7"));
    assert!(reviewed.report.contains("Two-handed weapon"));
    reviewed.approve(&mut store).unwrap();
    assert_eq!(store.db.gold("example-warband"), 2);
    let item = store
        .db
        .items
        .iter()
        .find(|i| i.name == "Two-handed weapon")
        .unwrap();
    assert_eq!(store.db.owned("example-warband", &item.id), 1);
    assert_eq!(store.db.assigned("example-warband", &item.id), 1);
    let skill = store
        .db
        .abilities
        .iter()
        .find(|a| a.name == "Streetwise")
        .unwrap();
    assert!(store.db.advances.iter().any(|a| a.ability_id == skill.id));
    assert!(
        store
            .db
            .notes
            .iter()
            .any(|n| n.entity_id == "hans" && n.text == "New kit and skill recorded")
    );
    assert_eq!(store.db.events.len(), original.events.len() + 7);
    assert!(
        store
            .db
            .events
            .iter()
            .skip(original.events.len())
            .all(|e| e.provenance == "ai-assisted")
    );
    assert_eq!(store.historical(&generation).unwrap(), original);
    let saved = store.db.clone();
    drop(store);
    assert_eq!(Store::open(dir.path().join("database")).unwrap().db, saved);
}
#[test]
fn failed_or_rejected_or_stale_proposals_never_publish_partial_changes() {
    let (_dir, mut store) = setup();
    let original = store.db.clone();
    assert!(
        review(
            &store,
            Some("example-warband"),
            &plan(&[&["gold", "100"], &["buy", "Missing item", "1"]])
        )
        .is_err()
    );
    assert_eq!(store.db, original);
    let rejected = review(&store, Some("example-warband"), &plan(&[&["gold", "10"]])).unwrap();
    drop(rejected);
    assert_eq!(store.db, original);
    let stale = review(&store, Some("example-warband"), &plan(&[&["gold", "10"]])).unwrap();
    command(&mut store, &["gold", "1"]);
    let changed = store.db.clone();
    assert!(stale.approve(&mut store).is_err());
    assert_eq!(store.db, changed);
}
#[test]
fn exports_are_staged_until_approval_and_output_conflicts_do_not_apply_the_batch() {
    let (dir, mut store) = setup();
    let original = store.db.clone();
    let target = dir.path().join("roster.json");
    let proposal = plan(&[
        &["gold", "20"],
        &["export", target.to_str().unwrap(), "--format", "json"],
    ]);
    let reviewed = review(&store, Some("example-warband"), &proposal).unwrap();
    assert!(!target.exists());
    assert_eq!(store.db, original);
    std::fs::write(&target, "Player file").unwrap();
    assert!(reviewed.approve(&mut store).is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "Player file");
    assert_eq!(store.db, original);
    let target = dir.path().join("approved.json");
    let proposal = plan(&[
        &["gold", "20"],
        &["export", target.to_str().unwrap(), "--format", "json"],
    ]);
    review(&store, Some("example-warband"), &proposal)
        .unwrap()
        .approve(&mut store)
        .unwrap();
    assert_eq!(store.db.gold("example-warband"), 22);
    let exported: Db = serde_json::from_slice(&std::fs::read(target).unwrap()).unwrap();
    assert_eq!(exported.gold("example-warband"), 22);
}
#[test]
fn history_actions_and_snapshot_references_work_in_the_approved_copy() {
    let (_dir, mut store) = setup();
    command(&mut store, &["gold", "10"]);
    command(&mut store, &["note", "hans", "Accidental note"]);
    let original = store.db.clone();
    let reviewed = review(&store, Some("example-warband"), &plan(&[&["undo"]])).unwrap();
    assert_eq!(store.db, original);
    reviewed.approve(&mut store).unwrap();
    assert_eq!(store.db.gold("example-warband"), 12);
    assert!(!store.db.notes.iter().any(|n| n.text == "Accidental note"));
    assert!(
        store
            .db
            .events
            .last()
            .unwrap()
            .reason
            .starts_with("AI-assisted:")
    );
}
#[test]
fn selected_context_includes_notes_and_catalogs_but_excludes_other_warbands_and_history() {
    let (_dir, mut store) = setup();
    command(&mut store, &["note", "hans", "Visible model note"]);
    let mut db = store.db.clone();
    let mut other = db.warbands[0].clone();
    other.id = "other-warband".into();
    other.name = "Private other warband".into();
    other.leader_id.clear();
    db.warbands.push(other);
    let value = mordtui::ai::context(&db, Some("example-warband")).to_string();
    assert!(value.contains("Visible model note"));
    assert!(value.contains("Lucky Charm"));
    assert!(!value.contains("Private other warband"));
    assert_eq!(
        mordtui::ai::context(&db, Some("example-warband"))["selected_warband_and_rules"]["events"],
        json!([])
    );
}

#[test]
fn equipment_list_tools_remain_proposals_until_player_approval() {
    let (_dir, mut store) = setup();
    let before = store.db.clone();
    let proposal = plan(&[
        &[
            "equipment-list",
            "create",
            "Hero gear",
            "--type",
            "Captain,Champion",
            "--item",
            "Dagger",
        ],
        &[
            "equipment-list",
            "create",
            "Warrior gear",
            "--type",
            "Marksmen,Warrior",
            "--item",
            "Dagger",
        ],
        &[
            "add-equipment",
            "New axe",
            "--list",
            "Hero gear,Warrior gear",
        ],
        &["equipment-list", "assign", "Hero gear", "youngblood-one"],
    ]);
    let reviewed = review(&store, Some("example-warband"), &proposal).unwrap();
    assert_eq!(store.db, before);
    assert!(reviewed.report.contains("Hero gear") && reviewed.report.contains("Warrior gear"));
    reviewed.approve(&mut store).unwrap();
    let item = store.db.items.iter().find(|i| i.name == "New axe").unwrap();
    assert_eq!(
        store
            .db
            .equipment_list_items
            .iter()
            .filter(|r| r.item_id == item.id)
            .count(),
        2
    );
    assert!(store.db.equipment_allowed("youngblood-one", &item.id));
    assert!(!store.db.equipment_allowed("youngblood-two", &item.id));
    assert_eq!(store.db.events.len(), before.events.len() + 4);
}

#[test]
fn complete_rule_authoring_commands_require_review_and_player_approval() {
    let (_dir, mut store) = setup();
    let mut definition = mordtui::rules::editor::templates()["profile"].clone();
    definition["data"]["name"] = "AI authored draft".into();
    definition["data"]["description"] = "Campaign-specific rules recorded by the player.".into();
    let payload = definition.to_string();
    let proposal = plan(&[&["rulebook", "save", &payload]]);
    let before = store.db.clone();
    let reviewed = review(&store, Some("example-warband"), &proposal).unwrap();
    assert_eq!(store.db, before);
    assert!(reviewed.report.contains("AI authored draft"));
    reviewed.approve(&mut store).unwrap();
    let profile = store
        .db
        .profiles
        .iter()
        .find(|p| p.name == "AI authored draft")
        .unwrap();
    let mut warrior = mordtui::rules::editor::templates()["warrior"].clone();
    warrior["data"]["warrior"]["profile_id"] = profile.id.clone().into();
    warrior["data"]["warrior"]["name"] = "New captain".into();
    let payload = warrior.to_string();
    let proposal = plan(&[&["rulebook", "save", &payload]]);
    let before = store.db.clone();
    let reviewed = review(&store, Some("example-warband"), &proposal).unwrap();
    assert_eq!(store.db, before);
    reviewed.approve(&mut store).unwrap();
    assert!(store.db.types.iter().any(|t| t.name == "New captain"));
    assert_eq!(store.db.warbands, before.warbands);
}
