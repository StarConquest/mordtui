use clap::Parser;
use mordtui::{
    cli::Cli,
    engine::{execute, summary},
    model::*,
    store::Store,
};
use serde_json::Value;
use tempfile::TempDir;
fn command(store: &mut Store, args: &[&str]) -> anyhow::Result<Value> {
    execute(
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
fn workbook_totals_and_csv_roundtrip() {
    let (dir, store) = example();
    let s = summary(&store.db, "example-warband");
    assert_eq!(s["members"], 12);
    assert_eq!(s["rating"], 122);
    assert_eq!(s["gold"], 2);
    assert_eq!(s["pending_advances"], 0);
    assert_eq!(store.db.stat(&store.db.members[0], "W", None), "2");
    for h in &store.db.holdings {
        assert_eq!(
            store.db.owned("example-warband", &h.item_id),
            store.db.assigned("example-warband", &h.item_id)
        );
    }
    let original = store.db.clone();
    drop(store);
    let loaded = Store::open(dir.path()).unwrap();
    assert_eq!(original, loaded.db);
}
#[test]
fn rare_searches_exclude_ooa_heroes_and_each_success_allows_one_matching_purchase() {
    let (_dir, mut store) = example();
    let mut db = store.db.clone();
    let mut other = db.items.iter().find(|i| i.rarity > 0).unwrap().clone();
    other.id = "other-rare-item".into();
    other.name = "Other rare item".into();
    db.items.push(other);
    store
        .commit(db, "Test catalog", "2026-10-20", "", "manual")
        .unwrap();
    command(&mut store, &["gold", "100"]).unwrap();
    command(&mut store, &["battle", "prepare", "Trading"]).unwrap();
    command(&mut store, &["battle", "start"]).unwrap();
    command(
        &mut store,
        &["battle", "record", "hans", "--condition", "out-of-action"],
    )
    .unwrap();
    command(&mut store, &["battle", "end", "win"]).unwrap();
    command(&mut store, &["battle", "injury", "hans", "recovery"]).unwrap();
    assert!(
        command(
            &mut store,
            &["rare-search", "hans", "Lucky Charm", "--found", "true"]
        )
        .is_err()
    );
    let heroes: Vec<_> = store
        .db
        .members_of("example-warband")
        .filter(|m| m.id != "hans" && store.db.unit(&m.unit_id).role == "hero")
        .map(|m| m.id.clone())
        .collect();
    command(
        &mut store,
        &["rare-search", &heroes[0], "Lucky Charm", "--found", "true"],
    )
    .unwrap();
    let before = store.db.clone();
    for args in [
        vec!["rare-search", &heroes[0], "Lucky Charm", "--found", "true"],
        vec!["buy", "other-rare-item", "1"],
        vec!["buy", "Lucky Charm", "2"],
        vec!["buy", "Lucky Charm", "1", "--price", "10000"],
    ] {
        assert!(command(&mut store, &args).is_err());
        assert_eq!(store.db, before);
    }
    command(&mut store, &["buy", "Lucky Charm", "1", "--price", "3"]).unwrap();
    assert!(store.db.searches[0].purchased);
    assert_eq!(store.db.ledger.last().unwrap().phase, "rare");
    assert!(command(&mut store, &["buy", "Lucky Charm", "1"]).is_err());
    command(
        &mut store,
        &[
            "rare-search",
            &heroes[1],
            "other-rare-item",
            "--found",
            "false",
        ],
    )
    .unwrap();
    assert!(command(&mut store, &["buy", "other-rare-item", "1"]).is_err());
    assert!(
        command(
            &mut store,
            &[
                "rare-search",
                &heroes[1],
                "other-rare-item",
                "--found",
                "true"
            ]
        )
        .is_err()
    );
    assert!(
        command(
            &mut store,
            &["search", &heroes[2], "rare", "--allowance", "2"]
        )
        .is_err()
    );
}
#[test]
fn old_campaign_and_search_csv_headers_load_with_safe_defaults() {
    let campaign: Campaign = csv::Reader::from_reader("id,name,notes\nc,Campaign,old\n".as_bytes())
        .deserialize()
        .next()
        .unwrap()
        .unwrap();
    assert!(!campaign.skip_dramatis_personae);
    let search: Search = csv::Reader::from_reader(
        "id,battle_id,member_id,kind,allowance,used\ns,b,m,rare,1,1\n".as_bytes(),
    )
    .deserialize()
    .next()
    .unwrap()
    .unwrap();
    assert!(search.item_id.is_empty());
    assert!(search.outcome.is_empty());
    assert!(!search.purchased);
}
#[test]
fn dramatis_rule_applies_to_all_warbands_in_only_the_selected_campaign() {
    let (_dir, mut store) = example();
    let campaign = store.db.campaigns[0].id.clone();
    let profile = store.db.warbands[0].profile_id.clone();
    command(
        &mut store,
        &[
            "--campaign",
            &campaign,
            "warband",
            "Same campaign",
            "--profile",
            &profile,
        ],
    )
    .unwrap();
    let same = store.db.warbands.last().unwrap().id.clone();
    command(&mut store, &["campaign", "Other campaign"]).unwrap();
    let other_campaign = store.db.campaigns.last().unwrap().id.clone();
    command(
        &mut store,
        &[
            "--campaign",
            &other_campaign,
            "warband",
            "Other warband",
            "--profile",
            &profile,
        ],
    )
    .unwrap();
    let other = store.db.warbands.last().unwrap().id.clone();
    for w in ["example-warband", same.as_str(), other.as_str()] {
        if w != "example-warband" {
            command(
                &mut store,
                &["--warband", w, "recruit", "Captain", "Leader"],
            )
            .unwrap();
            command(
                &mut store,
                &[
                    "--warband",
                    w,
                    "recruit",
                    "Marksmen",
                    "Archers",
                    "--count",
                    "2",
                ],
            )
            .unwrap();
        }
        command(
            &mut store,
            &["--warband", w, "battle", "prepare", "Rule scope"],
        )
        .unwrap();
    }
    command(
        &mut store,
        &[
            "--warband",
            "example-warband",
            "campaign-rules",
            "--skip-dramatis-personae",
            "true",
        ],
    )
    .unwrap();
    assert!(store.db.skips_dramatis("example-warband"));
    assert!(store.db.skips_dramatis(&same));
    assert!(!store.db.skips_dramatis(&other));
    for s in store.db.steps.iter().filter(|s| s.name == "dramatis") {
        let b = store
            .db
            .battles
            .iter()
            .find(|b| b.id == s.battle_id)
            .unwrap();
        assert_eq!(
            s.status,
            if b.warband_id == other {
                "pending"
            } else {
                "skipped"
            }
        );
    }
}
#[test]
fn group_equipment_is_atomic() {
    let (_dir, mut store) = example();
    command(&mut store, &["--reason", "Gift", "find", "Sword", "1"]).unwrap();
    let before = store.db.clone();
    assert!(command(&mut store, &["equip", "1st Arrows", "Sword"]).is_err());
    assert_eq!(before, store.db);
    command(&mut store, &["find", "Sword", "1"]).unwrap();
    command(&mut store, &["equip", "1st Arrows", "Sword"]).unwrap();
    assert_eq!(
        store.db.assigned("example-warband", "burgomeisters-sword"),
        3
    );
    command(
        &mut store,
        &[
            "remove",
            "arrows-one-1",
            "--status",
            "dead",
            "--equipment",
            "lose",
        ],
    )
    .unwrap();
    assert_eq!(store.db.owned("example-warband", "burgomeisters-sword"), 2);
    assert_eq!(
        store.db.assigned("example-warband", "burgomeisters-sword"),
        2
    );
}
#[test]
fn pending_advances_survive_restart_once() {
    let (dir, mut store) = example();
    command(&mut store, &["xp", "Youngblood Two", "4"]).unwrap();
    assert_eq!(
        store.db.pending(store.db.unit("youngblood-two-unit")),
        vec![4, 6]
    );
    command(
        &mut store,
        &[
            "--reason",
            "Physical roll",
            "advance",
            "Youngblood Two",
            "--stat",
            "W",
        ],
    )
    .unwrap();
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    assert_eq!(
        store.db.pending(store.db.unit("youngblood-two-unit")),
        vec![6]
    );
    command(
        &mut store,
        &[
            "--reason",
            "Physical roll",
            "advance",
            "Youngblood Two",
            "--stat",
            "WS",
        ],
    )
    .unwrap();
    assert!(
        store
            .db
            .pending(store.db.unit("youngblood-two-unit"))
            .is_empty()
    );
    assert!(
        command(
            &mut store,
            &[
                "--reason",
                "No extra advance",
                "advance",
                "Youngblood Two",
                "--stat",
                "S"
            ]
        )
        .is_err()
    );
}
#[test]
fn caps_and_ambiguous_names_rejected() {
    let (_dir, mut store) = example();
    command(&mut store, &["xp", "Hans", "4"]).unwrap();
    assert!(
        command(
            &mut store,
            &["--reason", "roll", "advance", "Hans", "--stat", "M"]
        )
        .is_err()
    );
    command(&mut store, &["rename", "champion-one", "Twin"]).unwrap();
    command(&mut store, &["rename", "champion-two", "Twin"]).unwrap();
    assert!(command(&mut store, &["status", "Twin", "active"]).is_err());
}
#[test]
fn battle_recovery_and_sale_idempotence() {
    let (dir, mut store) = example();
    command(&mut store, &["battle", "prepare", "Test"]).unwrap();
    command(&mut store, &["battle", "start"]).unwrap();
    assert!(command(&mut store, &["buy", "Hammer", "1"]).is_err());
    command(
        &mut store,
        &["battle", "record", "hans", "--condition", "out-of-action"],
    )
    .unwrap();
    assert_eq!(store.db.members[0].status, "active");
    command(&mut store, &["battle", "end", "win"]).unwrap();
    assert!(command(&mut store, &["step", "injuries"]).is_err());
    command(&mut store, &["battle", "injury", "hans", "recovery"]).unwrap();
    command(&mut store, &["step", "injuries"]).unwrap();
    command(&mut store, &["resolve", "experience", "No awards in test"]).unwrap();
    command(&mut store, &["step", "experience"]).unwrap();
    command(
        &mut store,
        &["resolve", "exploration", "Physical roll", "--shards", "3"],
    )
    .unwrap();
    assert!(
        command(
            &mut store,
            &["resolve", "exploration", "Again", "--shards", "3"]
        )
        .is_err()
    );
    command(&mut store, &["step", "exploration"]).unwrap();
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    command(
        &mut store,
        &["--reason", "House price", "wyrdstone", "2", "80"],
    )
    .unwrap();
    assert_eq!(store.db.gold("example-warband"), 82);
    assert!(command(&mut store, &["--reason", "Again", "wyrdstone", "1", "40"]).is_err());
    assert!(command(&mut store, &["battle", "injury", "hans", "dead"]).is_err());
    command(&mut store, &["step", "wyrdstone"]).unwrap();
    for s in [
        "veterans",
        "rare",
        "dramatis",
        "recruitment",
        "equipment",
        "rating",
    ] {
        command(
            &mut store,
            &["--reason", "Test skips this step", "step", s, "--skip"],
        )
        .unwrap();
    }
    command(&mut store, &["battle", "complete"]).unwrap();
    let last = store.db.snapshots.last().unwrap();
    assert_eq!(
        store
            .historical(&last.generation)
            .unwrap()
            .battles
            .last()
            .unwrap()
            .status,
        "complete"
    );
    assert!(command(&mut store, &["battle", "complete"]).is_err());
}
#[test]
fn locking_checksum_and_orphan_recovery() {
    let (dir, store) = example();
    assert!(Store::open(dir.path()).is_err());
    std::fs::create_dir(dir.path().join("staging-interrupted")).unwrap();
    drop(store);
    let store = Store::open(dir.path()).unwrap();
    let path = dir
        .path()
        .join("generations")
        .join(&store.generation)
        .join("members.csv");
    std::fs::write(path, "corrupt").unwrap();
    drop(store);
    assert!(Store::open(dir.path()).is_err());
}
#[test]
fn backdate_undo_and_portable_bundle() {
    let (dir, mut store) = example();
    command(&mut store, &["snapshot", "Opening"]).unwrap();
    command(
        &mut store,
        &["--reason", "Reward", "--date", "2020-01-01", "gold", "10"],
    )
    .unwrap();
    assert_eq!(store.db.gold("example-warband"), 12);
    command(&mut store, &["--reason", "Correct reward", "undo"]).unwrap();
    assert_eq!(store.db.gold("example-warband"), 2);
    assert_eq!(store.db.events.len(), 4);
    let bundle = store.bundle().unwrap();
    let recovered = TempDir::new().unwrap();
    let mut other = Store::open(recovered.path()).unwrap();
    other.restore_bundle(bundle).unwrap();
    assert_eq!(other.db, store.db);
    command(&mut other, &["diff", "Opening"]).unwrap();
    assert!(dir.path().join("CURRENT").exists());
}
#[test]
fn import_preview_and_invalid_references() {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    command(&mut store, &["import", "examples/burgomeisters.json"]).unwrap();
    assert!(store.db.warbands.is_empty());
    let mut db: Db = serde_json::from_str(include_str!("../examples/burgomeisters.json")).unwrap();
    db.allocations[0].member_id = "missing".into();
    assert!(db.validate().is_err());
}
#[test]
fn profile_budgets_and_cli_typos() {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    command(
        &mut store,
        &[
            "init",
            "--rules",
            "examples/rules",
            "--profile",
            "marienburg",
            "--campaign",
            "City",
            "--name",
            "Watch",
        ],
    )
    .unwrap();
    let w = store.db.warbands[0].id.clone();
    assert_eq!(store.db.gold(&w), 600);
    command(&mut store, &["recruit", "Captain", "Hans"]).unwrap();
    assert_eq!(
        command(&mut store, &["list", "heros"])
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(command(&mut store, &["--override", "recruit", "Captain", "Extra"]).is_err());
}
#[test]
fn physical_dice_and_ai_preview_no_writes() {
    let (_dir, mut store) = example();
    let roll = command(&mut store, &["roll", "2D6+1", "--results", "2,6"]).unwrap();
    assert_eq!(roll["total"], 9);
    let before = store.generation.clone();
    let preview = command(&mut store, &["ai", "What should I buy?"]).unwrap();
    assert_eq!(preview["preview"], true);
    assert_eq!(store.generation, before);
    assert!(command(&mut store, &["roll", "2D6", "--results", "1,7"]).is_err());
}
#[test]
fn promotion_preserves_identity_and_queues_immediate_advance() {
    let (_dir, mut store) = example();
    command(
        &mut store,
        &[
            "equipment-list",
            "create",
            "Group weapons",
            "--item",
            "Dagger,Bow",
        ],
    )
    .unwrap();
    command(
        &mut store,
        &[
            "equipment-list",
            "assign",
            "Group weapons",
            "arrows-one-unit",
        ],
    )
    .unwrap();
    command(&mut store, &["xp", "1st Arrows", "2"]).unwrap();
    command(
        &mut store,
        &[
            "--reason",
            "The lad has talent",
            "promote",
            "arrows-one-1",
            "--abilities",
            "shooting,strength",
        ],
    )
    .unwrap();
    let m = store
        .db
        .members
        .iter()
        .find(|m| m.id == "arrows-one-1")
        .unwrap();
    let new_id = m.unit_id.clone();
    assert_eq!(
        store.db.equipment_list_for(&m.id).unwrap().name,
        "Group weapons"
    );
    assert_eq!(store.db.unit(&new_id).role, "hero");
    assert_eq!(store.db.pending(store.db.unit(&new_id)), vec![0]);
    assert_eq!(store.db.pending(store.db.unit("arrows-one-unit")), vec![2]);
    command(
        &mut store,
        &[
            "--reason",
            "Immediate Hero roll",
            "advance",
            &new_id,
            "--ability",
            "Quick Shot",
        ],
    )
    .unwrap();
    assert!(store.db.pending(store.db.unit(&new_id)).is_empty());
    assert_eq!(
        store
            .db
            .members
            .iter()
            .find(|m| m.id == "arrows-one-1")
            .unwrap()
            .id,
        "arrows-one-1"
    );
}
#[test]
fn random_characteristics_and_frozen_battle_reference() {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let mut db: Db = serde_json::from_str(include_str!("../examples/burgomeisters.json")).unwrap();
    db.types
        .iter_mut()
        .find(|t| t.id == "burgomeisters-captain")
        .unwrap()
        .s = "D6".into();
    store
        .commit(
            db,
            "Custom random characteristic",
            "2026-10-19",
            "House profile",
            "manual",
        )
        .unwrap();
    assert_eq!(store.db.stat(&store.db.members[0], "S", None), "D6");
    command(&mut store, &["battle", "prepare", "Random game"]).unwrap();
    command(
        &mut store,
        &[
            "--reason",
            "Physical D6=5",
            "stat",
            "hans",
            "S",
            "5",
            "--battle",
        ],
    )
    .unwrap();
    let b = store.db.battles.last().unwrap();
    assert_eq!(store.db.stat(&store.db.members[0], "S", Some(&b.id)), "5");
    assert_eq!(store.db.stat(&store.db.members[0], "S", None), "D6");
    let view = command(&mut store, &["list", "battle"]).unwrap();
    assert_eq!(view["reference_roster"][0]["stats"]["S"], "D6");
}
#[test]
fn copied_csv_reconciliation_is_audited() {
    let (dir, mut store) = example();
    let exported = dir.path().join("editable");
    command(
        &mut store,
        &["export", exported.to_str().unwrap(), "--format", "csv"],
    )
    .unwrap();
    let path = exported.join("members.csv");
    let csv = std::fs::read_to_string(&path)
        .unwrap()
        .replace(",Hans,", ",Hans the Bold,");
    std::fs::write(&path, csv).unwrap();
    command(
        &mut store,
        &[
            "--reason",
            "Rename from edited copy",
            "import",
            exported.to_str().unwrap(),
            "--reconcile",
            "--apply",
        ],
    )
    .unwrap();
    assert_eq!(store.db.members[0].name, "Hans the Bold");
    assert_eq!(store.db.events.len(), 2);
}
#[test]
fn migration_maps_equipment_and_keeps_historical_rules() {
    let (_dir, mut store) = example();
    let rules = mordtui::rules::load(std::path::Path::new("examples/rules")).unwrap();
    let mut additional = rules.clone();
    additional.profiles.retain(|p| p.id == "reikland");
    additional.types.retain(|p| p.profile_id == "reikland");
    additional.items.retain(|p| p.profile_id == "reikland");
    additional
        .equipment_lists
        .retain(|p| p.profile_id == "reikland");
    additional
        .equipment_list_items
        .retain(|p| additional.equipment_lists.iter().any(|l| l.id == p.list_id));
    additional
        .equipment_list_assignments
        .retain(|p| additional.types.iter().any(|t| t.id == p.entity_id));
    additional.abilities.retain(|p| p.profile_id == "reikland");
    additional.thresholds.retain(|p| p.profile_id == "reikland");
    additional
        .caps
        .retain(|p| p.type_id.starts_with("reikland-"));
    additional
        .permissions
        .retain(|p| p.type_id.starts_with("reikland-"));
    let mut db = store.db.clone();
    mordtui::rules::merge(&mut db, additional).unwrap();
    store
        .commit(db, "Install rules", "2026-10-19", "", "manual")
        .unwrap();
    command(
        &mut store,
        &[
            "equipment-list",
            "create",
            "Personal weapons",
            "--item",
            "Dagger,Sword",
        ],
    )
    .unwrap();
    command(
        &mut store,
        &["equipment-list", "assign", "Personal weapons", "hans"],
    )
    .unwrap();
    command(
        &mut store,
        &[
            "--reason",
            "Agree rules update",
            "migrate-rules",
            "reikland",
            "--apply",
        ],
    )
    .unwrap();
    assert_eq!(store.db.warbands[0].profile_id, "reikland");
    assert_eq!(
        store.db.equipment_list_for("hans").unwrap().profile_id,
        "reikland"
    );
    assert!(store.db.equipment_allowed("hans", "reikland-sword"));
    assert!(!store.db.equipment_allowed("hans", "reikland-bow"));
    assert_eq!(store.db.owned("example-warband", "reikland-sword"), 1);
    assert!(store.db.profiles.iter().any(|p| p.id == "burgomeisters"));
}
#[test]
fn executable_reports_errors_and_json() {
    let dir = TempDir::new().unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_mordheim"))
            .arg("--db")
            .arg(dir.path())
            .args(args)
            .output()
            .unwrap()
    };
    assert!(
        run(&["import", "examples/burgomeisters.json", "--apply"])
            .status
            .success()
    );
    let listed = run(&["--json", "list", "heros"]);
    assert!(listed.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&listed.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert!(!run(&["buy", "Sword", "100"]).status.success());
}

#[test]
fn named_kill_credits_roundtrip_correct_individually_and_preserve_legacy_totals() {
    let (dir, mut store) = example();
    command(&mut store, &["battle", "prepare", "Kill records"]).unwrap();
    command(&mut store, &["battle", "start"]).unwrap();
    command(
        &mut store,
        &[
            "battle",
            "record",
            "hans",
            "--kills",
            "2",
            "--objectives",
            "3",
            "--attribution",
            "Hit by enemy archer",
        ],
    )
    .unwrap();
    assert!(
        store
            .db
            .notes
            .iter()
            .any(|n| n.entity_id == "hans" && n.text.contains("Hit by enemy archer"))
    );
    command(
        &mut store,
        &["battle", "kill", "hans", "--victim", "Goblin, 'Red Hat'"],
    )
    .unwrap();
    command(
        &mut store,
        &["battle", "kill", "hans", "--victim", "Orc captain"],
    )
    .unwrap();
    command(&mut store, &["battle", "record", "hans", "--wounds", "1"]).unwrap();
    let p = store
        .db
        .participants
        .iter()
        .find(|p| p.member_id == "hans")
        .unwrap();
    assert_eq!((p.kills, p.objectives, p.wounds), (4, 3, 1));
    assert_eq!(store.db.battle_kills(p).count(), 2);
    let first = store.db.battle_kills(p).next().unwrap().id.clone();
    let before = store.db.clone();
    let generation = store.generation.clone();
    assert!(command(&mut store, &["battle", "record", "hans", "--kills", "1"]).is_err());
    assert!(
        command(
            &mut store,
            &["battle", "kill", "champion-one", "--remove", &first]
        )
        .is_err()
    );
    assert!(
        command(
            &mut store,
            &[
                "battle",
                "kill",
                "hans",
                "--remove",
                &first,
                "--victim",
                "Unrelated opponent"
            ]
        )
        .is_err()
    );
    assert_eq!(store.db, before);
    assert_eq!(store.generation, generation);
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    assert_eq!(store.db, before);
    command(&mut store, &["battle", "kill", "hans", "--remove", &first]).unwrap();
    command(
        &mut store,
        &["battle", "kill", "hans", "--remove", "unspecified"],
    )
    .unwrap();
    command(
        &mut store,
        &["battle", "kill", "hans", "--remove", "unspecified"],
    )
    .unwrap();
    let p = store
        .db
        .participants
        .iter()
        .find(|p| p.member_id == "hans")
        .unwrap();
    assert_eq!((p.kills, p.objectives, p.wounds), (1, 3, 1));
    assert_eq!(
        store.db.battle_kills(p).next().unwrap().description,
        "Orc captain"
    );
    let last = store.db.battle_kills(p).next().unwrap().id.clone();
    let before = store.db.clone();
    assert!(
        command(
            &mut store,
            &["battle", "kill", "hans", "--remove", "unspecified"]
        )
        .is_err()
    );
    assert_eq!(store.db, before);
    assert!(
        store
            .historical(&generation)
            .unwrap()
            .resolutions
            .iter()
            .any(|r| r.id == first)
    );
    command(&mut store, &["battle", "kill", "hans", "--remove", &last]).unwrap();
    let before = store.db.clone();
    assert!(command(&mut store, &["battle", "kill", "hans", "--remove", &last]).is_err());
    assert_eq!(store.db, before);
    command(&mut store, &["battle", "end", "win"]).unwrap();
    let before = store.db.clone();
    assert!(
        command(
            &mut store,
            &["battle", "kill", "hans", "--victim", "Late entry"]
        )
        .is_err()
    );
    assert_eq!(store.db, before);
}
