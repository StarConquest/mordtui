use super::*;
use ratatui::{Terminal, backend::TestBackend};
use tempfile::TempDir;
mod external_agent_tests;
mod recruitment_tests;
fn setup() -> (TempDir, Store, Cli, App) {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let db: Db = serde_json::from_str(include_str!("../../examples/burgomeisters.json")).unwrap();
    store
        .commit(db, "Test opening state", "2026-10-19", "", "manual")
        .unwrap();
    let cli = Cli::try_parse_from(["mordheim"]).unwrap();
    let app = App::new(&store, &cli);
    (dir, store, cli, app)
}
fn field(app: &mut App, key: &str, value: &str) {
    if let Overlay::Form(f) = &mut app.overlay {
        f.set(key, value);
    } else {
        panic!("expected form");
    }
}
fn focus_field(app: &mut App, store: &mut Store, cli: &Cli, name: &str) {
    let Overlay::Form(f) = &app.overlay else {
        panic!("expected form")
    };
    let index = f.fields.iter().position(|f| f.key == name).unwrap();
    app.click(Hit::Field(index), store, cli);
}
fn submit(app: &mut App, store: &mut Store, cli: &Cli) {
    app.review(store);
    if let Overlay::Form(f) = &app.overlay {
        assert!(f.reviewing, "{}", f.error);
    }
    app.apply(store, cli);
    if let Overlay::Form(f) = &app.overlay {
        panic!("{}", f.error);
    }
}
fn screen(app: &mut App, store: &Store, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| views::draw(f, app, store)).unwrap();
    let b = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| b[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
#[test]
fn spreadsheet_roster_and_equipment_are_structured() {
    let (_dir, store, _cli, mut app) = setup();
    let s = screen(&mut app, &store, 140, 40);
    for label in [
        "Warband quick look",
        "Hans",
        "M",
        "WS",
        "BS",
        "XP",
        "Plus-ups",
        "Equipment / model",
        "Crossbow",
    ] {
        assert!(s.contains(label), "missing {label}: {s}");
    }
    app.selected[1] = app.units(&store.db).len() - 1;
    let last = screen(&mut app, &store, 100, 30);
    assert!(last.contains("Swordsmen"));
    app.tab = 2;
    let s = screen(&mut app, &store, 100, 30);
    for label in ["Owned", "Issued", "Stash", "Long Bow"] {
        assert!(s.contains(label));
    }
}
#[test]
fn purchase_equip_and_xp_forms_commit_through_engine() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Gold, &store, &cli);
    field(&mut app, "amount", "100");
    field(&mut app, "reason", "Test rewards");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Buy, &store, &cli);
    field(&mut app, "item", "burgomeisters-sword");
    field(&mut app, "quantity", "2");
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.gold("example-warband"), 82);
    app.open(Action::Equip, &store, &cli);
    field(&mut app, "unit", "arrows-one-unit");
    field(&mut app, "item", "burgomeisters-sword");
    submit(&mut app, &mut store, &cli);
    assert_eq!(
        store.db.assigned("example-warband", "burgomeisters-sword"),
        3
    );
    app.open(Action::Xp, &store, &cli);
    field(&mut app, "unit", "youngblood-two-unit");
    field(&mut app, "amount", "4");
    submit(&mut app, &mut store, &cli);
    assert_eq!(
        store.db.pending(store.db.unit("youngblood-two-unit")),
        vec![4, 6]
    );
}
#[test]
fn errors_preserve_form_and_cancel_is_non_mutating() {
    let (_dir, mut store, cli, mut app) = setup();
    let before = store.db.clone();
    app.open(Action::Buy, &store, &cli);
    field(&mut app, "item", "burgomeisters-sword");
    field(&mut app, "quantity", "20");
    app.review(&store);
    app.apply(&mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert!(!f.error.is_empty());
    assert_eq!(f.value("quantity"), "20");
    assert!(!f.reviewing);
    assert_eq!(before, store.db);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert!(matches!(app.overlay, Overlay::None));
    assert_eq!(before, store.db);
}
#[test]
fn picker_uses_ids_and_graphical_mouse_buttons_work() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Equip, &store, &cli);
    app.open_picker();
    let p = app.picker.as_mut().unwrap();
    p.query = "1st arrows".into();
    p.index = 0;
    app.key(key(KeyCode::Enter), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("unit"), "arrows-one-unit");
    app.key(key(KeyCode::Esc), &mut store, &cli);
    screen(&mut app, &store, 120, 36);
    let hit = app
        .hits
        .iter()
        .find(|(_, h)| matches!(h, Hit::Action(Action::Hire)))
        .unwrap()
        .1;
    app.click(hit, &mut store, &cli);
    assert!(matches!(
        app.overlay,
        Overlay::Form(Form {
            action: Action::Hire,
            ..
        })
    ));
}
#[test]
fn battle_wound_shortcuts_and_record_form_keep_permanent_stats() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Graphical battle");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Start, &store, &cli);
    submit(&mut app, &mut store, &cli);
    app.tab = 3;
    app.quick(&mut store, &cli, "-");
    let p = &store.db.participants[0];
    assert_eq!(p.wounds, 1);
    assert_eq!(store.db.stat(&store.db.members[0], "W", None), "2");
    app.quick(&mut store, &cli, "kill");
    submit(&mut app, &mut store, &cli);
    app.quick(&mut store, &cli, "ooa");
    assert_eq!(store.db.participants[0].kills, 1);
    assert_eq!(store.db.members[0].status, "active");
    assert_eq!(store.db.participants[0].condition, "out-of-action");
    app.open(Action::Record, &store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("wounds"), "1");
    assert_eq!(f.value("kills"), "1");
    assert_eq!(f.value("condition"), "out-of-action");
    let s = screen(&mut app, &store, 110, 32);
    assert!(s.contains("Wounds remaining"));
}
#[test]
fn form_keyboard_entry_and_review_then_apply() {
    let (_dir, mut store, cli, mut app) = setup();
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    if let Overlay::Form(f) = &mut app.overlay {
        f.focus = f.fields.iter().position(|f| f.key == "text").unwrap();
        f.cursor = 0;
    }
    for c in "Remember the bridge".chars() {
        app.key(key(KeyCode::Char(c)), &mut store, &cli);
    }
    app.key(key(KeyCode::F(10)), &mut store, &cli);
    let s = screen(&mut app, &store, 110, 34);
    assert!(s.contains("Review"));
    assert!(s.contains("Remember the bridge"));
    let before = store.db.events.len();
    app.key(key(KeyCode::Enter), &mut store, &cli);
    assert_eq!(store.db.events.len(), before + 1);
    assert!(
        store
            .db
            .notes
            .iter()
            .any(|n| n.text == "Remember the bridge")
    );
}
#[test]
fn small_terminals_and_empty_database_have_useful_views() {
    let (_dir, store, _cli, mut app) = setup();
    assert!(screen(&mut app, &store, 60, 15).contains("80 columns"));
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let cli = Cli::try_parse_from(["mordheim"]).unwrap();
    let mut app = App::new(&store, &cli);
    let s = screen(&mut app, &store, 100, 30);
    assert!(s.contains("Create campaign"));
    assert!(s.contains("Import warband"));
}
#[test]
fn hire_and_advance_without_a_command_prompt() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Gold, &store, &cli);
    field(&mut app, "amount", "100");
    field(&mut app, "reason", "Campaign reward");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Hire, &store, &cli);
    field(&mut app, "kind", "burgomeisters-warrior");
    field(&mut app, "name", "Guards");
    field(&mut app, "quantity", "2");
    submit(&mut app, &mut store, &cli);
    let u = store
        .db
        .units
        .iter()
        .find(|u| u.name == "Guards")
        .unwrap()
        .id
        .clone();
    assert_eq!(
        store.db.members.iter().filter(|m| m.unit_id == u).count(),
        2
    );
    app.open(Action::Xp, &store, &cli);
    field(&mut app, "unit", &u);
    field(&mut app, "amount", "2");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Advance, &store, &cli);
    field(&mut app, "unit", &u);
    field(&mut app, "stat", "WS");
    field(&mut app, "reason", "Witnessed Henchman advance roll");
    submit(&mut app, &mut store, &cli);
    let m = store.db.members.iter().find(|m| m.unit_id == u).unwrap();
    assert_eq!(store.db.stat(m, "WS", None), "4");
    assert!(store.db.pending(store.db.unit(&u)).is_empty());
}
#[test]
fn complete_post_battle_entirely_with_forms() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Post battle forms");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Start, &store, &cli);
    submit(&mut app, &mut store, &cli);
    app.tab = 3;
    app.quick(&mut store, &cli, "ooa");
    app.open(Action::End, &store, &cli);
    submit(&mut app, &mut store, &cli);
    app.tab = 4;
    app.open(Action::Injury, &store, &cli);
    submit(&mut app, &mut store, &cli);
    let stage = |app: &mut App, store: &mut Store, name: &str, skip: bool| {
        app.open(Action::Step, store, &cli);
        field(app, "stage", name);
        if skip {
            field(app, "mode", "skip");
            field(app, "reason", "No action needed this stage");
        }
        submit(app, store, &cli);
    };
    stage(&mut app, &mut store, "injuries", false);
    app.open(Action::Outcome, &store, &cli);
    field(&mut app, "description", "Scenario experience checked");
    submit(&mut app, &mut store, &cli);
    stage(&mut app, &mut store, "experience", false);
    app.open(Action::Outcome, &store, &cli);
    field(&mut app, "description", "Two shards found");
    field(&mut app, "shards", "2");
    submit(&mut app, &mut store, &cli);
    stage(&mut app, &mut store, "exploration", false);
    app.open(Action::Wyrdstone, &store, &cli);
    field(&mut app, "shards", "2");
    field(&mut app, "gold", "75");
    field(&mut app, "reason", "Agreed campaign price");
    submit(&mut app, &mut store, &cli);
    stage(&mut app, &mut store, "wyrdstone", false);
    stage(&mut app, &mut store, "veterans", true);
    stage(&mut app, &mut store, "rare", true);
    stage(&mut app, &mut store, "dramatis", true);
    stage(&mut app, &mut store, "recruitment", false);
    stage(&mut app, &mut store, "equipment", false);
    stage(&mut app, &mut store, "rating", false);
    app.open(Action::Complete, &store, &cli);
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.battles.last().unwrap().status, "complete");
    assert_eq!(store.db.gold("example-warband"), 77);
}
#[test]
fn selected_warband_survives_mutation_and_picker_cannot_cross_it() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Warband, &store, &cli);
    field(&mut app, "name", "Other Watch");
    submit(&mut app, &mut store, &cli);
    let other = store.db.warbands.last().unwrap().id.clone();
    app.wb = Some(other.clone());
    app.open(Action::Hire, &store, &cli);
    field(&mut app, "kind", "burgomeisters-captain");
    field(&mut app, "name", "Hans");
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.wb, Some(other.clone()));
    app.open(Action::Xp, &store, &cli);
    if let Overlay::Form(f) = &app.overlay {
        let Kind::Choice(cs) = &f.fields[0].kind else {
            panic!()
        };
        assert_eq!(cs.len(), 1);
        assert_ne!(cs[0].id, "hans-unit");
    }
    field(&mut app, "amount", "1");
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.wb, Some(other));
    assert_eq!(store.db.unit("hans-unit").earned_xp, 0);
}
#[test]
fn long_review_and_form_scroll_on_minimum_terminal() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Injury, &store, &cli);
    if let Overlay::Form(f) = &mut app.overlay {
        f.focus = f.fields.len() - 1;
        f.cursor = 0;
    }
    let s = screen(&mut app, &store, 80, 24);
    assert!(s.contains("Reason / justification"));
    assert!(!s.contains("house-rule override"));
    app.open(Action::Note, &store, &cli);
    field(&mut app, "text", &"A long note ".repeat(25));
    app.review(&store);
    for _ in 0..15 {
        app.key(key(KeyCode::Down), &mut store, &cli);
    }
    let s = screen(&mut app, &store, 80, 24);
    assert!(s.contains("Apply"));
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert!(matches!(
        app.overlay,
        Overlay::Form(Form {
            reviewing: false,
            ..
        })
    ));
}
#[test]
fn context_changes_reload_member_counters_and_death_disposition() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Counters");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Start, &store, &cli);
    submit(&mut app, &mut store, &cli);
    app.tab = 3;
    app.quick(&mut store, &cli, "kill");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Record, &store, &cli);
    if let Overlay::Form(f) = &mut app.overlay {
        f.focus = 0;
    }
    app.open_picker();
    app.choose("youngblood-two".into(), &store);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("kills"), "0");
    assert_eq!(f.value("wounds"), "1");
    app.open(Action::Remove, &store, &cli);
    if let Overlay::Form(f) = &mut app.overlay {
        f.focus = f.fields.iter().position(|f| f.key == "status").unwrap();
        f.set("status", "dead");
        changed(f, &store);
        assert_eq!(f.value("equipment"), "lose");
    }
}
#[test]
fn post_battle_stage_form_defaults_to_next_actionable_stage() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Next stage");
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.tab, 3);
    app.open(Action::Start, &store, &cli);
    submit(&mut app, &mut store, &cli);
    app.open(Action::End, &store, &cli);
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.tab, 4);
    app.open(Action::Step, &store, &cli);
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.selected[4], 1);
    app.open(Action::Step, &store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("stage"), "experience");
}
#[test]
fn duplicate_names_are_chosen_by_identity_and_snapshots_compare_graphically() {
    let (_dir, mut store, cli, mut app) = setup();
    let mut db = store.db.clone();
    db.members
        .iter_mut()
        .find(|m| m.id == "champion-one")
        .unwrap()
        .name = "Hans".into();
    store
        .commit(db, "Duplicate display name", "2026-10-19", "", "manual")
        .unwrap();
    app.open(Action::Remove, &store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    let Kind::Choice(cs) = &f.fields[0].kind else {
        panic!()
    };
    let hans: Vec<_> = cs.iter().filter(|c| c.label.starts_with("Hans")).collect();
    assert_eq!(hans.len(), 2);
    assert_ne!(hans[0].label, hans[1].label);
    assert_ne!(hans[0].id, hans[1].id);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    app.open(Action::Snapshot, &store, &cli);
    field(&mut app, "name", "Before spoils");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Gold, &store, &cli);
    field(&mut app, "amount", "10");
    field(&mut app, "reason", "Spoils");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Compare, &store, &cli);
    submit(&mut app, &mut store, &cli);
    let s = screen(&mut app, &store, 110, 34);
    assert!(s.contains("Gold: 2 → 12"));
    assert!(s.contains("Checkpoint comparison"));
}
#[test]
fn battle_profile_shows_resolved_random_values() {
    let (_dir, mut store, cli, mut app) = setup();
    let mut db = store.db.clone();
    db.types
        .iter_mut()
        .find(|t| t.id == "burgomeisters-captain")
        .unwrap()
        .s = "D6".into();
    store
        .commit(
            db,
            "Random strength house profile",
            "2026-10-19",
            "",
            "manual",
        )
        .unwrap();
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Random profile");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Stat, &store, &cli);
    field(&mut app, "member", "hans");
    field(&mut app, "stat", "S");
    field(&mut app, "value", "5");
    field(&mut app, "battle", "true");
    field(&mut app, "reason", "Physical D6 result");
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::Char('V')), &mut store, &cli);
    let s = screen(&mut app, &store, 110, 36);
    assert!(s.contains("Current battle profile"));
    assert!(s.contains("S 5"));
    assert!(s.contains("Base:"));
    assert!(s.contains("S D6"));
}

fn after_action() -> (TempDir, Store, Cli, App) {
    let (dir, mut store, cli, mut app) = setup();
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Quality control");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Start, &store, &cli);
    submit(&mut app, &mut store, &cli);
    app.open(Action::End, &store, &cli);
    submit(&mut app, &mut store, &cli);
    (dir, store, cli, app)
}
fn stage_fixture(store: &mut Store, app: &mut App, name: &str) {
    let mut db = store.db.clone();
    let bid = db.battles.last().unwrap().id.clone();
    let pos = db
        .steps
        .iter()
        .position(|s| s.battle_id == bid && s.name == name)
        .unwrap();
    let position = db.steps[pos].position;
    for s in db.steps.iter_mut().filter(|s| s.battle_id == bid) {
        s.status = if s.position < position {
            "completed"
        } else {
            "pending"
        }
        .into();
    }
    store
        .commit(db, "QC stage fixture", "2026-10-20", "", "manual")
        .unwrap();
    app.tab = 4;
    app.selected[4] = position as usize;
    app.post_selected = 0;
    app.post_focus = PostFocus::Targets;
}
#[test]
fn every_visible_unit_keeps_stat_labels_when_scrolled() {
    let (_dir, store, _cli, mut app) = setup();
    for selected in [0, 4, 8] {
        app.selected[1] = selected;
        let s = screen(&mut app, &store, 140, 40);
        let lines: Vec<_> = s.lines().collect();
        let expected = &app.units(&store.db)[selected].name;
        let row = lines
            .iter()
            .position(|line| {
                line.contains(expected) && !line.contains("expands") && !line.contains("PLUS-UP(S)")
            })
            .unwrap();
        assert!(row > 0, "{s}");
        for stat in ["M", "WS", "BS", "S", "T", "W", "I", "A", "Ld"] {
            assert!(
                lines[row - 1].split_whitespace().any(|label| label == stat),
                "{stat} label not directly above {expected}:\n{s}"
            );
        }
    }
}
#[test]
fn pending_plus_up_is_visible_and_records_consequence_without_dice_or_reason() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Xp, &store, &cli);
    field(&mut app, "unit", "youngblood-two-unit");
    field(&mut app, "amount", "2");
    submit(&mut app, &mut store, &cli);
    app.selected[1] = 4;
    let s = screen(&mut app, &store, 120, 36);
    assert!(s.contains("PLUS-UP READY"));
    assert!(s.contains("READY 1"));
    app.open(Action::Advance, &store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert!(
        f.fields
            .iter()
            .all(|f| f.key != "dice" && f.key != "override")
    );
    assert_eq!(f.value("unit"), "youngblood-two-unit");
    submit(&mut app, &mut store, &cli);
    assert!(
        store
            .db
            .pending(store.db.unit("youngblood-two-unit"))
            .is_empty()
    );
    assert!(store.db.dice.is_empty());
    assert!(store.db.advances.last().unwrap().dice_id.is_empty());
    let s = screen(&mut app, &store, 120, 36);
    assert!(!s.contains("PLUS-UP READY"));
}
#[test]
fn manual_forms_have_no_generic_override_but_fixed_profile_values_can_be_replaced() {
    let (_dir, mut store, cli, mut app) = setup();
    for action in [
        Action::Note,
        Action::Gold,
        Action::Wyrdstone,
        Action::Xp,
        Action::Outcome,
        Action::Veterans,
        Action::Search,
    ] {
        app.open(action, &store, &cli);
        let Overlay::Form(f) = &app.overlay else {
            panic!()
        };
        assert!(
            f.fields
                .iter()
                .all(|f| f.key != "override" && f.key != "dice"),
            "{action:?}"
        );
    }
    app.open(Action::Stat, &store, &cli);
    field(&mut app, "member", "hans");
    field(&mut app, "stat", "S");
    field(&mut app, "value", "5");
    app.review(&store);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert!(
        f.fields
            .iter()
            .any(|f| f.label == "Replace fixed profile stat")
    );
    app.key(key(KeyCode::Esc), &mut store, &cli);
    field(&mut app, "override", "true");
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.stat(&store.db.members[0], "S", None), "5");
}
#[test]
fn stage_hotkeys_are_unique_and_other_stages_cannot_fire() {
    let (_dir, mut store, cli, mut app) = after_action();
    for name in STEPS {
        stage_fixture(&mut store, &mut app, name);
        let bindings = app.post_bindings(&store.db);
        let keys: std::collections::HashSet<_> = bindings.iter().map(|b| b.key).collect();
        assert_eq!(keys.len(), bindings.len(), "duplicate keys in {name}");
        let actions = app.filtered_actions(&store.db, "");
        assert_eq!(
            actions,
            bindings.iter().map(|b| b.action).collect::<Vec<_>>()
        );
        for key_value in [
            'h', 'p', 'e', 'x', 'u', 'i', 'r', 't', 'v', 's', 'g', 'f', 'o', 'd',
        ] {
            if !keys.contains(&key_value) {
                let before = store.db.events.len();
                app.key(key(KeyCode::Char(key_value)), &mut store, &cli);
                assert!(
                    matches!(app.overlay, Overlay::None),
                    "inactive hotkey {key_value} fired in {name}"
                );
                assert_eq!(before, store.db.events.len());
            }
        }
    }
    // Pending stages expose their own actions even before earlier stages finish.
    stage_fixture(&mut store, &mut app, "injuries");
    app.selected[4] = 1;
    assert!(app.post_bindings(&store.db).iter().any(|b| b.key == 'k'));
    app.key(key(KeyCode::Char('x')), &mut store, &cli);
    assert!(matches!(&app.overlay, Overlay::Form(f) if f.action == Action::Xp));
}
#[test]
fn after_action_selects_targets_and_applies_xp_and_plus_up_without_tab_switching() {
    let (_dir, mut store, cli, mut app) = after_action();
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    assert_eq!(app.tab, 4);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "experience");
    app.post_selected = app
        .post_targets(&store.db)
        .iter()
        .position(|t| t.unit.as_deref() == Some("youngblood-two-unit"))
        .unwrap();
    app.key(key(KeyCode::Char('x')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("unit"), "youngblood-two-unit");
    field(&mut app, "amount", "2");
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.tab, 4);
    let s = screen(&mut app, &store, 120, 36);
    assert!(s.contains("PLUS-UP READY"));
    app.key(key(KeyCode::Char('u')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("unit"), "youngblood-two-unit");
    assert!(
        f.fields
            .iter()
            .all(|f| f.key != "dice" && f.key != "initial")
    );
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    assert!(matches!(app.overlay, Overlay::None));
    assert_eq!(app.post_stage(&store.db).unwrap().name, "exploration");
    assert_eq!(app.tab, 4);
}
#[test]
fn physical_veteran_result_and_arbitrary_shard_sale_need_no_dice_or_override() {
    let (_dir, mut store, cli, mut app) = after_action();
    stage_fixture(&mut store, &mut app, "veterans");
    app.key(key(KeyCode::Char('v')), &mut store, &cli);
    field(&mut app, "experience", "9");
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.battles.last().unwrap().veteran_budget, 9);
    assert!(store.db.dice.is_empty());
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "rare");
    stage_fixture(&mut store, &mut app, "exploration");
    app.key(key(KeyCode::Char('o')), &mut store, &cli);
    field(&mut app, "description", "Physical exploration outcome");
    field(&mut app, "shards", "3");
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    app.key(key(KeyCode::Char('t')), &mut store, &cli);
    field(&mut app, "shards", "2");
    field(&mut app, "gold", "87");
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.gold("example-warband"), 89);
    assert_eq!(
        store.db.owned("example-warband", "burgomeisters-wyrdstone"),
        1
    );
}
#[test]
fn post_battle_mouse_targets_filter_and_tab_preserve_stage_context() {
    let (_dir, mut store, cli, mut app) = after_action();
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    let s = screen(&mut app, &store, 100, 30);
    assert!(s.contains("Targets · Tab to focus"));
    let (rect, hit) = app
        .hits
        .iter()
        .find(|(_, h)| matches!(h, Hit::PostTarget(2)))
        .copied()
        .unwrap();
    assert!(rect.height > 0);
    app.click(hit, &mut store, &cli);
    let target = app.post_targets(&store.db)[2].unit.clone();
    app.key(key(KeyCode::Char('x')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(Some(f.value("unit").to_string()), target);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    app.key(key(KeyCode::Char('/')), &mut store, &cli);
    for c in "Youngblood".chars() {
        app.key(key(KeyCode::Char(c)), &mut store, &cli);
    }
    app.key(key(KeyCode::Enter), &mut store, &cli);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "experience");
    assert!(
        app.post_targets(&store.db)
            .iter()
            .all(|t| t.cells[0].contains("Youngblood"))
    );
    app.switch_tab(1);
    app.switch_tab(4);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "experience");
    app.key(key(KeyCode::Tab), &mut store, &cli);
    assert_eq!(app.post_focus, PostFocus::Stages);
    assert_eq!(app.tab, 4);
}
#[test]
fn post_battle_retains_global_navigation_and_help_in_both_panes() {
    let (_dir, mut store, cli, mut app) = after_action();
    for stage in STEPS {
        stage_fixture(&mut store, &mut app, stage);
        app.post_selected = app.post_targets(&store.db).len().saturating_sub(1);
        let target = app.post_selected;
        let selected_stage = app.selected[4];
        let events = store.db.events.len();
        for focus in [PostFocus::Stages, PostFocus::Targets] {
            app.post_focus = focus;
            for (direction, tab, back) in [
                (KeyCode::Left, 3, KeyCode::Right),
                (KeyCode::Right, 5, KeyCode::Left),
            ] {
                assert!(app.key(key(direction), &mut store, &cli));
                assert_eq!(app.tab, tab, "global {direction:?} failed in {stage}");
                assert!(app.key(key(back), &mut store, &cli));
                assert_eq!(app.tab, 4);
                assert_eq!(app.selected[4], selected_stage);
                assert_eq!(app.post_selected, target);
                assert_eq!(app.post_focus, focus);
            }
            app.key(key(KeyCode::Char('2')), &mut store, &cli);
            assert_eq!(app.tab, 1);
            app.key(key(KeyCode::Char('5')), &mut store, &cli);
            assert_eq!(app.tab, 4);
            app.key(key(KeyCode::Char('?')), &mut store, &cli);
            assert!(matches!(app.overlay, Overlay::Detail { .. }));
            app.key(key(KeyCode::Esc), &mut store, &cli);
            app.key(key(KeyCode::F(2)), &mut store, &cli);
            assert!(matches!(&app.overlay, Overlay::Form(f) if f.action == Action::Switch));
            app.key(key(KeyCode::Esc), &mut store, &cli);
        }
        assert_eq!(store.db.events.len(), events);
    }
    app.key(key(KeyCode::Tab), &mut store, &cli);
    assert_eq!(app.tab, 4);
    assert_eq!(app.post_focus, PostFocus::Stages);
    app.key(key(KeyCode::BackTab), &mut store, &cli);
    assert_eq!(app.tab, 4);
    assert_eq!(app.post_focus, PostFocus::Targets);
}
#[test]
fn finishing_from_the_stage_menu_closes_it_and_final_stage_saves_checkpoint() {
    let (_dir, mut store, cli, mut app) = after_action();
    app.key(key(KeyCode::Char('a')), &mut store, &cli);
    for c in "finish".chars() {
        app.key(key(KeyCode::Char(c)), &mut store, &cli);
    }
    app.key(key(KeyCode::Enter), &mut store, &cli);
    assert!(matches!(app.overlay, Overlay::None));
    assert_eq!(app.post_stage(&store.db).unwrap().name, "experience");
    stage_fixture(&mut store, &mut app, "rating");
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    assert_eq!(app.tab, 0);
    let b = store.db.battles.last().unwrap();
    assert_eq!(b.status, "complete");
    let snapshot = store
        .db
        .snapshots
        .iter()
        .find(|s| s.id == b.post_snapshot)
        .unwrap();
    assert_eq!(
        store
            .historical(&snapshot.generation)
            .unwrap()
            .battles
            .last()
            .unwrap()
            .status,
        "complete"
    );
}
#[test]
fn roster_mouse_rows_and_stage_controls_remain_correct_at_80_columns() {
    let (_dir, mut store, cli, mut app) = setup();
    app.selected[1] = 8;
    screen(&mut app, &store, 80, 24);
    let hit = app
        .hits
        .iter()
        .find(|(_, h)| matches!(h, Hit::Row(7)))
        .unwrap()
        .1;
    app.click(hit, &mut store, &cli);
    assert_eq!(app.selected[1], 7);
    app.key(key(KeyCode::Char('x')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("unit"), "arrows-three-unit");
    let (_dir, mut store, cli, mut app) = after_action();
    let s = screen(&mut app, &store, 80, 24);
    assert!(s.contains("C Finish stage"), "{s}");
    assert!(s.contains("K Skip stage"), "{s}");
    app.key(key(KeyCode::End), &mut store, &cli);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "rating");
    assert_eq!(app.post_focus, PostFocus::Stages);
    app.key(key(KeyCode::Home), &mut store, &cli);
    assert_eq!(app.selected[4], 0);
    app.key(key(KeyCode::Tab), &mut store, &cli);
    assert_eq!(app.post_focus, PostFocus::Targets);
    app.key(key(KeyCode::Tab), &mut store, &cli);
    app.key(key(KeyCode::End), &mut store, &cli);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "rating");
}
#[test]
fn first_entry_replaces_numeric_defaults_once_and_keeps_edits_on_return() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Gold, &store, &cli);
    assert!(screen(&mut app, &store, 100, 30).contains("Type to replace"));
    for c in "52".chars() {
        app.key(key(KeyCode::Char(c)), &mut store, &cli);
    }
    // Returning to an edited field continues the user's entry.
    app.key(key(KeyCode::Tab), &mut store, &cli);
    app.key(key(KeyCode::BackTab), &mut store, &cli);
    app.key(key(KeyCode::Char('3')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("amount"), "523");
    app.key(key(KeyCode::F(10)), &mut store, &cli);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    app.key(key(KeyCode::Backspace), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("amount"), "52");
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.gold("example-warband"), 54);

    app.open(Action::Xp, &store, &cli);
    focus_field(&mut app, &mut store, &cli, "amount");
    app.key(key(KeyCode::Char('2')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("amount"), "2"); // Default was 1.
}
#[test]
fn text_defaults_replace_but_cursor_movement_allows_deliberate_edits() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Injury, &store, &cli);
    focus_field(&mut app, &mut store, &cli, "outcome");
    for c in "Leg wound".chars() {
        app.key(key(KeyCode::Char(c)), &mut store, &cli);
    }
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("outcome"), "Leg wound");
    focus_field(&mut app, &mut store, &cli, "date");
    for c in "2026-11-01".chars() {
        app.key(key(KeyCode::Char(c)), &mut store, &cli);
    }
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("date"), "2026-11-01");

    app.open(Action::Injury, &store, &cli);
    focus_field(&mut app, &mut store, &cli, "outcome");
    app.key(key(KeyCode::End), &mut store, &cli);
    app.key(key(KeyCode::Char('!')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("outcome"), "recovery!");

    app.open(Action::Gold, &store, &cli);
    app.key(key(KeyCode::Delete), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.value("amount"), "");
    for c in "-2".chars() {
        app.key(key(KeyCode::Char(c)), &mut store, &cli);
    }
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.gold("example-warband"), 0);
}
#[test]
fn post_battle_arrows_browse_every_stage_and_recover_from_empty_targets() {
    let (_dir, mut store, cli, mut app) = after_action();
    let before = store.db.clone();
    assert_eq!(app.post_focus, PostFocus::Stages);
    for (i, stage) in STEPS.iter().enumerate() {
        assert_eq!(app.post_stage(&store.db).unwrap().name, *stage);
        assert!(screen(&mut app, &store, 80, 24).contains(stage));
        if i + 1 < STEPS.len() {
            app.key(key(KeyCode::Down), &mut store, &cli);
        }
    }
    for stage in STEPS.iter().rev().skip(1) {
        app.key(key(KeyCode::Up), &mut store, &cli);
        assert_eq!(app.post_stage(&store.db).unwrap().name, *stage);
    }
    assert_eq!(store.db, before); // Browsing never completes/skips a stage.
    app.key(key(KeyCode::Tab), &mut store, &cli);
    assert_eq!(app.post_focus, PostFocus::Targets);
    assert!(app.post_targets(&store.db).is_empty());
    app.key(key(KeyCode::Down), &mut store, &cli);
    assert_eq!(app.post_focus, PostFocus::Stages);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "experience");
    app.key(key(KeyCode::Tab), &mut store, &cli);
    app.key(key(KeyCode::Down), &mut store, &cli);
    assert_eq!(app.post_selected, 1);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "experience");
    app.key(key(KeyCode::Tab), &mut store, &cli);
    app.key(key(KeyCode::Up), &mut store, &cli);
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    assert_eq!(app.post_focus, PostFocus::Stages);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "experience");
    app.key(key(KeyCode::Down), &mut store, &cli);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "exploration");
}
#[test]
fn zero_shard_sale_is_recorded_once_without_changing_gold_or_inventory() {
    let (dir, mut store, cli, mut app) = after_action();
    stage_fixture(&mut store, &mut app, "wyrdstone");
    let holdings = store.db.holdings.clone();
    let gold = store.db.gold("example-warband");
    app.key(key(KeyCode::Char('t')), &mut store, &cli);
    submit(&mut app, &mut store, &cli); // Defaults record zero shards for zero gold.
    assert_eq!(store.db.holdings, holdings);
    assert_eq!(store.db.gold("example-warband"), gold);
    let entry = store.db.ledger.last().unwrap();
    assert_eq!(entry.category, "wyrdstone-sale");
    assert_eq!((entry.quantity, entry.gold), (0, 0));
    let before = store.db.clone();
    app.key(key(KeyCode::Char('t')), &mut store, &cli);
    app.review(&store);
    app.apply(&mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!("Duplicate zero sale must be rejected")
    };
    assert!(f.error.contains("already recorded"));
    assert_eq!(store.db, before);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    assert_eq!(app.post_stage(&store.db).unwrap().name, "veterans");
    let saved = store.db.clone();
    drop(store);
    assert_eq!(Store::open(dir.path()).unwrap().db, saved);
}
#[test]
fn after_action_catalogs_only_offer_entries_for_the_active_stage() {
    let (_dir, mut store, cli, mut app) = after_action();
    for (stage, rare) in [("rare", true), ("recruitment", false)] {
        stage_fixture(&mut store, &mut app, stage);
        app.key(key(KeyCode::Char('p')), &mut store, &cli);
        let Overlay::Form(f) = &app.overlay else {
            panic!("Purchase should open in {stage}")
        };
        let Kind::Choice(items) = &f.fields.iter().find(|f| f.key == "item").unwrap().kind else {
            panic!()
        };
        assert!(!items.is_empty());
        for choice in items.iter().filter(|c| c.id != "__new_equipment__") {
            let item = store.db.items.iter().find(|i| i.id == choice.id).unwrap();
            assert_eq!(item.rarity > 0, rare);
        }
        app.key(key(KeyCode::Esc), &mut store, &cli);
    }
    let mut character = store.db.types[0].clone();
    character.id = "qc-dramatis".into();
    character.name = "Guest character".into();
    character.role = "dramatis".into();
    store.db.types.push(character);
    stage_fixture(&mut store, &mut app, "dramatis");
    app.key(key(KeyCode::Char('h')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    let Kind::Choice(types) = &f.fields.iter().find(|f| f.key == "kind").unwrap().kind else {
        panic!()
    };
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].id, "qc-dramatis");
    assert_eq!(f.value("group"), "");
}
#[test]
fn campaign_phase_is_visible_on_every_tab_through_the_battle_lifecycle() {
    let (_dir, mut store, cli, mut app) = setup();
    for (action, phase) in [
        (None, "[PRE-BATTLE]"),
        (Some(Action::Prepare), "[PRE-BATTLE · READY]"),
        (Some(Action::Start), "[BATTLE]"),
        (Some(Action::End), "[POST-BATTLE]"),
    ] {
        if let Some(action) = action {
            app.open(action, &store, &cli);
            if action == Action::Prepare {
                field(&mut app, "name", "Phase check");
            }
            submit(&mut app, &mut store, &cli);
        }
        for tab in 0..TABS.len() {
            app.switch_tab(tab);
            let s = screen(&mut app, &store, 80, 24);
            assert!(
                s.lines().next().unwrap().contains(phase),
                "{phase} absent from tab {tab}: {s}"
            );
        }
    }
    stage_fixture(&mut store, &mut app, "rating");
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    assert!(screen(&mut app, &store, 80, 24).starts_with(" [PRE-BATTLE]"));
}
#[test]
fn reasons_follow_form_values_and_preserve_a_typed_explanation_in_the_journal() {
    let (_dir, mut store, cli, mut app) = setup();
    for action in Action::ALL
        .into_iter()
        .filter(|a| !matches!(a, Action::Injuries | Action::AiRequest | Action::RuleSelect))
    {
        let f = Form::new(action, &store, app.wb.as_deref(), &Selection::default());
        assert!(
            !f.value("reason").is_empty(),
            "missing default for {action:?}"
        );
    }
    app.open(Action::Gold, &store, &cli);
    field(&mut app, "amount", "100");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Buy, &store, &cli);
    field(&mut app, "item", "burgomeisters-sword");
    field(&mut app, "quantity", "2");
    app.review(&store);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert!(f.value("reason").contains("Sword"));
    assert!(f.value("reason").contains("Quantity: 2"));
    app.apply(&mut store, &cli);
    assert!(store.db.ledger.last().unwrap().notes.contains("Sword"));
    app.open(Action::Xp, &store, &cli);
    focus_field(&mut app, &mut store, &cli, "reason");
    for c in "Scenario bonus".chars() {
        app.key(key(KeyCode::Char(c)), &mut store, &cli);
    }
    field(&mut app, "amount", "3");
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.events.last().unwrap().reason, "Scenario bonus");
    app.key(key(KeyCode::Char('6')), &mut store, &cli);
    assert_eq!(app.journal, Journal::History);
    assert!(screen(&mut app, &store, 140, 32).contains("Scenario bonus"));
}
#[test]
fn pending_future_stages_offer_hotkeys_and_can_be_skipped_without_finishing_earlier_stages() {
    let (_dir, mut store, cli, mut app) = after_action();
    app.selected[4] = app
        .steps(&store.db)
        .iter()
        .position(|s| s.name == "veterans")
        .unwrap();
    let s = screen(&mut app, &store, 80, 24);
    for label in ["C Finish stage", "K Skip stage", "V Available veteran XP"] {
        assert!(s.contains(label), "{s}");
    }
    app.key(key(KeyCode::Char('k')), &mut store, &cli);
    submit(&mut app, &mut store, &cli);
    assert_eq!(
        app.steps(&store.db)
            .iter()
            .find(|s| s.name == "veterans")
            .unwrap()
            .status,
        "skipped"
    );
    assert_eq!(app.steps(&store.db)[0].status, "pending");
    assert!(store.db.events.last().unwrap().reason.contains("veterans"));
}
#[test]
fn campaign_dramatis_rule_hides_stage_and_survives_restart_and_new_battles() {
    let (dir, mut store, cli, mut app) = after_action();
    app.key(key(KeyCode::F(3)), &mut store, &cli);
    field(&mut app, "skip_dramatis", "true");
    submit(&mut app, &mut store, &cli);
    assert!(store.db.skips_dramatis("example-warband"));
    assert_eq!(app.steps(&store.db).len(), 9);
    assert!(!app.steps(&store.db).iter().any(|s| s.name == "dramatis"));
    assert_eq!(
        store
            .db
            .steps
            .iter()
            .find(|s| s.name == "dramatis")
            .unwrap()
            .status,
        "skipped"
    );
    for name in STEPS.into_iter().filter(|s| *s != "dramatis") {
        app.selected[4] = app
            .steps(&store.db)
            .iter()
            .position(|s| s.name == name)
            .unwrap();
        app.key(key(KeyCode::Char('k')), &mut store, &cli);
        submit(&mut app, &mut store, &cli);
    }
    app.selected[4] = 0; // Finish remains available while reviewing any resolved stage.
    app.key(key(KeyCode::Char('b')), &mut store, &cli);
    submit(&mut app, &mut store, &cli);
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    let mut app = App::new(&store, &cli);
    assert!(store.db.campaigns[0].skip_dramatis_personae);
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Next battle");
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.steps(&store.db).len(), 9);
    assert_eq!(store.db.steps.last().unwrap().name, "rating");
    app.key(key(KeyCode::F(3)), &mut store, &cli);
    field(&mut app, "skip_dramatis", "false");
    submit(&mut app, &mut store, &cli);
    assert_eq!(app.steps(&store.db).len(), 10);
    let dp = app
        .steps(&store.db)
        .into_iter()
        .find(|s| s.name == "dramatis")
        .unwrap();
    assert_eq!(dp.status, "pending");
    assert_eq!(store.db.battles[0].status, "complete");
    assert_eq!(
        store
            .db
            .steps
            .iter()
            .find(|s| s.name == "dramatis")
            .unwrap()
            .status,
        "skipped"
    );
}
#[test]
fn injury_report_keeps_unresolved_casualties_and_resolved_nonstat_outcomes() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Injury check");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Start, &store, &cli);
    submit(&mut app, &mut store, &cli);
    app.quick(&mut store, &cli, "ooa");
    app.open(Action::End, &store, &cli);
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::F(4)), &mut store, &cli);
    assert_eq!(app.journal, Journal::Injuries);
    assert!(screen(&mut app, &store, 100, 32).contains("UNRESOLVED"));
    app.key(key(KeyCode::Char('5')), &mut store, &cli);
    app.key(key(KeyCode::Char('i')), &mut store, &cli);
    field(&mut app, "outcome", "Frenzy");
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::F(4)), &mut store, &cli);
    let s = screen(&mut app, &store, 120, 32);
    assert!(s.contains("Hans"));
    assert!(s.contains("Frenzy"));
    assert!(!s.contains("UNRESOLVED"));
    let details = views::unit_details(&store.db, store.db.unit("hans-unit"), None);
    assert!(details.contains("Injuries / recovery outcomes"));
    assert!(details.contains("Frenzy"));
}
#[test]
fn rare_search_form_uses_eligible_heroes_items_and_physical_consequences() {
    let (_dir, mut store, cli, mut app) = after_action();
    app.selected[4] = app
        .steps(&store.db)
        .iter()
        .position(|s| s.name == "rare")
        .unwrap();
    app.key(key(KeyCode::Char('s')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert!(
        f.fields
            .iter()
            .all(|f| !matches!(f.key, "allowance" | "dice" | "kind"))
    );
    let member = f.value("member").to_string();
    field(&mut app, "outcome", "failed");
    submit(&mut app, &mut store, &cli);
    assert!(
        app.post_targets(&store.db)
            .iter()
            .any(|t| t.cells.join(" ").contains("failed"))
    );
    app.key(key(KeyCode::Char('s')), &mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    let Kind::Choice(heroes) = &f.fields.iter().find(|f| f.key == "member").unwrap().kind else {
        panic!()
    };
    assert!(heroes.iter().all(|c| c.id != member));
}
#[test]
fn battle_compact_profiles_notes_and_counter_corrections_preserve_other_data() {
    let (_dir, mut store, cli, mut app) = setup();
    app.key(key(KeyCode::F(5)), &mut store, &cli);
    field(&mut app, "name", "Profile check");
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::F(5)), &mut store, &cli);
    submit(&mut app, &mut store, &cli);
    let mut legacy = store.db.clone();
    legacy.participants[0].attribution = "Enemy marksman".into();
    store
        .commit(
            legacy,
            "Legacy battle observation",
            "2026-10-19",
            "",
            "manual",
        )
        .unwrap();
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    field(&mut app, "text", "Fear immune; veteran scar");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Record, &store, &cli);
    assert!(
        matches!(&app.overlay, Overlay::Form(f) if f.fields.iter().all(|f| f.key != "attribution"))
    );
    field(&mut app, "objectives", "2");
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::Char('-')), &mut store, &cli);
    app.key(key(KeyCode::Char('k')), &mut store, &cli);
    field(&mut app, "victim", "Enemy champion");
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::Char('k')), &mut store, &cli);
    field(&mut app, "victim", "Goblin scout");
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::Char('j')), &mut store, &cli);
    assert!(matches!(&app.overlay, Overlay::Form(f) if f.label("kill").contains("Goblin scout")));
    submit(&mut app, &mut store, &cli);
    let p = app.participants(&store.db)[0];
    assert_eq!((p.kills, p.objectives, p.wounds), (1, 2, 1));
    assert_eq!(p.attribution, "Enemy marksman");
    assert_eq!(views::kill_log(&store.db, p), "Enemy champion");
    for width in [80, 140] {
        let s = screen(&mut app, &store, width, 36);
        let header = s
            .lines()
            .find(|line| line.contains("Member") && line.contains("WS"))
            .unwrap();
        for stat in STATS {
            assert!(
                header.split_whitespace().any(|s| s == stat),
                "{stat}: {header}"
            );
        }
        assert!(s.contains("Rem"));
        assert!(s.contains("[N]"));
        assert!(!s.contains("Attribution"));
    }
    app.key(key(KeyCode::F(6)), &mut store, &cli);
    assert!(
        matches!(&app.overlay,Overlay::Detail{text,..} if text.contains("Fear immune; veteran scar"))
    );
    app.key(key(KeyCode::Esc), &mut store, &cli);
    app.key(key(KeyCode::Char('v')), &mut store, &cli);
    assert!(
        matches!(&app.overlay,Overlay::Detail{text,..} if text.contains("Enemy marksman") && text.contains("Fear immune; veteran scar"))
    );
    app.key(key(KeyCode::Esc), &mut store, &cli);
    app.key(key(KeyCode::Char('j')), &mut store, &cli);
    submit(&mut app, &mut store, &cli);
    let events = store.db.events.len();
    app.key(key(KeyCode::Char('j')), &mut store, &cli);
    assert_eq!(store.db.events.len(), events);
    assert_eq!(app.participants(&store.db)[0].kills, 0);
    app.key(key(KeyCode::F(5)), &mut store, &cli);
    assert!(matches!(&app.overlay,Overlay::Form(f) if f.action==Action::End));
}
#[test]
fn new_equipment_picker_can_cancel_or_save_and_resume_the_same_purchase() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Gold, &store, &cli);
    field(&mut app, "amount", "100");
    submit(&mut app, &mut store, &cli);
    app.key(key(KeyCode::Char('p')), &mut store, &cli);
    field(&mut app, "price", "19");
    app.open_picker();
    app.choose("__new_equipment__".into(), &store);
    assert!(matches!(&app.overlay,Overlay::Form(f) if f.action==Action::NewEquipment));
    let before = store.db.clone();
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert_eq!(store.db, before);
    assert!(
        matches!(&app.overlay,Overlay::Form(f) if f.action==Action::Buy && f.value("price")=="19")
    );
    app.open_picker();
    app.choose("__new_equipment__".into(), &store);
    field(&mut app, "name", "Two-handed weapon");
    field(&mut app, "cost", "20");
    field(&mut app, "description", "Two hands; tabletop modifiers");
    let old = store.generation.clone();
    app.review(&store);
    app.apply(&mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.action, Action::Buy);
    assert_eq!(f.value("price"), "19");
    let item = f.value("item").to_string();
    assert_eq!(
        f.label("item").split(" ·").next().unwrap(),
        "Two-handed weapon"
    );
    assert!(
        !store
            .historical(&old)
            .unwrap()
            .items
            .iter()
            .any(|i| i.name == "Two-handed weapon")
    );
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.gold("example-warband"), 83);
    assert_eq!(store.db.owned("example-warband", &item), 1);
    app.open(Action::Equip, &store, &cli);
    field(&mut app, "item", &item);
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db.assigned("example-warband", &item), 1);
}
#[test]
fn new_skill_from_advance_picker_returns_to_the_pending_unit_and_resolves_it() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Xp, &store, &cli);
    field(&mut app, "unit", "youngblood-two-unit");
    field(&mut app, "amount", "2");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Advance, &store, &cli);
    focus_field(&mut app, &mut store, &cli, "ability");
    app.choose("__new_ability__".into(), &store);
    field(&mut app, "name", "Streetwise");
    field(&mut app, "kind", "academic");
    field(&mut app, "description", "Player-entered skill rules");
    app.review(&store);
    app.apply(&mut store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(f.action, Action::Advance);
    assert_eq!(f.value("unit"), "youngblood-two-unit");
    assert_eq!(f.value("stat"), "");
    let skill = f.value("ability").to_string();
    submit(&mut app, &mut store, &cli);
    assert!(
        store
            .db
            .pending(store.db.unit("youngblood-two-unit"))
            .is_empty()
    );
    assert!(
        store
            .db
            .advances
            .iter()
            .any(|a| a.ability_id == skill && a.unit_id == "youngblood-two-unit")
    );
    assert!(
        views::unit_details(&store.db, store.db.unit("youngblood-two-unit"), None)
            .contains("Streetwise")
    );
}
fn offer_plan(app: &mut App, store: &Store) {
    let plan=crate::ai::plan::Plan::parse(r#"{"summary":"Requested changes","actions":[{"description":"Add ten gold","args":["gold","10"]},{"description":"Record a model note","args":["note","hans","AI reviewed note"]}]}"#).unwrap();
    crate::ai::exchange::submit(
        &store.root,
        &store.generation,
        Some("example-warband"),
        None,
        plan,
    )
    .unwrap();
    app.next_ai(store);
    app.tab = AI_TAB;
}
#[test]
fn ai_workbench_cannot_apply_before_review_and_explicit_confirmation() {
    let (_dir, mut store, cli, mut app) = setup();
    offer_plan(&mut app, &store);
    let before = store.db.clone();
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    assert_eq!(store.db, before);
    app.key(key(KeyCode::Char('r')), &mut store, &cli);
    assert_eq!(store.db, before);
    let s = screen(&mut app, &store, 80, 24);
    assert!(s.contains("AI assistant"));
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    assert!(matches!(app.overlay, Overlay::AiApproval { .. }));
    app.key(key(KeyCode::Enter), &mut store, &cli);
    assert_eq!(store.db, before);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert_eq!(store.db, before);
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    assert_eq!(store.db.gold("example-warband"), 12);
    assert!(store.db.notes.iter().any(|n| n.text == "AI reviewed note"));
    assert!(app.ai.plan.is_none());
    assert!(app.ai.reviewed.is_none());
    let approved = store.db.clone();
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    assert_eq!(store.db, approved);
}
#[test]
fn rejecting_or_staling_a_workbench_plan_leaves_the_campaign_unchanged() {
    let (_dir, mut store, cli, mut app) = setup();
    offer_plan(&mut app, &store);
    let before = store.db.clone();
    app.key(key(KeyCode::Char('r')), &mut store, &cli);
    app.key(key(KeyCode::Char('d')), &mut store, &cli);
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    assert_eq!(store.db, before);
    offer_plan(&mut app, &store);
    app.key(key(KeyCode::Char('r')), &mut store, &cli);
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    engine::execute(
        &mut store,
        &Cli::try_parse_from(["mordheim", "gold", "1"]).unwrap(),
    )
    .unwrap();
    let changed = store.db.clone();
    app.key(key(KeyCode::Char('y')), &mut store, &cli);
    assert_eq!(store.db, changed);
    assert!(app.ai.reviewed.is_none());
}

#[test]
fn dismiss_shortcut_reviews_equipment_and_preserves_history_after_reopening() {
    for equipment in ["stash", "lose"] {
        let (dir, mut store, cli, mut app) = setup();
        let before = store.db.clone();
        let gold = before.gold("example-warband");
        let carried: Vec<_> = before
            .allocations
            .iter()
            .filter(|a| a.member_id == "hans")
            .cloned()
            .collect();
        assert!(screen(&mut app, &store, 80, 24).contains("D Dismiss"));
        app.key(key(KeyCode::Char('d')), &mut store, &cli);
        assert!(
            matches!(&app.overlay, Overlay::Form(f) if f.action == Action::Remove && f.value("member") == "hans" && f.value("status") == "dismissed")
        );
        app.key(key(KeyCode::Esc), &mut store, &cli);
        assert_eq!(store.db, before);
        app.key(key(KeyCode::Char('d')), &mut store, &cli);
        field(&mut app, "equipment", equipment);
        app.review(&store);
        let review = screen(&mut app, &store, 140, 35);
        assert!(
            review.contains("Hans")
                && review.contains("no gold is refunded")
                && review.contains("new leader"),
            "{review}"
        );
        app.apply(&mut store, &cli);
        assert!(matches!(app.overlay, Overlay::None));
        assert!(!app.units(&store.db).iter().any(|u| u.id == "hans-unit"));
        assert_eq!(
            store
                .db
                .members
                .iter()
                .find(|m| m.id == "hans")
                .unwrap()
                .status,
            "dismissed"
        );
        assert_eq!(store.db.gold("example-warband"), gold);
        assert!(store.db.warbands[0].leader_id.is_empty());
        assert!(!store.db.allocations.iter().any(|a| a.member_id == "hans"));
        for a in carried {
            assert_eq!(
                store.db.owned("example-warband", &a.item_id),
                before.owned("example-warband", &a.item_id)
                    - if equipment == "lose" { a.quantity } else { 0 }
            );
        }
        assert_eq!(store.db.members.len(), before.members.len());
        assert!(store.db.events.last().unwrap().reason.contains("Hans"));
        let saved = store.db.clone();
        drop(store);
        assert_eq!(Store::open(dir.path()).unwrap().db, saved);
    }
}

#[test]
fn recent_notes_follow_hover_and_keyboard_selection_without_mutations() {
    let (_dir, mut store, cli, mut app) = setup();
    let mut db = store.db.clone();
    for text in ["Older note", "Second newest", "Third newest", "Newest note"] {
        db.notes.push(Note {
            id: id(),
            entity_id: "hans".into(),
            text: text.into(),
            date: "2026-10-19".into(),
        });
    }
    db.notes.push(Note {
        id: id(),
        entity_id: "champion-one".into(),
        text: "Other model note".into(),
        date: "2026-10-19".into(),
    });
    store
        .commit(db, "Notes fixture", "2026-10-19", "", "manual")
        .unwrap();
    let before = store.db.clone();
    let s = screen(&mut app, &store, 80, 24);
    for note in ["Newest note", "Third newest", "Second newest"] {
        assert!(s.contains(note), "{s}");
    }
    assert!(!s.contains("Older note"));
    let row = app
        .hits
        .iter()
        .find(|(_, hit)| matches!(hit, Hit::Row(1)))
        .unwrap()
        .0;
    app.hover(row.x, row.y);
    assert_eq!(app.selected[1], 1);
    let s = screen(&mut app, &store, 80, 24);
    assert!(s.contains("Other model note"), "{s}");
    assert!(!s.contains("Newest note"));
    app.key(key(KeyCode::Up), &mut store, &cli);
    assert!(screen(&mut app, &store, 80, 24).contains("Newest note"));
    app.key(key(KeyCode::F(6)), &mut store, &cli);
    assert!(matches!(&app.overlay, Overlay::Detail { text, .. } if text.contains("Older note")));
    app.hover(row.x, row.y);
    assert_eq!(app.selected[1], 0); // A popup cannot change the model underneath it.
    assert_eq!(store.db, before);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Note previews");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Start, &store, &cli);
    submit(&mut app, &mut store, &cli);
    let s = screen(&mut app, &store, 80, 24);
    for note in ["Newest note", "Third newest", "Second newest"] {
        assert!(s.contains(note), "{s}");
    }
    let row = app
        .hits
        .iter()
        .find(|(_, hit)| matches!(hit, Hit::Row(1)))
        .unwrap()
        .0;
    app.hover(row.x, row.y);
    let s = screen(&mut app, &store, 80, 24);
    assert!(
        s.contains("Other model note") && !s.contains("Newest note"),
        "{s}"
    );
    app.selected[3] = 0;
    app.quick(&mut store, &cli, "ooa");
    app.open(Action::End, &store, &cli);
    submit(&mut app, &mut store, &cli);
    let s = screen(&mut app, &store, 80, 24);
    for note in ["Newest note", "Third newest", "Second newest"] {
        assert!(s.contains(note), "{s}");
    }
    app.key(key(KeyCode::F(4)), &mut store, &cli);
    let s = screen(&mut app, &store, 80, 24);
    for note in ["Newest note", "Third newest", "Second newest"] {
        assert!(s.contains(note), "{s}");
    }
}

#[test]
fn group_note_previews_include_individuals_and_superseding_notes() {
    let (_dir, mut store, cli, mut app) = setup();
    let mut db = store.db.clone();
    db.notes.push(Note {
        id: "old-note".into(),
        entity_id: "arrows-one-1".into(),
        text: "Superseded warning".into(),
        date: "2026-10-19".into(),
    });
    for (entity, text) in [
        ("arrows-one-unit", "Group reminder"),
        ("arrows-one-2", "Second member reminder"),
        ("arrows-one-1", "Supersedes note old-note: New reminder"),
    ] {
        db.notes.push(Note {
            id: id(),
            entity_id: entity.into(),
            text: text.into(),
            date: "2026-10-19".into(),
        });
    }
    store
        .commit(db, "Group note fixture", "2026-10-19", "", "manual")
        .unwrap();
    app.selected[1] = app
        .units(&store.db)
        .iter()
        .position(|u| u.id == "arrows-one-unit")
        .unwrap();
    let s = screen(&mut app, &store, 140, 30);
    for note in ["Group reminder", "Second member reminder", "New reminder"] {
        assert!(s.contains(note), "{s}");
    }
    // The old version remains in the expanded dossier/history, while previews use the replacement.
    assert!(!s.contains("Superseded warning") && !s.contains("Supersedes note"));
    app.key(key(KeyCode::F(6)), &mut store, &cli);
    assert!(
        matches!(&app.overlay, Overlay::Detail { text, .. } if text.contains("Second member reminder") && text.contains("Superseded warning"))
    );
}

#[test]
fn external_agent_tab_is_available_without_build_features_or_credentials() {
    let (_dir, mut store, cli, mut app) = setup();
    assert_eq!(TABS.len(), 8);
    assert!(app.actions(&store.db).contains(&Action::AiRequest));
    assert!(screen(&mut app, &store, 140, 30).contains("8 AI assistant"));
    app.key(key(KeyCode::Char('8')), &mut store, &cli);
    assert_eq!(app.tab, AI_TAB);
    let s = screen(&mut app, &store, 140, 40);
    assert!(
        s.contains("MCP")
            && !s.contains("--features")
            && !s.contains("cargo build")
            && !s.contains("MORDTUI_AI_KEY"),
        "{s}"
    );
    app.key(key(KeyCode::Char('n')), &mut store, &cli);
    field(&mut app, "prompt", "Help me buy a weapon");
    let before = store.db.clone();
    submit(&mut app, &mut store, &cli);
    assert_eq!(store.db, before);
    let requests = crate::ai::exchange::requests(&store.root).unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].warband.as_deref(), Some("example-warband"));
    assert_eq!(requests[0].prompt, "Help me buy a weapon");
    app.switch_tab(TABS.len() - 1);
    app.key(key(KeyCode::Right), &mut store, &cli);
    assert_eq!(app.tab, 0);
    app.key(key(KeyCode::Left), &mut store, &cli);
    assert_eq!(app.tab, TABS.len() - 1);
}

#[test]
fn kill_credit_forms_cancel_cleanly_and_reload_corrections_for_the_chosen_warrior() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::Prepare, &store, &cli);
    field(&mut app, "name", "Kill picker");
    submit(&mut app, &mut store, &cli);
    app.open(Action::Start, &store, &cli);
    submit(&mut app, &mut store, &cli);
    let before = store.db.clone();
    app.key(key(KeyCode::Char('k')), &mut store, &cli);
    field(&mut app, "victim", "Cancelled opponent");
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert_eq!(store.db, before);
    for (member, victim) in [
        ("hans", "Goblin"),
        ("champion-one", "Skaven"),
        ("hans", "Goblin"),
    ] {
        app.open(Action::AddKill, &store, &cli);
        field(&mut app, "member", member);
        field(&mut app, "victim", victim);
        submit(&mut app, &mut store, &cli);
    }
    app.open(Action::RemoveKill, &store, &cli);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    let Kind::Choice(kills) = &f.fields.iter().find(|f| f.key == "kill").unwrap().kind else {
        panic!()
    };
    assert_eq!(kills.len(), 2);
    assert_ne!(kills[0].id, kills[1].id);
    assert_ne!(kills[0].label, kills[1].label);
    app.open_picker();
    app.choose("champion-one".into(), &store);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    let Kind::Choice(kills) = &f.fields.iter().find(|f| f.key == "kill").unwrap().kind else {
        panic!()
    };
    assert_eq!(kills.len(), 1);
    assert!(kills[0].label.contains("Skaven"));
    let before = store.db.clone();
    app.review(&store);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert_eq!(store.db, before);
    app.open(Action::RemoveKill, &store, &cli);
    app.open_picker();
    app.choose("champion-one".into(), &store);
    submit(&mut app, &mut store, &cli);
    assert_eq!(
        store
            .db
            .participants
            .iter()
            .find(|p| p.member_id == "champion-one")
            .unwrap()
            .kills,
        0
    );
    assert_eq!(
        store
            .db
            .participants
            .iter()
            .find(|p| p.member_id == "hans")
            .unwrap()
            .kills,
        2
    );
}

#[test]
fn new_equipment_checkbox_picker_selects_several_lists_and_cancel_keeps_the_original_choices() {
    let (dir, mut store, cli, mut app) = setup();
    app.key(key(KeyCode::Char('c')), &mut store, &cli);
    field(&mut app, "name", "Two-handed axe");
    let before = store.db.clone();
    let original = if let Overlay::Form(f) = &app.overlay {
        f.value("lists").to_string()
    } else {
        panic!()
    };
    focus_field(&mut app, &mut store, &cli, "lists");
    assert!(app.picker.as_ref().unwrap().checked.is_some());
    app.key(
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        &mut store,
        &cli,
    );
    app.key(key(KeyCode::Char(' ')), &mut store, &cli);
    app.key(key(KeyCode::Down), &mut store, &cli);
    app.key(key(KeyCode::Char(' ')), &mut store, &cli);
    let selected = app.picker.as_ref().unwrap().checked.clone().unwrap();
    assert_eq!(selected.len(), 2);
    for c in "Swordsmen".chars() {
        app.key(key(KeyCode::Char(c)), &mut store, &cli);
    }
    let s = screen(&mut app, &store, 80, 24);
    assert!(s.contains("Swordsmen") && s.contains("Enter: Done"), "{s}");
    assert_eq!(
        app.picker.as_ref().unwrap().checked.as_ref().unwrap(),
        &selected
    );
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert!(matches!(&app.overlay, Overlay::Form(f) if f.value("lists") == original));
    assert_eq!(store.db, before);
    focus_field(&mut app, &mut store, &cli, "lists");
    app.key(
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        &mut store,
        &cli,
    );
    for list in &selected {
        app.choose(list.clone(), &store);
    }
    app.key(key(KeyCode::Enter), &mut store, &cli);
    assert!(app.picker.is_none());
    assert_eq!(store.db, before);
    submit(&mut app, &mut store, &cli);
    let item = store
        .db
        .items
        .iter()
        .find(|i| i.name == "Two-handed axe")
        .unwrap();
    let memberships: std::collections::BTreeSet<_> = store
        .db
        .equipment_list_items
        .iter()
        .filter(|r| r.item_id == item.id)
        .map(|r| r.list_id.clone())
        .collect();
    assert_eq!(memberships, selected.into_iter().collect());
    assert_eq!(store.db.owned("example-warband", &item.id), 0);
    assert!(
        store
            .db
            .events
            .last()
            .unwrap()
            .reason
            .contains("Equipment lists")
    );
    let saved = store.db.clone();
    drop(store);
    assert_eq!(Store::open(dir.path()).unwrap().db, saved);
}

#[test]
fn shared_list_creation_assignment_and_item_membership_are_graphical() {
    let (_dir, mut store, cli, mut app) = setup();
    app.open(Action::NewEquipmentList, &store, &cli);
    field(&mut app, "name", "Hero weapons");
    focus_field(&mut app, &mut store, &cli, "types");
    app.key(
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        &mut store,
        &cli,
    );
    for kind in ["burgomeisters-captain", "burgomeisters-champion"] {
        app.choose(kind.into(), &store);
    }
    screen(&mut app, &store, 80, 24);
    assert!(app.hits.iter().any(|(_, h)| matches!(h, Hit::PickerDone)));
    app.click(Hit::PickerDone, &mut store, &cli);
    focus_field(&mut app, &mut store, &cli, "items");
    app.choose("burgomeisters-dagger".into(), &store);
    app.click(Hit::PickerDone, &mut store, &cli);
    submit(&mut app, &mut store, &cli);
    let list = store
        .db
        .equipment_lists
        .iter()
        .find(|l| l.name == "Hero weapons")
        .unwrap()
        .id
        .clone();
    for model in ["hans", "champion-one", "champion-two"] {
        assert_eq!(store.db.equipment_list_for(model).unwrap().id, list);
    }
    let s = screen(&mut app, &store, 80, 24);
    assert!(s.contains("G Lists"), "{s}");
    app.key(key(KeyCode::Char('g')), &mut store, &cli);
    field(&mut app, "list", "inherit");
    let before = store.db.clone();
    app.key(key(KeyCode::Esc), &mut store, &cli);
    assert_eq!(store.db, before);
    app.switch_tab(2);
    app.selected[2] = app
        .items(&store.db)
        .iter()
        .position(|i| i.id == "burgomeisters-sword")
        .unwrap();
    app.key(key(KeyCode::Char('g')), &mut store, &cli);
    assert!(
        matches!(&app.overlay, Overlay::Form(f) if f.action == Action::EditEquipmentLists && f.value("item") == "burgomeisters-sword")
    );
    focus_field(&mut app, &mut store, &cli, "lists");
    app.key(
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        &mut store,
        &cli,
    );
    app.choose(list.clone(), &store);
    app.click(Hit::PickerDone, &mut store, &cli);
    submit(&mut app, &mut store, &cli);
    assert!(store.db.equipment_allowed("hans", "burgomeisters-sword"));
    assert!(
        !store
            .db
            .equipment_allowed("swordsmen", "burgomeisters-sword")
    );
    assert!(
        views::item_details(
            &store.db,
            "example-warband",
            store
                .db
                .items
                .iter()
                .find(|i| i.id == "burgomeisters-sword")
                .unwrap()
        )
        .contains("Hero weapons")
    );
    app.open(Action::EditEquipmentLists, &store, &cli);
    focus_field(&mut app, &mut store, &cli, "item");
    app.choose("burgomeisters-sword".into(), &store);
    let Overlay::Form(f) = &app.overlay else {
        panic!()
    };
    assert_eq!(
        serde_json::from_str::<Vec<String>>(f.value("lists")).unwrap(),
        vec![list]
    );
}

mod rulebook_tests;
