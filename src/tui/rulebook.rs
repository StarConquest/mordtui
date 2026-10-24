use super::*;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem},
};

pub(super) const SECTIONS: [&str; 7] = [
    "Profile defaults",
    "Warrior types",
    "Equipment lists",
    "Equipment",
    "Skills / spells",
    "XP progression",
    "Starting gear",
];
pub(super) const KINDS: [&str; 7] = [
    "profile",
    "warrior",
    "equipment-list",
    "equipment",
    "ability",
    "progression",
    "starting-gear",
];
#[derive(Default)]
pub(super) struct State {
    pub profile: Option<String>,
    pub section: usize,
    pub records_focus: bool,
}
pub(super) struct RuleRow {
    pub id: String,
    pub cells: Vec<String>,
}
impl App {
    pub(super) fn rule_profile<'a>(&self, db: &'a Db) -> Option<&'a Profile> {
        self.rules
            .profile
            .as_deref()
            .and_then(|p| db.profiles.iter().find(|r| r.id == p))
            .or_else(|| {
                self.warband(db)
                    .and_then(|w| db.profiles.iter().find(|p| p.id == w.profile_id))
            })
            .or_else(|| db.profiles.first())
    }
    pub(super) fn rule_rows(&self, db: &Db) -> Vec<RuleRow> {
        let Some(p) = self.rule_profile(db) else {
            return vec![];
        };
        let rows: Vec<RuleRow> = match self.rules.section {
            0 => vec![RuleRow {
                id: p.id.clone(),
                cells: vec![
                    p.name.clone(),
                    p.classification.clone(),
                    format!(
                        "{}+{} gc · {} models · {} Heroes",
                        p.starting_gold, p.bonus_gold, p.max_members, p.max_heroes
                    ),
                ],
            }],
            1 => db
                .types
                .iter()
                .filter(|r| r.profile_id == p.id)
                .map(|r| RuleRow {
                    id: r.id.clone(),
                    cells: vec![
                        r.name.clone(),
                        r.role.clone(),
                        format!(
                            "{} gc · {} upkeep · {} XP · group {}",
                            r.cost, r.upkeep, r.starting_xp, r.group_max
                        ),
                    ],
                })
                .collect(),
            2 => db
                .equipment_lists
                .iter()
                .filter(|r| r.profile_id == p.id)
                .map(|r| RuleRow {
                    id: r.id.clone(),
                    cells: vec![
                        r.name.clone(),
                        "list".into(),
                        format!(
                            "{} items · {} types",
                            db.equipment_list_items
                                .iter()
                                .filter(|i| i.list_id == r.id)
                                .count(),
                            db.equipment_list_assignments
                                .iter()
                                .filter(|a| a.list_id == r.id
                                    && db.types.iter().any(|t| t.id == a.entity_id))
                                .count()
                        ),
                    ],
                })
                .collect(),
            3 => db
                .items
                .iter()
                .filter(|r| r.profile_id == p.id)
                .map(|r| RuleRow {
                    id: r.id.clone(),
                    cells: vec![
                        r.name.clone(),
                        r.category.clone(),
                        format!("{} gc · rarity {}", r.cost, r.rarity),
                    ],
                })
                .collect(),
            4 => db
                .abilities
                .iter()
                .filter(|r| r.profile_id == p.id)
                .map(|r| RuleRow {
                    id: r.id.clone(),
                    cells: vec![r.name.clone(), r.kind.clone(), r.description.clone()],
                })
                .collect(),
            5 => ["hero", "henchman", "hired", "dramatis"]
                .into_iter()
                .map(|role| {
                    let mut xp = db
                        .thresholds
                        .iter()
                        .filter(|t| t.profile_id == p.id && t.role == role)
                        .map(|t| t.xp)
                        .collect::<Vec<_>>();
                    xp.sort();
                    RuleRow {
                        id: role.into(),
                        cells: vec![
                            role.into(),
                            "XP".into(),
                            if xp.is_empty() {
                                "No advance thresholds".into()
                            } else {
                                xp.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
                            },
                        ],
                    }
                })
                .collect(),
            _ => db
                .permissions
                .iter()
                .filter(|r| {
                    r.kind == "starting-item"
                        && db
                            .types
                            .iter()
                            .any(|t| t.id == r.type_id && t.profile_id == p.id)
                })
                .map(|r| RuleRow {
                    id: r.id.clone(),
                    cells: vec![
                        db.types
                            .iter()
                            .find(|t| t.id == r.type_id)
                            .unwrap()
                            .name
                            .clone(),
                        format!("×{}", r.quantity),
                        db.items
                            .iter()
                            .find(|i| i.id == r.target_id)
                            .map(|i| i.name.clone())
                            .unwrap_or_default(),
                    ],
                })
                .collect(),
        };
        rows.into_iter()
            .filter(|r| {
                r.cells
                    .join(" ")
                    .to_lowercase()
                    .contains(&self.filter.to_lowercase())
            })
            .collect()
    }
    fn rule_action(&self) -> Action {
        [
            Action::RuleProfile,
            Action::RuleWarrior,
            Action::RuleList,
            Action::RuleItem,
            Action::RuleAbility,
            Action::RuleProgression,
            Action::RuleStartingGear,
        ][self.rules.section]
    }
    pub(super) fn rule_section(&mut self, section: usize) {
        self.rules.section = section.min(SECTIONS.len() - 1);
        self.rules.records_focus = false;
        self.selected[RULES_TAB] = 0;
        self.filter.clear();
    }
    pub(super) fn rule_new(&mut self, store: &Store, cli: &Cli) {
        let action = if self.rule_profile(&store.db).is_none() {
            Action::RuleProfile
        } else {
            self.rule_action()
        };
        self.open(action, store, cli);
    }
    pub(super) fn rule_edit(&mut self, store: &Store, cli: &Cli) {
        if self
            .rule_rows(&store.db)
            .get(self.selected[RULES_TAB])
            .is_none()
        {
            self.message = "N creates a definition in this section.".into();
            return;
        }
        self.open_selected(self.rule_action(), store, cli, self.selection(&store.db));
    }
    pub(super) fn rule_delete(&mut self, store: &Store, cli: &Cli) {
        if self.rules.section == 5 {
            self.message =
                "Edit the role and clear its XP thresholds to remove progression.".into();
            return;
        }
        if self
            .rule_rows(&store.db)
            .get(self.selected[RULES_TAB])
            .is_none()
        {
            self.message = "Choose a definition to remove.".into();
            return;
        }
        self.open_selected(Action::RuleRemove, store, cli, self.selection(&store.db));
    }
    fn rule_detail_text(&self, store: &Store) -> String {
        if self
            .rule_rows(&store.db)
            .get(self.selected[RULES_TAB])
            .is_none()
        {
            return "No definitions in this section. N adds one.".into();
        }
        Form::new(
            self.rule_action(),
            store,
            self.wb.as_deref(),
            &self.selection(&store.db),
        )
        .fields
        .iter()
        .filter(|f| !matches!(f.key, "date" | "reason"))
        .map(|f| format!("{}: {}", f.label, f.display()))
        .collect::<Vec<_>>()
        .join("\n")
    }
    pub(super) fn rule_details(&mut self, store: &Store) {
        self.overlay = Overlay::Detail {
            title: format!("Rules · {}", SECTIONS[self.rules.section]),
            text: self.rule_detail_text(store),
            scroll: 0,
        };
    }
    pub(super) fn rule_key(&mut self, key: KeyEvent, store: &mut Store, cli: &Cli) -> bool {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => self.rules.records_focus = !self.rules.records_focus,
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                let delta = match key.code {
                    KeyCode::Up => -1,
                    KeyCode::PageUp => -8,
                    KeyCode::PageDown => 8,
                    _ => 1,
                };
                if self.rules.records_focus {
                    self.move_row(&store.db, delta);
                } else {
                    self.rule_section(
                        self.rules
                            .section
                            .saturating_add_signed(delta)
                            .min(SECTIONS.len() - 1),
                    );
                }
            }
            KeyCode::Home | KeyCode::End => {
                if self.rules.records_focus {
                    self.selected[RULES_TAB] = if key.code == KeyCode::Home {
                        0
                    } else {
                        self.rule_rows(&store.db).len().saturating_sub(1)
                    };
                } else {
                    self.rule_section(if key.code == KeyCode::Home {
                        0
                    } else {
                        SECTIONS.len() - 1
                    });
                }
            }
            KeyCode::Enter => {
                if self.rules.records_focus {
                    self.rule_edit(store, cli);
                } else {
                    self.rules.records_focus = true;
                }
            }
            KeyCode::Char('p') => self.open(Action::RuleSelect, store, cli),
            KeyCode::Char('n') => self.rule_new(store, cli),
            KeyCode::Char('e') => self.rule_edit(store, cli),
            KeyCode::Char('v') => self.rule_details(store),
            KeyCode::Char('d') => self.rule_delete(store, cli),
            KeyCode::Char('c') => self.open(Action::RuleCopy, store, cli),
            KeyCode::Char('w') => self.open(Action::RuleCreateWarband, store, cli),
            KeyCode::Char(c)
                if ![
                    'q', 'a', '/', '?', ':', 'b', '1', '2', '3', '4', '5', '6', '7', '8',
                ]
                .contains(&c) => {}
            _ => return false,
        }
        true
    }
    pub(super) fn draw_rules(&mut self, f: &mut Frame, store: &Store, area: Rect) {
        let db = &store.db;
        let Some(profile) = self.rule_profile(db).cloned() else {
            views::text(
                f,
                area,
                "Program a warband from scratch",
                "N: create a rules profile.\nThen define Heroes, Henchmen, Hired Swords, equipment lists, items, skills and XP progression.\n\nEverything is entered through forms; no rules files or existing campaign are required.\nW creates a warband from the profile when its warrior types are ready.\n\nPlayers roll dice and adjudicate descriptive rules at the table.",
            );
            return;
        };
        let parts = Layout::vertical([Constraint::Length(3), Constraint::Min(8)]).split(area);
        views::text(
            f,
            parts[0],
            format!("{} · {} · P selects profile", profile.name, profile.version),
            format!(
                "{} · {} warriors · {} lists · {} items · {} skills",
                profile.faction,
                db.types
                    .iter()
                    .filter(|t| t.profile_id == profile.id)
                    .count(),
                db.equipment_lists
                    .iter()
                    .filter(|t| t.profile_id == profile.id)
                    .count(),
                db.items
                    .iter()
                    .filter(|t| t.profile_id == profile.id)
                    .count(),
                db.abilities
                    .iter()
                    .filter(|t| t.profile_id == profile.id)
                    .count()
            ),
        );
        let panes =
            Layout::horizontal([Constraint::Length(21), Constraint::Min(30)]).split(parts[1]);
        let b = Block::default()
            .borders(Borders::ALL)
            .title(if self.rules.records_focus {
                "Sections · Tab focus"
            } else {
                "SECTIONS · ↑↓"
            });
        let inner = b.inner(panes[0]);
        f.render_widget(
            List::new(
                SECTIONS
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        ListItem::new(format!(
                            "{} {}",
                            if i == self.rules.section { "›" } else { " " },
                            s
                        ))
                        .style(if i == self.rules.section {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default()
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .block(b),
            panes[0],
        );
        for i in 0..SECTIONS.len().min(inner.height as usize) {
            self.hits.push((
                Rect::new(inner.x, inner.y + i as u16, inner.width, 1),
                Hit::RuleSection(i),
            ));
        }
        let rows = self.rule_rows(db);
        let data = rows.iter().map(|r| r.cells.clone()).collect();
        let right =
            Layout::vertical([Constraint::Percentage(52), Constraint::Min(5)]).split(panes[1]);
        views::table(
            f,
            self,
            right[0],
            if self.rules.records_focus {
                "DEFINITIONS · Enter edits"
            } else {
                "Definitions · Tab focus"
            },
            vec!["Name", "Kind", "Details"],
            data,
            vec![
                Constraint::Percentage(35),
                Constraint::Length(9),
                Constraint::Min(10),
            ],
        );
        views::text(
            f,
            right[1],
            "Selected rules · V expands",
            self.rule_detail_text(store),
        );
    }
}
