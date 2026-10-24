use super::*;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap},
};
const ACCENT: Color = Color::Cyan;
fn block(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(title.into())
        .border_style(Style::default().fg(Color::DarkGray))
}
fn focus() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}
pub(super) fn text(f: &mut Frame, area: Rect, title: impl Into<String>, body: impl Into<String>) {
    f.render_widget(
        Paragraph::new(body.into())
            .wrap(Wrap { trim: false })
            .block(block(title)),
        area,
    );
}
fn button(f: &mut Frame, app: &mut App, rect: Rect, label: &str, hit: Hit) {
    f.render_widget(
        Paragraph::new(label).style(Style::default().fg(ACCENT)),
        rect,
    );
    app.hits.push((rect, hit));
}
fn line_buttons(f: &mut Frame, app: &mut App, area: Rect, buttons: &[(&str, Hit)]) {
    let mut x = area.x;
    let mut y = area.y;
    for (label, hit) in buttons {
        let needed = label.chars().count() as u16 + 3;
        if x > area.x && area.right().saturating_sub(x) < needed {
            x = area.x;
            y += 1;
        }
        if y >= area.bottom() {
            break;
        }
        let w = needed.min(area.width);
        if w < 3 {
            continue;
        }
        button(f, app, Rect::new(x, y, w, 1), label, *hit);
        x += w;
    }
}

pub(super) fn table(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    title: &str,
    headers: Vec<&str>,
    rows: Vec<Vec<String>>,
    widths: Vec<Constraint>,
) {
    let tab = app.tab;
    let b = block(if app.filter.is_empty() || app.tab == 4 {
        title.to_string()
    } else {
        format!("{title} · filter: {}", app.filter)
    });
    let inner = b.inner(area);
    let n = rows.len();
    app.selected[tab] = app.selected[tab].min(n.saturating_sub(1));
    app.tables[tab].select(if n == 0 {
        None
    } else {
        Some(app.selected[tab])
    });
    let rows: Vec<_> = rows.into_iter().map(Row::new).collect();
    let t = Table::new(rows, widths)
        .header(Row::new(headers).style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)))
        .column_spacing(1)
        .row_highlight_style(focus())
        .highlight_symbol("› ")
        .block(b);
    f.render_stateful_widget(t, area, &mut app.tables[tab]);
    if n == 0 {
        f.render_widget(
            Paragraph::new("No matching records. Use the actions above to get started.")
                .wrap(Wrap { trim: false }),
            Rect::new(
                inner.x,
                inner.y + 2,
                inner.width,
                inner.height.saturating_sub(2),
            ),
        );
    }
    let offset = app.tables[tab].offset();
    for y in 0..inner.height.saturating_sub(1) {
        let index = offset + y as usize;
        if index >= n {
            break;
        }
        app.hits.push((
            Rect::new(inner.x, inner.y + 1 + y, inner.width, 1),
            Hit::Row(index),
        ));
    }
}
fn toolbar(app: &App, db: &Db) -> Vec<(String, Hit)> {
    if app.tab == RULES_TAB {
        return vec![
            ("P Profile".into(), Hit::Action(Action::RuleSelect)),
            ("N New".into(), Hit::RuleNew),
            ("E Edit".into(), Hit::RuleEdit),
            ("V Details".into(), Hit::RuleDetails),
            ("D Delete".into(), Hit::RuleDelete),
            ("C Copy profile".into(), Hit::Action(Action::RuleCopy)),
            (
                "W Create warband".into(),
                Hit::Action(Action::RuleCreateWarband),
            ),
            ("A Actions".into(), Hit::Menu),
        ];
    }
    if app.tab == AI_TAB {
        return vec![
            ("N Ask assistant".into(), Hit::Action(Action::AiRequest)),
            ("R Review plan".into(), Hit::AiReview),
            ("Y Approve".into(), Hit::AiApprove),
            ("D Reject".into(), Hit::AiReject),
        ];
    }
    if app.tab == 4 {
        let bindings = app.post_bindings(db);
        let mut out: Vec<_> = bindings
            .iter()
            .filter(|b| matches!(b.action, Action::Step | Action::SkipStage))
            .map(|b| {
                (
                    format!("{} {}", b.key.to_ascii_uppercase(), b.label),
                    Hit::Action(b.action),
                )
            })
            .collect();
        out.push(("A Stage actions".into(), Hit::Menu));
        out.extend(
            bindings
                .iter()
                .filter(|b| !matches!(b.action, Action::Step | Action::SkipStage))
                .map(|b| {
                    (
                        format!("{} {}", b.key.to_ascii_uppercase(), b.label),
                        Hit::Action(b.action),
                    )
                }),
        );
        return out;
    }
    let mut buttons = vec![("A Actions", Hit::Menu)];
    if matches!(app.tab, 1 | 2 | 5) {
        buttons.push(("Enter Details", Hit::Details));
    } // The Actions entry is handled as a menu by draw.
    buttons.extend(match app.tab {
        1 => vec![
            ("H Hire", Hit::Action(Action::Hire)),
            ("D Dismiss", Hit::Action(Action::Remove)),
            ("P Purchase", Hit::Action(Action::Buy)),
            ("E Equip", Hit::Action(Action::Equip)),
            ("G Lists", Hit::Action(Action::AssignEquipmentList)),
            ("X XP", Hit::Action(Action::Xp)),
            ("U Advance", Hit::Action(Action::Advance)),
            ("S New skill", Hit::Action(Action::NewAbility)),
            ("L Record skill", Hit::Action(Action::GrantAbility)),
            ("C New gear", Hit::Action(Action::NewEquipment)),
            ("N Note", Hit::Action(Action::Note)),
            ("F4 Injuries", Hit::Action(Action::Injuries)),
        ],
        2 => vec![
            ("P Purchase", Hit::Action(Action::Buy)),
            ("G Lists", Hit::Action(Action::EditEquipmentLists)),
            ("E Equip", Hit::Action(Action::Equip)),
            ("S Sell", Hit::Action(Action::Sell)),
            ("F Find", Hit::Action(Action::Find)),
            ("L Lose", Hit::Action(Action::Lose)),
            ("C New gear", Hit::Action(Action::NewEquipment)),
        ],
        3 => {
            let action = match app.battle(db).map(|b| b.status.as_str()) {
                Some("prepared") => Action::Start,
                Some("in-progress") => Action::End,
                Some("post-battle") => Action::Complete,
                _ => Action::Prepare,
            };
            vec![
                (
                    match action {
                        Action::Start => "F5 Start battle",
                        Action::End => "F5 End battle",
                        Action::Complete => "F5 Post-battle",
                        _ => "F5 Prepare",
                    },
                    Hit::BattleLifecycle,
                ),
                ("V Profile", Hit::BattleProfile),
                ("W Wounds", Hit::Action(Action::Record)),
                ("- W", Hit::Quick("-")),
                ("+ W", Hit::Quick("+")),
                ("K +Kill", Hit::Quick("kill")),
                ("J -Kill", Hit::Quick("unkill")),
                ("O OOA", Hit::Quick("ooa")),
                ("C Use item", Hit::Action(Action::Consume)),
                ("] +Obj", Hit::Quick("objective")),
                ("[ -Obj", Hit::Quick("unobjective")),
                ("N Note", Hit::Action(Action::Note)),
            ]
        }
        5 => vec![
            ("N Note", Hit::Action(Action::Note)),
            ("S Snapshot", Hit::Action(Action::Snapshot)),
            ("C Compare", Hit::Action(Action::Compare)),
            ("E Export", Hit::Action(Action::Export)),
            ("V Change journal", Hit::Journal),
        ],
        _ => vec![
            ("F2 Warband", Hit::Action(Action::Switch)),
            ("F3 Campaign", Hit::Action(Action::CampaignRules)),
            ("F4 Injuries", Hit::Action(Action::Injuries)),
            ("F5 Battle", Hit::BattleLifecycle),
            ("S Snapshot", Hit::Action(Action::Snapshot)),
        ],
    });
    buttons
        .into_iter()
        .map(|(label, hit)| (label.to_string(), hit))
        .collect()
}
pub(super) fn draw(f: &mut Frame, app: &mut App, store: &Store) {
    app.hits.clear();
    let area = f.area();
    if area.width < 80 || area.height < 24 {
        f.render_widget(Paragraph::new("Mordtui needs at least 80 columns × 24 rows for the interactive roster. Resize the terminal; Q exits.").wrap(Wrap{trim:false}),area);
        return;
    }
    let db = &store.db;
    let regions = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(8),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .split(area);
    let phase = match app.battle(db).map(|b| b.status.as_str()) {
        Some("in-progress") => "BATTLE",
        Some("post-battle") => "POST-BATTLE",
        Some("prepared") => "PRE-BATTLE · READY",
        _ => "PRE-BATTLE",
    };
    let header = if let Some(w) = app.warband(db) {
        format!(
            " [{phase}] {} · {} gc · Rating {} · {}",
            w.name,
            db.gold(&w.id),
            db.rating(&w.id),
            db.profile(&w.profile_id).faction
        )
    } else {
        format!(" [{phase}] Choose or create a warband")
    };
    f.render_widget(
        Paragraph::new(header).style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        regions[0],
    );
    if let Some(w) = app.warband(db) {
        let pending: Vec<_> = db
            .units
            .iter()
            .filter(|u| u.warband_id == w.id && !db.pending(u).is_empty())
            .map(|u| format!("{} ({})", u.name, db.pending(u).len()))
            .collect();
        if !pending.is_empty() {
            f.render_widget(
                Paragraph::new(format!(" PLUS-UP READY: {}", pending.join(", "))).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Rect::new(regions[0].x, regions[0].y + 1, regions[0].width, 1),
            );
        }
    }
    let tabs = Layout::horizontal(vec![Constraint::Ratio(1, TABS.len() as u32); TABS.len()])
        .split(regions[1]);
    for (i, r) in tabs.iter().enumerate() {
        f.render_widget(
            Paragraph::new(format!(
                "{} {}",
                i + 1,
                if area.width < 105 {
                    [
                        "Home", "Roster", "Stash", "Battle", "Post", "Journal", "Rules", "AI",
                    ][i]
                } else {
                    TABS[i]
                }
            ))
            .style(if app.tab == i {
                focus()
            } else {
                Style::default()
            })
            .block(block("")),
            *r,
        );
        app.hits.push((*r, Hit::Tab(i)));
    }
    let buttons = toolbar(app, db);
    let buttons: Vec<_> = buttons.iter().map(|(s, h)| (s.as_str(), *h)).collect();
    line_buttons(f, app, regions[2], &buttons);
    // Overlay hit regions replace the underlying hit map below.
    if app.tab == RULES_TAB {
        app.draw_rules(f, store, regions[3]);
    } else if app.tab == AI_TAB {
        workbench(f, app, regions[3]);
    } else if app.warband(db).is_none() {
        welcome(f, app, regions[3]);
    } else {
        match app.tab {
            0 => overview(f, app, db, regions[3]),
            1 => warband(f, app, db, regions[3]),
            2 => stash(f, app, db, regions[3]),
            3 => battle(f, app, store, regions[3]),
            4 => post_battle(f, app, db, regions[3]),
            _ => journal(f, app, db, regions[3]),
        }
    }
    f.render_widget(
        Paragraph::new(app.message.clone())
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(Color::Yellow)),
        regions[4],
    );
    let hint = if app.tab == RULES_TAB {
        "Tab sections/records · ↑↓ select · Enter edit · P profile · N new · F7 rules"
    } else if app.tab == AI_TAB {
        "N Ask · R Review · Y Approve · D Reject · ↑↓ scroll · ←→ views · ? help"
    } else if app.tab == 4 {
        if app.post_focus == PostFocus::Stages {
            "↑↓ stages · Tab targets · ←→ views · A actions · F3 rules · F4 injuries"
        } else {
            "↑↓ targets · Tab stages · ←→ views · Enter act · F3 rules · F4 injuries"
        }
    } else if app.tab == 3 {
        "- / + wounds · K +kill · J -kill · F5 battle · F6 notes · V full profile"
    } else {
        "↑↓ select · Enter detail · A actions · F3 rules · F4 injuries · ? help · Q quit"
    };
    f.render_widget(
        Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)),
        regions[5],
    );
    overlay(f, app, store, area);
}
fn welcome(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(8),
        Constraint::Length(3),
        Constraint::Min(0),
    ])
    .split(area);
    text(
        f,
        rows[0],
        "Welcome to Mordtui",
        "Manage your warband with tables, choice pickers and forms.\n\nCreate a new campaign, import the supplied warband, or choose an existing warband.\nPress A for the searchable action menu. No command syntax is needed.",
    );
    line_buttons(
        f,
        app,
        rows[1],
        &[
            ("Program rules", Hit::Tab(RULES_TAB)),
            ("Create campaign", Hit::Action(Action::Init)),
            ("Import warband", Hit::Action(Action::Import)),
            ("Choose warband", Hit::Action(Action::Switch)),
        ],
    );
}
fn overview(f: &mut Frame, app: &mut App, db: &Db, area: Rect) {
    let Some(w) = app.warband(db) else { return };
    let w = w.clone();
    let rows = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(3),
        Constraint::Length(5),
    ])
    .split(area);
    let ms: Vec<_> = db.members_of(&w.id).filter(|m| active(m)).collect();
    let pending: usize = db
        .units
        .iter()
        .filter(|u| u.warband_id == w.id)
        .map(|u| db.pending(u).len())
        .sum();
    let metrics = Layout::horizontal([Constraint::Ratio(1, 4); 4]).split(rows[0]);
    let shards = db
        .items
        .iter()
        .find(|i| i.profile_id == w.profile_id && i.name == "Wyrdstone")
        .map_or(0, |i| db.owned(&w.id, &i.id));
    for (r, (title, val)) in metrics.iter().zip([
        ("Treasury", format!("{} gc", db.gold(&w.id))),
        ("Wyrdstone", shards.to_string()),
        (
            "Available / roster",
            format!(
                "{} / {}",
                ms.iter()
                    .filter(|m| m.status == "active" && m.missed_games == 0)
                    .count(),
                ms.len()
            ),
        ),
        ("Pending advances", pending.to_string()),
    ]) {
        text(f, *r, title, val);
    }
    let middle =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(rows[1]);
    let heroes = ms
        .iter()
        .filter(|m| db.unit(&m.unit_id).role == "hero")
        .count();
    let upkeep: i64 = ms
        .iter()
        .map(|m| db.typ(&db.unit(&m.unit_id).type_id).upkeep)
        .sum();
    text(
        f,
        middle[0],
        "Campaign overview",
        format!(
            "{}\nFaction: {} · rules {}\nHeroes: {} / {}\nLeader: {}\nRating: {}\nUpkeep: {} gc",
            db.campaigns
                .iter()
                .find(|c| c.id == w.campaign_id)
                .map(|c| c.name.as_str())
                .unwrap_or(""),
            db.profile(&w.profile_id).faction,
            db.profile(&w.profile_id).version,
            heroes,
            db.profile(&w.profile_id).max_heroes,
            if w.leader_id.is_empty() {
                "Choose a successor".into()
            } else {
                entity_name(db, &w.leader_id)
            },
            db.rating(&w.id),
            upkeep
        ),
    );
    let next = if let Some(b) = app.battle(db) {
        let stage = db
            .steps
            .iter()
            .find(|s| s.battle_id == b.id && !matches!(s.status.as_str(), "completed" | "skipped"));
        format!(
            "{}\n{} vs {} · {}\nNext stage: {}\nUnresolved injuries: {}\nUse Battle for wounds; Post-battle for campaign outcomes.",
            b.name,
            b.scenario,
            b.opponent,
            b.status,
            stage
                .map(|s| s.name.as_str())
                .unwrap_or("All stages resolved"),
            db.participants
                .iter()
                .filter(|p| p.battle_id == b.id
                    && p.condition == "out-of-action"
                    && !p.injury_resolved)
                .count()
        )
    } else {
        "No active battle.\nPrepare a battle when your roster and loadouts are ready.\nH Hire · P Purchase · E Equip · B Prepare battle".into()
    };
    text(f, middle[1], "Session tasks", next);
    let notes = db
        .notes
        .iter()
        .rev()
        .filter(|n| n.entity_id == w.id)
        .take(3)
        .map(|n| n.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    text(f, rows[2], "Recent warband notes · N add / update", notes);
}
fn warband(f: &mut Frame, app: &mut App, db: &Db, area: Rect) {
    let parts = Layout::vertical([Constraint::Percentage(62), Constraint::Min(5)]).split(area);
    let mut headers = vec!["Unit", "Type", "#"];
    headers.extend(STATS);
    headers.extend(["XP", "Plus-ups"]);
    let mut widths = vec![
        Constraint::Min(12),
        Constraint::Length(8),
        Constraint::Length(2),
    ];
    widths.extend([Constraint::Length(3); 9]);
    widths.extend([Constraint::Length(4), Constraint::Length(9)]);
    let wide = area.width >= 110;
    if wide {
        headers.push("Equipment / model");
        widths.push(Constraint::Min(20));
    }
    let units = app.units(db);
    let n = units.len();
    let rows: Vec<_> = units
        .iter()
        .map(|u| {
            let ms: Vec<_> = db
                .members
                .iter()
                .filter(|m| m.unit_id == u.id && active(m))
                .collect();
            let pending = db.pending(u).len();
            let mut values = vec![
                u.name.clone(),
                db.typ(&u.type_id).name.clone(),
                ms.len().to_string(),
            ];
            values.extend(STATS.iter().map(|stat| {
                let values: std::collections::BTreeSet<_> =
                    ms.iter().map(|m| db.stat(m, stat, None)).collect();
                if values.len() > 1 {
                    "*".into()
                } else {
                    values.into_iter().next().unwrap_or_else(|| "—".into())
                }
            }));
            values.extend([
                (u.starting_xp + u.earned_xp).to_string(),
                if pending > 0 {
                    format!("READY {pending}")
                } else {
                    "—".into()
                },
            ]);
            if wide {
                values.push(ms.first().map(|m| gear(db, &m.id)).unwrap_or_default());
            }
            Row::new(values.into_iter().zip(&headers).map(|(value, label)| {
                Cell::from(Text::from(vec![
                    Line::styled(*label, Style::default().fg(ACCENT)),
                    Line::styled(
                        value,
                        if *label == "Plus-ups" && pending > 0 {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                ]))
            }))
            .height(2)
            .bottom_margin(1)
        })
        .collect();
    let b = block("Warband quick look · labels repeat per group · * = member profiles differ");
    let inner = b.inner(parts[0]);
    app.selected[1] = app.selected[1].min(n.saturating_sub(1));
    app.tables[1].select((n > 0).then_some(app.selected[1]));
    f.render_stateful_widget(
        Table::new(rows, widths)
            .column_spacing(1)
            .highlight_symbol("› ")
            .row_highlight_style(focus())
            .block(b),
        parts[0],
        &mut app.tables[1],
    );
    let offset = app.tables[1].offset();
    for y in 0..inner.height {
        let index = offset + (y / 3) as usize;
        if index < n {
            app.hits.push((
                Rect::new(inner.x, inner.y + y, inner.width, 1),
                Hit::Row(index),
            ));
        }
    }
    if let Some(u) = app.units(db).get(app.selected[1]) {
        let count = db.pending(u).len();
        let title = if count > 0 {
            format!("{} · {count} PLUS-UP(S) READY · U resolves", u.name)
        } else {
            format!("{} · {} · Enter expands", u.name, u.role)
        };
        let panels = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(parts[1]);
        text(f, panels[0], title, unit_details(db, u, None));
        recent_notes_panel(f, panels[1], db, &u.id, None);
    }
}
pub(super) fn gear(db: &Db, mid: &str) -> String {
    db.allocations
        .iter()
        .filter(|a| a.member_id == mid)
        .map(|a| {
            format!(
                "{}{}",
                db.items
                    .iter()
                    .find(|i| i.id == a.item_id)
                    .map(|i| i.name.as_str())
                    .unwrap_or("?"),
                if a.quantity > 1 {
                    format!(" ×{}", a.quantity)
                } else {
                    String::new()
                }
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
pub(super) fn member_notes(db: &Db, member: &Member) -> String {
    let mut notes = db
        .notes
        .iter()
        .filter(|n| n.entity_id == member.id || n.entity_id == member.unit_id)
        .map(|n| {
            format!(
                "{}: {}",
                if n.entity_id == member.id {
                    member.name.as_str()
                } else {
                    db.unit(&member.unit_id).name.as_str()
                },
                n.text
            )
        })
        .collect::<Vec<_>>();
    notes.extend(legacy_battle_notes(db, &member.unit_id, Some(&member.id)));
    if notes.is_empty() {
        "No model or unit notes.".into()
    } else {
        notes.join("\n")
    }
}

fn legacy_battle_notes(db: &Db, unit: &str, member: Option<&str>) -> Vec<String> {
    db.participants
        .iter()
        .rev()
        .filter(|p| !p.attribution.trim().is_empty())
        .filter_map(|p| {
            let m = db.members.iter().find(|m| {
                m.id == p.member_id && m.unit_id == unit && member.is_none_or(|id| id == m.id)
            })?;
            let b = db.battles.iter().find(|b| b.id == p.battle_id)?;
            Some(format!("{} · {}: {}", m.name, b.name, p.attribution))
        })
        .collect()
}

fn recent_notes(db: &Db, unit: &str, member: Option<&str>) -> Vec<String> {
    let superseded: std::collections::HashSet<_> = db
        .notes
        .iter()
        .filter_map(|n| {
            n.text
                .strip_prefix("Supersedes note ")?
                .split_once(": ")
                .map(|(id, _)| id)
        })
        .collect();
    let mut notes: Vec<_> = db
        .notes
        .iter()
        .rev()
        .filter(|n| !superseded.contains(n.id.as_str()))
        .filter(|n| {
            n.entity_id == unit
                || db.members.iter().any(|m| {
                    m.unit_id == unit && m.id == n.entity_id && member.is_none_or(|id| id == m.id)
                })
        })
        .take(3)
        .map(|n| {
            let value = n
                .text
                .strip_prefix("Supersedes note ")
                .and_then(|s| s.split_once(": "))
                .map_or(n.text.as_str(), |(_, text)| text);
            format!(
                "{}: {}",
                entity_name(db, &n.entity_id),
                value.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect();
    if notes.len() < 3 {
        notes.extend(
            legacy_battle_notes(db, unit, member)
                .into_iter()
                .take(3 - notes.len())
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")),
        );
    }
    notes
}

fn recent_notes_panel(f: &mut Frame, area: Rect, db: &Db, unit: &str, member: Option<&str>) {
    let notes = recent_notes(db, unit, member);
    let body = if notes.is_empty() {
        "No model or unit notes.".into()
    } else {
        notes.join("\n")
    };
    // One line per recent entry keeps all three visible in a small terminal.
    f.render_widget(
        Paragraph::new(body).block(block("Recent notes · F6 all")),
        area,
    );
}

pub(super) fn kill_log(db: &Db, p: &Participant) -> String {
    let mut kills: Vec<_> = db
        .battle_kills(p)
        .map(|r| {
            if r.description.is_empty() {
                "Unspecified opponent".into()
            } else {
                r.description.clone()
            }
        })
        .collect();
    let unspecified = p.kills as usize - kills.len();
    if unspecified > 0 {
        kills.push(format!("{unspecified} unspecified older credit(s)"));
    }
    if kills.is_empty() {
        "None".into()
    } else {
        kills.join(", ")
    }
}
pub(super) fn unit_details(db: &Db, u: &Unit, battle: Option<&str>) -> String {
    let t = db.typ(&u.type_id);
    let ms: Vec<_> = db.members.iter().filter(|m| m.unit_id == u.id).collect();
    let abilities = db
        .advances
        .iter()
        .filter(|a| a.unit_id == u.id && !a.ability_id.is_empty())
        .filter_map(|a| db.abilities.iter().find(|s| s.id == a.ability_id))
        .map(|a| format!("{}: {}", a.name, a.description))
        .collect::<Vec<_>>();
    let advances = db
        .advances
        .iter()
        .filter(|a| a.unit_id == u.id && !a.characteristic.is_empty())
        .map(|a| format!("{}{:+}", a.characteristic, a.delta))
        .collect::<Vec<_>>()
        .join(", ");
    let mut s = format!(
        "{} · {} · XP {} (opening {} + earned {}) · pending {:?}\nEquipment lists: {}\nBase: {}\nAdvances: {}\n",
        u.role,
        t.name,
        u.starting_xp + u.earned_xp,
        u.starting_xp,
        u.earned_xp,
        db.pending(u),
        ms.iter()
            .filter_map(|m| db.equipment_list_for(&m.id))
            .map(|l| l.name.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", "),
        STATS
            .iter()
            .map(|stat| format!("{stat} {}", t.stat(stat)))
            .collect::<Vec<_>>()
            .join("  "),
        if advances.is_empty() {
            "None"
        } else {
            &advances
        }
    );
    if !t.description.is_empty() {
        s.push_str(&format!("Rules: {}\n", t.description));
    }
    for m in ms {
        let p = battle.and_then(|bid| {
            db.participants
                .iter()
                .find(|p| p.battle_id == bid && p.member_id == m.id)
        });
        s.push_str(&format!(
            "\n{} · {}{}{}\n  {}\n  Equipment: {}\n",
            m.name,
            m.status,
            if m.missed_games > 0 {
                format!(" · misses {} game(s)", m.missed_games)
            } else {
                String::new()
            },
            p.map(|p| format!(" · {} wounds remaining · {}", p.wounds, p.condition))
                .unwrap_or_default(),
            STATS
                .iter()
                .map(|stat| format!("{stat} {}", db.stat(m, stat, battle)))
                .collect::<Vec<_>>()
                .join("  "),
            gear(db, &m.id)
        ));
        s.push_str(&format!(
            "  Equipment list: {}\n",
            db.equipment_list_for(&m.id)
                .map_or("Unassigned", |l| l.name.as_str())
        ));
        s.push_str("  Injuries / recovery outcomes (F4 for full list):\n");
        let injuries: Vec<_> = db
            .resolutions
            .iter()
            .filter(|r| r.key == format!("injury-{}", m.id))
            .collect();
        if injuries.is_empty() {
            s.push_str("    No recorded injury outcomes.\n");
        }
        for r in injuries {
            let source = db
                .battles
                .iter()
                .find(|b| b.id == r.battle_id)
                .map(|b| b.name.as_str())
                .unwrap_or("Previous battle");
            s.push_str(&format!("    {}: {}\n", source, r.description));
        }
        for e in db.effects.iter().filter(|e| {
            e.member_id == m.id && (e.battle_id.is_empty() || Some(e.battle_id.as_str()) == battle)
        }) {
            s.push_str(&format!(
                "  Effect: {} {}{:+}{}\n",
                e.description,
                e.characteristic,
                e.delta,
                if e.condition.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", e.condition)
                }
            ));
        }
        for n in db.notes.iter().filter(|n| n.entity_id == m.id) {
            s.push_str(&format!("  Note: {}\n", n.text));
        }
        for note in legacy_battle_notes(db, &u.id, Some(&m.id)) {
            s.push_str(&format!("  Note: {note}\n"));
        }
    }
    if !abilities.is_empty() {
        s.push_str(&format!("\nSkills / abilities\n{}\n", abilities.join("\n")));
    }
    for n in db.notes.iter().filter(|n| n.entity_id == u.id) {
        s.push_str(&format!("Note: {}\n", n.text));
    }
    s
}
fn stash(f: &mut Frame, app: &mut App, db: &Db, area: Rect) {
    let parts = Layout::vertical([Constraint::Percentage(65), Constraint::Min(5)]).split(area);
    let warband = app.wb.clone().unwrap_or_default();
    let w = warband.as_str();
    let data = app
        .items(db)
        .iter()
        .map(|i| {
            vec![
                i.name.clone(),
                i.category.clone(),
                i.cost.to_string(),
                if i.rarity == 0 {
                    "Common".into()
                } else {
                    format!("Rare {}", i.rarity)
                },
                db.owned(w, &i.id).to_string(),
                db.assigned(w, &i.id).to_string(),
                (db.owned(w, &i.id) - db.assigned(w, &i.id)).to_string(),
            ]
        })
        .collect();
    table(
        f,
        app,
        parts[0],
        "Equipment · quantities count physical copies",
        vec![
            "Equipment",
            "Kind",
            "Price",
            "Availability",
            "Owned",
            "Issued",
            "Stash",
        ],
        data,
        vec![
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(13),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
        ],
    );
    if let Some(i) = app.items(db).get(app.selected[2]) {
        text(f, parts[1], &i.name, item_details(db, w, i));
    }
}
pub(super) fn item_details(db: &Db, w: &str, i: &Item) -> String {
    let owners = db
        .allocations
        .iter()
        .filter(|a| {
            a.item_id == i.id
                && db
                    .members
                    .iter()
                    .any(|m| m.id == a.member_id && db.unit(&m.unit_id).warband_id == w)
        })
        .map(|a| format!("{} ×{}", entity_name(db, &a.member_id), a.quantity))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Equipment lists: {}\n{}\n\nCatalog: {} gc · {}\nIssued to: {}\nOwned {} = issued {} + stash {}",
        db.equipment_lists
            .iter()
            .filter(|l| db
                .equipment_list_items
                .iter()
                .any(|r| r.list_id == l.id && r.item_id == i.id))
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        i.description,
        i.cost,
        if i.rarity > 0 {
            format!("Rare {}", i.rarity)
        } else {
            "Common".into()
        },
        if owners.is_empty() { "Nobody" } else { &owners },
        db.owned(w, &i.id),
        db.assigned(w, &i.id),
        db.owned(w, &i.id) - db.assigned(w, &i.id)
    )
}
fn battle(f: &mut Frame, app: &mut App, store: &Store, area: Rect) {
    let db = &store.db;
    let Some(b) = app.battle(db).cloned() else {
        text(
            f,
            area,
            "Battle dashboard",
            "No active battle. B prepares one. The pre-battle roster is preserved as a reference.\n\nDuring play: select an individual, -/+ adjusts wounds, K credits a kill, O marks out of action, W opens all counters.",
        );
        return;
    };
    let parts = Layout::vertical([
        Constraint::Length(3),
        Constraint::Percentage(48),
        Constraint::Min(5),
    ])
    .split(area);
    let ps = app.participants(db);
    let ooa = ps.iter().filter(|p| p.condition == "out-of-action").count();
    text(
        f,
        parts[0],
        format!("{} · {}", b.name, b.status),
        format!(
            "{} vs {} · result {} · OOA {} / {} · rout threshold {}",
            b.scenario,
            b.opponent,
            if b.result.is_empty() {
                "pending"
            } else {
                &b.result
            },
            ooa,
            ps.len(),
            ps.len().div_ceil(4)
        ),
    );
    let wide = area.width >= 120;
    let data = ps
        .iter()
        .map(|p| {
            let m = db.members.iter().find(|m| m.id == p.member_id).unwrap();
            let noted = db
                .notes
                .iter()
                .any(|n| n.entity_id == m.id || n.entity_id == m.unit_id)
                || !legacy_battle_notes(db, &m.unit_id, Some(&m.id)).is_empty();
            let mut cells = vec![format!("{}{}", m.name, if noted { " [N]" } else { "" })];
            cells.extend(STATS.iter().map(|s| db.stat(m, s, Some(&b.id))));
            cells.extend([
                p.wounds.to_string(),
                p.condition.clone(),
                p.kills.to_string(),
                p.objectives.to_string(),
            ]);
            if wide {
                cells.push(db.unit(&m.unit_id).name.clone());
                cells.push(kill_log(db, p));
            }
            cells
        })
        .collect();
    let mut headers = vec!["Member"];
    headers.extend(STATS);
    headers.extend(["Rem", "Condition", "K", "Obj"]);
    let mut widths = vec![Constraint::Min(14)];
    widths.extend([Constraint::Length(3); 9]);
    widths.extend([
        Constraint::Length(3),
        Constraint::Length(10),
        Constraint::Length(2),
        Constraint::Length(3),
    ]);
    if wide {
        headers.extend(["Unit", "Opponents taken out"]);
        widths.extend([Constraint::Length(14), Constraint::Min(16)]);
    }
    table(
        f,
        app,
        parts[1],
        "Battle profiles · Rem = wounds left · [N] notes · K/J kills",
        headers,
        data,
        widths,
    );
    if let Some(p) = app.participants(db).get(app.selected[3]) {
        let m = db.members.iter().find(|m| m.id == p.member_id).unwrap();
        let snap = db.snapshots.iter().find(|s| s.id == b.pre_snapshot);
        let frozen = snap.and_then(|s| store.historical(&s.generation).ok());
        let reference = if let Some(old) = &frozen {
            old.members
                .iter()
                .find(|m| m.id == p.member_id)
                .map(|m| {
                    let u = old.unit(&m.unit_id);
                    unit_details(old, u, None)
                })
                .unwrap_or_default()
        } else {
            "Pre-battle reference unavailable; inspect backup/history.".into()
        };
        let r = Layout::horizontal(if wide {
            vec![
                Constraint::Percentage(32),
                Constraint::Percentage(34),
                Constraint::Percentage(34),
            ]
        } else {
            vec![Constraint::Percentage(55), Constraint::Percentage(45)]
        })
        .split(parts[2]);
        if wide {
            text(f, r[0], "Frozen pre-battle profile", &reference);
        }
        let current = usize::from(wide);
        text(
            f,
            r[current],
            "Current battle · V expands",
            format!(
                "{} · {}\nRem {} · {} · K {} · Obj {}\nOpponents: {}\n{}{}",
                m.name,
                db.unit(&m.unit_id).name,
                p.wounds,
                p.condition,
                p.kills,
                p.objectives,
                kill_log(db, p),
                STATS
                    .iter()
                    .map(|s| format!("{s} {}", db.stat(m, s, Some(&b.id))))
                    .collect::<Vec<_>>()
                    .join("  "),
                if wide {
                    String::new()
                } else {
                    format!("\n\nFrozen pre-battle reference:\n{reference}")
                }
            ),
        );
        recent_notes_panel(f, r[current + 1], db, &m.unit_id, Some(&m.id));
    }
}
fn post_battle(f: &mut Frame, app: &mut App, db: &Db, area: Rect) {
    let Some(b) = app.battle(db).cloned() else {
        text(
            f,
            area,
            "Post-battle",
            "No active after-action. End a battle to resolve its outcomes.",
        );
        return;
    };
    let regions = Layout::horizontal([Constraint::Length(23), Constraint::Min(30)]).split(area);
    let selected = app.post_stage(db).cloned();
    let current = app
        .steps(db)
        .iter()
        .find(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
        .map(|s| s.name.clone());
    let rows = app
        .steps(db)
        .iter()
        .map(|s| {
            vec![format!(
                "{}{}",
                if current.as_deref() == Some(&s.name) {
                    "→"
                } else if s.status == "completed" {
                    "✓"
                } else if s.status == "skipped" {
                    "–"
                } else {
                    " "
                },
                s.name
            )]
        })
        .collect();
    table(
        f,
        app,
        regions[0],
        if app.post_focus == PostFocus::Stages {
            "STAGES · ↑↓ browse"
        } else {
            "Stages · Tab to focus"
        },
        vec!["After action"],
        rows,
        vec![Constraint::Min(18)],
    );
    let selected_model = app
        .post_targets(db)
        .get(app.post_selected)
        .and_then(|t| t.member.as_ref())
        .and_then(|id| db.members.iter().find(|m| m.id == *id))
        .map(|m| (m.unit_id.clone(), m.id.clone()));
    let content = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(3),
        Constraint::Length(if selected_model.is_some() { 5 } else { 4 }),
    ])
    .split(regions[1]);
    let name = selected.as_ref().map(|s| s.name.as_str()).unwrap_or("");
    let hint = match name {
        "injuries" => {
            "Choose an injured member here; I records recovery, death or a lasting injury."
        }
        "experience" => {
            "Choose a unit here. X adds shared XP; U records the plus-up consequence. No dice entry needed."
        }
        "exploration" => {
            "Enter the exploration consequence and rewards. Physical rolls stay at the table."
        }
        "wyrdstone" => {
            "Enter shards sold and total gold agreed by your campaign. Zero shards records no sale."
        }
        "veterans" => "V records available veteran experience directly from the physical roll.",
        "rare" => {
            "One search per Hero not taken out of action. S records the item and found/failed; P buys one found item."
        }
        "dramatis" => "Choose a Hero; S manages searches, H hires the character found.",
        "recruitment" => {
            "Hire, buy common items or settle hired-sword upkeep without changing tabs."
        }
        "equipment" => "Choose a unit; E equips it, R returns its loadout to the stash.",
        _ => "Inspect final rating and finish the sequence. Checkpoints are created automatically.",
    };
    let state = if b.status != "post-battle" {
        "End the battle before recording after-action outcomes.".into()
    } else if app.post_unlocked(db) {
        hint.into()
    } else if current.is_none() {
        "All stages resolved. B finishes the battle and saves the checkpoint.".into()
    } else {
        format!(
            "Read only: {}. Select a pending stage to act or skip; next unfinished is {}.",
            selected
                .as_ref()
                .map(|s| s.status.as_str())
                .unwrap_or("pending"),
            current.as_deref().unwrap_or("previous stages")
        )
    };
    text(f, content[0], format!("{} · {}", name, b.name), state);
    let targets = app.post_targets(db);
    let n = targets.len();
    app.post_selected = app.post_selected.min(n.saturating_sub(1));
    app.post_table.select((n > 0).then_some(app.post_selected));
    let headers =
        if name == "experience" || name == "rating" || name == "equipment" || name == "recruitment"
        {
            vec!["Unit", "XP", "Status / equipment"]
        } else {
            vec!["Member", "Type", "Status / searches"]
        };
    let title = if app.post_focus == PostFocus::Targets {
        "TARGETS · ↑↓ selects · Enter acts"
    } else {
        "Targets · Tab to focus"
    };
    let title = if app.filter.is_empty() {
        title.to_string()
    } else {
        format!("{title} · {}", app.filter)
    };
    let block = block(title);
    let inner = block.inner(content[1]);
    f.render_stateful_widget(
        Table::new(
            targets.iter().map(|t| Row::new(t.cells.clone())),
            [
                Constraint::Percentage(32),
                Constraint::Length(10),
                Constraint::Min(12),
            ],
        )
        .header(Row::new(headers).style(Style::default().fg(ACCENT)))
        .highlight_symbol("› ")
        .row_highlight_style(focus())
        .block(block),
        content[1],
        &mut app.post_table,
    );
    for y in 0..inner.height.saturating_sub(1) {
        let i = app.post_table.offset() + y as usize;
        if i < n {
            app.hits.push((
                Rect::new(inner.x, inner.y + 1 + y, inner.width, 1),
                Hit::PostTarget(i),
            ));
        }
    }
    if targets.is_empty() {
        let msg = match name {
            "injuries" => "No unresolved injuries. C finishes this stage.",
            "wyrdstone" => "T opens the shard sale form.",
            "veterans" => "V enters the available veteran XP.",
            _ => "Record the outcome using the actions above.",
        };
        f.render_widget(
            Paragraph::new(msg).wrap(Wrap { trim: false }),
            Rect::new(
                inner.x,
                inner.y + 2,
                inner.width,
                inner.height.saturating_sub(2),
            ),
        );
    }
    let pending: usize = db
        .units
        .iter()
        .filter(|u| u.warband_id == b.warband_id)
        .map(|u| db.pending(u).len())
        .sum();
    let shards = db
        .items
        .iter()
        .find(|i| {
            i.profile_id
                == db
                    .warbands
                    .iter()
                    .find(|w| w.id == b.warband_id)
                    .unwrap()
                    .profile_id
                && i.name == "Wyrdstone"
        })
        .map_or(0, |i| db.owned(&b.warband_id, &i.id));
    let target = targets
        .get(app.post_selected)
        .map(|t| t.cells.join(" · "))
        .unwrap_or_default();
    let panels = Layout::horizontal(if selected_model.is_some() {
        vec![Constraint::Percentage(50), Constraint::Percentage(50)]
    } else {
        vec![Constraint::Percentage(100)]
    })
    .split(content[2]);
    text(
        f,
        panels[0],
        "Selected target / campaign totals",
        format!(
            "{}\n{} plus-up(s) pending · {} shards · {} gc · {} veteran XP",
            target,
            pending,
            shards,
            db.gold(&b.warband_id),
            b.veteran_budget
        ),
    );
    if let Some((unit, member)) = selected_model {
        recent_notes_panel(f, panels[1], db, &unit, Some(&member));
    }
    app.hits.push((
        Rect::new(regions[1].x, regions[1].y, regions[1].width, 1),
        Hit::PostFocus,
    ));
}
fn workbench(f: &mut Frame, app: &mut App, area: Rect) {
    let title = if app.ai.receiver.is_some() {
        "AI assistant · waiting for provider · no changes applied"
    } else if app.ai.reviewed.is_some() {
        "AI assistant · validated proposal · explicit approval required"
    } else {
        "AI assistant workbench · proposals require player approval"
    };
    let default = "N: ask the assistant. The configured provider receives the selected warband, pinned rules, notes and battle data.\n\nThe assistant may propose any player action. Every action is listed before execution. R validates the whole plan in a disposable copy and shows actual data changes. Y opens an approval confirmation; Y in that confirmation applies all actions. D rejects. Changes to campaign data invalidate approval.\n\nConfigure MORDTUI_AI_MODEL, MORDTUI_AI_KEY and optionally MORDTUI_AI_ENDPOINT. No provider is contacted until you send a request.";
    let content = format!(
        "REQUEST: {}\n\n{}",
        app.ai.prompt,
        if app.ai.report.is_empty() {
            default
        } else {
            &app.ai.report
        }
    );
    let b = block(title);
    let inner = b.inner(area);
    let paragraph = Paragraph::new(content).wrap(Wrap { trim: false });
    app.ai.max_scroll = paragraph
        .line_count(inner.width)
        .saturating_sub(inner.height as usize)
        .min(u16::MAX as usize) as u16;
    app.ai.scroll = app.ai.scroll.min(app.ai.max_scroll);
    f.render_widget(paragraph.scroll((app.ai.scroll, 0)).block(b), area);
}
fn journal(f: &mut Frame, app: &mut App, db: &Db, area: Rect) {
    let (headers, widths) = match app.journal {
        Journal::Ledger => (
            vec!["Date", "Category", "Item", "Qty", "Gold", "Notes"],
            vec![
                Constraint::Length(10),
                Constraint::Length(14),
                Constraint::Length(16),
                Constraint::Length(4),
                Constraint::Length(7),
                Constraint::Min(12),
            ],
        ),
        Journal::Notes => (
            vec!["Date", "Attached to", "Comment"],
            vec![
                Constraint::Length(10),
                Constraint::Length(20),
                Constraint::Min(25),
            ],
        ),
        Journal::History => (
            vec!["#", "Date", "Action", "Reason"],
            vec![
                Constraint::Length(5),
                Constraint::Length(10),
                Constraint::Length(22),
                Constraint::Min(25),
            ],
        ),
        Journal::Snapshots => (
            vec!["Checkpoint", "Captured state"],
            vec![Constraint::Min(25), Constraint::Min(35)],
        ),
        Journal::Injuries => (
            vec!["Date", "Warrior", "Injury / outcome", "Change", "Status"],
            vec![
                Constraint::Length(10),
                Constraint::Min(14),
                Constraint::Min(24),
                Constraint::Length(8),
                Constraint::Length(14),
            ],
        ),
    };
    let title = format!("{:?} · V changes journal · Enter expands", app.journal);
    let data = app.journal_rows(db);
    let empty_injuries = app.journal == Journal::Injuries && data.is_empty();
    let parts = Layout::vertical([Constraint::Min(3), Constraint::Length(5)]).split(area);
    let model_notes = app.journal == Journal::Injuries;
    table(
        f,
        app,
        if model_notes { parts[0] } else { area },
        &title,
        headers,
        data,
        widths,
    );
    if model_notes
        && let Some(m) = app
            .selection(db)
            .member
            .and_then(|id| db.members.iter().find(|m| m.id == id))
    {
        recent_notes_panel(f, parts[1], db, &m.unit_id, Some(&m.id));
    }
    if empty_injuries {
        f.render_widget(
            Paragraph::new("No recorded injuries or unresolved casualties.")
                .wrap(Wrap { trim: false }),
            Rect::new(area.x + 1, area.y + 3, area.width.saturating_sub(2), 2),
        );
    }
}
fn overlay(f: &mut Frame, app: &mut App, store: &Store, area: Rect) {
    let wb = app.wb.clone();
    match &mut app.overlay {
        Overlay::None => return,
        Overlay::AiApproval { scroll } => {
            let r = centered(area, 112, area.height - 2);
            f.render_widget(Clear, r);
            let b = block("PLAYER APPROVAL · all proposed actions · Esc cancels");
            let inner = b.inner(r);
            f.render_widget(b, r);
            let parts = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).split(inner);
            let report = app
                .ai
                .reviewed
                .as_ref()
                .map(|r| r.report.as_str())
                .unwrap_or("No validated proposal.");
            f.render_widget(
                Paragraph::new(report)
                    .wrap(Wrap { trim: false })
                    .scroll((*scroll, 0)),
                parts[0],
            );
            app.hits.clear();
            line_buttons(
                f,
                app,
                parts[1],
                &[
                    ("Y Approve and apply ALL actions", Hit::AiApprove),
                    ("Esc Cancel", Hit::Cancel),
                ],
            );
        }
        Overlay::Detail {
            title,
            text,
            scroll,
        } => {
            let r = centered(area, 110, area.height - 2);
            f.render_widget(Clear, r);
            f.render_widget(
                Paragraph::new(text.clone())
                    .wrap(Wrap { trim: false })
                    .scroll((*scroll, 0))
                    .block(block(format!("{title} · ↑↓ scroll · Esc closes"))),
                r,
            );
            app.hits.clear();
        }
        Overlay::Command(input) | Overlay::Filter(input) => {
            let r = centered(area, 100, 5);
            f.render_widget(Clear, r);
            f.render_widget(
                Paragraph::new(input.clone()).block(block("Enter submits · Esc cancels")),
                r,
            );
            app.hits.clear();
            f.set_cursor_position((
                r.x + 1 + input.chars().count().min(r.width as usize - 3) as u16,
                r.y + 1,
            ));
        }
        Overlay::Menu(_) => {
            let query = if let Overlay::Menu(m) = &app.overlay {
                m.query.clone()
            } else {
                String::new()
            };
            let choices = app.filtered_actions(&store.db, &query);
            let r = centered(area, 74, area.height - 4);
            f.render_widget(Clear, r);
            let inner = block("Actions · type to search · Enter selects · Esc cancels").inner(r);
            f.render_widget(
                block("Actions · type to search · Enter selects · Esc cancels"),
                r,
            );
            f.render_widget(
                Paragraph::new(format!("Search: {query}")),
                Rect::new(inner.x, inner.y, inner.width, 2),
            );
            let list_area = Rect::new(
                inner.x,
                inner.y + 2,
                inner.width,
                inner.height.saturating_sub(2),
            );
            let Overlay::Menu(m) = &mut app.overlay else {
                return;
            };
            m.index = m.index.min(choices.len().saturating_sub(1));
            m.state.select(if choices.is_empty() {
                None
            } else {
                Some(m.index)
            });
            f.render_stateful_widget(
                List::new(
                    choices
                        .iter()
                        .map(|a| ListItem::new(a.label()))
                        .collect::<Vec<_>>(),
                )
                .highlight_style(focus())
                .highlight_symbol("› "),
                list_area,
                &mut m.state,
            );
            app.hits.clear();
            for y in 0..list_area.height {
                let i = m.state.offset() + y as usize;
                if i < choices.len() {
                    app.hits.push((
                        Rect::new(list_area.x, list_area.y + y, list_area.width, 1),
                        Hit::MenuRow(i),
                    ));
                }
            }
            if choices.is_empty() {
                f.render_widget(Paragraph::new("No matching actions"), list_area);
            }
        }
        Overlay::Form(form) => {
            let r = centered(area, 104, area.height - 2);
            f.render_widget(Clear, r);
            let b = block(format!(
                "{} · {}",
                if form.reviewing { "Review" } else { "Form" },
                form.action.label()
            ));
            let inner = b.inner(r);
            f.render_widget(b, r);
            app.hits.clear();
            let parts = Layout::vertical([
                Constraint::Min(3),
                Constraint::Length(3),
                Constraint::Length(2),
            ])
            .split(inner);
            if form.reviewing {
                let mut lines = vec![
                    Line::from(form.impact(store, wb.as_deref()).unwrap_or_default())
                        .style(Style::default().fg(ACCENT)),
                    Line::from(""),
                ];
                for field in &form.fields {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}: ", field.label),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(field.display()),
                    ]));
                }
                f.render_widget(
                    Paragraph::new(lines)
                        .wrap(Wrap { trim: false })
                        .scroll((form.review_scroll, 0)),
                    parts[0],
                );
                f.render_widget(
                    Paragraph::new(
                        "↑↓ / PgUp/PgDn scroll · Enter applies once · Esc returns to editing.",
                    )
                    .wrap(Wrap { trim: false }),
                    parts[1],
                );
                let y = parts[2].y;
                let apply = Rect::new(parts[2].x, y, 22, 1);
                let cancel = Rect::new(parts[2].x + 25, y, 22, 1);
                f.render_widget(
                    Paragraph::new(if form.action == Action::AiRequest {
                        "[ Enter: Send request ]"
                    } else {
                        "[ Enter: Apply ]"
                    })
                    .style(focus()),
                    apply,
                );
                f.render_widget(
                    Paragraph::new("[ Esc: Back ]").style(Style::default().fg(ACCENT)),
                    cancel,
                );
                app.hits.push((apply, Hit::Apply));
                app.hits.push((cancel, Hit::Cancel));
            } else {
                let visible = (parts[0].height / 2).max(1) as usize;
                if form.focus < form.offset {
                    form.offset = form.focus;
                } else if form.focus >= form.offset + visible {
                    form.offset = form.focus + 1 - visible;
                }
                for (row, index) in (form.offset..form.fields.len()).take(visible).enumerate() {
                    let field = &form.fields[index];
                    let rect =
                        Rect::new(parts[0].x, parts[0].y + row as u16 * 2, parts[0].width, 1);
                    let regions = Layout::horizontal([Constraint::Length(27), Constraint::Min(10)])
                        .split(rect);
                    f.render_widget(
                        Paragraph::new(format!(
                            "{} {}",
                            if index == form.focus { "›" } else { " " },
                            field.label
                        )),
                        regions[0],
                    );
                    let start = if index == form.focus && matches!(field.kind, Kind::Text) {
                        form.cursor
                            .saturating_sub(regions[1].width.saturating_sub(1) as usize)
                    } else {
                        0
                    };
                    f.render_widget(
                        Paragraph::new(if field.display().is_empty() {
                            " ".into()
                        } else {
                            field.display().chars().skip(start).collect::<String>()
                        })
                        .style(if index == form.focus {
                            focus()
                        } else {
                            Style::default()
                        }),
                        regions[1],
                    );
                    app.hits.push((rect, Hit::Field(index)));
                    if index == form.focus && matches!(field.kind, Kind::Text) {
                        let cursor = form
                            .cursor
                            .saturating_sub(start)
                            .min(regions[1].width.saturating_sub(1) as usize);
                        f.set_cursor_position((regions[1].x + cursor as u16, regions[1].y));
                    }
                }
                let explanation = if form.error.is_empty() {
                    let field = &form.fields[form.focus];
                    format!(
                        "Field {}/{} · {}{}",
                        form.focus + 1,
                        form.fields.len(),
                        if field.replace_on_input {
                            "Type to replace; arrows to edit. "
                        } else {
                            ""
                        },
                        field.hint
                    )
                } else {
                    format!("Not saved: {}", form.error)
                };
                f.render_widget(
                    Paragraph::new(explanation)
                        .wrap(Wrap { trim: false })
                        .style(Style::default().fg(if form.error.is_empty() {
                            Color::DarkGray
                        } else {
                            Color::Yellow
                        })),
                    parts[1],
                );
                let review = Rect::new(parts[2].x, parts[2].y, 27, 1);
                let cancel = Rect::new(parts[2].x + 30, parts[2].y, 24, 1);
                f.render_widget(
                    Paragraph::new("[ F10 / Ctrl-S: Review ]").style(focus()),
                    review,
                );
                f.render_widget(
                    Paragraph::new("[ Esc: Cancel ]").style(Style::default().fg(ACCENT)),
                    cancel,
                );
                app.hits.push((review, Hit::Review));
                app.hits.push((cancel, Hit::Cancel));
            }
        }
    }
    if let Some(p) = &mut app.picker
        && let Overlay::Form(form) = &app.overlay
    {
        let r = centered(area, 90, (area.height - 6).min(22));
        f.render_widget(Clear, r);
        let b = block(format!("{} · type to search", form.fields[p.field].label));
        let inner = b.inner(r);
        f.render_widget(b, r);
        let parts = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);
        f.render_widget(Paragraph::new(format!("Search: {}", p.query)), parts[0]);
        let cs = picker_choices(form, p);
        p.index = p.index.min(cs.len().saturating_sub(1));
        p.state
            .select(if cs.is_empty() { None } else { Some(p.index) });
        f.render_stateful_widget(
            List::new(
                cs.iter()
                    .map(|c| {
                        ListItem::new(if let Some(checked) = &p.checked {
                            format!(
                                "[{}] {}",
                                if checked.contains(&c.id) { "x" } else { " " },
                                c.label
                            )
                        } else {
                            c.label.clone()
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .highlight_style(focus())
            .highlight_symbol("› "),
            parts[1],
            &mut p.state,
        );
        if cs.is_empty() {
            f.render_widget(Paragraph::new("No matching choices"), parts[1]);
        }
        f.render_widget(
            Paragraph::new(if p.checked.is_some() {
                "[Enter: Done] · Space/click toggle · Esc cancel · Ctrl-A all · Ctrl-U clear"
            } else {
                "↑↓ choose · Enter select · Esc back"
            }),
            parts[2],
        );
        app.hits.clear();
        if p.checked.is_some() {
            app.hits.push((
                Rect::new(
                    parts[2].x,
                    parts[2].y,
                    13.min(parts[2].width),
                    parts[2].height,
                ),
                Hit::PickerDone,
            ));
        }
        for y in 0..parts[1].height {
            let i = p.state.offset() + y as usize;
            if i < cs.len() {
                app.hits.push((
                    Rect::new(parts[1].x, parts[1].y + y, parts[1].width, 1),
                    Hit::PickerRow(i),
                ));
            }
        }
    }
}

pub(super) fn comparison(value: &Value) -> String {
    let a = &value["left"];
    let b = &value["right"];
    let mut text = String::from("Checkpoint → current state\n\n");
    for (key, label) in [
        ("gold", "Gold"),
        ("rating", "Rating"),
        ("members", "Members"),
        ("pending_advances", "Pending advances"),
    ] {
        text.push_str(&format!(
            "{}: {} → {}\n",
            label, a["summary"][key], b["summary"][key]
        ));
    }
    text.push_str("\nEquipment stock\n");
    if let Some(items) = b["inventory"].as_array() {
        for item in items {
            let name = item["name"].as_str().unwrap_or("Equipment");
            let old = a["inventory"]
                .as_array()
                .and_then(|rows| rows.iter().find(|i| i["name"] == item["name"]));
            text.push_str(&format!(
                "{}: owned {} → {}, stash {} → {}\n",
                name,
                old.map(|i| &i["owned"]).unwrap_or(&Value::Null),
                item["owned"],
                old.map(|i| &i["stash"]).unwrap_or(&Value::Null),
                item["stash"]
            ));
        }
    }
    text.push_str("\nMembers\n");
    if let Some(rows) = b["roster"].as_array() {
        for member in rows {
            let old = a["roster"]
                .as_array()
                .and_then(|rows| rows.iter().find(|m| m["id"] == member["id"]));
            text.push_str(&format!(
                "{} · {} → {} · XP {} → {}\n",
                member["name"].as_str().unwrap_or("Member"),
                old.and_then(|m| m["status"].as_str())
                    .unwrap_or("not present"),
                member["status"].as_str().unwrap_or(""),
                old.map(|m| m["unit"]["starting_xp"].as_u64().unwrap_or(0)
                    + m["unit"]["earned_xp"].as_u64().unwrap_or(0))
                    .unwrap_or(0),
                member["unit"]["starting_xp"].as_u64().unwrap_or(0)
                    + member["unit"]["earned_xp"].as_u64().unwrap_or(0)
            ));
        }
    }
    text
}
