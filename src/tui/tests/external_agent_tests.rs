use super::*;
use crate::ai::exchange;

#[test]
fn proposals_survive_restart_pin_their_target_and_record_player_decisions() {
    let (dir, mut store, cli, mut app) = setup();
    let mut db = store.db.clone();
    let mut other = db.warbands[0].clone();
    other.id = "other-warband".into();
    other.name = "Other warband".into();
    other.leader_id.clear();
    db.warbands.push(other);
    store
        .commit(db, "Second warband", "2026-10-24", "", "manual")
        .unwrap();
    offer_plan(&mut app, &store);
    let id = app.ai.proposal.as_ref().unwrap().id.clone();
    // Close/reopen the workbench before reviewing, then browse a different warband.
    app = App::new(&store, &cli);
    app.wb = Some("other-warband".into());
    app.next_ai(&store);
    app.tab = AI_TAB;
    let before = store.db.clone();
    let text = screen(&mut app, &store, 80, 24);
    assert!(text.contains("Target: Burgeoning Burgomeisters"), "{text}");
    for key in ['r', 'y'] {
        app.key(super::key(KeyCode::Char(key)), &mut store, &cli);
    }
    // A late external arrival cannot replace the plan behind the approval dialog.
    let late_plan =
        crate::ai::plan::Plan::parse(r#"{"summary":"Advice arriving during review","actions":[]}"#)
            .unwrap();
    let late = exchange::submit(
        &store.root,
        &store.generation,
        Some("example-warband"),
        None,
        late_plan,
    )
    .unwrap();
    app.poll_ai(&store);
    assert_eq!(app.ai.proposal.as_ref().unwrap().id, id);
    assert!(matches!(app.overlay, Overlay::AiApproval { .. }));
    assert_eq!(store.db, before);
    assert_eq!(
        exchange::status(&store.root, &id).unwrap()["review"]["valid"],
        true
    );
    app.key(super::key(KeyCode::Char('y')), &mut store, &cli);
    assert_eq!(store.db.gold("example-warband"), 12);
    assert_eq!(store.db.gold("other-warband"), 0);
    assert_eq!(
        exchange::status(&store.root, &id).unwrap()["decision"]["status"],
        "applied"
    );
    assert_eq!(exchange::pending(&store.root).unwrap().len(), 1);
    assert_eq!(
        exchange::status(&store.root, &late.id).unwrap()["decision"]["status"],
        "stale"
    );
    exchange::decide(
        &store.root,
        &late,
        "rejected",
        "Dismiss stale advice",
        &store.generation,
    )
    .unwrap();
    let saved = store.db.clone();
    drop(store);
    store = Store::open(dir.path()).unwrap();
    app = App::new(&store, &cli);
    app.poll_ai(&store);
    assert!(app.ai.plan.is_none());
    assert_eq!(store.db, saved);
}

#[test]
fn cycling_proposals_clears_review_and_rejecting_persists_without_campaign_mutation() {
    let (_dir, mut store, cli, mut app) = setup();
    offer_plan(&mut app, &store);
    let first = app.ai.proposal.as_ref().unwrap().id.clone();
    let plan = crate::ai::plan::Plan::parse(r#"{"summary":"Alternative award","actions":[{"description":"Add five gold","args":["gold","5"]}]}"#).unwrap();
    let second = exchange::submit(
        &store.root,
        &store.generation,
        Some("example-warband"),
        None,
        plan,
    )
    .unwrap();
    let before = store.db.clone();
    app.key(key(KeyCode::Char('r')), &mut store, &cli);
    assert!(app.ai.reviewed.is_some());
    app.key(key(KeyCode::Char('p')), &mut store, &cli);
    assert_eq!(app.ai.proposal.as_ref().unwrap().id, second.id);
    assert!(app.ai.reviewed.is_none());
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    assert!(matches!(app.overlay, Overlay::None));
    app.key(key(KeyCode::Char('d')), &mut store, &cli);
    assert_eq!(
        exchange::status(&store.root, &second.id).unwrap()["decision"]["status"],
        "rejected"
    );
    app.key(key(KeyCode::Char('p')), &mut store, &cli);
    assert_eq!(app.ai.proposal.as_ref().unwrap().id, first);
    assert_eq!(store.db, before);
    let text = screen(&mut app, &store, 80, 24);
    assert!(
        text.contains("P Next proposal") && text.contains("D Reject"),
        "{text}"
    );
}

#[test]
fn agent_validation_feedback_and_stale_status_are_visible_without_partial_changes() {
    let (_dir, mut store, cli, mut app) = setup();
    let plan = crate::ai::plan::Plan::parse(r#"{"summary":"Invalid batch","actions":[{"description":"Add funds","args":["gold","100"]},{"description":"Buy unknown item","args":["buy","No such item","1"]}]}"#).unwrap();
    let proposal = exchange::submit(
        &store.root,
        &store.generation,
        Some("example-warband"),
        None,
        plan,
    )
    .unwrap();
    app.tab = AI_TAB;
    app.next_ai(&store);
    let before = store.db.clone();
    app.key(key(KeyCode::Char('r')), &mut store, &cli);
    assert!(app.ai.reviewed.is_none());
    let status = exchange::status(&store.root, &proposal.id).unwrap();
    assert_eq!(status["review"]["valid"], false);
    assert!(
        status["review"]["report"]
            .as_str()
            .unwrap()
            .contains("Action 2 failed")
    );
    assert_eq!(store.db, before);
    engine::execute(
        &mut store,
        &Cli::try_parse_from(["mordheim", "gold", "1"]).unwrap(),
    )
    .unwrap();
    assert!(screen(&mut app, &store, 80, 24).contains("STALE"));
    app.key(key(KeyCode::Char('r')), &mut store, &cli);
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    assert!(app.ai.reviewed.is_none());
    assert_eq!(store.db.gold("example-warband"), 3);
}
