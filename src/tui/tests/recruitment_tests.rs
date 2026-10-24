use super::*;

#[test]
fn completed_battle_veteran_entry_and_equipped_reinforcement_are_accessible_with_cost_review() {
    let (_dir, mut store, cli, mut app) = after_action();
    stage_fixture(&mut store, &mut app, "rating");
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    assert_eq!(store.db.battles.last().unwrap().status, "complete");
    let mut db = store.db.clone();
    db.units
        .iter_mut()
        .find(|u| u.id == "arrows-one-unit")
        .unwrap()
        .starting_xp = 2;
    store
        .commit(db, "Veteran group fixture", "2026-10-24", "", "manual")
        .unwrap();
    engine::execute(
        &mut store,
        &Cli::try_parse_from(["mordheim", "gold", "100"]).unwrap(),
    )
    .unwrap();
    app.tab = 4;
    assert!(screen(&mut app, &store, 80, 24).contains("V Correct veteran XP"));
    app.key(key(KeyCode::Char('v')), &mut store, &cli);
    assert!(
        matches!(&app.overlay,Overlay::Form(f) if f.action == Action::Veterans && f.value("battle") == store.db.battles[0].id)
    );
    let before = store.db.clone();
    field(&mut app, "experience", "6");
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert_eq!(store.db, before);
    app.tab = 1;
    assert!(screen(&mut app, &store, 80, 24).contains("V Veteran XP"));
    app.key(key(KeyCode::Char('v')), &mut store, &cli);
    field(&mut app, "experience", "6");
    submit(&mut app, &mut store, &cli);
    let stages = store.db.steps.clone();
    app.key(key(KeyCode::Char('h')), &mut store, &cli);
    focus_field(&mut app, &mut store, &cli, "group");
    app.choose("arrows-one-unit".into(), &store);
    assert!(
        matches!(&app.overlay,Overlay::Form(f) if f.value("kind")=="burgomeisters-marksmen" && f.value("name")=="1st Arrows")
    );
    app.review(&store);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    let impact = f.impact(&store, app.wb.as_deref()).unwrap();
    for text in [
        "Hire: 1 × 29",
        "Hammer",
        "Long Bow",
        "Shield",
        "1 included",
        "Equipment purchases: 23",
        "Total: 52",
        "Veteran XP: 2 required",
    ] {
        assert!(impact.contains(text), "missing {text}: {impact}");
    }
    let before = store.db.clone();
    app.key(key(KeyCode::Esc), &mut store, &cli);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert_eq!(store.db, before);
    app.key(key(KeyCode::Char('h')), &mut store, &cli);
    field(&mut app, "kind", "burgomeisters-marksmen");
    field(&mut app, "name", "1st Arrows");
    field(&mut app, "group", "arrows-one-unit");
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.gold("example-warband"), 50);
    assert_eq!(
        (
            store.db.battles[0].veteran_budget,
            store.db.battles[0].veteran_spent
        ),
        (4, 2)
    );
    assert_eq!(store.db.steps, stages);
    app.key(key(KeyCode::Char('v')), &mut store, &cli);
    assert!(matches!(&app.overlay,Overlay::Form(f) if f.value("experience")=="6"));
    field(&mut app, "experience", "8");
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.battles[0].veteran_budget, 6);
    assert_eq!(store.db.battles[0].status, "complete");
}

#[test]
fn choosing_a_completed_battle_reloads_its_total_and_preserves_the_other_allowance() {
    let (_dir, mut store, cli, mut app) = after_action();
    let mut db = store.db.clone();
    db.battles[0].status = "complete".into();
    db.battles[0].veteran_budget = 3;
    db.battles[0].veteran_spent = 2;
    for s in &mut db.steps {
        s.status = "skipped".into();
    }
    let first = db.battles[0].id.clone();
    let mut second = db.battles[0].clone();
    second.id = id();
    second.name = "Later battle".into();
    second.veteran_budget = 7;
    second.veteran_spent = 0;
    let second_id = second.id.clone();
    db.battles.push(second);
    store
        .commit(db, "Completed battle fixtures", "2026-10-24", "", "manual")
        .unwrap();
    app.tab = 1;
    app.key(key(KeyCode::Char('v')), &mut store, &cli);
    assert!(
        matches!(&app.overlay,Overlay::Form(f) if f.value("battle")==second_id && f.value("experience")=="7")
    );
    focus_field(&mut app, &mut store, &cli, "battle");
    app.choose(first.clone(), &store);
    assert!(matches!(&app.overlay,Overlay::Form(f) if f.value("experience")=="5"));
    field(&mut app, "experience", "8");
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.battles[0].veteran_budget, 6);
    assert_eq!(store.db.battles[1].veteran_budget, 7);
}
