use super::*;
impl App {
    pub(super) fn model_notes(&mut self, db: &Db) {
        let selected = self.selection(db);
        let members: Vec<_> = db
            .members
            .iter()
            .filter(|m| {
                self.tab != 1 && selected.member.as_ref().is_some_and(|id| m.id == *id)
                    || (selected.member.is_none() || self.tab == 1)
                        && selected.unit.as_ref().is_some_and(|id| m.unit_id == *id)
            })
            .collect();
        let notes = if members.is_empty() {
            db.notes
                .iter()
                .filter(|n| Some(&n.entity_id) == self.wb.as_ref())
                .map(|n| n.text.clone())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            members
                .iter()
                .map(|m| views::member_notes(db, m))
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        self.overlay = Overlay::Detail {
            title: "Model / unit notes".into(),
            text: if notes.is_empty() {
                "No notes recorded.".into()
            } else {
                notes
            },
            scroll: 0,
        };
    }
    pub(super) fn begin_catalog_entry(&mut self, id: &str, store: &Store) -> bool {
        let action = match id {
            "__new_equipment__" => Action::NewEquipment,
            "__new_ability__" => Action::NewAbility,
            _ => return false,
        };
        let Overlay::Form(parent) = &self.overlay else {
            return false;
        };
        let mut definition = Form::new(
            action,
            store,
            self.wb.as_deref(),
            &self.selection(&store.db),
        );
        if action == Action::NewEquipment {
            if !parent.value("price").is_empty() {
                definition.set("cost", parent.value("price"));
            }
            if parent.action == Action::Search {
                definition.set("rarity", "6");
            }
        } else if !parent.value("unit").is_empty() {
            definition.set("allowed", parent.value("unit"));
        }
        self.suspended_form = Some(parent.clone());
        self.overlay = Overlay::Form(definition);
        self.picker = None;
        true
    }
    pub(super) fn restore_catalog_form(
        &mut self,
        action: Action,
        result: &Value,
        store: &Store,
    ) -> bool {
        if !matches!(action, Action::NewEquipment | Action::NewAbility) {
            return false;
        }
        let Some(old) = self.suspended_form.take() else {
            return false;
        };
        let mut form = Form::new(
            old.action,
            store,
            self.wb.as_deref(),
            &self.selection(&store.db),
        );
        for field in &mut form.fields {
            if let Some(previous) = old.fields.iter().find(|f| f.key == field.key) {
                if let Kind::Choice(options) = &field.kind {
                    if options.iter().any(|c| c.id == previous.value) {
                        field.value = previous.value.clone();
                    }
                } else {
                    *field = previous.clone();
                }
            }
        }
        form.automatic_reason = old.automatic_reason;
        let (key, object) = if action == Action::NewEquipment {
            ("item", "item")
        } else {
            ("ability", "ability")
        };
        if let Some(id) = result[object]["id"].as_str() {
            form.set(key, id);
        }
        if key == "ability" && form.action == Action::Advance {
            form.set("stat", "");
        }
        if form.action == Action::EditEquipmentLists {
            let lists = store
                .db
                .equipment_list_items
                .iter()
                .filter(|r| r.item_id == form.value("item"))
                .map(|r| r.list_id.clone())
                .collect::<Vec<_>>();
            form.set("lists", serde_json::to_string(&lists).unwrap());
        }
        form.focus = form.fields.iter().position(|f| f.key == key).unwrap_or(0);
        form.cursor = form.fields[form.focus].value.chars().count();
        form.refresh_limits(&store.db);
        self.overlay = Overlay::Form(form);
        self.message =
            "Catalog entry saved. Continue the original form; stock and learning are separate."
                .into();
        true
    }
    pub(super) fn cancel_form(&mut self) {
        self.picker = None;
        self.overlay = self
            .suspended_form
            .take()
            .map(Overlay::Form)
            .unwrap_or(Overlay::None);
    }
    pub(super) fn battle_action(&self, db: &Db) -> Action {
        match self.battle(db).map(|b| b.status.as_str()) {
            Some("prepared") => Action::Start,
            Some("in-progress") => Action::End,
            Some("post-battle") => Action::Complete,
            _ => Action::Prepare,
        }
    }
    pub(super) fn battle_lifecycle(&mut self, store: &Store, cli: &Cli) {
        if self
            .battle(&store.db)
            .is_some_and(|b| b.status == "post-battle")
            && self
                .steps(&store.db)
                .iter()
                .any(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
        {
            self.tab = 4;
            self.post_focus = PostFocus::Stages;
            self.selected[4] = self
                .steps(&store.db)
                .iter()
                .position(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
                .unwrap_or(0);
            self.message =
                "Resolve or skip the remaining post-battle stages, then finish the battle.".into();
        } else {
            self.open(self.battle_action(&store.db), store, cli);
        }
    }
}
