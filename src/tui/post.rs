//! One source for after-action targets, buttons and keyboard bindings.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PostFocus {
    Stages,
    Targets,
}
#[derive(Clone, Debug)]
pub(super) struct Target {
    pub unit: Option<String>,
    pub member: Option<String>,
    pub cells: Vec<String>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Binding {
    pub key: char,
    pub label: &'static str,
    pub action: Action,
}
fn bind(key: char, label: &'static str, action: Action) -> Binding {
    Binding { key, label, action }
}
impl App {
    pub(super) fn post_stage<'a>(&self, db: &'a Db) -> Option<&'a Step> {
        self.steps(db).get(self.selected[4]).copied()
    }
    pub(super) fn post_unlocked(&self, db: &Db) -> bool {
        self.battle(db).is_some_and(|b| b.status == "post-battle")
            && self
                .post_stage(db)
                .is_some_and(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
    }
    pub(super) fn post_bindings(&self, db: &Db) -> Vec<Binding> {
        if self.battle(db).is_none()
            && self
                .wb
                .as_deref()
                .and_then(|w| crate::engine::recruitment::veteran_battle(db, w))
                .is_some()
        {
            return vec![bind('v', "Correct veteran XP", Action::Veterans)];
        }
        if !self.post_unlocked(db) {
            let mut bindings = vec![];
            if self.battle(db).is_some_and(|b| b.status == "post-battle")
                && self.post_stage(db).is_some_and(|s| s.name == "veterans")
            {
                bindings.push(bind('v', "Correct veteran XP", Action::Veterans));
            }
            if self.battle(db).is_some_and(|b| b.status == "post-battle")
                && self
                    .steps(db)
                    .iter()
                    .all(|s| matches!(s.status.as_str(), "completed" | "skipped"))
            {
                bindings.push(bind('b', "Finish battle", Action::Complete));
            }
            return bindings;
        }
        let mut bindings = match self.post_stage(db).map(|s| s.name.as_str()).unwrap_or("") {
            "injuries" => vec![
                bind('i', "Resolve injury", Action::Injury),
                bind('r', "Recovery / leader", Action::Status),
                bind('f', "Lasting effect", Action::Effect),
                bind('l', "Injury list", Action::Injuries),
            ],
            "experience" => vec![
                bind('x', "Award XP", Action::Xp),
                bind('u', "Resolve plus-up", Action::Advance),
                bind('p', "Promote", Action::Promote),
            ],
            "exploration" => vec![
                bind('o', "Exploration result", Action::Outcome),
                bind('f', "Found equipment", Action::Find),
                bind('g', "Gold", Action::Gold),
            ],
            "wyrdstone" => vec![bind('t', "Sell shards", Action::Wyrdstone)],
            "veterans" => vec![bind('v', "Available veteran XP", Action::Veterans)],
            "rare" => vec![
                bind('s', "Record search", Action::Search),
                bind('p', "Buy found item", Action::Buy),
            ],
            "dramatis" => vec![
                bind('s', "Search attempts", Action::Search),
                bind('h', "Hire", Action::Hire),
                bind('o', "Search result", Action::Outcome),
            ],
            "recruitment" => vec![
                bind('h', "Hire", Action::Hire),
                bind('v', "Veteran XP", Action::Veterans),
                bind('p', "Purchase", Action::Buy),
                bind('u', "Upkeep", Action::Upkeep),
                bind('r', "Dismiss / remove", Action::Remove),
            ],
            "equipment" => vec![
                bind('g', "Equipment lists", Action::AssignEquipmentList),
                bind('e', "Equip", Action::Equip),
                bind('r', "Return to stash", Action::Unequip),
            ],
            _ => vec![],
        };
        bindings.push(bind('n', "Note", Action::Note));
        bindings.push(bind('c', "Finish stage", Action::Step));
        bindings.push(bind('k', "Skip stage", Action::SkipStage));
        bindings
    }
    pub(super) fn post_targets(&self, db: &Db) -> Vec<Target> {
        let Some(b) = self.battle(db) else {
            return vec![];
        };
        let name = self.post_stage(db).map(|s| s.name.as_str()).unwrap_or("");
        let members = matches!(name, "injuries" | "exploration" | "rare" | "dramatis");
        if members {
            db.members_of(&b.warband_id)
                .filter(|m| {
                    if name == "injuries" {
                        db.participants.iter().any(|p| {
                            p.battle_id == b.id
                                && p.member_id == m.id
                                && p.condition == "out-of-action"
                                && !p.injury_resolved
                        })
                    } else {
                        active(m) && db.unit(&m.unit_id).role == "hero"
                    }
                })
                .map(|m| {
                    let u = db.unit(&m.unit_id);
                    let detail = if name == "injuries" {
                        "Injury unresolved".into()
                    } else if name == "rare" || name == "dramatis" {
                        let s = db
                            .searches
                            .iter()
                            .find(|s| s.battle_id == b.id && s.member_id == m.id && s.kind == name);
                        if name == "rare" {
                            if !db.rare_search_eligible(&b.id, m) {
                                "INELIGIBLE · taken out of action / absent".into()
                            } else if let Some(s) = s.filter(|s| s.used > 0) {
                                format!(
                                    "{} · {}{}",
                                    if s.outcome.is_empty() {
                                        "Search recorded"
                                    } else {
                                        &s.outcome
                                    },
                                    db.items
                                        .iter()
                                        .find(|i| i.id == s.item_id)
                                        .map(|i| i.name.as_str())
                                        .unwrap_or("Legacy search"),
                                    if s.purchased { " · purchased" } else { "" }
                                )
                            } else {
                                "1 search available".into()
                            }
                        } else {
                            s.map(|s| format!("{} searches left", s.allowance - s.used))
                                .unwrap_or_else(|| "No search allowance set".into())
                        }
                    } else {
                        db.participants
                            .iter()
                            .find(|p| p.battle_id == b.id && p.member_id == m.id)
                            .map(|p| p.condition.clone())
                            .unwrap_or_else(|| m.status.clone())
                    };
                    Target {
                        unit: Some(u.id.clone()),
                        member: Some(m.id.clone()),
                        cells: vec![m.name.clone(), db.typ(&u.type_id).name.clone(), detail],
                    }
                })
                .filter(|t| {
                    t.cells
                        .join(" ")
                        .to_lowercase()
                        .contains(&self.filter.to_lowercase())
                })
                .collect()
        } else if matches!(name, "experience" | "recruitment" | "equipment" | "rating") {
            let mut units: Vec<_> = db
                .units
                .iter()
                .filter(|u| {
                    u.warband_id == b.warband_id
                        && db.members.iter().any(|m| m.unit_id == u.id && active(m))
                })
                .collect();
            if name == "experience" {
                units.sort_by_key(|u| db.pending(u).is_empty());
            }
            units
                .into_iter()
                .map(|u| {
                    let ms: Vec<_> = db
                        .members
                        .iter()
                        .filter(|m| m.unit_id == u.id && active(m))
                        .collect();
                    let n = db.pending(u).len();
                    let details = match name {
                        "experience" => {
                            if n > 0 {
                                format!("PLUS-UP READY ({n})")
                            } else {
                                "No plus-up owed".into()
                            }
                        }
                        "equipment" => ms
                            .first()
                            .map(|m| views::gear(db, &m.id))
                            .unwrap_or_default(),
                        "recruitment" => format!(
                            "{} members · {} gc upkeep",
                            ms.len(),
                            db.typ(&u.type_id).upkeep * ms.len() as i64
                        ),
                        _ => format!(
                            "{} rating",
                            ms.len() as u32
                                * (db.typ(&u.type_id).rating + u.starting_xp + u.earned_xp)
                        ),
                    };
                    Target {
                        unit: Some(u.id.clone()),
                        member: ms.first().map(|m| m.id.clone()),
                        cells: vec![
                            u.name.clone(),
                            format!("{} XP", u.starting_xp + u.earned_xp),
                            details,
                        ],
                    }
                })
                .filter(|t| {
                    t.cells
                        .join(" ")
                        .to_lowercase()
                        .contains(&self.filter.to_lowercase())
                })
                .collect()
        } else {
            vec![]
        }
    }
    pub(super) fn post_move(&mut self, db: &Db, delta: isize) {
        if self.post_focus == PostFocus::Targets && self.post_targets(db).is_empty() {
            self.post_focus = PostFocus::Stages;
        }
        if self.post_focus == PostFocus::Stages {
            self.selected[4] = self.selected[4]
                .saturating_add_signed(delta)
                .min(self.steps(db).len().saturating_sub(1));
            self.post_selected = 0;
            self.filter.clear();
        } else {
            self.post_selected = self
                .post_selected
                .saturating_add_signed(delta)
                .min(self.post_targets(db).len().saturating_sub(1));
        }
    }
    pub(super) fn post_open_primary(&mut self, store: &mut Store, cli: &Cli) {
        let stage = self
            .post_stage(&store.db)
            .map(|s| s.name.as_str())
            .unwrap_or("");
        let action = match stage {
            "injuries" => Action::Injury,
            "experience" => {
                if self
                    .post_targets(&store.db)
                    .get(self.post_selected)
                    .and_then(|t| t.unit.as_ref())
                    .is_some_and(|id| !store.db.pending(store.db.unit(id)).is_empty())
                {
                    Action::Advance
                } else {
                    Action::Xp
                }
            }
            "exploration" => Action::Outcome,
            "wyrdstone" => Action::Wyrdstone,
            "veterans" => Action::Veterans,
            "rare" => {
                let member = self
                    .post_targets(&store.db)
                    .get(self.post_selected)
                    .and_then(|t| t.member.clone());
                if store.db.searches.iter().any(|s| {
                    Some(&s.member_id) == member.as_ref()
                        && s.kind == "rare"
                        && s.outcome == "found"
                        && !s.purchased
                        && self.battle(&store.db).is_some_and(|b| b.id == s.battle_id)
                }) {
                    Action::Buy
                } else {
                    Action::Search
                }
            }
            "dramatis" => Action::Search,
            "recruitment" => Action::Hire,
            "equipment" => Action::Equip,
            _ => Action::Step,
        };
        if self
            .post_bindings(&store.db)
            .iter()
            .any(|b| b.action == action)
        {
            self.activate(action, store, cli);
        }
    }
}

impl App {
    pub(super) fn activate(&mut self, action: Action, store: &mut Store, cli: &Cli) {
        if action == Action::Injuries {
            self.tab = 5;
            self.journal = Journal::Injuries;
            self.selected[5] = 0;
            self.filter.clear();
            self.overlay = Overlay::None;
        } else if action == Action::Step && self.tab == 4 {
            self.overlay = Overlay::None;
            self.finish_post_stage(store, cli);
        } else {
            self.open(action, store, cli);
        }
    }
    pub(super) fn finish_post_stage(&mut self, store: &mut Store, cli: &Cli) {
        if !self.post_unlocked(&store.db) {
            self.message = "Select the current unfinished stage to complete it.".into();
            return;
        }
        let name = self.post_stage(&store.db).unwrap().name.clone();
        let result = self.run_command(
            store,
            cli,
            Command::Step {
                name: name.clone(),
                skip: false,
                begin: false,
            },
            cli.date.clone(),
            format!("Player completed after-action stage {name}"),
            false,
        );
        match result {
            Err(e) => self.message = friendly_error(&format!("{e:#}")),
            Ok(_) => {
                self.selected[4] = self
                    .steps(&store.db)
                    .iter()
                    .position(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
                    .unwrap_or_else(|| self.steps(&store.db).len().saturating_sub(1));
                self.post_selected = 0;
                self.post_focus = PostFocus::Stages;
                self.filter.clear();
                self.message = format!(
                    "Completed {name}. Next: {}",
                    self.post_stage(&store.db)
                        .map(|s| s.name.as_str())
                        .unwrap_or("finish battle")
                );
                if self
                    .steps(&store.db)
                    .iter()
                    .all(|s| matches!(s.status.as_str(), "completed" | "skipped"))
                {
                    match self.run_command(
                        store,
                        cli,
                        Command::Battle(BattleCommand::Complete),
                        cli.date.clone(),
                        "After-action completed".into(),
                        false,
                    ) {
                        Ok(_) => {
                            self.tab = 0;
                            self.message =
                                "After-action complete. Post-battle checkpoint saved.".into();
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
            }
        }
    }
}
