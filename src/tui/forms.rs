//! Typed session forms. Choice labels are presentation; values are stable IDs.
use crate::{
    cli::{BattleCommand, Command, EquipmentListCommand},
    model::*,
    store::Store,
};
use anyhow::{Context, Result, ensure};
use std::path::PathBuf;
mod rulebook;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Hire,
    Buy,
    Equip,
    Unequip,
    Xp,
    Advance,
    Promote,
    Remove,
    Status,
    Rename,
    Effect,
    Stat,
    Sell,
    Find,
    Lose,
    Gold,
    Wyrdstone,
    Note,
    Veterans,
    Prepare,
    Start,
    Record,
    AddKill,
    RemoveKill,
    Consume,
    End,
    Injury,
    Outcome,
    Step,
    SkipStage,
    Search,
    Upkeep,
    Complete,
    Abandon,
    Snapshot,
    Compare,
    Restore,
    Undo,
    Export,
    Switch,
    Init,
    Import,
    Campaign,
    Warband,
    CampaignRules,
    Injuries,
    NewEquipment,
    NewEquipmentList,
    AssignEquipmentList,
    EditEquipmentLists,
    NewAbility,
    GrantAbility,
    AiRequest,
    RuleSelect,
    RuleProfile,
    RuleWarrior,
    RuleList,
    RuleItem,
    RuleAbility,
    RuleProgression,
    RuleCopy,
    RuleRemove,
    RuleCreateWarband,
    RuleStartingGear,
}
impl Action {
    pub const ALL: [Self; 64] = [
        Self::Hire,
        Self::Buy,
        Self::Equip,
        Self::Unequip,
        Self::Xp,
        Self::Advance,
        Self::Promote,
        Self::Remove,
        Self::Status,
        Self::Rename,
        Self::Effect,
        Self::Stat,
        Self::Sell,
        Self::Find,
        Self::Lose,
        Self::Gold,
        Self::Wyrdstone,
        Self::Note,
        Self::Veterans,
        Self::Prepare,
        Self::Start,
        Self::Record,
        Self::AddKill,
        Self::RemoveKill,
        Self::Consume,
        Self::End,
        Self::Injury,
        Self::Outcome,
        Self::Step,
        Self::SkipStage,
        Self::Search,
        Self::Upkeep,
        Self::Complete,
        Self::Abandon,
        Self::Snapshot,
        Self::Compare,
        Self::Restore,
        Self::Undo,
        Self::Export,
        Self::Switch,
        Self::Init,
        Self::Import,
        Self::Campaign,
        Self::Warband,
        Self::CampaignRules,
        Self::Injuries,
        Self::NewEquipment,
        Self::NewEquipmentList,
        Self::AssignEquipmentList,
        Self::EditEquipmentLists,
        Self::NewAbility,
        Self::GrantAbility,
        Self::AiRequest,
        Self::RuleSelect,
        Self::RuleProfile,
        Self::RuleWarrior,
        Self::RuleList,
        Self::RuleItem,
        Self::RuleAbility,
        Self::RuleProgression,
        Self::RuleCopy,
        Self::RuleRemove,
        Self::RuleCreateWarband,
        Self::RuleStartingGear,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Hire => "Hire warriors / reinforce group",
            Self::Buy => "Purchase equipment",
            Self::Equip => "Equip unit / group",
            Self::Unequip => "Return equipment to stash",
            Self::Xp => "Add experience",
            Self::Advance => "Resolve pending plus-up",
            Self::Promote => "Promote henchman to Hero",
            Self::Remove => "Fire / dismiss, retire or record death",
            Self::Status => "Recovery, capture or leader",
            Self::Rename => "Rename member / unit",
            Self::Effect => "Record injury / skill effect",
            Self::Stat => "Resolve random characteristic",
            Self::Sell => "Sell equipment",
            Self::Find => "Record found equipment",
            Self::Lose => "Record lost equipment",
            Self::Gold => "Adjust treasury / record reward",
            Self::Wyrdstone => "Sell wyrdstone",
            Self::Note => "Add or update a note",
            Self::Veterans => "Record available veteran experience",
            Self::Prepare => "Prepare a battle",
            Self::Start => "Start prepared battle",
            Self::Record => "Wounds, condition, kills and objectives",
            Self::AddKill => "Credit a kill / OOA",
            Self::RemoveKill => "Remove a kill credit",
            Self::Consume => "Consume carried equipment",
            Self::End => "End battle / record result",
            Self::Injury => "Resolve post-battle injury",
            Self::Outcome => "Resolve exploration / other outcome",
            Self::Step => "Finish selected stage",
            Self::SkipStage => "Skip selected stage",
            Self::Search => "Record rare-item / character search",
            Self::Upkeep => "Settle hired-sword upkeep",
            Self::Complete => "Complete post-battle sequence",
            Self::Abandon => "Abandon active battle",
            Self::Snapshot => "Create snapshot",
            Self::Compare => "Compare checkpoint with current state",
            Self::Restore => "Restore snapshot (whole database)",
            Self::Undo => "Undo last change (whole database)",
            Self::Export => "Export roster / backup",
            Self::Switch => "Switch warband",
            Self::Init => "Create campaign and warband",
            Self::Import => "Import opening warband",
            Self::Campaign => "Create campaign",
            Self::Warband => "Create another warband",
            Self::CampaignRules => "Campaign rules",
            Self::Injuries => "Injury list",
            Self::NewEquipment => "Define new equipment",
            Self::NewEquipmentList => "Create shared equipment list",
            Self::AssignEquipmentList => "Assign equipment list to types / models",
            Self::EditEquipmentLists => "Change an item's equipment lists",
            Self::NewAbility => "Define new skill / spell / prayer",
            Self::GrantAbility => "Record a learned skill (no XP advance)",
            Self::AiRequest => "Ask the AI assistant",
            Self::RuleSelect => "Choose rules profile",
            Self::RuleProfile => "Define rules profile / funding / limits",
            Self::RuleWarrior => "Define Hero / Henchman / Hired Sword",
            Self::RuleList => "Define equipment list",
            Self::RuleItem => "Define equipment and list membership",
            Self::RuleAbility => "Define skill / spell / prayer and access",
            Self::RuleProgression => "Define XP progression thresholds",
            Self::RuleCopy => "Copy rules profile",
            Self::RuleRemove => "Remove unused rule definition",
            Self::RuleCreateWarband => "Create warband from authored rules",
            Self::RuleStartingGear => "Define starting equipment / quantities",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Choice {
    pub id: String,
    pub label: String,
}
impl Choice {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum Kind {
    Text,
    Choice(Vec<Choice>),
    Multiple(Vec<Choice>),
    Toggle,
}
#[derive(Clone, Debug)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub kind: Kind,
    pub hint: &'static str,
    pub replace_on_input: bool,
}
impl Field {
    fn text(
        key: &'static str,
        label: &'static str,
        value: impl Into<String>,
        hint: &'static str,
    ) -> Self {
        let value = value.into();
        Self {
            key,
            label,
            replace_on_input: !value.is_empty(),
            value,
            kind: Kind::Text,
            hint,
        }
    }
    fn choices(
        key: &'static str,
        label: &'static str,
        choices: Vec<Choice>,
        preferred: Option<&str>,
    ) -> Self {
        let value = preferred
            .filter(|v| choices.iter().any(|c| c.id == *v))
            .map(str::to_string)
            .or_else(|| choices.first().map(|c| c.id.clone()))
            .unwrap_or_default();
        Self {
            key,
            label,
            value,
            kind: Kind::Choice(choices),
            hint: "Enter: searchable picker. Left/Right: cycle choices.",
            replace_on_input: false,
        }
    }
    fn multiple(
        key: &'static str,
        label: &'static str,
        choices: Vec<Choice>,
        selected: Vec<String>,
    ) -> Self {
        Self {
            key,
            label,
            value: serde_json::to_string(&selected).unwrap(),
            kind: Kind::Multiple(choices),
            hint: "Enter: check multiple choices. Space/click toggles; Enter accepts; Esc cancels. Ctrl-A all shown, Ctrl-U clears shown.",
            replace_on_input: false,
        }
    }
    pub fn selected_values(&self) -> Vec<String> {
        serde_json::from_str(&self.value).unwrap_or_default()
    }
    fn toggle(key: &'static str, label: &'static str) -> Self {
        Self {
            key,
            label,
            value: "false".into(),
            kind: Kind::Toggle,
            hint: "Space or Left/Right: toggle.",
            replace_on_input: false,
        }
    }
    pub fn display(&self) -> String {
        match &self.kind {
            Kind::Multiple(cs) => {
                let selected = self.selected_values();
                let labels = cs
                    .iter()
                    .filter(|c| selected.contains(&c.id))
                    .map(|c| c.label.as_str())
                    .collect::<Vec<_>>();
                if labels.is_empty() {
                    "None selected".into()
                } else {
                    labels.join("; ")
                }
            }
            Kind::Choice(cs) => cs
                .iter()
                .find(|c| c.id == self.value)
                .map(|c| c.label.clone())
                .unwrap_or_else(|| "No choices available".into()),
            Kind::Toggle => if self.value == "true" {
                "[x] Yes"
            } else {
                "[ ] No"
            }
            .into(),
            Kind::Text => self.value.clone(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Form {
    pub action: Action,
    pub fields: Vec<Field>,
    pub focus: usize,
    pub cursor: usize,
    pub offset: usize,
    pub error: String,
    pub reviewing: bool,
    pub review_scroll: u16,
    pub warband_id: Option<String>,
    pub automatic_reason: bool,
    pub rule_id: Option<String>,
}
#[derive(Default)]
pub struct Selection {
    pub unit: Option<String>,
    pub member: Option<String>,
    pub item: Option<String>,
    pub snapshot: Option<String>,
    pub note: Option<String>,
    pub stage: Option<String>,
    pub rule_profile: Option<String>,
    pub rule_record: Option<String>,
    pub rule_kind: Option<String>,
}
fn opts(values: &[&str]) -> Vec<Choice> {
    values.iter().map(|v| Choice::new(*v, *v)).collect()
}
fn maybe(mut c: Vec<Choice>, label: &str) -> Vec<Choice> {
    c.insert(0, Choice::new("", label));
    c
}
impl Form {
    fn multiple(&self, key: &str, required: bool) -> Result<Vec<String>> {
        let field = self
            .fields
            .iter()
            .find(|f| f.key == key)
            .context("Missing selection field")?;
        let Kind::Multiple(choices) = &field.kind else {
            anyhow::bail!("Expected a multiple choice field")
        };
        let mut selected: Vec<String> =
            serde_json::from_str(&field.value).context("Invalid multiple selection")?;
        selected.sort();
        selected.dedup();
        ensure!(
            selected
                .iter()
                .all(|id| choices.iter().any(|c| c.id == *id)),
            "Selection contains an unavailable choice"
        );
        ensure!(
            !required || !selected.is_empty(),
            "Select at least one {}",
            field.label
        );
        Ok(selected)
    }
    pub fn new(action: Action, store: &Store, wb: Option<&str>, selection: &Selection) -> Self {
        let db = &store.db;
        let profile = wb
            .and_then(|w| db.warbands.iter().find(|r| r.id == w))
            .map(|w| w.profile_id.as_str());
        let b = wb.and_then(|w| {
            db.battles.iter().rev().find(|b| {
                b.warband_id == w && !matches!(b.status.as_str(), "complete" | "abandoned")
            })
        });
        let current = b.and_then(|b| {
            db.steps.iter().find(|s| {
                s.battle_id == b.id && !matches!(s.status.as_str(), "completed" | "skipped")
            })
        });
        let post_stage = b.filter(|b| b.status == "post-battle").and_then(|_| {
            selection
                .stage
                .as_deref()
                .or(current.map(|s| s.name.as_str()))
        });
        let units: Vec<_> = db
            .units
            .iter()
            .filter(|u| {
                Some(u.warband_id.as_str()) == wb
                    && db.members.iter().any(|m| m.unit_id == u.id && active(m))
            })
            .collect();
        let members: Vec<_> = db
            .members
            .iter()
            .filter(|m| units.iter().any(|u| u.id == m.unit_id) && active(m))
            .collect();
        let unit_choices = || {
            units
                .iter()
                .map(|u| {
                    Choice::new(
                        &u.id,
                        format!(
                            "{} · {} · {} XP",
                            u.name,
                            db.typ(&u.type_id).name,
                            u.starting_xp + u.earned_xp
                        ),
                    )
                })
                .collect::<Vec<_>>()
        };
        let member_choices = || {
            members
                .iter()
                .map(|m| {
                    Choice::new(
                        &m.id,
                        format!(
                            "{} · {} · member {}",
                            m.name,
                            db.unit(&m.unit_id).name,
                            db.members
                                .iter()
                                .filter(|r| r.unit_id == m.unit_id)
                                .position(|r| r.id == m.id)
                                .unwrap_or(0)
                                + 1
                        ),
                    )
                })
                .collect::<Vec<_>>()
        };
        let items = || {
            let mut choices: Vec<_> = db
                .items
                .iter()
                .filter(|i| Some(i.profile_id.as_str()) == profile)
                .filter(|i| {
                    action != Action::Buy
                        || match post_stage {
                            Some("rare") => i.rarity > 0,
                            Some("recruitment") => i.rarity == 0,
                            _ => true,
                        }
                })
                .map(|i| {
                    let (owned, assigned) = wb
                        .map(|w| (db.owned(w, &i.id), db.assigned(w, &i.id)))
                        .unwrap_or_default();
                    Choice::new(
                        &i.id,
                        format!(
                            "{} · {} gc · {}/{} in stash",
                            i.name,
                            i.cost,
                            owned - assigned,
                            owned
                        ),
                    )
                })
                .collect();
            choices.push(Choice::new("__new_equipment__", "+ New equipment..."));
            choices
        };
        let abilities = || {
            let mut choices: Vec<_> = db
                .abilities
                .iter()
                .filter(|a| Some(a.profile_id.as_str()) == profile)
                .map(|a| Choice::new(&a.id, format!("{} · {}", a.name, a.kind)))
                .collect();
            choices.push(Choice::new(
                "__new_ability__",
                "+ New skill / spell / prayer...",
            ));
            choices
        };
        let unit = || {
            Field::choices(
                "unit",
                "Unit / group",
                unit_choices(),
                selection.unit.as_deref(),
            )
        };
        let member = || {
            Field::choices(
                "member",
                "Individual member",
                member_choices(),
                selection.member.as_deref(),
            )
        };
        let item = || Field::choices("item", "Equipment", items(), selection.item.as_deref());
        let quantity = || {
            Field::text(
                "quantity",
                "Quantity",
                "1",
                "Whole copies; group equipment is per model.",
            )
        };
        let price = || {
            Field::text(
                "price",
                "Unit price (gold)",
                "",
                "Leave blank to use catalog / hire cost.",
            )
        };
        let text = |key, label, value, hint| Field::text(key, label, value, hint);
        let choices = |key, label, values: &[&str]| Field::choices(key, label, opts(values), None);
        let allowed = || {
            let mut targets = vec![
                Choice::new("all", "All warrior types in this profile"),
                Choice::new("heroes", "Hero / hired-sword types and promoted Heroes"),
            ];
            targets.extend(
                db.types
                    .iter()
                    .filter(|t| Some(t.profile_id.as_str()) == profile)
                    .map(|t| Choice::new(&t.id, format!("Type: {}", t.name))),
            );
            targets.extend(
                units
                    .iter()
                    .map(|u| Choice::new(&u.id, format!("This unit: {}", u.name))),
            );
            Field::choices(
                "allowed",
                "Available to",
                targets,
                if action == Action::NewAbility {
                    selection.unit.as_deref().or(Some("heroes"))
                } else {
                    Some("all")
                },
            )
        };
        let list_choices = || {
            db.equipment_lists
                .iter()
                .filter(|l| Some(l.profile_id.as_str()) == profile)
                .map(|l| {
                    let types = db
                        .types
                        .iter()
                        .filter(|t| {
                            db.equipment_list_for(&t.id)
                                .is_some_and(|list| list.id == l.id)
                        })
                        .map(|t| t.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    Choice::new(
                        &l.id,
                        if types.is_empty() {
                            l.name.clone()
                        } else {
                            format!("{} · {}", l.name, types)
                        },
                    )
                })
                .collect::<Vec<_>>()
        };
        let list_ids = || {
            db.equipment_lists
                .iter()
                .filter(|l| Some(l.profile_id.as_str()) == profile)
                .map(|l| l.id.clone())
                .collect::<Vec<_>>()
        };
        let mut fields = match action {
            Action::RuleSelect
            | Action::RuleProfile
            | Action::RuleWarrior
            | Action::RuleList
            | Action::RuleItem
            | Action::RuleAbility
            | Action::RuleProgression
            | Action::RuleCopy
            | Action::RuleRemove
            | Action::RuleCreateWarband
            | Action::RuleStartingGear => Self::rule_fields(
                action,
                db,
                selection.rule_profile.as_deref().or(profile),
                selection,
            ),
            Action::NewEquipment => vec![
                text(
                    "name",
                    "Equipment name",
                    "",
                    "New catalog entry; no rule-file editing needed.",
                ),
                text(
                    "cost",
                    "Catalog cost (gold)",
                    "0",
                    "Actual purchase price can be entered separately.",
                ),
                choices(
                    "category",
                    "Category",
                    &["melee", "missile", "armour", "equipment", "consumable"],
                ),
                text(
                    "rarity",
                    "Rarity (0 = common)",
                    "0",
                    "Normal rare-item search rules apply.",
                ),
                text(
                    "description",
                    "Rules / description",
                    "",
                    "Include two-handed use, range, effects or other rules.",
                ),
                Field::multiple("lists", "Equipment lists", list_choices(), list_ids()),
            ],
            Action::NewEquipmentList => vec![
                text(
                    "name",
                    "List name",
                    "",
                    "A named list shared by multiple warrior types.",
                ),
                text(
                    "description",
                    "Description",
                    "",
                    "Rules source or notes about this equipment list.",
                ),
                Field::multiple(
                    "types",
                    "Assign to warrior types",
                    db.types
                        .iter()
                        .filter(|t| Some(t.profile_id.as_str()) == profile)
                        .map(|t| Choice::new(&t.id, &t.name))
                        .collect(),
                    selection
                        .unit
                        .as_deref()
                        .map(|u| vec![db.unit(u).type_id.clone()])
                        .unwrap_or_default(),
                ),
                Field::multiple(
                    "items",
                    "Equipment in this list",
                    db.items
                        .iter()
                        .filter(|i| Some(i.profile_id.as_str()) == profile)
                        .map(|i| Choice::new(&i.id, &i.name))
                        .collect(),
                    selection.item.iter().cloned().collect(),
                ),
            ],
            Action::AssignEquipmentList => {
                let mut lists = list_choices();
                lists.push(Choice::new(
                    "inherit",
                    "Inherit default (models / groups only)",
                ));
                let target = selection.unit.as_deref().or(selection.member.as_deref());
                vec![
                    Field::choices(
                        "list",
                        "Equipment list",
                        lists,
                        target
                            .and_then(|e| db.equipment_list_for(e))
                            .map(|l| l.id.as_str()),
                    ),
                    Field::multiple(
                        "targets",
                        "Types / units / models",
                        db.types
                            .iter()
                            .filter(|t| Some(t.profile_id.as_str()) == profile)
                            .map(|t| {
                                Choice::new(
                                    &t.id,
                                    format!(
                                        "Type: {} · {}",
                                        t.name,
                                        db.equipment_list_for(&t.id)
                                            .map_or("No list", |l| l.name.as_str())
                                    ),
                                )
                            })
                            .chain(
                                units
                                    .iter()
                                    .map(|u| Choice::new(&u.id, format!("Unit: {}", u.name))),
                            )
                            .chain(members.iter().map(|m| {
                                Choice::new(
                                    &m.id,
                                    format!("Model: {} · {}", m.name, db.unit(&m.unit_id).name),
                                )
                            }))
                            .collect(),
                        target.map(|t| vec![t.to_string()]).unwrap_or_default(),
                    ),
                ]
            }
            Action::EditEquipmentLists => {
                let item = item();
                let selected = db
                    .equipment_list_items
                    .iter()
                    .filter(|r| r.item_id == item.value)
                    .map(|r| r.list_id.clone())
                    .collect();
                vec![
                    item,
                    Field::multiple("lists", "Equipment lists", list_choices(), selected),
                ]
            }
            Action::NewAbility => vec![
                text(
                    "name",
                    "Skill / spell name",
                    "",
                    "Adds a definition; learning it is a separate action.",
                ),
                choices(
                    "kind",
                    "Category",
                    &[
                        "combat", "shooting", "strength", "academic", "speed", "special", "spell",
                        "prayer",
                    ],
                ),
                text(
                    "description",
                    "Rules / description",
                    "",
                    "Player-entered rules; no automatic stat changes.",
                ),
                allowed(),
            ],
            Action::GrantAbility => vec![
                unit(),
                Field::choices("ability", "Skill / spell / prayer", abilities(), None),
            ],
            Action::AiRequest => vec![text(
                "prompt",
                "Request",
                "",
                "Send selected warband, rules, notes and battle data to your configured AI. Changes always need your approval.",
            )],
            Action::Hire => vec![
                Field::choices(
                    "kind",
                    "Warrior type",
                    db.types
                        .iter()
                        .filter(|t| Some(t.profile_id.as_str()) == profile)
                        .filter(|t| post_stage != Some("dramatis") || t.role == "dramatis")
                        .map(|t| {
                            Choice::new(&t.id, format!("{} · {} gc · {}", t.name, t.cost, t.role))
                        })
                        .collect(),
                    None,
                ),
                text(
                    "name",
                    "Name / group name",
                    "",
                    "Required; group members get individual names.",
                ),
                quantity(),
                Field::choices(
                    "group",
                    "Reinforce group",
                    maybe(
                        units
                            .iter()
                            .filter(|u| u.role == "henchman" && post_stage != Some("dramatis"))
                            .map(|u| Choice::new(&u.id, &u.name))
                            .collect(),
                        "Create a new unit / group",
                    ),
                    None,
                ),
                price(),
            ],
            Action::Buy | Action::Sell => vec![item(), quantity(), price()],
            Action::Find | Action::Lose => vec![item(), quantity()],
            Action::Equip => vec![unit(), item(), quantity()],
            Action::Unequip => vec![unit(), item()],
            Action::Xp => vec![
                unit(),
                text(
                    "amount",
                    "Experience to add",
                    "1",
                    "Awarded once to the unit; group XP is shared.",
                ),
            ],
            Action::Advance => vec![
                Field::choices(
                    "unit",
                    "Unit / group",
                    units
                        .iter()
                        .filter(|u| !db.pending(u).is_empty())
                        .map(|u| {
                            Choice::new(
                                &u.id,
                                format!("{} · {} PLUS-UP(S) READY", u.name, db.pending(u).len()),
                            )
                        })
                        .collect(),
                    selection.unit.as_deref(),
                ),
                Field::choices(
                    "stat",
                    "Characteristic",
                    maybe(opts(&STATS), "Choose an ability instead"),
                    None,
                ),
                Field::choices(
                    "ability",
                    "Skill / spell / prayer",
                    maybe(abilities(), "Characteristic advance instead"),
                    None,
                ),
                Field::toggle("initial", "Purchased initial advance"),
            ],
            Action::Promote => vec![
                member(),
                Field::choices(
                    "access1",
                    "First skill category",
                    opts(&["shooting", "strength", "combat", "academic", "speed"]),
                    None,
                ),
                Field::choices(
                    "access2",
                    "Second skill category",
                    opts(&["strength", "shooting", "combat", "academic", "speed"]),
                    None,
                ),
            ],
            Action::Remove => vec![
                member(),
                choices("status", "Departure", &["dismissed", "dead", "retired"]),
                choices("equipment", "Carried equipment", &["stash", "lose"]),
            ],
            Action::Status => {
                let mut fields = vec![
                    member(),
                    choices(
                        "status",
                        "Availability",
                        &["active", "recovering", "captured"],
                    ),
                    text("missed", "Games to miss", "0", "Recovery time remaining."),
                    Field::toggle("leader", "Make this Hero the leader"),
                ];
                if let Some(m) = members.iter().find(|m| m.id == fields[0].value) {
                    fields[1].value = m.status.clone();
                    fields[2].value = m.missed_games.to_string();
                }
                fields
            }
            Action::Rename => {
                let mut targets: Vec<_> = units
                    .iter()
                    .map(|u| Choice::new(&u.id, format!("Unit / group: {}", u.name)))
                    .collect();
                targets.extend(members.iter().map(|m| {
                    Choice::new(
                        &m.id,
                        format!("Member: {} · {}", m.name, db.unit(&m.unit_id).name),
                    )
                }));
                vec![
                    Field::choices(
                        "entity",
                        "Rename target",
                        targets,
                        selection.unit.as_deref(),
                    ),
                    text("name", "New name", "", "Identity and history are retained."),
                ]
            }
            Action::Effect => vec![
                member(),
                text(
                    "description",
                    "Injury / effect",
                    "",
                    "Describe the lasting or conditional effect.",
                ),
                Field::choices(
                    "stat",
                    "Characteristic",
                    maybe(opts(&STATS), "Descriptive effect only"),
                    None,
                ),
                text(
                    "delta",
                    "Change",
                    "0",
                    "Signed change, e.g. -1 for a leg wound.",
                ),
                text(
                    "condition",
                    "Applies when",
                    "",
                    "Blank: always; e.g. Close combat only.",
                ),
            ],
            Action::Stat => vec![
                member(),
                choices("stat", "Characteristic", &STATS),
                text(
                    "value",
                    "Resolved base value",
                    "1",
                    "Advances and lasting effects remain additive.",
                ),
                Field::toggle("battle", "This battle only"),
            ],
            Action::Gold => vec![
                text(
                    "amount",
                    "Gold change",
                    "0",
                    "Positive reward / funding; negative expense.",
                ),
                choices(
                    "category",
                    "Category",
                    &["reward", "funding", "expense", "adjustment"],
                ),
            ],
            Action::Wyrdstone => vec![
                text(
                    "shards",
                    "Shards to sell",
                    "0",
                    "Zero records no sale; sold shards leave unassigned inventory.",
                ),
                text(
                    "gold",
                    "Total gold received",
                    "0",
                    "Campaign-agreed total, not price per shard.",
                ),
            ],
            Action::Note => {
                let mut targets = vec![Choice::new(wb.unwrap_or_default(), "Warband")];
                targets.extend(
                    db.units
                        .iter()
                        .filter(|u| Some(u.warband_id.as_str()) == wb)
                        .map(|u| Choice::new(&u.id, format!("Unit: {}", u.name))),
                );
                targets.extend(
                    db.members
                        .iter()
                        .filter(|m| {
                            db.units
                                .iter()
                                .any(|u| u.id == m.unit_id && Some(u.warband_id.as_str()) == wb)
                        })
                        .map(|m| Choice::new(&m.id, member_label(db, m))),
                );
                targets.extend(
                    db.battles
                        .iter()
                        .filter(|b| Some(b.warband_id.as_str()) == wb)
                        .map(|b| Choice::new(&b.id, format!("Battle: {}", b.name))),
                );
                vec![
                    Field::choices(
                        "entity",
                        "Attach note to",
                        targets,
                        selection.member.as_deref().or(selection.unit.as_deref()),
                    ),
                    text(
                        "text",
                        "Note / comment",
                        "",
                        "Free text. Previous notes stay in history.",
                    ),
                    Field::choices(
                        "replace",
                        "Supersede note",
                        maybe(
                            db.notes
                                .iter()
                                .filter(|n| {
                                    n.entity_id != "selection"
                                        && (Some(n.entity_id.as_str()) == wb
                                            || db.members.iter().any(|m| {
                                                m.id == n.entity_id
                                                    && db.units.iter().any(|u| {
                                                        u.id == m.unit_id
                                                            && Some(u.warband_id.as_str()) == wb
                                                    })
                                            })
                                            || db.units.iter().any(|u| {
                                                u.id == n.entity_id
                                                    && Some(u.warband_id.as_str()) == wb
                                            })
                                            || db.battles.iter().any(|b| {
                                                b.id == n.entity_id
                                                    && Some(b.warband_id.as_str()) == wb
                                            }))
                                })
                                .map(|n| {
                                    Choice::new(
                                        &n.id,
                                        format!(
                                            "{} · {}: {}",
                                            n.date,
                                            super::entity_name(db, &n.entity_id),
                                            n.text
                                        ),
                                    )
                                })
                                .collect(),
                            "Add a new note",
                        ),
                        selection.note.as_deref(),
                    ),
                ]
            }
            Action::Veterans => vec![text(
                "experience",
                "Available veteran XP",
                "0",
                "Enter the total available experience from your physical roll.",
            )],
            Action::Prepare => vec![
                text("name", "Battle name", "", "Required."),
                text(
                    "scenario",
                    "Scenario",
                    "Skirmish",
                    "Scenario determines XP eligibility.",
                ),
                text("opponent", "Opponent", "Unknown", "Warband name."),
                text(
                    "rating",
                    "Opponent rating",
                    "0",
                    "Pre-battle rating for underdog awards.",
                ),
            ],
            Action::Record => {
                let participants: Vec<_> = b
                    .map(|b| {
                        db.participants
                            .iter()
                            .filter(|p| p.battle_id == b.id)
                            .collect()
                    })
                    .unwrap_or_default();
                let mut f = vec![
                    Field::choices(
                        "member",
                        "Battle participant",
                        participants
                            .iter()
                            .map(|p| {
                                let m = db.members.iter().find(|m| m.id == p.member_id).unwrap();
                                Choice::new(&m.id, member_label(db, m))
                            })
                            .collect(),
                        selection.member.as_deref(),
                    ),
                    text(
                        "wounds",
                        "Wounds remaining",
                        "0",
                        "Current wounds, separate from the W characteristic.",
                    ),
                    choices(
                        "condition",
                        "Condition",
                        &[
                            "standing",
                            "knocked-down",
                            "stunned",
                            "out-of-action",
                            "routed",
                        ],
                    ),
                    text(
                        "kills",
                        "Kills / OOA credited",
                        "0",
                        "A bookkeeping total; confirm XP eligibility.",
                    ),
                    text("objectives", "Objectives", "0", "Scenario-specific count."),
                ];
                if let Some(p) = participants.iter().find(|p| p.member_id == f[0].value) {
                    f[1].value = p.wounds.to_string();
                    f[2].value = p.condition.clone();
                    f[3].value = p.kills.to_string();
                    f[4].value = p.objectives.to_string();
                }
                f
            }
            Action::AddKill | Action::RemoveKill => {
                let mut fields = vec![Field::choices(
                    "member",
                    "Credited warrior",
                    b.map(|b| {
                        db.participants
                            .iter()
                            .filter(|p| p.battle_id == b.id)
                            .filter_map(|p| db.members.iter().find(|m| m.id == p.member_id))
                            .map(|m| Choice::new(&m.id, member_label(db, m)))
                            .collect()
                    })
                    .unwrap_or_default(),
                    selection.member.as_deref(),
                )];
                if action == Action::AddKill {
                    fields.push(text("victim", "Opponent taken out", "", "Name or description; blank records an unspecified opponent. This does not record a death or award XP."));
                } else {
                    fields.push(Field::choices(
                        "kill",
                        "Credit to remove",
                        kill_choices(db, b, &fields[0].value),
                        None,
                    ));
                }
                fields
            }
            Action::Consume => vec![member(), item(), quantity()],
            Action::End => vec![choices("result", "Battle result", &["win", "loss", "draw"])],
            Action::Injury => vec![
                Field::choices(
                    "member",
                    "Out-of-action member",
                    b.map(|b| {
                        db.participants
                            .iter()
                            .filter(|p| {
                                p.battle_id == b.id
                                    && p.condition == "out-of-action"
                                    && !p.injury_resolved
                            })
                            .map(|p| {
                                let m = db.members.iter().find(|m| m.id == p.member_id).unwrap();
                                Choice::new(&m.id, member_label(db, m))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                    selection.member.as_deref(),
                ),
                text(
                    "outcome",
                    "Outcome",
                    "recovery",
                    "recovery, dead, captured or a named lasting injury.",
                ),
                Field::choices(
                    "stat",
                    "Characteristic affected",
                    maybe(opts(&STATS), "No characteristic change"),
                    None,
                ),
                text("delta", "Characteristic change", "0", "Signed change."),
                text("missed", "Games to miss", "0", "Missed future battles."),
            ],
            Action::Outcome => vec![
                text(
                    "key",
                    "Outcome label",
                    selection
                        .stage
                        .as_deref()
                        .or(current.map(|s| s.name.as_str()))
                        .unwrap_or("exploration"),
                    "Unique in this battle; prevents duplicate rewards.",
                ),
                text(
                    "description",
                    "What happened",
                    "",
                    "Include participants, choices and table result.",
                ),
                text(
                    "gold",
                    "Gold change",
                    "0",
                    "Optional signed reward / expense.",
                ),
                text(
                    "shards",
                    "Shards found",
                    "0",
                    "Added to stock, not automatically sold.",
                ),
            ],
            Action::SkipStage => vec![text(
                "stage",
                "Stage",
                selection
                    .stage
                    .as_deref()
                    .or(current.map(|s| s.name.as_str()))
                    .unwrap_or(""),
                "The selected stage will be explicitly skipped.",
            )],
            Action::Step => vec![
                Field::choices(
                    "stage",
                    "Post-battle stage",
                    b.map(|b| {
                        db.steps
                            .iter()
                            .filter(|s| s.battle_id == b.id)
                            .map(|s| Choice::new(&s.name, format!("{} · {}", s.name, s.status)))
                            .collect()
                    })
                    .unwrap_or_default(),
                    current
                        .map(|s| s.name.as_str())
                        .or(selection.stage.as_deref()),
                ),
                choices("mode", "Action", &["complete", "begin", "skip"]),
            ],
            Action::Search if post_stage != Some("dramatis") => vec![
                Field::choices(
                    "member",
                    "Searching Hero",
                    members
                        .iter()
                        .filter(|m| {
                            b.is_some_and(|b| {
                                db.rare_search_eligible(&b.id, m)
                                    && db.rare_search_remaining(&b.id, &m.id) > 0
                            })
                        })
                        .map(|m| Choice::new(&m.id, format!("{} · 1 search left", m.name)))
                        .collect(),
                    selection.member.as_deref(),
                ),
                Field::choices(
                    "item",
                    "Rare item sought",
                    db.items
                        .iter()
                        .filter(|i| Some(i.profile_id.as_str()) == profile && i.rarity > 0)
                        .map(|i| {
                            Choice::new(
                                &i.id,
                                format!("{} · Rare {} · {} gc", i.name, i.rarity, i.cost),
                            )
                        })
                        .chain(std::iter::once(Choice::new(
                            "__new_equipment__",
                            "+ New rare equipment...",
                        )))
                        .collect(),
                    selection.item.as_deref(),
                ),
                choices("outcome", "Search result", &["found", "failed"]),
            ],
            Action::Search => vec![
                Field::choices(
                    "member",
                    "Searching Hero",
                    members
                        .iter()
                        .filter(|m| db.unit(&m.unit_id).role == "hero" && m.status == "active")
                        .map(|m| Choice::new(&m.id, member_label(db, m)))
                        .collect(),
                    selection.member.as_deref(),
                ),
                text(
                    "kind",
                    "Search kind",
                    if selection.stage.as_deref() == Some("dramatis") {
                        "dramatis"
                    } else {
                        "rare"
                    },
                    "Dramatis Personae searches are separate from rare-item trading.",
                ),
                text(
                    "allowance",
                    "Total allowance",
                    "",
                    "Enter the agreed total; blank keeps the existing allowance.",
                ),
                Field::toggle("use", "Spend one attempt"),
            ],
            Action::Upkeep => vec![
                Field::choices(
                    "member",
                    "Hired member",
                    members
                        .iter()
                        .filter(|m| db.typ(&db.unit(&m.unit_id).type_id).upkeep > 0)
                        .map(|m| {
                            Choice::new(
                                &m.id,
                                format!(
                                    "{} · {} gc",
                                    m.name,
                                    db.typ(&db.unit(&m.unit_id).type_id).upkeep
                                ),
                            )
                        })
                        .collect(),
                    selection.member.as_deref(),
                ),
                choices("mode", "Settlement", &["pay", "waive", "depart"]),
            ],
            Action::Abandon => vec![text(
                "disposition",
                "Items and pending outcomes",
                "",
                "Describe what is retained, spent or abandoned.",
            )],
            Action::Snapshot => vec![text("name", "Snapshot name", "", "Name this checkpoint.")],
            Action::Compare => vec![Field::choices(
                "snapshot",
                "Compare checkpoint",
                db.snapshots
                    .iter()
                    .filter(|s| Some(s.warband_id.as_str()) == wb)
                    .rev()
                    .map(|s| Choice::new(&s.id, &s.name))
                    .collect(),
                selection.snapshot.as_deref(),
            )],
            Action::Restore => vec![Field::choices(
                "snapshot",
                "Snapshot",
                db.snapshots
                    .iter()
                    .rev()
                    .map(|s| Choice::new(&s.id, &s.name))
                    .collect(),
                selection.snapshot.as_deref(),
            )],
            Action::Export => vec![
                text(
                    "path",
                    "Output path",
                    "roster.md",
                    "Existing files will not be overwritten.",
                ),
                choices(
                    "format",
                    "Format",
                    &["markdown", "ledger", "inventory", "bundle", "json", "csv"],
                ),
            ],
            Action::Switch => vec![Field::choices(
                "warband",
                "Warband",
                db.warbands
                    .iter()
                    .map(|w| {
                        Choice::new(
                            &w.id,
                            format!(
                                "{} · {}",
                                w.name,
                                db.campaigns
                                    .iter()
                                    .find(|c| c.id == w.campaign_id)
                                    .map(|c| c.name.as_str())
                                    .unwrap_or("")
                            ),
                        )
                    })
                    .collect(),
                wb,
            )],
            Action::Init => vec![
                text(
                    "rules",
                    "Rules folder",
                    "examples/rules",
                    "CSV rules definitions to pin.",
                ),
                Field::choices(
                    "profile",
                    "Profile",
                    crate::rules::load(std::path::Path::new("examples/rules"))
                        .map(|rules| {
                            rules
                                .profiles
                                .iter()
                                .map(|p| Choice::new(&p.id, &p.name))
                                .collect()
                        })
                        .unwrap_or_default(),
                    Some("reikland"),
                ),
                text("campaign", "Campaign name", "", "Required."),
                text("name", "Warband name", "", "Required."),
            ],
            Action::Import => vec![text(
                "path",
                "Opening-state file",
                "examples/burgomeisters.json",
                "JSON file or CSV tables folder; requires empty database.",
            )],
            Action::Campaign => vec![text("name", "Campaign name", "", "Required.")],
            Action::CampaignRules => {
                let mut f = Field::toggle("skip_dramatis", "Skip Dramatis Personae");
                f.value = wb.is_some_and(|w| db.skips_dramatis(w)).to_string();
                f.hint = "Campaign-wide: automatically skip and hide this stage for all warbands and future battles.";
                vec![f]
            }
            Action::Injuries => vec![],
            Action::Warband => vec![
                Field::choices(
                    "campaign",
                    "Campaign",
                    db.campaigns
                        .iter()
                        .map(|c| Choice::new(&c.id, &c.name))
                        .collect(),
                    None,
                ),
                Field::choices(
                    "profile",
                    "Profile",
                    db.profiles
                        .iter()
                        .map(|p| Choice::new(&p.id, &p.name))
                        .collect(),
                    profile,
                ),
                text("name", "Warband name", "", "Required."),
            ],
            Action::Start | Action::Complete | Action::Undo => vec![],
        };
        if action == Action::Advance && b.is_some() {
            fields.retain(|f| f.key != "initial");
        }
        fields.push(Field::text(
            "date",
            "Effective date",
            chrono::Local::now().date_naive().to_string(),
            "YYYY-MM-DD; defaults to today.",
        ));
        fields.push(Field::text(
            "reason",
            "Reason / justification",
            "",
            "Optional note; restoring history requires a reason.",
        ));
        if matches!(action, Action::AiRequest | Action::RuleSelect) {
            fields.retain(|f| !matches!(f.key, "date" | "reason"));
        }
        let cursor = fields.first().map_or(0, |f| f.value.chars().count());
        let mut form = Self {
            action,
            fields,
            focus: 0,
            cursor,
            offset: 0,
            error: String::new(),
            reviewing: false,
            review_scroll: 0,
            warband_id: wb.map(str::to_string),
            automatic_reason: true,
            rule_id: selection.rule_record.clone(),
        };
        form.refresh_limits(db);
        form
    }
    pub fn value(&self, key: &str) -> &str {
        self.fields
            .iter()
            .find(|f| f.key == key)
            .map_or("", |f| f.value.as_str())
    }
    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        if key == "reason" {
            self.automatic_reason = false;
        }
        if let Some(f) = self.fields.iter_mut().find(|f| f.key == key) {
            f.value = value.into();
            f.replace_on_input = matches!(f.kind, Kind::Text) && !f.value.is_empty();
        }
    }
    pub fn label(&self, key: &str) -> String {
        self.fields
            .iter()
            .find(|f| f.key == key)
            .map_or(String::new(), Field::display)
    }
    fn required(&self, key: &str) -> Result<String> {
        let s = self.value(key).trim();
        ensure!(
            !s.is_empty(),
            "{} is required",
            self.fields
                .iter()
                .find(|f| f.key == key)
                .map_or(key, |f| f.label)
        );
        ensure!(
            !s.starts_with("__new_"),
            "Use Enter in the picker to create the new entry first"
        );
        Ok(s.into())
    }
    fn num<T: std::str::FromStr>(&self, key: &str) -> Result<T> {
        self.value(key).trim().parse().map_err(|_| {
            anyhow::anyhow!(
                "{} must be a valid whole number",
                self.fields
                    .iter()
                    .find(|f| f.key == key)
                    .map_or(key, |f| f.label)
            )
        })
    }
    fn optional_num<T: std::str::FromStr>(&self, key: &str) -> Result<Option<T>> {
        if self.value(key).trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.num(key)?))
        }
    }
    fn optional(&self, key: &str) -> Option<String> {
        if self.value(key).is_empty() {
            None
        } else {
            Some(self.value(key).into())
        }
    }
    pub fn command(&self) -> Result<Command> {
        if self.action == Action::RuleSelect {
            self.required("profile")?;
            return self.rule_command();
        }
        if self.action == Action::AiRequest {
            return Ok(Command::Ai {
                prompt: self.required("prompt")?,
                send: true,
            });
        }
        chrono::NaiveDate::parse_from_str(self.value("date"), "%Y-%m-%d")
            .context("Date must be YYYY-MM-DD")?;
        let req = |key| self.required(key);
        let opt = |key| self.optional(key);
        let flag = |key| self.value(key) == "true";
        Ok(match self.action {
            Action::RuleSelect
            | Action::RuleProfile
            | Action::RuleWarrior
            | Action::RuleList
            | Action::RuleItem
            | Action::RuleAbility
            | Action::RuleProgression
            | Action::RuleCopy
            | Action::RuleRemove
            | Action::RuleCreateWarband
            | Action::RuleStartingGear => return self.rule_command(),
            Action::NewEquipment => Command::AddEquipment {
                name: req("name")?,
                cost: self.num("cost")?,
                category: req("category")?,
                rarity: self.num("rarity")?,
                description: self.value("description").into(),
                lists: self.multiple("lists", true)?,
                allowed: vec![],
            },
            Action::NewEquipmentList => Command::EquipmentList(EquipmentListCommand::Create {
                name: req("name")?,
                description: self.value("description").into(),
                types: self.multiple("types", false)?,
                items: self.multiple("items", false)?,
            }),
            Action::AssignEquipmentList => Command::EquipmentList(EquipmentListCommand::Assign {
                list: req("list")?,
                targets: self.multiple("targets", true)?,
            }),
            Action::EditEquipmentLists => Command::EquipmentList(EquipmentListCommand::Item {
                item: req("item")?,
                lists: self.multiple("lists", false)?,
            }),
            Action::NewAbility => Command::AddAbility {
                name: req("name")?,
                kind: req("kind")?,
                description: self.value("description").into(),
                allowed: vec![req("allowed")?],
            },
            Action::GrantAbility => Command::GrantAbility {
                unit: req("unit")?,
                ability: req("ability")?,
            },
            Action::AiRequest => Command::Ai {
                prompt: req("prompt")?,
                send: true,
            },
            Action::Hire => Command::Recruit {
                kind: req("kind")?,
                name: req("name")?,
                count: self.num("quantity")?,
                group: opt("group"),
                price: self.optional_num("price")?,
            },
            Action::Buy => Command::Buy {
                item: req("item")?,
                quantity: self.num("quantity")?,
                price: self.optional_num("price")?,
            },
            Action::Sell => Command::Sell {
                item: req("item")?,
                quantity: self.num("quantity")?,
                price: self.num("price")?,
            },
            Action::Find => Command::Find {
                item: req("item")?,
                quantity: self.num("quantity")?,
            },
            Action::Lose => Command::Lose {
                item: req("item")?,
                quantity: self.num("quantity")?,
            },
            Action::Equip => Command::Equip {
                unit: req("unit")?,
                item: req("item")?,
                quantity: self.num("quantity")?,
            },
            Action::Unequip => Command::Unequip {
                unit: req("unit")?,
                item: req("item")?,
            },
            Action::Xp => Command::Xp {
                unit: req("unit")?,
                amount: self.num("amount")?,
            },
            Action::Advance => {
                ensure!(
                    opt("stat").is_some() != opt("ability").is_some(),
                    "Choose one characteristic OR one ability"
                );
                Command::Advance {
                    unit: req("unit")?,
                    stat: opt("stat"),
                    ability: opt("ability"),
                    dice: None,
                    initial: flag("initial"),
                }
            }
            Action::Promote => {
                ensure!(
                    self.value("access1") != self.value("access2"),
                    "Choose two different skill categories"
                );
                Command::Promote {
                    member: req("member")?,
                    abilities: vec![req("access1")?, req("access2")?],
                    dice: None,
                }
            }
            Action::Remove => Command::Remove {
                member: req("member")?,
                status: req("status")?,
                equipment: req("equipment")?,
            },
            Action::Status => Command::Status {
                member: req("member")?,
                status: req("status")?,
                missed: self.num("missed")?,
                leader: flag("leader"),
            },
            Action::Rename => Command::Rename {
                entity: req("entity")?,
                name: req("name")?,
            },
            Action::Effect => Command::Effect {
                member: req("member")?,
                description: req("description")?,
                stat: opt("stat"),
                delta: self.num("delta")?,
                condition: self.value("condition").into(),
                battle: None,
            },
            Action::Stat => Command::Stat {
                member: req("member")?,
                characteristic: req("stat")?,
                value: self.num("value")?,
                battle: flag("battle"),
                dice: None,
            },
            Action::Gold => Command::Gold {
                amount: self.num("amount")?,
                category: req("category")?,
            },
            Action::Wyrdstone => Command::Wyrdstone {
                shards: self.num("shards")?,
                gold: self.num("gold")?,
            },
            Action::Note => Command::Note {
                entity: req("entity")?,
                text: req("text")?,
                replace: opt("replace"),
            },
            Action::Veterans => Command::Veterans {
                experience: self.num("experience")?,
            },
            Action::Prepare => Command::Battle(BattleCommand::Prepare {
                name: req("name")?,
                scenario: req("scenario")?,
                opponent: req("opponent")?,
                rating: self.num("rating")?,
            }),
            Action::Start => Command::Battle(BattleCommand::Start),
            Action::End => Command::Battle(BattleCommand::End {
                result: req("result")?,
            }),
            Action::Record => Command::Battle(BattleCommand::Record {
                member: req("member")?,
                condition: Some(req("condition")?),
                wounds: Some(self.num("wounds")?),
                kills: Some(self.num("kills")?),
                objectives: Some(self.num("objectives")?),
                attribution: String::new(),
            }),
            Action::AddKill => Command::Battle(BattleCommand::Kill {
                member: req("member")?,
                victim: self.value("victim").into(),
                remove: None,
            }),
            Action::RemoveKill => Command::Battle(BattleCommand::Kill {
                member: req("member")?,
                victim: String::new(),
                remove: Some(req("kill")?),
            }),
            Action::Consume => Command::Battle(BattleCommand::Consume {
                member: req("member")?,
                item: req("item")?,
                quantity: self.num("quantity")?,
            }),
            Action::Injury => Command::Battle(BattleCommand::Injury {
                member: req("member")?,
                outcome: req("outcome")?,
                dice: None,
                stat: opt("stat"),
                delta: self.num("delta")?,
                missed: self.num("missed")?,
            }),
            Action::Outcome => Command::Resolve {
                key: req("key")?,
                description: req("description")?,
                gold: self.num("gold")?,
                shards: self.num("shards")?,
                dice: None,
            },
            Action::SkipStage => Command::Step {
                name: req("stage")?,
                skip: true,
                begin: false,
            },
            Action::Step => Command::Step {
                name: req("stage")?,
                skip: self.value("mode") == "skip",
                begin: self.value("mode") == "begin",
            },
            Action::Search if self.fields.iter().any(|f| f.key == "outcome") => {
                Command::RareSearch {
                    member: req("member")?,
                    item: req("item")?,
                    found: self.value("outcome") == "found",
                }
            }
            Action::Search => Command::Search {
                member: req("member")?,
                kind: req("kind")?,
                allowance: self.optional_num("allowance")?,
                use_attempt: flag("use"),
            },
            Action::Upkeep => Command::Upkeep {
                member: req("member")?,
                waive: self.value("mode") == "waive",
                depart: self.value("mode") == "depart",
            },
            Action::Complete => Command::Battle(BattleCommand::Complete),
            Action::Abandon => Command::Battle(BattleCommand::Abandon {
                disposition: req("disposition")?,
            }),
            Action::Snapshot => Command::Snapshot { name: req("name")? },
            Action::Compare => Command::Diff {
                left: req("snapshot")?,
                right: None,
            },
            Action::Restore => Command::Restore {
                snapshot: req("snapshot")?,
            },
            Action::Undo => Command::Undo,
            Action::Export => Command::Export {
                file: PathBuf::from(req("path")?),
                format: req("format")?,
            },
            Action::Switch => Command::Select {
                warband: req("warband")?,
            },
            Action::Init => Command::Init {
                rules: PathBuf::from(req("rules")?),
                profile: req("profile")?,
                campaign: req("campaign")?,
                name: req("name")?,
            },
            Action::Import => Command::Import {
                file: PathBuf::from(req("path")?),
                apply: true,
                reconcile: false,
            },
            Action::Campaign => Command::Campaign { name: req("name")? },
            Action::CampaignRules => Command::CampaignRules {
                skip_dramatis_personae: flag("skip_dramatis"),
            },
            Action::Injuries => anyhow::bail!("Injuries is a report, not a data-entry form"),
            Action::Warband => Command::Warband {
                name: req("name")?,
                profile: req("profile")?,
            },
        })
    }
    pub fn impact(&self, store: &Store, wb: Option<&str>) -> Result<String> {
        let db = &store.db;
        match self.action {
            Action::RuleSelect | Action::RuleProfile | Action::RuleWarrior | Action::RuleList | Action::RuleItem | Action::RuleAbility | Action::RuleProgression | Action::RuleCopy | Action::RuleRemove | Action::RuleCreateWarband | Action::RuleStartingGear => self.rule_impact(store),
            Action::Hire | Action::Buy | Action::Sell => {
                let quantity:u32=self.num("quantity")?;
                ensure!(quantity>0,"Quantity must be positive");
                let price=if let Some(price)=self.optional_num::<i64>("price")? {price}
                else if self.action==Action::Hire {
                    let base=db.types.iter().find(|t|t.id==self.value("kind")).context("Select warrior type")?.cost;
                    let veteran=if self.value("group").is_empty(){0}else{let u=db.unit(self.value("group"));2*(u.starting_xp+u.earned_xp) as i64};
                    base+veteran
                } else {db.items.iter().find(|i|i.id==self.value("item")).context("Select equipment")?.cost};
                ensure!(price>=0,"Price must be nonnegative");
                let total=price.checked_mul(quantity as i64).context("Cost overflow")?;
                let signed=if self.action==Action::Sell{total}else{-total};
                Ok(format!("{quantity} × {price} gc. Treasury change {signed:+} gc{}",wb.map(|w|format!("; {} → {} gc",db.gold(w),db.gold(w).saturating_add(signed))).unwrap_or_default()))
            },
            Action::Equip=>{let n=db.members.iter().filter(|m|m.unit_id==self.value("unit")&&active(m)).count();let q:u32=self.num("quantity")?;Ok(format!("{}: {} per model × {} members = {} copies required. All group members receive the same loadout.",self.label("item"),q,n,q as u64*n as u64))},
            Action::Import=>{let path=std::path::Path::new(self.value("path"));let imported:Db=if path.is_dir(){crate::store::read_tables(path)?}else{serde_json::from_slice(&std::fs::read(path).context("Cannot read opening-state file")?)?};imported.validate()?;Ok(imported.warbands.iter().map(|w|format!("Import {}: {} members, rating {}, {} gc. {}",w.name,imported.members_of(&w.id).filter(|m|active(m)).count(),imported.rating(&w.id),imported.gold(&w.id),imported.notes.iter().filter(|n|n.entity_id==w.id).map(|n|n.text.as_str()).collect::<Vec<_>>().join(" "))).collect::<Vec<_>>().join("\n"))},
            Action::Xp=>Ok(format!("{} gains {} XP once. Group XP is shared; advances will be queued.",self.label("unit"),self.value("amount"))),
            Action::NewEquipment|Action::NewAbility=>Ok("Add this definition and its eligibility to the selected profile. Other warbands using it can see the catalog entry; historical generations and pre-battle references stay intact. Stock, learning and purchases are separate actions.".into()),
            Action::NewEquipmentList=>Ok("Create this shared equipment list with the selected items. Selected warrior types and their inheriting models will use it, including future recruits. Existing carried equipment stays in place; future equip checks use the assigned list. Other warbands using this profile share type defaults; historical generations stay intact.".into()),
            Action::AssignEquipmentList=>Ok("Replace the selected type, group or model assignments. Models inherit group then warrior-type defaults unless individually assigned. Inherit clears a group/model override. Shared type defaults affect all warbands using the profile. Carried equipment and historical references stay intact; future equip checks use the assigned list.".into()),
            Action::EditEquipmentLists=>Ok("Replace this item's equipment-list memberships with the checked lists. Every type/model using those lists gains or loses access for future equipping. No items are bought, removed from stock or unequipped. Historical generations stay intact.".into()),
            Action::GrantAbility=>Ok("Record this learned skill without consuming a pending plus-up. Use Resolve plus-up for an XP-earned skill.".into()),
            Action::AiRequest=>Ok("Send the request plus selected roster, rules/catalogs, model notes, inventory, ledger, battle state and checkpoint names to the configured AI provider. No changes are applied by sending. The AI returns a proposed action list for separate review and explicit approval.".into()),
            Action::Wyrdstone=>Ok(format!("Remove {} shards from stock; receive {} gc total.",self.value("shards"),self.value("gold"))),
            Action::Restore|Action::Undo=>Ok("This restores the WHOLE DATABASE, including other warbands. Original history remains accessible.".into()),
            Action::Remove=>Ok(format!("{} leaves the active roster as {}. Carried equipment: {}. {}{} History, notes and past battle records are retained; no gold is refunded.",self.label("member"),self.value("status"),self.label("equipment"),if self.value("equipment")=="stash"{"Return all carried items to the stash."}else{"Remove all carried items from inventory."},if wb.and_then(|w|db.warbands.iter().find(|r|r.id==w)).is_some_and(|w|w.leader_id==self.value("member")){" Select a new leader before the next battle."}else{""})),
            Action::Injury=>Ok("The member remains in history. Death loses carried equipment; confirm equipment disposition.".into()),
            Action::Record=>Ok("Update this participant's current battle state. To reduce a total below its recorded kill entries, remove those entries with J first. N adds a model note. The permanent W characteristic stays unchanged.".into()),
            Action::AddKill=>Ok(format!("Credit exactly one kill / OOA to {}. Opponent: {}. XP and opponent injuries are recorded separately.",self.label("member"),if self.value("victim").trim().is_empty(){"Unspecified opponent"}else{self.value("victim")})),
            Action::RemoveKill=>Ok(format!("Remove one credit from {}: {}. Other counters and historical generations are retained.",self.label("member"),self.label("kill"))),
            _=>Ok("Review the selected values. Nothing is saved until Apply. Rules and resource validation run on submission.".into()),
        }
    }
}

pub(super) fn kill_choices(db: &Db, battle: Option<&Battle>, member: &str) -> Vec<Choice> {
    let Some(p) = battle.and_then(|b| {
        db.participants
            .iter()
            .find(|p| p.battle_id == b.id && p.member_id == member)
    }) else {
        return vec![];
    };
    let count = db.battle_kills(p).count();
    let mut choices: Vec<_> = db
        .battle_kills(p)
        .rev()
        .enumerate()
        .map(|(index, r)| {
            Choice::new(
                &r.id,
                format!(
                    "{} · credit {}",
                    if r.description.is_empty() {
                        "Unspecified opponent"
                    } else {
                        &r.description
                    },
                    count - index
                ),
            )
        })
        .collect();
    let unspecified = p.kills as usize - choices.len();
    if unspecified > 0 {
        choices.push(Choice::new(
            "unspecified",
            format!("Unspecified opponent · {unspecified} older credit(s)"),
        ));
    }
    choices
}

fn member_label(db: &Db, m: &Member) -> String {
    format!(
        "{} · {} · member {} · {}",
        m.name,
        db.unit(&m.unit_id).name,
        db.members
            .iter()
            .filter(|r| r.unit_id == m.unit_id)
            .position(|r| r.id == m.id)
            .unwrap_or(0)
            + 1,
        m.status
    )
}

impl Form {
    pub(super) fn default_reason(&self) -> String {
        let details = self
            .fields
            .iter()
            .filter(|f| !matches!(f.key, "reason" | "date" | "override") && !f.value.is_empty())
            .map(|f| format!("{}: {}", f.label, f.display()))
            .collect::<Vec<_>>()
            .join("; ");
        if details.is_empty() {
            self.action.label().into()
        } else {
            format!("{} — {details}", self.action.label())
        }
    }
    pub(super) fn refresh_reason(&mut self) {
        if self.automatic_reason || self.value("reason").trim().is_empty() {
            let reason = self.default_reason();
            if let Some(f) = self.fields.iter_mut().find(|f| f.key == "reason") {
                f.value = reason;
                f.replace_on_input = true;
            }
        }
    }
    pub(super) fn refresh_limits(&mut self, db: &Db) {
        let previous = self.value("override").to_string();
        self.fields.retain(|f| f.key != "override");
        let u = db.units.iter().find(|u| u.id == self.value("unit"));
        let label = match self.action {
            Action::Stat => db
                .members
                .iter()
                .find(|m| m.id == self.value("member"))
                .filter(|m| {
                    db.typ(&db.unit(&m.unit_id).type_id)
                        .stat(self.value("stat"))
                        .parse::<i16>()
                        .is_ok()
                })
                .map(|_| "Replace fixed profile stat"),
            Action::Advance => u.and_then(|u| {
                let s = self.value("stat");
                let ability = self.value("ability");
                let cap_conflict = db
                    .members
                    .iter()
                    .filter(|m| m.unit_id == u.id && active(m))
                    .any(|m| {
                        db.stat(m, s, None).parse::<i16>().ok().is_some_and(|v| {
                            db.caps.iter().any(|c| {
                                c.type_id == u.type_id && c.characteristic == s && v >= c.maximum
                            })
                        })
                    });
                let table_conflict = (!s.is_empty()
                    && ((u.role == "henchman"
                        && (!["I", "S", "BS", "WS", "A", "Ld"].contains(&s)
                            || db
                                .advances
                                .iter()
                                .any(|a| a.unit_id == u.id && a.characteristic == s)))
                        || s == "M"))
                    || (!ability.is_empty()
                        && !db.permissions.iter().any(|p| {
                            (p.type_id == u.type_id || p.type_id == u.id)
                                && p.kind == "ability"
                                && p.target_id == ability
                        }));
                (cap_conflict || table_conflict).then_some("Allow beyond profile limits")
            }),
            Action::Equip => u.and_then(|u| {
                let item = db.items.iter().find(|i| i.id == self.value("item"))?;
                let allowed = db
                    .members
                    .iter()
                    .filter(|m| m.unit_id == u.id && active(m))
                    .all(|m| db.equipment_allowed(&m.id, &item.id));
                let max = match item.category.as_str() {
                    "melee" => 2,
                    "missile" => 2,
                    "armour" => 1,
                    _ => u32::MAX,
                };
                let quantity = self.value("quantity").parse::<u32>().unwrap_or(0);
                let exceeds = db
                    .members
                    .iter()
                    .find(|m| m.unit_id == u.id && active(m))
                    .is_some_and(|m| {
                        let others: u64 = db
                            .allocations
                            .iter()
                            .filter(|a| {
                                a.member_id == m.id
                                    && a.item_id != item.id
                                    && db.items.iter().any(|i| {
                                        i.id == a.item_id
                                            && i.category == item.category
                                            && i.name != "Dagger"
                                    })
                            })
                            .map(|a| a.quantity as u64)
                            .sum();
                        others
                            + if item.name == "Dagger" {
                                0
                            } else {
                                quantity as u64
                            }
                            > max as u64
                    });
                (!allowed || exceeds).then_some("Allow nonstandard loadout")
            }),
            Action::Hire => db
                .types
                .iter()
                .find(|t| t.id == self.value("kind"))
                .and_then(|t| {
                    let p = db.profile(&t.profile_id);
                    let count = self.value("quantity").parse::<u32>().unwrap_or(0);
                    let active: Vec<_> = db
                        .members
                        .iter()
                        .filter(|m| {
                            active(m)
                                && Some(db.unit(&m.unit_id).warband_id.as_str())
                                    == self.warband_id.as_deref()
                        })
                        .collect();
                    let group = db.units.iter().find(|u| u.id == self.value("group"));
                    let exceeds = (t.limit > 0
                        && active
                            .iter()
                            .filter(|m| db.unit(&m.unit_id).type_id == t.id)
                            .count() as u32
                            + count
                            > t.limit)
                        || (t.counts_roster
                            && active
                                .iter()
                                .filter(|m| db.typ(&db.unit(&m.unit_id).type_id).counts_roster)
                                .count() as u32
                                + count
                                > p.max_members)
                        || (t.role == "hero"
                            && active
                                .iter()
                                .filter(|m| db.unit(&m.unit_id).role == "hero")
                                .count() as u32
                                + count
                                > p.max_heroes)
                        || group
                            .map(|u| active.iter().filter(|m| m.unit_id == u.id).count() as u32)
                            .unwrap_or(0)
                            + count
                            > t.group_max;
                    exceeds.then_some("Allow extra recruitment slots")
                }),
            Action::Promote => db
                .members
                .iter()
                .find(|m| m.id == self.value("member"))
                .and_then(|m| {
                    let u = db.unit(&m.unit_id);
                    let wb = db.warbands.iter().find(|w| w.id == u.warband_id)?;
                    let max = db.profile(&wb.profile_id).max_heroes;
                    let n = db
                        .members_of(&wb.id)
                        .filter(|m| active(m) && db.unit(&m.unit_id).role == "hero")
                        .count() as u32;
                    (n >= max).then_some("Allow an extra Hero slot")
                }),
            _ => None,
        };
        if let Some(label) = label {
            let mut f = Field::toggle("override", label);
            f.hint = "Explicitly replace this rule-defined preset; resource and ownership checks still apply.";
            if previous == "true" {
                f.value = previous;
            }
            self.fields.push(f);
        }
        self.focus = self.focus.min(self.fields.len().saturating_sub(1));
        self.refresh_reason();
    }
}
