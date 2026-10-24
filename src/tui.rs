//! Interactive terminal frontend. Session forms construct typed engine commands.
mod catalog;
mod forms;
mod injuries;
mod post;
mod rulebook;
mod workbench;
use post::PostFocus;
mod views;
use crate::{
    cli::{BattleCommand, Cli, Command},
    engine,
    model::*,
    store::Store,
};
use anyhow::{Result, ensure};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseEventKind,
    },
    execute,
};
use forms::{Action, Choice, Form, Kind, Selection};
use ratatui::{
    layout::Rect,
    widgets::{ListState, TableState},
};
use serde_json::Value;
use std::io::IsTerminal;

const TABS: &[&str] = &[
    "Overview",
    "Warband",
    "Stash",
    "Battle",
    "Post-battle",
    "Journal",
    "Rules",
    #[cfg(feature = "ai")]
    "AI assistant",
];
const RULES_TAB: usize = 6;
const AI_TAB: usize = 7;
#[derive(Clone, Copy, PartialEq, Debug)]
enum Journal {
    Ledger,
    Notes,
    History,
    Snapshots,
    Injuries,
}
#[derive(Clone, Copy, Debug)]
enum Hit {
    Menu,
    Tab(usize),
    Row(usize),
    Action(Action),
    MenuRow(usize),
    PickerRow(usize),
    PickerDone,
    Field(usize),
    Review,
    Apply,
    Cancel,
    Journal,
    Quick(&'static str),
    Details,
    BattleProfile,
    BattleLifecycle,
    PostTarget(usize),
    PostFocus,
    AiReview,
    AiApprove,
    AiReject,
    RuleSection(usize),
    RuleNew,
    RuleEdit,
    RuleDetails,
    RuleDelete,
}
struct Picker {
    field: usize,
    query: String,
    index: usize,
    state: ListState,
    checked: Option<Vec<String>>,
}
struct Menu {
    query: String,
    index: usize,
    state: ListState,
}
enum Overlay {
    None,
    Form(Form),
    Menu(Menu),
    Detail {
        title: String,
        text: String,
        scroll: u16,
    },
    Command(String),
    Filter(String),
    AiApproval {
        scroll: u16,
    },
}
struct App {
    tab: usize,
    selected: [usize; 8],
    tables: [TableState; 8],
    filter: String,
    post_focus: PostFocus,
    post_selected: usize,
    post_table: TableState,
    wb: Option<String>,
    journal: Journal,
    overlay: Overlay,
    picker: Option<Picker>,
    message: String,
    hits: Vec<(Rect, Hit)>,
    suspended_form: Option<Form>,
    ai: workbench::Workbench,
    rules: rulebook::State,
}
impl App {
    fn new(store: &Store, cli: &Cli) -> Self {
        let mut app = Self {
            tab: 1,
            selected: [0; 8],
            tables: std::array::from_fn(|_| TableState::default()),
            filter: String::new(),
            post_focus: PostFocus::Stages,
            post_selected: 0,
            post_table: TableState::default(),
            wb: engine::selected(&store.db, cli).ok(),
            journal: Journal::History,
            overlay: Overlay::None,
            picker: None,
            message: "Choose a unit. H Hire · P Purchase · E Equip · X Experience · A All actions"
                .into(),
            hits: vec![],
            suspended_form: None,
            ai: workbench::Workbench::default(),
            rules: rulebook::State::default(),
        };
        app.selected[4] = app
            .steps(&store.db)
            .iter()
            .position(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
            .unwrap_or_else(|| app.steps(&store.db).len().saturating_sub(1));
        app
    }
    fn warband<'a>(&self, db: &'a Db) -> Option<&'a Warband> {
        self.wb
            .as_ref()
            .and_then(|id| db.warbands.iter().find(|w| w.id == *id))
    }
    fn battle<'a>(&self, db: &'a Db) -> Option<&'a Battle> {
        self.wb.as_ref().and_then(|id| {
            db.battles.iter().rev().find(|b| {
                b.warband_id == *id && !matches!(b.status.as_str(), "complete" | "abandoned")
            })
        })
    }
    fn units<'a>(&self, db: &'a Db) -> Vec<&'a Unit> {
        let mut rows: Vec<_> = db
            .units
            .iter()
            .filter(|u| Some(&u.warband_id) == self.wb.as_ref())
            .filter(|u| db.members.iter().any(|m| m.unit_id == u.id && active(m)))
            .filter(|u| {
                let text =
                    format!("{} {} {}", u.name, db.typ(&u.type_id).name, u.role).to_lowercase();
                text.contains(&self.filter.to_lowercase())
            })
            .collect();
        rows.sort_by_key(|u| match u.role.as_str() {
            "hero" => 0,
            "henchman" => 1,
            _ => 2,
        });
        rows
    }
    fn items<'a>(&self, db: &'a Db) -> Vec<&'a Item> {
        db.items
            .iter()
            .filter(|i| {
                self.warband(db)
                    .is_some_and(|w| i.profile_id == w.profile_id)
                    && format!("{} {}", i.name, i.category)
                        .to_lowercase()
                        .contains(&self.filter.to_lowercase())
            })
            .collect()
    }
    fn participants<'a>(&self, db: &'a Db) -> Vec<&'a Participant> {
        self.battle(db)
            .map(|b| {
                db.participants
                    .iter()
                    .filter(|p| p.battle_id == b.id)
                    .filter(|p| {
                        db.members
                            .iter()
                            .find(|m| m.id == p.member_id)
                            .is_some_and(|m| {
                                m.name.to_lowercase().contains(&self.filter.to_lowercase())
                            })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    fn steps<'a>(&self, db: &'a Db) -> Vec<&'a Step> {
        self.battle(db)
            .map(|b| {
                db.steps
                    .iter()
                    .filter(|s| {
                        s.battle_id == b.id
                            && !(s.name == "dramatis" && db.skips_dramatis(&b.warband_id))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    fn row_count(&self, db: &Db) -> usize {
        match self.tab {
            1 => self.units(db).len(),
            2 => self.items(db).len(),
            3 => self.participants(db).len(),
            4 => self.steps(db).len(),
            5 => self.journal_rows(db).len(),
            RULES_TAB => self.rule_rows(db).len(),
            _ => 0,
        }
    }
    fn selection(&self, db: &Db) -> Selection {
        let mut s = Selection {
            rule_profile: self.rule_profile(db).map(|p| p.id.clone()),
            ..Selection::default()
        };
        match self.tab {
            1 => {
                if let Some(u) = self.units(db).get(self.selected[1]) {
                    s.unit = Some(u.id.clone());
                    s.member = db
                        .members
                        .iter()
                        .find(|m| m.unit_id == u.id && active(m))
                        .map(|m| m.id.clone());
                }
            }
            2 => s.item = self.items(db).get(self.selected[2]).map(|i| i.id.clone()),
            3 => {
                if let Some(p) = self.participants(db).get(self.selected[3]) {
                    s.member = Some(p.member_id.clone());
                    s.unit = db
                        .members
                        .iter()
                        .find(|m| m.id == p.member_id)
                        .map(|m| m.unit_id.clone());
                }
            }
            4 => {
                s.stage = self.post_stage(db).map(|s| s.name.clone());
                if let Some(t) = self.post_targets(db).get(self.post_selected) {
                    s.unit = t.unit.clone();
                    s.member = t.member.clone();
                }
            }
            5 => {
                let rows = self.journal_ids(db);
                if let Some(id) = rows.get(self.selected[5]) {
                    match self.journal {
                        Journal::Snapshots => s.snapshot = Some(id.clone()),
                        Journal::Injuries => {
                            if let Some(injury) = self.injuries(db).iter().find(|r| r.id == *id) {
                                s.member = Some(injury.member.clone());
                                s.unit = db
                                    .members
                                    .iter()
                                    .find(|m| m.id == injury.member)
                                    .map(|m| m.unit_id.clone());
                            }
                        }
                        Journal::Notes => {
                            s.note = Some(id.clone());
                            if let Some(n) = db.notes.iter().find(|n| n.id == *id) {
                                if db.members.iter().any(|m| m.id == n.entity_id) {
                                    s.member = Some(n.entity_id.clone());
                                } else if db.units.iter().any(|u| u.id == n.entity_id) {
                                    s.unit = Some(n.entity_id.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            RULES_TAB => {
                s.rule_record = self
                    .rule_rows(db)
                    .get(self.selected[RULES_TAB])
                    .map(|r| r.id.clone());
                s.rule_kind = Some(rulebook::KINDS[self.rules.section].into());
            }
            _ => {}
        }
        s
    }
    fn journal_ids(&self, db: &Db) -> Vec<String> {
        let matches = |text: String| text.to_lowercase().contains(&self.filter.to_lowercase());
        match self.journal {
            Journal::Ledger => db
                .ledger
                .iter()
                .rev()
                .filter(|l| {
                    Some(&l.warband_id) == self.wb.as_ref()
                        && matches(format!("{} {} {}", l.category, l.notes, l.date))
                })
                .map(|l| l.id.clone())
                .collect(),
            Journal::Notes => db
                .notes
                .iter()
                .rev()
                .filter(|n| {
                    n.entity_id != "selection"
                        && self.note_in_warband(db, &n.entity_id)
                        && matches(n.text.clone())
                })
                .map(|n| n.id.clone())
                .collect(),
            Journal::History => db
                .events
                .iter()
                .rev()
                .filter(|e| matches(format!("{} {} {}", e.action, e.reason, e.date)))
                .map(|e| e.id.clone())
                .collect(),
            Journal::Snapshots => db
                .snapshots
                .iter()
                .rev()
                .filter(|s| Some(&s.warband_id) == self.wb.as_ref() && matches(s.name.clone()))
                .map(|s| s.id.clone())
                .collect(),
            Journal::Injuries => self.injuries(db).iter().map(|i| i.id.clone()).collect(),
        }
    }
    fn note_in_warband(&self, db: &Db, id: &str) -> bool {
        self.wb.as_deref() == Some(id)
            || db.units.iter().any(|u| {
                Some(&u.warband_id) == self.wb.as_ref()
                    && (u.id == id || db.members.iter().any(|m| m.id == id && m.unit_id == u.id))
            })
            || db
                .battles
                .iter()
                .any(|b| Some(&b.warband_id) == self.wb.as_ref() && b.id == id)
    }
    fn journal_rows(&self, db: &Db) -> Vec<Vec<String>> {
        self.journal_ids(db)
            .iter()
            .map(|id| match self.journal {
                Journal::Ledger => {
                    let l = db.ledger.iter().find(|l| l.id == *id).unwrap();
                    vec![
                        l.date.clone(),
                        l.category.clone(),
                        db.items
                            .iter()
                            .find(|i| i.id == l.item_id)
                            .map(|i| i.name.clone())
                            .unwrap_or_default(),
                        l.quantity.to_string(),
                        format!("{:+}", l.gold),
                        l.notes.clone(),
                    ]
                }
                Journal::Notes => {
                    let n = db.notes.iter().find(|n| n.id == *id).unwrap();
                    vec![
                        n.date.clone(),
                        entity_name(db, &n.entity_id),
                        n.text.clone(),
                    ]
                }
                Journal::History => {
                    let e = db.events.iter().find(|e| e.id == *id).unwrap();
                    vec![
                        e.sequence.to_string(),
                        e.date.clone(),
                        event_label(&e.action),
                        e.reason.clone(),
                    ]
                }
                Journal::Snapshots => {
                    let s = db.snapshots.iter().find(|s| s.id == *id).unwrap();
                    let captured = db
                        .events
                        .iter()
                        .find(|e| e.generation == s.generation)
                        .map(|e| format!("{} · change {}", e.date, e.sequence))
                        .unwrap_or_else(|| "Opening state".into());
                    vec![s.name.clone(), captured]
                }
                Journal::Injuries => self
                    .injuries(db)
                    .into_iter()
                    .find(|i| i.id == *id)
                    .map(|i| i.cells)
                    .unwrap_or_default(),
            })
            .collect()
    }
    fn open(&mut self, action: Action, store: &Store, cli: &Cli) {
        let mut selection = self.selection(&store.db);
        if matches!(
            action,
            Action::RuleProfile
                | Action::RuleWarrior
                | Action::RuleList
                | Action::RuleItem
                | Action::RuleAbility
                | Action::RuleProgression
                | Action::RuleStartingGear
        ) {
            selection.rule_record = None;
        }
        self.open_selected(action, store, cli, selection);
    }
    fn open_selected(&mut self, action: Action, store: &Store, cli: &Cli, selection: Selection) {
        if action == Action::AiRequest && !cfg!(feature = "ai") {
            return;
        }
        let mut form = Form::new(action, store, self.wb.as_deref(), &selection);
        if let Some(date) = &cli.date {
            form.set("date", date);
        }
        if !cli.reason.is_empty() {
            form.set("reason", &cli.reason);
        }
        if action == Action::Advance {
            form.set("stat", "WS");
        }
        if action == Action::Sell
            && let Some(i) = store.db.items.iter().find(|i| i.id == form.value("item"))
        {
            form.set("price", (i.cost / 2).to_string());
        }
        if action == Action::Note
            && let Some(n) = store
                .db
                .notes
                .iter()
                .find(|n| n.id == form.value("replace"))
        {
            form.set("text", &n.text);
            form.set("entity", &n.entity_id);
        }
        form.refresh_limits(&store.db);
        form.cursor = form.fields[0].value.chars().count();
        self.overlay = Overlay::Form(form);
        self.picker = None;
    }
    fn actions(&self, db: &Db) -> Vec<Action> {
        if self.tab == RULES_TAB {
            return vec![
                Action::RuleSelect,
                Action::RuleProfile,
                Action::RuleWarrior,
                Action::RuleList,
                Action::RuleItem,
                Action::RuleAbility,
                Action::RuleProgression,
                Action::RuleStartingGear,
                Action::RuleCopy,
                Action::RuleRemove,
                Action::RuleCreateWarband,
            ];
        }
        if self.tab == 4 {
            return self.post_bindings(db).iter().map(|b| b.action).collect();
        }
        let preferred: Vec<_> = match self.tab {
            1 => vec![
                Action::Hire,
                Action::Equip,
                Action::Xp,
                Action::Advance,
                Action::Note,
                Action::Remove,
            ],
            2 => vec![
                Action::Buy,
                Action::Equip,
                Action::Sell,
                Action::Find,
                Action::Lose,
            ],
            3 => vec![
                Action::Prepare,
                Action::Start,
                Action::Record,
                Action::Consume,
                Action::End,
            ],
            4 => vec![
                Action::Injury,
                Action::Xp,
                Action::Advance,
                Action::Outcome,
                Action::Wyrdstone,
                Action::Search,
                Action::Upkeep,
                Action::Step,
                Action::Complete,
            ],
            5 => vec![
                Action::Note,
                Action::Snapshot,
                Action::Compare,
                Action::Export,
                Action::Restore,
                Action::Undo,
            ],
            _ => vec![
                Action::Switch,
                Action::Prepare,
                Action::Hire,
                Action::Buy,
                Action::Snapshot,
            ],
        };
        let mut all = preferred;
        for a in Action::ALL {
            if !all.contains(&a) && (a != Action::AiRequest || cfg!(feature = "ai")) {
                all.push(a);
            }
        }
        all
    }
    fn filtered_actions(&self, db: &Db, query: &str) -> Vec<Action> {
        self.actions(db)
            .into_iter()
            .filter(|a| a.label().to_lowercase().contains(&query.to_lowercase()))
            .collect()
    }
    fn switch_tab(&mut self, tab: usize) {
        if tab >= TABS.len() {
            return;
        }
        self.tab = tab;
        self.filter.clear();
        self.overlay = Overlay::None;
    }
    fn move_row(&mut self, db: &Db, delta: isize) {
        let n = self.row_count(db);
        self.selected[self.tab] = self.selected[self.tab]
            .saturating_add_signed(delta)
            .min(n.saturating_sub(1));
    }
    fn run_command(
        &mut self,
        store: &mut Store,
        cli: &Cli,
        command: Command,
        date: Option<String>,
        reason: String,
        override_rules: bool,
    ) -> Result<Value> {
        let switching = matches!(command, Command::Select { .. });
        let parsed = Cli {
            db: cli.db.clone(),
            campaign: None,
            warband: self.wb.clone(),
            date,
            reason,
            override_rules,
            json: false,
            command: Some(command),
        };
        let result = engine::execute(store, &parsed)?;
        if switching {
            self.wb = result["id"].as_str().map(str::to_string);
        } else if self
            .wb
            .as_ref()
            .is_none_or(|id| !store.db.warbands.iter().any(|w| w.id == *id))
        {
            self.wb = engine::selected(
                &store.db,
                &Cli {
                    warband: None,
                    ..parsed
                },
            )
            .ok();
        }
        if self.wb.is_none() && store.db.warbands.len() == 1 {
            self.wb = Some(store.db.warbands[0].id.clone());
        }
        self.selected[self.tab] =
            self.selected[self.tab].min(self.row_count(&store.db).saturating_sub(1));
        Ok(result)
    }
    fn review(&mut self, store: &Store) {
        if let Overlay::Form(f) = &mut self.overlay {
            f.refresh_limits(&store.db);
            match f
                .command()
                .and_then(|_| f.impact(store, self.wb.as_deref()))
            {
                Ok(_) => {
                    f.reviewing = true;
                    f.review_scroll = 0;
                    f.error.clear();
                }
                Err(e) => f.error = format!("{e:#}"),
            }
        }
    }
    fn apply(&mut self, store: &mut Store, cli: &Cli) {
        let Overlay::Form(f) = &self.overlay else {
            return;
        };
        let command = match f.command() {
            Ok(c) => c,
            Err(e) => {
                if let Overlay::Form(f) = &mut self.overlay {
                    f.error = e.to_string();
                    f.reviewing = false;
                }
                return;
            }
        };
        let action = f.action;
        if action == Action::RuleSelect {
            self.rules.profile = Some(f.value("profile").into());
            self.selected[RULES_TAB] = 0;
            self.filter.clear();
            self.overlay = Overlay::None;
            self.message = "Rules profile selected".into();
            return;
        }
        if action == Action::AiRequest {
            let prompt = f.value("prompt").to_string();
            self.ask_ai(store, prompt);
            return;
        }
        let target_unit = f.value("unit").to_string();
        let target_member = f.value("member").to_string();
        let date = Some(f.value("date").into());
        let override_rules = f.value("override") == "true";
        let mut reason = if f.value("reason").trim().is_empty() {
            f.default_reason()
        } else {
            f.value("reason").to_string()
        };
        if override_rules {
            reason = format!("Replace profile preset: {reason}");
        }
        let campaign = if action == Action::Warband {
            Some(f.value("campaign").to_string())
        } else {
            None
        };
        let result = if let Some(campaign) = campaign {
            let parsed = Cli {
                db: cli.db.clone(),
                campaign: Some(campaign),
                warband: None,
                date,
                reason,
                override_rules,
                json: false,
                command: Some(command),
            };
            engine::execute(store, &parsed)
        } else {
            self.run_command(store, cli, command, date, reason, override_rules)
        };
        match result {
            Ok(value) => {
                if action.is_rulebook() {
                    self.rules.profile = value["profile_id"]
                        .as_str()
                        .filter(|p| !p.is_empty())
                        .map(str::to_string);
                    self.filter.clear();
                    if let Some(id) = value["record_id"].as_str() {
                        self.selected[RULES_TAB] = self
                            .rule_rows(&store.db)
                            .iter()
                            .position(|r| r.id == id)
                            .unwrap_or(0);
                    }
                    if action == Action::RuleCreateWarband {
                        self.wb = value["warband_id"].as_str().map(str::to_string);
                        self.tab = 1;
                    }
                }
                if self.restore_catalog_form(action, &value, store) {
                    return;
                }
                self.overlay = Overlay::None;
                self.message = if action == Action::Compare {
                    "Checkpoint comparison ready".into()
                } else {
                    format!("Saved: {}", action.label())
                };
                if action == Action::Switch {
                    self.wb = engine::selected(
                        &store.db,
                        &Cli {
                            db: cli.db.clone(),
                            campaign: None,
                            warband: None,
                            date: None,
                            reason: String::new(),
                            override_rules: false,
                            json: false,
                            command: None,
                        },
                    )
                    .ok();
                    self.selected = [0; 8];
                    self.filter.clear();
                }
                if (action == Action::Xp || action == Action::Advance)
                    && let Some(u) = store.db.units.iter().find(|u| u.id == target_unit)
                {
                    let count = store.db.pending(u).len();
                    self.message = if count > 0 {
                        format!(
                            "{} saved for {}. {count} PLUS-UP(S) READY · U records the next result.",
                            if action == Action::Xp {
                                "XP"
                            } else {
                                "Plus-up"
                            },
                            u.name
                        )
                    } else {
                        format!(
                            "{} saved for {}. No plus-up owed.",
                            if action == Action::Xp {
                                "XP"
                            } else {
                                "Plus-up"
                            },
                            u.name
                        )
                    };
                }
                if action == Action::Compare {
                    self.overlay = Overlay::Detail {
                        title: "Checkpoint comparison".into(),
                        text: views::comparison(&value),
                        scroll: 0,
                    };
                }
                if action == Action::Init || action == Action::Import {
                    self.tab = 1;
                }
                match action {
                    Action::Prepare | Action::Start => self.tab = 3,
                    Action::End | Action::Step | Action::SkipStage => {
                        self.filter.clear();
                        self.tab = 4;
                        self.post_focus = PostFocus::Stages;
                        self.selected[4] = self
                            .steps(&store.db)
                            .iter()
                            .position(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
                            .unwrap_or_else(|| self.steps(&store.db).len().saturating_sub(1));
                    }
                    Action::Complete => self.tab = 0,
                    Action::CampaignRules => {
                        self.selected[4] = self
                            .steps(&store.db)
                            .iter()
                            .position(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
                            .unwrap_or(0);
                        self.post_focus = PostFocus::Stages;
                        self.filter.clear();
                    }
                    _ => {}
                }
                if self.tab == 4 {
                    let targets = self.post_targets(&store.db);
                    self.post_selected = targets
                        .iter()
                        .position(|t| {
                            (!target_unit.is_empty() && t.unit.as_deref() == Some(&target_unit))
                                || (!target_member.is_empty()
                                    && t.member.as_deref() == Some(&target_member))
                        })
                        .unwrap_or(self.post_selected)
                        .min(targets.len().saturating_sub(1));
                }
            }
            Err(e) => {
                if let Overlay::Form(f) = &mut self.overlay {
                    f.error = friendly_error(&format!("{e:#}"));
                    if e.to_string().contains("override")
                        && let Some(field) = f.fields.iter().find(|field| field.key == "override")
                    {
                        f.error
                            .push_str(&format!(". Enable '{}' to record this value.", field.label));
                    }
                    f.reviewing = false;
                }
            }
        }
    }
    fn open_picker(&mut self) {
        if let Overlay::Form(f) = &self.overlay
            && let Kind::Choice(choices) | Kind::Multiple(choices) = &f.fields[f.focus].kind
        {
            let index = choices
                .iter()
                .position(|c| c.id == f.fields[f.focus].value)
                .unwrap_or(0);
            self.picker = Some(Picker {
                field: f.focus,
                query: String::new(),
                index,
                state: ListState::default(),
                checked: matches!(f.fields[f.focus].kind, Kind::Multiple(_))
                    .then(|| f.fields[f.focus].selected_values()),
            });
        }
    }
    fn choose(&mut self, id: String, store: &Store) {
        if let Some(p) = &mut self.picker
            && let Some(checked) = &mut p.checked
        {
            if checked.contains(&id) {
                checked.retain(|value| value != &id);
            } else {
                checked.push(id);
            }
            return;
        }
        if self.begin_catalog_entry(&id, store) {
            return;
        }
        if let Some(p) = self.picker.take()
            && let Overlay::Form(f) = &mut self.overlay
        {
            f.fields[p.field].value = id;
            changed(f, store);
        }
    }
    fn finish_picker(&mut self, store: &Store) {
        if let Some(p) = self.picker.take()
            && let Some(checked) = p.checked
            && let Overlay::Form(f) = &mut self.overlay
        {
            f.fields[p.field].value = serde_json::to_string(&checked).unwrap();
            changed(f, store);
        }
    }
    fn quick(&mut self, store: &mut Store, cli: &Cli, change: &str) {
        let s = self.selection(&store.db);
        let Some(mid) = s.member else {
            self.message = "Choose a battle participant first".into();
            return;
        };
        let Some(p) = self
            .battle(&store.db)
            .and_then(|b| {
                store
                    .db
                    .participants
                    .iter()
                    .find(|p| p.battle_id == b.id && p.member_id == mid)
            })
            .cloned()
        else {
            return;
        };
        if matches!(change, "kill" | "unkill") {
            if change == "unkill" && p.kills == 0 {
                self.message = "No credited kills to remove.".into();
                return;
            }
            self.open(
                if change == "kill" {
                    Action::AddKill
                } else {
                    Action::RemoveKill
                },
                store,
                cli,
            );
            return;
        }
        let (wounds, kills, condition) = match change {
            "-" => (Some(p.wounds.saturating_sub(1).max(0)), None, None),
            "+" => (Some(p.wounds.saturating_add(1)), None, None),
            "objective" | "unobjective" => (None, None, None),
            _ => (None, None, Some("out-of-action".into())),
        };
        let reason = format!(
            "{}: {}",
            entity_name(&store.db, &mid),
            match change {
                "-" => "reduce current wounds by one",
                "+" => "restore one current wound",
                "kill" => "credit one kill",
                "unkill" => "remove one credited kill",
                "objective" => "credit one objective",
                "unobjective" => "remove one objective",
                _ => "taken out of action",
            }
        );
        let command = Command::Battle(BattleCommand::Record {
            member: mid,
            condition,
            wounds,
            kills,
            objectives: match change {
                "objective" => Some(p.objectives.saturating_add(1)),
                "unobjective" => Some(p.objectives.saturating_sub(1)),
                _ => None,
            },
            attribution: String::new(),
        });
        match self.run_command(store, cli, command, cli.date.clone(), reason, false) {
            Ok(_) => {
                self.message = if change == "ooa" {
                    "Marked out of action; serious injury is resolved after battle.".into()
                } else {
                    "Battle state saved. Permanent characteristics unchanged.".into()
                }
            }
            Err(e) => self.message = format!("{e:#}"),
        }
    }
    fn detail(&mut self, store: &mut Store, cli: &Cli) {
        if self.tab == 1 {
            if let Some(u) = self.units(&store.db).get(self.selected[1]) {
                self.overlay = Overlay::Detail {
                    title: u.name.clone(),
                    text: views::unit_details(&store.db, u, None),
                    scroll: 0,
                };
            }
        } else if self.tab == 2 {
            if let Some(i) = self.items(&store.db).get(self.selected[2]) {
                self.overlay = Overlay::Detail {
                    title: i.name.clone(),
                    text: views::item_details(&store.db, self.wb.as_deref().unwrap_or(""), i),
                    scroll: 0,
                };
            }
        } else if self.tab == 3 {
            self.open(Action::Record, store, cli);
        } else if self.tab == 4 {
            self.post_open_primary(store, cli);
        } else if self.tab == 5 {
            if self.journal == Journal::Snapshots {
                if let Some(id) = self.journal_ids(&store.db).get(self.selected[5])
                    && let Some(s) = store.db.snapshots.iter().find(|s| s.id == *id)
                {
                    match store.historical(&s.generation) {
                        Ok(db) => {
                            let mut content = String::new();
                            if let Some(w) = db.warbands.iter().find(|w| w.id == s.warband_id) {
                                content.push_str(&format!(
                                    "{} · {} gold · rating {}\n\n",
                                    w.name,
                                    db.gold(&w.id),
                                    db.rating(&w.id)
                                ));
                                for u in db.units.iter().filter(|u| u.warband_id == w.id) {
                                    content.push_str(&views::unit_details(&db, u, None));
                                    content.push_str("\n\n");
                                }
                            }
                            self.overlay = Overlay::Detail {
                                title: s.name.clone(),
                                text: content,
                                scroll: 0,
                            };
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
                return;
            }
            let rows = self.journal_rows(&store.db);
            if let Some(row) = rows.get(self.selected[5]) {
                self.overlay = Overlay::Detail {
                    title: format!("{:?}", self.journal),
                    text: format!(
                        "{}{}",
                        row.join("\n\n"),
                        if self.journal == Journal::Injuries {
                            self.selection(&store.db)
                                .member
                                .as_deref()
                                .and_then(|id| store.db.members.iter().find(|m| m.id == id))
                                .map(|m| {
                                    format!("\n\nNotes\n{}", views::member_notes(&store.db, m))
                                })
                                .unwrap_or_default()
                        } else {
                            String::new()
                        }
                    ),
                    scroll: 0,
                };
            }
        }
    }
    fn battle_profile(&mut self, store: &Store) {
        let db = &store.db;
        if let Some(b) = self.battle(db)
            && let Some(p) = self.participants(db).get(self.selected[3])
            && let Some(m) = db.members.iter().find(|m| m.id == p.member_id)
        {
            self.overlay = Overlay::Detail {
                title: format!("Current battle profile · {}", m.name),
                text: format!(
                    "{}\nWounds remaining: {} · {}\nKills: {} · Objectives: {}\nOpponents taken out:\n{}\n\nNotes\n{}\n\n{}\n\nFrozen pre-battle reference\n{}",
                    m.name,
                    p.wounds,
                    p.condition,
                    p.kills,
                    p.objectives,
                    views::kill_log(db, p),
                    views::member_notes(db, m),
                    views::unit_details(db, db.unit(&m.unit_id), Some(&b.id)),
                    db.snapshots
                        .iter()
                        .find(|s| s.id == b.pre_snapshot)
                        .and_then(|s| store.historical(&s.generation).ok())
                        .and_then(|old| old
                            .members
                            .iter()
                            .find(|old_m| old_m.id == m.id)
                            .map(|old_m| views::unit_details(&old, old.unit(&old_m.unit_id), None)))
                        .unwrap_or_else(|| "Unavailable; inspect campaign backup/history.".into())
                ),
                scroll: 0,
            };
        }
    }
    fn hover(&mut self, column: u16, row: u16) {
        if !matches!(self.overlay, Overlay::None) || self.picker.is_some() {
            return;
        }
        let hit = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains((column, row).into()))
            .map(|(_, h)| *h);
        match hit {
            Some(Hit::Row(i))
                if matches!(self.tab, 1 | 3)
                    || self.tab == 5 && self.journal == Journal::Injuries =>
            {
                self.selected[self.tab] = i
            }
            Some(Hit::PostTarget(i)) => self.post_selected = i,
            _ => {}
        }
    }
    fn click(&mut self, hit: Hit, store: &mut Store, cli: &Cli) {
        match hit {
            Hit::RuleSection(i) => self.rule_section(i),
            Hit::RuleNew => self.rule_new(store, cli),
            Hit::RuleEdit => self.rule_edit(store, cli),
            Hit::RuleDetails => self.rule_details(store),
            Hit::RuleDelete => self.rule_delete(store, cli),
            Hit::PostTarget(i) => {
                self.post_focus = PostFocus::Targets;
                self.post_selected = i;
            }
            Hit::PostFocus => {
                self.post_focus = if self.post_focus == PostFocus::Stages {
                    PostFocus::Targets
                } else {
                    PostFocus::Stages
                }
            }
            Hit::Details => self.detail(store, cli),
            Hit::BattleProfile => self.battle_profile(store),
            Hit::BattleLifecycle => self.battle_lifecycle(store, cli),
            Hit::Quick(change) => self.quick(store, cli, change),
            Hit::Menu => {
                self.overlay = Overlay::Menu(Menu {
                    query: String::new(),
                    index: 0,
                    state: ListState::default(),
                })
            }
            Hit::AiReview => self.review_ai(store),
            Hit::AiApprove => {
                if matches!(self.overlay, Overlay::AiApproval { .. }) {
                    self.apply_ai(store);
                } else {
                    self.confirm_ai(store);
                }
            }
            Hit::AiReject => self.reject_ai(),
            Hit::Tab(t) => {
                if matches!(self.overlay, Overlay::None) {
                    self.switch_tab(t)
                }
            }
            Hit::Action(a) => {
                if matches!(self.overlay, Overlay::None) {
                    self.activate(a, store, cli)
                }
            }
            Hit::Row(i) => {
                if self.tab == RULES_TAB {
                    self.rules.records_focus = true;
                }
                self.selected[self.tab] = i;
                if self.tab == 4 {
                    self.post_focus = PostFocus::Stages;
                    self.post_selected = 0;
                    self.filter.clear();
                }
            }
            Hit::Journal => self.cycle_journal(),
            Hit::Field(i) => {
                if let Overlay::Form(f) = &mut self.overlay {
                    f.focus = i;
                    f.cursor = f.fields[i].value.chars().count();
                    match f.fields[i].kind {
                        Kind::Toggle => {
                            toggle(&mut f.fields[i]);
                            changed(f, store);
                        }
                        Kind::Choice(_) | Kind::Multiple(_) => self.open_picker(),
                        Kind::Text => {}
                    }
                }
            }
            Hit::Review => self.review(store),
            Hit::Apply => self.apply(store, cli),
            Hit::Cancel => {
                self.picker = None;
                if let Overlay::Form(f) = &mut self.overlay
                    && f.reviewing
                {
                    f.reviewing = false;
                } else {
                    self.cancel_form();
                }
            }
            Hit::MenuRow(i) => {
                let query = if let Overlay::Menu(m) = &self.overlay {
                    m.query.clone()
                } else {
                    return;
                };
                if let Some(a) = self.filtered_actions(&store.db, &query).get(i) {
                    self.activate(*a, store, cli);
                }
            }
            Hit::PickerRow(i) => {
                if let Some(p) = &self.picker
                    && let Overlay::Form(f) = &self.overlay
                {
                    let rows = picker_choices(f, p);
                    if let Some(c) = rows.get(i) {
                        let id = c.id.clone();
                        if let Some(picker) = &mut self.picker {
                            picker.index = i;
                        }
                        self.choose(id, store);
                    }
                }
            }
            Hit::PickerDone => self.finish_picker(store),
        }
    }
    fn cycle_journal(&mut self) {
        self.journal = match self.journal {
            Journal::Ledger => Journal::Notes,
            Journal::Notes => Journal::History,
            Journal::History => Journal::Snapshots,
            Journal::Snapshots => Journal::Injuries,
            Journal::Injuries => Journal::Ledger,
        };
        self.selected[5] = 0;
        self.filter.clear();
    }
    fn key(&mut self, key: KeyEvent, store: &mut Store, cli: &Cli) -> bool {
        let key = if matches!(self.overlay, Overlay::None) {
            match key.code {
                KeyCode::Char(c) => KeyEvent {
                    code: KeyCode::Char(c.to_ascii_lowercase()),
                    ..key
                },
                _ => key,
            }
        } else {
            key
        };
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return false;
        }
        if matches!(self.overlay, Overlay::None)
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char(_))
        {
            return true;
        }
        if self.tab == RULES_TAB
            && matches!(self.overlay, Overlay::None)
            && self.rule_key(key, store, cli)
        {
            return true;
        }
        if self.tab == AI_TAB && matches!(self.overlay, Overlay::None) {
            match key.code {
                KeyCode::Char('n') => {
                    self.open(Action::AiRequest, store, cli);
                    return true;
                }
                KeyCode::Char('r') => {
                    self.review_ai(store);
                    return true;
                }
                KeyCode::Char('y') => {
                    self.confirm_ai(store);
                    return true;
                }
                KeyCode::Char('d') => {
                    self.reject_ai();
                    return true;
                }
                KeyCode::Up => {
                    self.ai.scroll = self.ai.scroll.saturating_sub(1);
                    return true;
                }
                KeyCode::Down => {
                    self.ai.scroll = self.ai.scroll.saturating_add(1);
                    return true;
                }
                KeyCode::PageUp => {
                    self.ai.scroll = self.ai.scroll.saturating_sub(10);
                    return true;
                }
                KeyCode::PageDown => {
                    self.ai.scroll = self.ai.scroll.saturating_add(10);
                    return true;
                }
                KeyCode::Home => {
                    self.ai.scroll = 0;
                    return true;
                }
                KeyCode::End => {
                    self.ai.scroll = self.ai.max_scroll;
                    return true;
                }
                _ => {}
            }
        }
        if self.tab == 4 && matches!(self.overlay, Overlay::None) {
            // Left/Right retain global tab navigation; Tab changes the local pane.
            match key.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    self.post_focus = if self.post_focus == PostFocus::Stages {
                        PostFocus::Targets
                    } else {
                        PostFocus::Stages
                    };
                    return true;
                }
                KeyCode::Home => {
                    if self.post_focus == PostFocus::Stages {
                        self.selected[4] = 0;
                        self.filter.clear();
                    }
                    self.post_selected = 0;
                    return true;
                }
                KeyCode::End => {
                    if self.post_focus == PostFocus::Stages {
                        self.selected[4] = self.steps(&store.db).len().saturating_sub(1);
                        self.post_selected = 0;
                        self.filter.clear();
                    } else {
                        self.post_selected = self.post_targets(&store.db).len().saturating_sub(1);
                    }
                    return true;
                }
                KeyCode::Up => {
                    self.post_move(&store.db, -1);
                    return true;
                }
                KeyCode::Down => {
                    self.post_move(&store.db, 1);
                    return true;
                }
                KeyCode::PageUp => {
                    self.post_move(&store.db, -5);
                    return true;
                }
                KeyCode::PageDown => {
                    self.post_move(&store.db, 5);
                    return true;
                }
                KeyCode::Enter => {
                    if self.post_focus == PostFocus::Stages {
                        self.post_focus = PostFocus::Targets;
                    } else {
                        self.post_open_primary(store, cli);
                    }
                    return true;
                }
                KeyCode::Char(c)
                    if ![
                        'q', 'a', '/', '?', ':', '1', '2', '3', '4', '5', '6', '7', '8',
                    ]
                    .contains(&c) =>
                {
                    if let Some(b) = self.post_bindings(&store.db).iter().find(|b| b.key == c) {
                        self.activate(b.action, store, cli);
                    }
                    return true;
                }
                _ => {}
            }
        }
        if self.picker.is_some() {
            let p = self.picker.as_ref().unwrap();
            let rows = if let Overlay::Form(f) = &self.overlay {
                picker_choices(f, p)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };
            let p = self.picker.as_mut().unwrap();
            match key.code {
                KeyCode::Esc => self.picker = None,
                KeyCode::Up => p.index = p.index.saturating_sub(1),
                KeyCode::Down => p.index = (p.index + 1).min(rows.len().saturating_sub(1)),
                KeyCode::PageDown => p.index = (p.index + 10).min(rows.len().saturating_sub(1)),
                KeyCode::PageUp => p.index = p.index.saturating_sub(10),
                KeyCode::Enter if p.checked.is_some() => self.finish_picker(store),
                KeyCode::Char(' ') if p.checked.is_some() => {
                    if let Some(c) = rows.get(p.index) {
                        self.choose(c.id.clone(), store);
                    }
                }
                KeyCode::Char('a')
                    if p.checked.is_some() && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if let Some(checked) = &mut p.checked {
                        for c in &rows {
                            if !checked.contains(&c.id) {
                                checked.push(c.id.clone());
                            }
                        }
                    }
                }
                KeyCode::Char('u')
                    if p.checked.is_some() && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if let Some(checked) = &mut p.checked {
                        checked.retain(|id| !rows.iter().any(|c| c.id == *id));
                    }
                }
                KeyCode::Enter => {
                    if let Some(c) = rows.get(p.index) {
                        self.choose(c.id.clone(), store)
                    }
                }
                KeyCode::Backspace => {
                    p.query.pop();
                    p.index = 0;
                }
                KeyCode::Char(c) => {
                    p.query.push(c);
                    p.index = 0;
                }
                _ => {}
            }
            return true;
        }
        let menu_len = if let Overlay::Menu(m) = &self.overlay {
            self.filtered_actions(&store.db, &m.query).len()
        } else {
            0
        };
        match &mut self.overlay {
            Overlay::AiApproval { scroll } => {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => self.apply_ai(store),
                    KeyCode::Esc => self.overlay = Overlay::None,
                    KeyCode::Down => *scroll = scroll.saturating_add(1),
                    KeyCode::Up => *scroll = scroll.saturating_sub(1),
                    KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                    KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                    _ => {}
                }
                return true;
            }
            Overlay::Form(f) => {
                if f.reviewing {
                    match key.code {
                        KeyCode::Enter | KeyCode::F(10) => self.apply(store, cli),
                        KeyCode::Esc => f.reviewing = false,
                        KeyCode::Down => f.review_scroll = f.review_scroll.saturating_add(1),
                        KeyCode::Up => f.review_scroll = f.review_scroll.saturating_sub(1),
                        KeyCode::PageDown => f.review_scroll = f.review_scroll.saturating_add(8),
                        KeyCode::PageUp => f.review_scroll = f.review_scroll.saturating_sub(8),
                        _ => {}
                    }
                    return true;
                }
                let is_submit = key.code == KeyCode::F(10)
                    || (key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL));
                if is_submit {
                    self.review(store);
                    return true;
                }
                match key.code {
                    KeyCode::Esc => self.cancel_form(),
                    KeyCode::PageDown => {
                        f.focus = (f.focus + 6).min(f.fields.len() - 1);
                        f.cursor = f.fields[f.focus].value.chars().count();
                    }
                    KeyCode::PageUp => {
                        f.focus = f.focus.saturating_sub(6);
                        f.cursor = f.fields[f.focus].value.chars().count();
                    }
                    KeyCode::Tab | KeyCode::Down => {
                        f.focus = (f.focus + 1) % f.fields.len();
                        f.cursor = f.fields[f.focus].value.chars().count();
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        f.focus = (f.focus + f.fields.len() - 1) % f.fields.len();
                        f.cursor = f.fields[f.focus].value.chars().count();
                    }
                    KeyCode::Enter => match f.fields[f.focus].kind {
                        Kind::Choice(_) | Kind::Multiple(_) => self.open_picker(),
                        Kind::Toggle => {
                            toggle(&mut f.fields[f.focus]);
                            changed(f, store);
                        }
                        Kind::Text => {
                            f.focus = (f.focus + 1) % f.fields.len();
                            f.cursor = f.fields[f.focus].value.chars().count();
                        }
                    },
                    _ => {
                        match &f.fields[f.focus].kind {
                            Kind::Text => edit(&mut f.fields[f.focus], &mut f.cursor, key),
                            Kind::Toggle => {
                                if matches!(
                                    key.code,
                                    KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right
                                ) {
                                    toggle(&mut f.fields[f.focus]);
                                }
                            }
                            Kind::Choice(cs) => {
                                if matches!(key.code, KeyCode::Left | KeyCode::Right)
                                    && !cs.is_empty()
                                {
                                    let index = cs
                                        .iter()
                                        .position(|c| c.id == f.fields[f.focus].value)
                                        .unwrap_or(0);
                                    let next = if key.code == KeyCode::Left {
                                        (index + cs.len() - 1) % cs.len()
                                    } else {
                                        (index + 1) % cs.len()
                                    };
                                    f.fields[f.focus].value = cs[next].id.clone();
                                }
                            }
                            Kind::Multiple(_) => {}
                        }
                        changed(f, store);
                    }
                }
            }
            Overlay::Menu(m) => {
                let query = m.query.clone();
                let index = m.index;
                match key.code {
                    KeyCode::Esc => self.overlay = Overlay::None,
                    KeyCode::Up => m.index = m.index.saturating_sub(1),
                    KeyCode::Down => {
                        let len = menu_len;
                        m.index = (m.index + 1).min(len.saturating_sub(1));
                    }
                    KeyCode::PageDown => {
                        let n = menu_len;
                        m.index = (m.index + 10).min(n.saturating_sub(1));
                    }
                    KeyCode::PageUp => m.index = m.index.saturating_sub(10),
                    KeyCode::Enter => {
                        if let Some(a) = self.filtered_actions(&store.db, &query).get(index) {
                            self.activate(*a, store, cli)
                        }
                    }
                    KeyCode::Backspace => {
                        m.query.pop();
                        m.index = 0;
                    }
                    KeyCode::Char(c) => {
                        m.query.push(c);
                        m.index = 0;
                    }
                    _ => {}
                }
            }
            Overlay::Detail { scroll, .. } => match key.code {
                KeyCode::Esc | KeyCode::Enter => self.overlay = Overlay::None,
                KeyCode::Down => *scroll = scroll.saturating_add(1),
                KeyCode::Up => *scroll = scroll.saturating_sub(1),
                KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                _ => {}
            },
            Overlay::Filter(s) => match key.code {
                KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Enter => {
                    self.filter = s.clone();
                    if self.tab == 4 {
                        self.post_selected = 0;
                        self.post_focus = PostFocus::Targets;
                    } else {
                        self.selected[self.tab] = 0;
                    }
                    self.overlay = Overlay::None;
                }
                KeyCode::Backspace => {
                    s.pop();
                }
                KeyCode::Char(c) => s.push(c),
                _ => {}
            },
            Overlay::Command(input) => match key.code {
                KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) => input.push(c),
                KeyCode::Enter => {
                    let parsed = shell_words::split(input)
                        .map_err(anyhow::Error::from)
                        .and_then(|a| {
                            Cli::try_parse_from(std::iter::once("mordheim".to_string()).chain(a))
                                .map_err(anyhow::Error::from)
                        });
                    match parsed {
                        Ok(mut c) => {
                            c.db = cli.db.clone();
                            if c.reason.trim().is_empty() {
                                c.reason = format!("Command: {input}");
                            }
                            if c.warband.is_none() {
                                c.warband = self.wb.clone();
                            }
                            if c.command.is_some() && !matches!(c.command, Some(Command::Tui)) {
                                match engine::execute(store, &c) {
                                    Ok(v) => {
                                        self.overlay = Overlay::Detail {
                                            title: "Command result".into(),
                                            text: pretty(&v),
                                            scroll: 0,
                                        };
                                        self.wb = engine::selected(
                                            &store.db,
                                            &Cli { warband: None, ..c },
                                        )
                                        .ok();
                                    }
                                    Err(e) => self.message = e.to_string(),
                                }
                            }
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
                _ => {}
            },
            Overlay::None => match key.code {
                KeyCode::Char('q') => return false,
                KeyCode::Tab => self.switch_tab((self.tab + 1) % TABS.len()),
                KeyCode::BackTab => self.switch_tab((self.tab + TABS.len() - 1) % TABS.len()),
                KeyCode::Char(c @ '1'..='8') => self.switch_tab(c as usize - '1' as usize),
                KeyCode::Left => self.switch_tab((self.tab + TABS.len() - 1) % TABS.len()),
                KeyCode::Right => self.switch_tab((self.tab + 1) % TABS.len()),
                KeyCode::Down => self.move_row(&store.db, 1),
                KeyCode::Up => self.move_row(&store.db, -1),
                KeyCode::PageDown => self.move_row(&store.db, 10),
                KeyCode::PageUp => self.move_row(&store.db, -10),
                KeyCode::Home => self.selected[self.tab] = 0,
                KeyCode::End => {
                    self.selected[self.tab] = self.row_count(&store.db).saturating_sub(1)
                }
                KeyCode::Enter => self.detail(store, cli),
                KeyCode::Char('a') => {
                    self.overlay = Overlay::Menu(Menu {
                        query: String::new(),
                        index: 0,
                        state: ListState::default(),
                    })
                }
                KeyCode::Char('/') => self.overlay = Overlay::Filter(self.filter.clone()),
                KeyCode::Esc => self.filter.clear(),
                KeyCode::Char(':') => self.overlay = Overlay::Command(String::new()),
                KeyCode::F(2) => self.open(Action::Switch, store, cli),
                KeyCode::F(3) => self.open(Action::CampaignRules, store, cli),
                KeyCode::F(4) => self.activate(Action::Injuries, store, cli),
                KeyCode::F(5) => self.battle_lifecycle(store, cli),
                KeyCode::F(6) => self.model_notes(&store.db),
                KeyCode::F(7) => self.switch_tab(RULES_TAB),
                KeyCode::Char('?') => {
                    self.overlay = Overlay::Detail {
                        title: "Keyboard and mouse help".into(),
                        text: help_text(),
                        scroll: 0,
                    }
                }
                KeyCode::Char('-') if self.tab == 3 => self.quick(store, cli, "-"),
                KeyCode::Char('+') | KeyCode::Char('=') if self.tab == 3 => {
                    self.quick(store, cli, "+")
                }
                KeyCode::Char('k') if self.tab == 3 => self.quick(store, cli, "kill"),
                KeyCode::Char('j') if self.tab == 3 => self.quick(store, cli, "unkill"),
                KeyCode::Char(']') if self.tab == 3 => self.quick(store, cli, "objective"),
                KeyCode::Char('[') if self.tab == 3 => self.quick(store, cli, "unobjective"),
                KeyCode::Char('c') if self.tab == 3 => self.open(Action::Consume, store, cli),
                KeyCode::Char('c') if matches!(self.tab, 1 | 2) => {
                    self.open(Action::NewEquipment, store, cli)
                }
                KeyCode::Char('s') if self.tab == 1 => self.open(Action::NewAbility, store, cli),
                KeyCode::Char('l') if self.tab == 1 => self.open(Action::GrantAbility, store, cli),
                KeyCode::Char('d') if self.tab == 1 => self.open(Action::Remove, store, cli),
                KeyCode::Char('g') if self.tab == 1 => {
                    self.open(Action::AssignEquipmentList, store, cli)
                }
                KeyCode::Char('g') if self.tab == 2 => {
                    self.open(Action::EditEquipmentLists, store, cli)
                }
                KeyCode::Char('f') if self.tab == 2 => self.open(Action::Find, store, cli),
                KeyCode::Char('l') if self.tab == 2 => self.open(Action::Lose, store, cli),
                KeyCode::Char('c') if self.tab == 5 => self.open(Action::Compare, store, cli),
                KeyCode::Char('e') if self.tab == 5 => self.open(Action::Export, store, cli),
                KeyCode::Char('s') if self.tab == 0 => self.open(Action::Snapshot, store, cli),
                KeyCode::Char('o') if self.tab == 3 => self.quick(store, cli, "ooa"),
                KeyCode::Char('v') if self.tab == 3 => self.battle_profile(store),
                KeyCode::Char('v') if self.tab == 5 => self.cycle_journal(),
                KeyCode::Char('h') => self.open(Action::Hire, store, cli),
                KeyCode::Char('p') => self.open(Action::Buy, store, cli),
                KeyCode::Char('e') => self.open(Action::Equip, store, cli),
                KeyCode::Char('x') => self.open(Action::Xp, store, cli),
                KeyCode::Char('u') => self.open(Action::Advance, store, cli),
                KeyCode::Char('n') => self.open(Action::Note, store, cli),
                KeyCode::Char('w') if self.tab == 3 => self.open(Action::Record, store, cli),
                KeyCode::Char('i') => self.open(Action::Injury, store, cli),
                KeyCode::Char('r') if self.tab == 4 => self.open(Action::Outcome, store, cli),
                KeyCode::Char('t') => self.open(Action::Wyrdstone, store, cli),

                KeyCode::Char('s') if self.tab == 2 => self.open(Action::Sell, store, cli),
                KeyCode::Char('s') if self.tab == 5 => self.open(Action::Snapshot, store, cli),
                KeyCode::Char('b') => {
                    self.battle_lifecycle(store, cli);
                }
                _ => {}
            },
        }
        true
    }
}
fn picker_choices<'a>(f: &'a Form, p: &Picker) -> Vec<&'a Choice> {
    if let Kind::Choice(cs) | Kind::Multiple(cs) = &f.fields[p.field].kind {
        cs.iter()
            .filter(|c| c.label.to_lowercase().contains(&p.query.to_lowercase()))
            .collect()
    } else {
        vec![]
    }
}
fn toggle(f: &mut forms::Field) {
    f.value = if f.value == "true" { "false" } else { "true" }.into();
}
fn changed(f: &mut Form, store: &Store) {
    let db = &store.db;
    let key = f.fields[f.focus].key;
    if key == "reason" {
        f.automatic_reason = false;
    }
    if f.action == Action::Advance && key == "initial" {
        let wb = f.warband_id.as_deref();
        let choices: Vec<_> = db
            .units
            .iter()
            .filter(|u| {
                Some(u.warband_id.as_str()) == wb
                    && (f.value("initial") == "true" || !db.pending(u).is_empty())
            })
            .map(|u| Choice::new(&u.id, &u.name))
            .collect();
        if let Some(field) = f.fields.iter_mut().find(|f| f.key == "unit") {
            field.kind = Kind::Choice(choices.clone());
            if !choices.iter().any(|c| c.id == field.value) {
                field.value = choices.first().map(|c| c.id.clone()).unwrap_or_default();
            }
        }
    }

    if f.action == Action::Init
        && key == "rules"
        && let Ok(rules) = crate::rules::load(std::path::Path::new(f.value("rules")))
    {
        let choices: Vec<_> = rules
            .profiles
            .iter()
            .map(|p| Choice::new(&p.id, &p.name))
            .collect();
        if let Some(field) = f.fields.iter_mut().find(|f| f.key == "profile") {
            if !choices.iter().any(|c| c.id == field.value) {
                field.value = choices.first().map(|c| c.id.clone()).unwrap_or_default();
            }
            field.kind = Kind::Choice(choices);
        }
    }

    if f.action == Action::Record
        && key == "member"
        && let Some(p) = db
            .participants
            .iter()
            .rev()
            .find(|p| p.member_id == f.value("member"))
    {
        let p = p.clone();
        f.set("wounds", p.wounds.to_string());
        f.set("condition", p.condition);
        f.set("kills", p.kills.to_string());
        f.set("objectives", p.objectives.to_string());
    }
    if f.action == Action::RuleProgression && key == "role" {
        let mut values = db
            .thresholds
            .iter()
            .filter(|t| t.profile_id == f.value("profile") && t.role == f.value("role"))
            .map(|t| t.xp)
            .collect::<Vec<_>>();
        values.sort();
        f.set(
            "thresholds",
            values
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if f.action == Action::RemoveKill && key == "member" {
        let battle = db.battles.iter().rev().find(|b| {
            Some(b.warband_id.as_str()) == f.warband_id.as_deref()
                && !matches!(b.status.as_str(), "complete" | "abandoned")
        });
        let choices = forms::kill_choices(db, battle, f.value("member"));
        if let Some(field) = f.fields.iter_mut().find(|field| field.key == "kill") {
            field.value = choices.first().map(|c| c.id.clone()).unwrap_or_default();
            field.kind = Kind::Choice(choices);
        }
    }
    if f.action == Action::EditEquipmentLists && key == "item" {
        let lists = db
            .equipment_list_items
            .iter()
            .filter(|r| r.item_id == f.value("item"))
            .map(|r| r.list_id.clone())
            .collect::<Vec<_>>();
        f.set("lists", serde_json::to_string(&lists).unwrap());
    }
    if f.action == Action::Status
        && key == "member"
        && let Some(m) = db.members.iter().find(|m| m.id == f.value("member"))
    {
        f.set("status", &m.status);
        f.set("missed", m.missed_games.to_string());
    }
    if f.action == Action::Remove && key == "status" {
        f.set(
            "equipment",
            if f.value("status") == "dead" {
                "lose"
            } else {
                "stash"
            },
        );
    }
    if f.action == Action::Advance && key == "stat" && !f.value("stat").is_empty() {
        f.set("ability", "");
    } else if f.action == Action::Advance && key == "ability" && !f.value("ability").is_empty() {
        f.set("stat", "");
    }
    if f.action == Action::Hire && key == "group" && !f.value("group").is_empty() {
        let u = db.unit(f.value("group"));
        f.set("kind", &u.type_id);
        f.set("name", &u.name);
    }
    if f.action == Action::Sell
        && key == "item"
        && let Some(i) = db.items.iter().find(|i| i.id == f.value("item"))
    {
        f.set("price", (i.cost / 2).to_string());
    }
    if f.action == Action::Note
        && key == "replace"
        && let Some(n) = db.notes.iter().find(|n| n.id == f.value("replace"))
    {
        f.set("entity", &n.entity_id);
        f.set("text", &n.text);
    }
    f.refresh_limits(db);
}
fn edit(field: &mut forms::Field, cursor: &mut usize, key: KeyEvent) {
    let modifies = matches!(key.code, KeyCode::Backspace | KeyCode::Delete)
        || matches!(key.code, KeyCode::Char(_) if !key.modifiers.contains(KeyModifiers::CONTROL))
        || (key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL));
    if modifies && field.replace_on_input {
        field.value.clear();
        *cursor = 0;
    }
    if modifies
        || matches!(
            key.code,
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
        )
    {
        field.replace_on_input = false;
    }
    let mut chars: Vec<_> = field.value.chars().collect();
    *cursor = (*cursor).min(chars.len());
    match key.code {
        KeyCode::Left => *cursor = cursor.saturating_sub(1),
        KeyCode::Right => *cursor = (*cursor + 1).min(chars.len()),
        KeyCode::Home => *cursor = 0,
        KeyCode::End => *cursor = chars.len(),
        KeyCode::Backspace => {
            if *cursor > 0 {
                *cursor -= 1;
                chars.remove(*cursor);
            }
        }
        KeyCode::Delete => {
            if *cursor < chars.len() {
                chars.remove(*cursor);
            }
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            chars.clear();
            *cursor = 0;
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            chars.insert(*cursor, c);
            *cursor += 1;
        }
        _ => {}
    }
    field.value = chars.into_iter().collect();
}
fn friendly_error(message: &str) -> String {
    message
        .replace(
            "; use --override --reason to record a house-rule exception",
            "; this entry conflicts with a profile preset",
        )
        .replace("--reason", "a note")
        .replace("--dice ID", "the physical outcome")
        .replace("--override", "the specific preset replacement control")
}

fn event_label(action: &str) -> String {
    let override_rule = action.starts_with("RULES OVERRIDE:");
    let name = action
        .trim_start_matches("RULES OVERRIDE: ")
        .split(['{', '('])
        .next()
        .unwrap_or(action)
        .trim();
    format!(
        "{}{}",
        name,
        if override_rule { " (house rule)" } else { "" }
    )
}
fn entity_name(db: &Db, id: &str) -> String {
    db.members
        .iter()
        .find(|m| m.id == id)
        .map(|m| m.name.clone())
        .or_else(|| db.units.iter().find(|u| u.id == id).map(|u| u.name.clone()))
        .or_else(|| {
            db.warbands
                .iter()
                .find(|w| w.id == id)
                .map(|w| w.name.clone())
        })
        .or_else(|| {
            db.battles
                .iter()
                .find(|b| b.id == id)
                .map(|b| b.name.clone())
        })
        .unwrap_or_else(|| id.to_string())
}

pub fn pretty(value: &Value) -> String {
    fn render(v: &Value, depth: usize) -> String {
        match v {
            Value::Array(rs) => rs
                .iter()
                .map(|r| render(r, depth))
                .collect::<Vec<_>>()
                .join("\n\n"),
            Value::Object(map) => map
                .iter()
                .map(|(k, v)| {
                    let indent = "  ".repeat(depth);
                    match v {
                        Value::Object(_) | Value::Array(_) => {
                            format!("{indent}{k}:\n{}", render(v, depth + 1))
                        }
                        _ => format!(
                            "{indent}{k}: {}",
                            v.as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| v.to_string())
                        ),
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Value::String(s) => s.clone(),
            _ => v.to_string(),
        }
    }
    render(value, 0)
}
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        ratatui::restore();
    }
}
pub fn run(store: &mut Store, cli: &Cli) -> Result<()> {
    ensure!(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "TUI requires a terminal; use `list` or --help for the CLI"
    );
    let mut terminal = ratatui::try_init()?;
    let _guard = TerminalGuard;
    execute!(std::io::stdout(), EnableMouseCapture)?;
    let mut app = App::new(store, cli);
    loop {
        app.poll_ai(store);
        terminal.draw(|f| views::draw(f, &mut app, store))?;
        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }
        match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => {
                if !app.key(k, store, cli) {
                    break;
                }
            }
            Event::Mouse(m) => match m.kind {
                MouseEventKind::Moved => app.hover(m.column, m.row),
                MouseEventKind::Down(event::MouseButton::Left) => {
                    if let Some((_, hit)) = app
                        .hits
                        .iter()
                        .rev()
                        .find(|(r, _)| r.contains((m.column, m.row).into()))
                        .copied()
                    {
                        app.click(hit, store, cli);
                    }
                }
                MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                    let code = if m.kind == MouseEventKind::ScrollDown {
                        KeyCode::Down
                    } else {
                        KeyCode::Up
                    };
                    app.key(KeyEvent::new(code, KeyModifiers::NONE), store, cli);
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}
fn help_text() -> String {
    if cfg!(feature = "ai") {
        HELP.into()
    } else {
        HELP.split("\n\n")
            .filter(|paragraph| !paragraph.starts_with("AI tab 8:"))
            .collect::<Vec<_>>()
            .join("\n\n")
            .replace("1–8", "1–7")
            .replace(" Tab 8: AI assistant workbench.", "")
    }
}

const HELP: &str = "Mordtui session controls\n\n1–8 or Left/Right: change tabs. Tab/Shift-Tab changes tabs outside Post-battle and Rules; inside them it changes pane focus. Up/Down: select a row. Enter: unit dossier / battle form / stage form. Hover model rows to preview recent notes; click tabs, rows, action buttons and form fields. Mouse wheel scrolls lists.\n\nA: searchable menu of every session action. H: hire. P: purchase. E: equip. X: experience. U: advance. N: note. F2: switch warband. F3: campaign rules (including persistent Skip Dramatis Personae). F4: injury list. F5: prepare/start/end battle. F6: selected model/unit notes. F7 / Tab 7: Rules editor. Tab 8: AI assistant workbench.\n\nBattle: -/+ adjust current wounds by one, K opens a kill credit with the opponent name; J chooses a credit to remove; ]/[ adjusts objectives. O marks out of action, W opens all battle counters. OOA is not death. F5 (or B) prepares, starts or ends the active battle. Rem is wounds remaining, separate from W. V expands all details and F6 opens notes.\n\nPost-battle: Up/Down initially browses all stages, including completed/future stages. Tab/Shift-Tab switches between stages and targets; Up/Down selects within the focused pane. Empty target lists return arrow navigation to stages. Left/Right keeps its global meaning: change main tabs. Enter acts on the selected target. Only the selected stage's visible shortcuts are enabled. Every pending stage offers C to complete and K to skip, even when browsing ahead. Rare searches: one per Hero not taken out of action; S records a specific item found/failed, P buys one found item. Dice stay on the tabletop: record consequences directly.\n\nWarband: G assigns equipment lists to types, groups or models. Create shared lists in A Actions. D dismisses a warrior (choose whether gear returns to stash or is lost). S defines a skill, L records a learned skill, C defines equipment. Pickers offer New equipment/skill and resume the original form. Stash: G changes the selected item's equipment lists; C defines equipment, F finds and L loses stock. Enter opens Details. Journal: C compares checkpoints; E exports.\n\nRules (7 / F7): P selects a profile independently of the warband. Up/Down selects sections, Tab switches sections/definitions. N adds, E edits, V expands, D removes unused definitions, C copies a profile and W creates a warband from stored rules. Define profiles, warrior types, equipment lists, items, skills, XP thresholds and starting gear without a campaign or rules files.\n\nJournal opens on History. V cycles History / Snapshots / Injuries / Ledger / Notes. S creates a snapshot; F4 opens the injury list.\n\nForms: Tab/Shift-Tab or Up/Down changes field; PgUp/PgDn jumps through long forms. Enter opens a searchable choice picker; type to filter it, then Enter selects. For checkboxes, Space/click toggles multiple choices, Enter accepts, Esc cancels, Ctrl-A selects all shown and Ctrl-U clears shown. Left/Right cycles single choices. Space toggles checkboxes. Typing first replaces a prefilled value, then edits your entry. Move the cursor first to edit a default in place. Backspace/Delete clears an untouched default. Text fields support Home/End, arrows and Ctrl-U. F10 or Ctrl-S reviews; Enter then applies. Esc backs out without saving. Errors retain your entries.\n\nAI tab 8: N sends a request; R validates every proposed action in a disposable copy. Y opens approval, then Y explicitly applies all actions; Esc cancels confirmation and D rejects. Changed campaign data invalidates approval; proposals never execute themselves.\n\nForms generate readable journal reasons from the action and values; type a replacement if desired. Common session actions never require command syntax or copied IDs. All forms share CLI validation and audited persistence. The optional : command prompt remains for advanced use. / filters a table; Esc clears it. Q quits. Every successful change is already saved.";

#[cfg(test)]
mod tests;
