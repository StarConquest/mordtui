use clap::Parser;
use mordtui::{cli::Cli, engine, model::*, store::Store};
use serde_json::Value;

fn command(store: &mut Store, args: &[&str]) -> anyhow::Result<Value> {
    engine::execute(
        store,
        &Cli::try_parse_from(std::iter::once("mordheim").chain(args.iter().copied()))?,
    )
}
fn setup(xp: u32) -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let mut db: Db = serde_json::from_str(include_str!("../examples/burgomeisters.json")).unwrap();
    db.types
        .iter_mut()
        .find(|t| t.id == "burgomeisters-marksmen")
        .unwrap()
        .limit = 20;
    db.units
        .iter_mut()
        .find(|u| u.id == "arrows-one-unit")
        .unwrap()
        .starting_xp = xp;
    db.normalize_equipment_lists().unwrap();
    store
        .commit(db, "Opening", "2026-10-24", "", "manual")
        .unwrap();
    command(&mut store, &["gold", "1000"]).unwrap();
    (dir, store)
}
fn post_battle(store: &mut Store, name: &str) {
    command(store, &["battle", "prepare", name]).unwrap();
    command(store, &["battle", "start"]).unwrap();
    command(store, &["battle", "end", "win"]).unwrap();
}
fn complete(store: &mut Store) {
    for step in STEPS {
        command(
            store,
            &["--reason", "No other outcomes", "step", step, "--skip"],
        )
        .unwrap();
    }
    command(store, &["battle", "complete"]).unwrap();
}
fn hire(store: &mut Store, count: &str) -> anyhow::Result<Value> {
    command(
        store,
        &[
            "recruit",
            "Marksmen",
            "1st Arrows",
            "--group",
            "arrows-one-unit",
            "--count",
            count,
        ],
    )
}
fn gear(db: &Db, member: &str) -> std::collections::BTreeMap<String, u32> {
    db.allocations
        .iter()
        .filter(|a| a.member_id == member)
        .map(|a| (a.item_id.clone(), a.quantity))
        .collect()
}

#[test]
fn published_experienced_group_examples_charge_base_cost_plus_two_gold_per_new_xp() {
    // Trading, "New recruits and existing henchmen groups", printed page 102:
    // https://broheim.net/downloads/rules/Mordheim%20-%20Part%203%20-%20Campaigns%20%26%20Optional%20Rules.pdf
    // A roll of 7 permits one recruit with 7 XP or two with 3 XP apiece.
    // These expected prices come from that rule and the fixture's 25 gc Marksmen,
    // with 23 gc of equipment per recruit (hammer, long bow, shield; free dagger).
    for (xp, count, expected_hire, expected_gear, expected_xp) in
        [(7, 1, 39, 23, 7), (3, 2, 62, 46, 6), (0, 2, 50, 46, 0)]
    {
        let (_dir, mut store) = setup(xp);
        post_battle(&mut store, "Published recruitment example");
        complete(&mut store);
        command(&mut store, &["veterans", "7"]).unwrap();
        let gold = store.db.gold("example-warband");
        let first_entry = store.db.ledger.len();
        hire(&mut store, &count.to_string()).unwrap();
        let entries = &store.db.ledger[first_entry..];
        let hire = entries.iter().find(|l| l.category == "hire").unwrap();
        assert_eq!(hire.quantity, count);
        assert_eq!(hire.gold, -expected_hire);
        assert_eq!(
            entries
                .iter()
                .filter(|l| l.category == "purchase")
                .map(|l| -l.gold)
                .sum::<i64>(),
            expected_gear
        );
        assert_eq!(
            store.db.gold("example-warband"),
            gold - expected_hire - expected_gear
        );
        let battle = store.db.battles.last().unwrap();
        assert_eq!(battle.veteran_spent, expected_xp);
        assert_eq!(battle.veteran_budget, 7 - expected_xp);
    }
}

#[test]
fn completed_battle_allowance_can_be_added_and_corrected_and_common_shortfalls_are_bought() {
    let (dir, mut store) = setup(2);
    post_battle(&mut store, "First battle");
    complete(&mut store);
    let original = store.db.clone();
    let error = hire(&mut store, "2").unwrap_err().to_string();
    assert!(
        error.contains("need 4") && error.contains("V on Warband"),
        "{error}"
    );
    assert_eq!(store.db, original);
    let snapshot = store.db.battles.last().unwrap().post_snapshot.clone();
    let stages = store.db.steps.clone();
    command(&mut store, &["veterans", "7"]).unwrap();
    command(&mut store, &["find", "Hammer", "1"]).unwrap();
    let gold = store.db.gold("example-warband");
    let ledger = store.db.ledger.len();
    hire(&mut store, "2").unwrap();
    // Two hires at 25 + 2*2 XP; included daggers, one spare hammer, buy only shortages.
    assert_eq!(store.db.gold("example-warband"), gold - 58 - 43);
    let purchases: std::collections::BTreeMap<_, _> = store.db.ledger[ledger..]
        .iter()
        .filter(|l| l.category == "purchase")
        .map(|l| (l.item_id.as_str(), (l.quantity, l.gold)))
        .collect();
    assert_eq!(purchases.get("burgomeisters-hammer"), Some(&(1, -3)));
    assert_eq!(purchases.get("burgomeisters-long-bow"), Some(&(2, -30)));
    assert_eq!(purchases.get("burgomeisters-shield"), Some(&(2, -10)));
    assert!(!purchases.contains_key("burgomeisters-dagger"));
    let members: Vec<_> = store
        .db
        .members
        .iter()
        .filter(|m| m.unit_id == "arrows-one-unit" && active(m))
        .collect();
    assert_eq!(members.len(), 4);
    assert!(
        members
            .iter()
            .all(|m| gear(&store.db, &m.id) == gear(&store.db, &members[0].id))
    );
    let b = store.db.battles.last().unwrap();
    assert_eq!(
        (b.status.as_str(), b.veteran_budget, b.veteran_spent),
        ("complete", 3, 4)
    );
    assert_eq!(b.post_snapshot, snapshot);
    assert_eq!(store.db.steps, stages);
    command(&mut store, &["veterans", "7"]).unwrap();
    assert_eq!(store.db.battles.last().unwrap().veteran_budget, 3);
    command(&mut store, &["veterans", "9"]).unwrap();
    assert_eq!(store.db.battles.last().unwrap().veteran_budget, 5);
    let before = store.db.clone();
    assert!(
        command(&mut store, &["veterans", "3"])
            .unwrap_err()
            .to_string()
            .contains("already spent")
    );
    assert_eq!(store.db, before);
    let saved = store.db.clone();
    drop(store);
    store = Store::open(dir.path()).unwrap();
    assert_eq!(store.db, saved);
    hire(&mut store, "1").unwrap();
    assert_eq!(store.db.battles.last().unwrap().veteran_budget, 3);
    assert_eq!(store.db.battles.last().unwrap().veteran_spent, 6);
}

#[test]
fn active_battles_stay_locked_and_new_post_battle_does_not_reuse_an_older_allowance() {
    let (_dir, mut store) = setup(2);
    post_battle(&mut store, "First");
    complete(&mut store);
    command(&mut store, &["veterans", "20"]).unwrap();
    let older = store.db.battles.last().unwrap().id.clone();
    command(&mut store, &["battle", "prepare", "Second"]).unwrap();
    let before = store.db.clone();
    assert!(
        hire(&mut store, "1")
            .unwrap_err()
            .to_string()
            .contains("locked")
    );
    assert!(
        command(&mut store, &["veterans", "30"])
            .unwrap_err()
            .to_string()
            .contains("locked")
    );
    assert_eq!(store.db, before);
    command(&mut store, &["battle", "start"]).unwrap();
    assert!(hire(&mut store, "1").is_err());
    command(&mut store, &["battle", "end", "win"]).unwrap();
    let before = store.db.clone();
    assert!(
        hire(&mut store, "1")
            .unwrap_err()
            .to_string()
            .contains("Insufficient available veteran")
    );
    assert_eq!(store.db, before);
    command(&mut store, &["veterans", "6"]).unwrap();
    hire(&mut store, "2").unwrap();
    let b = store.db.battles.last().unwrap();
    assert_eq!((b.veteran_budget, b.veteran_spent), (2, 4));
    assert_eq!(
        store
            .db
            .battles
            .iter()
            .find(|b| b.id == older)
            .unwrap()
            .veteran_budget,
        20
    );
    command(&mut store, &["step", "veterans", "--skip"]).unwrap();
    command(&mut store, &["veterans", "7"]).unwrap();
    assert_eq!(store.db.battles.last().unwrap().veteran_budget, 3);
}

#[test]
fn rare_equipment_is_used_from_stash_but_never_automatically_purchased() {
    let (_dir, mut store) = setup(0);
    let mut db = store.db.clone();
    let list = db.equipment_list_for("arrows-one-unit").unwrap().id.clone();
    db.add_to_equipment_list(&list, "burgomeisters-lucky-charm");
    store
        .commit(db, "Permit charm", "2026-10-24", "", "manual")
        .unwrap();
    command(&mut store, &["find", "Lucky Charm", "2"]).unwrap();
    command(&mut store, &["equip", "1st Arrows", "Lucky Charm"]).unwrap();
    let before = store.db.clone();
    let error = hire(&mut store, "1").unwrap_err().to_string();
    assert!(
        error.contains("Lucky Charm") && error.contains("cannot be automatically purchased"),
        "{error}"
    );
    assert_eq!(store.db, before);
    command(&mut store, &["find", "Lucky Charm", "1"]).unwrap();
    hire(&mut store, "1").unwrap();
    assert_eq!(
        store
            .db
            .assigned("example-warband", "burgomeisters-lucky-charm"),
        3
    );
    assert!(
        !store
            .db
            .ledger
            .iter()
            .any(|l| l.category == "purchase" && l.item_id == "burgomeisters-lucky-charm")
    );
}

#[test]
fn insufficient_total_funds_or_unavailable_equipment_rolls_back_the_entire_hire() {
    let (_dir, mut store) = setup(2);
    post_battle(&mut store, "Battle");
    complete(&mut store);
    command(&mut store, &["veterans", "8"]).unwrap();
    let remove = store.db.gold("example-warband") - 30;
    command(&mut store, &["gold", &(-remove).to_string()]).unwrap();
    let before = store.db.clone();
    assert!(
        hire(&mut store, "1")
            .unwrap_err()
            .to_string()
            .contains("Insufficient gold")
    );
    assert_eq!(store.db, before); // Includes allowance, purchases, members and journal.
    command(&mut store, &["gold", "100"]).unwrap();
    let mut db = store.db.clone();
    let list = db.equipment_list_for("arrows-one-unit").unwrap().id.clone();
    db.equipment_list_items
        .retain(|i| !(i.list_id == list && i.item_id == "burgomeisters-long-bow"));
    store
        .commit(db, "Remove bow availability", "2026-10-24", "", "manual")
        .unwrap();
    let before = store.db.clone();
    assert!(
        hire(&mut store, "1")
            .unwrap_err()
            .to_string()
            .contains("not available")
    );
    assert_eq!(store.db, before);
}

#[test]
fn legacy_veteran_outcomes_preserve_previously_spent_xp_when_the_total_is_corrected() {
    let (_dir, mut store) = setup(2);
    post_battle(&mut store, "Legacy battle");
    complete(&mut store);
    let mut data = serde_json::to_value(&store.db).unwrap();
    let battle = &mut data["battles"][0];
    battle.as_object_mut().unwrap().remove("veteran_spent");
    battle["veteran_budget"] = 4.into();
    let mut db: Db = serde_json::from_value(data).unwrap();
    db.resolutions.push(Resolution {
        id: id(),
        battle_id: db.battles[0].id.clone(),
        key: "veterans".into(),
        description: "Available veteran experience: 8".into(),
        ..Resolution::default()
    });
    store
        .commit(db, "Legacy allowance", "2026-10-24", "", "manual")
        .unwrap();
    command(&mut store, &["veterans", "8"]).unwrap();
    assert_eq!(
        (
            store.db.battles[0].veteran_budget,
            store.db.battles[0].veteran_spent
        ),
        (4, 4)
    );
    command(&mut store, &["veterans", "10"]).unwrap();
    assert_eq!(
        (
            store.db.battles[0].veteran_budget,
            store.db.battles[0].veteran_spent
        ),
        (6, 4)
    );
}
