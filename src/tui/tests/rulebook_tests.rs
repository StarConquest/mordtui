use super::*;

#[test]
fn an_entire_rules_profile_and_playable_warband_can_be_authored_without_files() {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let cli = Cli::try_parse_from(["mordheim"]).unwrap();
    let mut app = App::new(&store, &cli);
    assert!(screen(&mut app, &store, 80, 24).contains("Program rules"));
    app.key(key(KeyCode::Char('7')), &mut store, &cli);
    assert_eq!(app.tab, RULES_TAB);
    assert!(screen(&mut app, &store, 80, 24).contains("from scratch"));
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    for (k, v) in [
        ("name", "Ash and Cinder"),
        ("faction", "Custom militia"),
        ("starting_gold", "675"),
        ("bonus_gold", "25"),
        ("max_members", "20"),
        ("max_heroes", "6"),
        ("initial_advance_cost", "10"),
        ("source", "Campaign handbook"),
        (
            "description",
            "Exploration and injuries use the campaign handbook.",
        ),
    ] {
        field(&mut app, k, v);
    }
    submit(&mut app, &mut store, &cli);
    assert!(store.db.warbands.is_empty());
    let profile = app.rule_profile(&store.db).unwrap().id.clone();
    assert_eq!(
        store.db.profiles[0].description,
        "Exploration and injuries use the campaign handbook."
    );
    app.rule_section(4);
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    for (k, v) in [
        ("name", "Rallying cry"),
        ("kind", "leadership"),
        ("source", "Militia handbook"),
        ("description", "Nearby warriors can hear the Marshal."),
    ] {
        field(&mut app, k, v);
    }
    submit(&mut app, &mut store, &cli);
    let ability = store.db.abilities[0].id.clone();
    app.rule_section(2);
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    field(&mut app, "name", "Militia equipment");
    submit(&mut app, &mut store, &cli);
    let list = store.db.equipment_lists[0].id.clone();
    app.rule_section(3);
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    field(&mut app, "name", "Dagger");
    field(&mut app, "category", "melee");
    field(&mut app, "description", "A basic blade.");
    focus_field(&mut app, &mut store, &cli, "lists");
    app.choose(list.clone(), &store);
    app.key(key(KeyCode::Enter), &mut store, &cli);
    submit(&mut app, &mut store, &cli);
    app.rule_section(1);
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    for (k, v) in [
        ("name", "Marshal"),
        ("cost", "50"),
        ("limit", "1"),
        ("starting_xp", "4"),
        ("ws", "4"),
        ("ld", "8"),
        ("cap_ws", "6"),
        ("cap_m", "6"),
        (
            "description",
            "Leads the militia; injury rules follow the handbook.",
        ),
        ("equipment_list", list.as_str()),
    ] {
        field(&mut app, k, v);
    }
    focus_field(&mut app, &mut store, &cli, "leader");
    focus_field(&mut app, &mut store, &cli, "free_dagger");
    focus_field(&mut app, &mut store, &cli, "abilities");
    app.choose(ability.clone(), &store);
    app.key(key(KeyCode::Enter), &mut store, &cli);
    focus_field(&mut app, &mut store, &cli, "starting_abilities");
    app.choose(ability.clone(), &store);
    app.key(key(KeyCode::Enter), &mut store, &cli);
    submit(&mut app, &mut store, &cli);
    let hero = store.db.types[0].id.clone();
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    for (k, v) in [
        ("name", "Militia"),
        ("role", "henchman"),
        ("cost", "20"),
        ("group_max", "5"),
        ("free_dagger", "true"),
        ("equipment_list", list.as_str()),
    ] {
        field(&mut app, k, v);
    }
    submit(&mut app, &mut store, &cli);
    let hench = store
        .db
        .types
        .iter()
        .find(|t| t.role == "henchman")
        .unwrap()
        .id
        .clone();
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    for (k, v) in [
        ("name", "Trail guide"),
        ("role", "hired"),
        ("cost", "35"),
        ("upkeep", "15"),
        ("gains_xp", "false"),
        ("counts_roster", "false"),
        ("counts_income", "false"),
        ("description", "Pay 15 gold after each battle."),
        ("equipment_list", list.as_str()),
    ] {
        field(&mut app, k, v);
    }
    submit(&mut app, &mut store, &cli);
    let hired = store
        .db
        .types
        .iter()
        .find(|t| t.role == "hired")
        .unwrap()
        .id
        .clone();
    app.rule_section(3);
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    field(&mut app, "name", "Walking staff");
    field(&mut app, "category", "melee");
    field(&mut app, "cost", "5");
    field(
        &mut app,
        "lists",
        &serde_json::to_string(&vec![list.clone()]).unwrap(),
    );
    submit(&mut app, &mut store, &cli);
    let staff = store
        .db
        .items
        .iter()
        .find(|i| i.name == "Walking staff")
        .unwrap()
        .id
        .clone();
    app.rule_section(6);
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    field(&mut app, "warrior", &hired);
    field(&mut app, "item", &staff);
    field(&mut app, "quantity", "2");
    submit(&mut app, &mut store, &cli);
    app.rule_section(5);
    app.key(key(KeyCode::Char('e')), &mut store, &cli);
    field(&mut app, "thresholds", "5, 7, 10");
    submit(&mut app, &mut store, &cli);
    app.selected[RULES_TAB] = 1;
    app.key(key(KeyCode::Char('e')), &mut store, &cli);
    field(&mut app, "thresholds", "2, 5, 9");
    submit(&mut app, &mut store, &cli);
    for section in 0..rulebook::SECTIONS.len() {
        app.rule_section(section);
        let s = screen(&mut app, &store, 80, 24);
        assert!(
            s.contains("P Profile") && s.contains("W Create warband"),
            "{s}"
        );
    }
    app.key(key(KeyCode::Char('w')), &mut store, &cli);
    field(&mut app, "name", "The Cinders");
    field(&mut app, "campaign_name", "Ashes");
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.tab, 1);
    let wb = app.wb.clone().unwrap();
    assert_eq!(store.db.gold(&wb), 700);
    for (kind, name, quantity) in [
        (&hero, "Brynn", "1"),
        (&hench, "Militia", "2"),
        (&hired, "Guide", "1"),
    ] {
        app.open(Action::Hire, &store, &cli);
        field(&mut app, "kind", kind);
        field(&mut app, "name", name);
        field(&mut app, "quantity", quantity);
        submit(&mut app, &mut store, &cli);
    }
    let leader = store.db.warbands[0].leader_id.clone();
    assert!(!leader.is_empty());
    assert_eq!(
        store
            .db
            .members
            .iter()
            .find(|m| m.id == leader)
            .unwrap()
            .name,
        "Brynn"
    );
    assert_eq!(store.db.gold(&wb), 575);
    assert_eq!(store.db.members_of(&wb).count(), 4);
    assert_eq!(engine::summary(&store.db, &wb)["roster_count"], 3);
    assert!(store.db.advances.iter().any(|a| a.ability_id == ability));
    assert_eq!(store.db.owned(&wb, &store.db.items[0].id), 3);
    assert_eq!(store.db.owned(&wb, &staff), 2);
    assert_eq!(store.db.assigned(&wb, &staff), 2);
    assert_eq!(store.db.typ(&hero).leader, Some(true));
    assert!(
        store
            .db
            .caps
            .iter()
            .any(|c| c.type_id == hero && c.characteristic == "WS" && c.maximum == 6)
    );
    app.key(key(KeyCode::F(5)), &mut store, &cli);
    field(&mut app, "name", "First outing");
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::F(5)), &mut store, &cli);
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.tab, 3);
    assert_eq!(app.participants(&store.db).len(), 4);
    app.key(key(KeyCode::Char('v')), &mut store, &cli);
    assert!(matches!(&app.overlay,Overlay::Detail{text,..}if text.contains("Leads the militia")));
    let saved = store.db.clone();
    drop(store);
    let store = Store::open(dir.path()).unwrap();
    assert_eq!(store.db, saved);
    let mut app = App::new(&store, &cli);
    app.switch_tab(RULES_TAB);
    assert_eq!(app.rule_profile(&store.db).unwrap().id, profile);
}

#[test]
fn rules_navigation_editing_copying_and_cancel_keep_profile_scope_and_history() {
    let (_dir, mut store, cli, mut app) = setup();
    let original = store.db.clone();
    let old_profile = store.db.warbands[0].profile_id.clone();
    app.key(key(KeyCode::F(7)), &mut store, &cli);
    assert_eq!(app.tab, RULES_TAB);
    app.key(key(KeyCode::Down), &mut store, &cli);
    assert_eq!(app.rules.section, 1);
    app.key(key(KeyCode::Tab), &mut store, &cli);
    assert!(app.rules.records_focus);
    app.key(key(KeyCode::Down), &mut store, &cli);
    assert_eq!(app.selected[RULES_TAB], 1);
    app.key(key(KeyCode::Char('e')), &mut store, &cli);
    assert!(matches!(&app.overlay,Overlay::Form(f)if f.value("name")=="Champion"));
    field(&mut app, "name", "Cancelled change");
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert_eq!(store.db, original);
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    field(&mut app, "name", "Independent copy");
    submit(&mut app, &mut store, &cli);
    let copied = app.rule_profile(&store.db).unwrap().id.clone();
    assert_ne!(copied, old_profile);
    let after_copy = store.db.clone();
    app.key(key(KeyCode::Char('h')), &mut store, &cli);
    assert!(matches!(app.overlay, Overlay::None));
    assert_eq!(store.db, after_copy);
    assert_eq!(store.db.warbands[0].profile_id, old_profile);
    app.rule_section(1);
    app.key(key(KeyCode::Char('e')), &mut store, &cli);
    field(&mut app, "cost", "77");
    app.key(key(KeyCode::PageDown), &mut store, &cli);
    assert!(matches!(&app.overlay,Overlay::Form(f)if f.focus==6));
    let s = screen(&mut app, &store, 80, 24);
    assert!(s.contains("Field 7/"), "{s}");
    submit(&mut app, &mut store, &cli);
    assert_eq!(
        store
            .db
            .types
            .iter()
            .find(|t| t.profile_id == copied && t.name == "Captain")
            .unwrap()
            .cost,
        77
    );
    assert_eq!(
        store
            .db
            .types
            .iter()
            .find(|t| t.profile_id == old_profile && t.name == "Captain")
            .unwrap()
            .cost,
        60
    );
    app.key(key(KeyCode::Left), &mut store, &cli);
    assert_eq!(app.tab, 5);
    app.key(key(KeyCode::Right), &mut store, &cli);
    assert_eq!(app.tab, RULES_TAB);
    let s = screen(&mut app, &store, 80, 24);
    assert!(s.contains("Independent copy"), "{s}");
    let hit = app
        .hits
        .iter()
        .find(|(_, hit)| matches!(hit, Hit::RuleSection(3)))
        .unwrap()
        .1;
    app.click(hit, &mut store, &cli);
    assert_eq!(app.rules.section, 3);
    app.key(key(KeyCode::Char('p')), &mut store, &cli);
    field(&mut app, "profile", &old_profile);
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.rule_profile(&store.db).unwrap().id, old_profile);
}
