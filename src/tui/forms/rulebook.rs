use super::*;
use crate::{cli::RulebookCommand, rules::editor::*};
use std::collections::BTreeMap;

const CAP_KEYS: [&str; 9] = [
    "cap_m", "cap_ws", "cap_bs", "cap_s", "cap_t", "cap_w", "cap_i", "cap_a", "cap_ld",
];
const STAT_KEYS: [&str; 9] = ["m", "ws", "bs", "s", "t", "w", "i", "a", "ld"];
fn value(key: &'static str, label: &'static str, value: impl ToString) -> Field {
    Field::text(
        key,
        label,
        value.to_string(),
        "Enter the rule value. Blank optional limits mean no cap; zero type limit means unrestricted.",
    )
}
fn boolean(key: &'static str, label: &'static str, selected: bool) -> Field {
    let mut f = Field::toggle(key, label);
    f.value = selected.to_string();
    f
}
impl Action {
    pub(in crate::tui) fn is_rulebook(self) -> bool {
        matches!(
            self,
            Self::RuleSelect
                | Self::RuleProfile
                | Self::RuleWarrior
                | Self::RuleList
                | Self::RuleItem
                | Self::RuleAbility
                | Self::RuleProgression
                | Self::RuleCopy
                | Self::RuleRemove
                | Self::RuleCreateWarband
                | Self::RuleStartingGear
        )
    }
}
impl Form {
    pub(super) fn rule_fields(
        action: Action,
        db: &Db,
        p: Option<&str>,
        s: &Selection,
    ) -> Vec<Field> {
        let profile = p.and_then(|p| db.profiles.iter().find(|r| r.id == p));
        let key = s.rule_record.as_deref().unwrap_or("");
        let profiles = || {
            db.profiles
                .iter()
                .map(|r| Choice::new(&r.id, format!("{} · {} · {}", r.name, r.version, r.faction)))
                .collect::<Vec<_>>()
        };
        let context = || {
            Field::choices(
                "profile",
                "Rules profile",
                profile
                    .map(|r| vec![Choice::new(&r.id, format!("{} · {}", r.name, r.version))])
                    .unwrap_or_default(),
                p,
            )
        };
        let types = || {
            db.types
                .iter()
                .filter(|r| Some(r.profile_id.as_str()) == p)
                .map(|r| Choice::new(&r.id, format!("{} · {}", r.name, r.role)))
                .collect::<Vec<_>>()
        };
        let items = || {
            db.items
                .iter()
                .filter(|r| Some(r.profile_id.as_str()) == p)
                .map(|r| Choice::new(&r.id, &r.name))
                .collect::<Vec<_>>()
        };
        let abilities = || {
            db.abilities
                .iter()
                .filter(|r| Some(r.profile_id.as_str()) == p)
                .map(|r| Choice::new(&r.id, format!("{} · {}", r.name, r.kind)))
                .collect::<Vec<_>>()
        };
        let lists = || {
            db.equipment_lists
                .iter()
                .filter(|r| Some(r.profile_id.as_str()) == p)
                .map(|r| Choice::new(&r.id, &r.name))
                .collect::<Vec<_>>()
        };
        match action {
            Action::RuleSelect => vec![Field::choices("profile", "Rules profile", profiles(), p)],
            Action::RuleProfile => {
                let default = Profile {
                    version: "1.0".into(),
                    classification: "house".into(),
                    starting_gold: 500,
                    max_members: 15,
                    max_heroes: 6,
                    ..Profile::default()
                };
                let r = db.profiles.iter().find(|r| r.id == key).unwrap_or(&default);
                vec![
                    value("name", "Profile name", &r.name),
                    value("faction", "Faction / subfaction", &r.faction),
                    value("version", "Content version", &r.version),
                    Field::choices(
                        "classification",
                        "Rules classification",
                        opts(&["house", "published", "optional"]),
                        Some(&r.classification),
                    ),
                    value("source", "Source / reference", &r.source),
                    value("starting_gold", "Starting gold", r.starting_gold),
                    value("bonus_gold", "Faction bonus gold", r.bonus_gold),
                    value("max_members", "Maximum roster size", r.max_members),
                    value("max_heroes", "Maximum Heroes", r.max_heroes),
                    boolean("allow_debt", "Allow treasury debt", r.allow_debt),
                    value(
                        "initial_advance_cost",
                        "Initial advance cost",
                        r.initial_advance_cost,
                    ),
                    Field::text(
                        "description",
                        "Campaign / faction rules",
                        &r.description,
                        "Record special recruitment, injury, exploration and other manually resolved rules here.",
                    ),
                ]
            }
            Action::RuleWarrior => {
                let default = WarriorType {
                    role: "hero".into(),
                    group_max: 1,
                    rating: 5,
                    gains_xp: true,
                    counts_roster: true,
                    counts_income: true,
                    counts_battle: true,
                    m: "4".into(),
                    ws: "3".into(),
                    bs: "3".into(),
                    s: "3".into(),
                    t: "3".into(),
                    w: "1".into(),
                    i: "3".into(),
                    a: "1".into(),
                    ld: "7".into(),
                    ..WarriorType::default()
                };
                let r = db.types.iter().find(|r| r.id == key).unwrap_or(&default);
                let mut choices = lists();
                choices.insert(0, Choice::new("", "Automatic empty list / keep current"));
                let mut f = vec![
                    context(),
                    value("name", "Warrior type name", &r.name),
                    Field::choices(
                        "role",
                        "Role",
                        opts(&["hero", "henchman", "hired", "dramatis"]),
                        Some(&r.role),
                    ),
                    value("cost", "Hire cost (gold)", r.cost),
                    value("upkeep", "Upkeep per model", r.upkeep),
                    value("starting_xp", "Starting experience", r.starting_xp),
                    boolean("gains_xp", "Gains experience", r.gains_xp),
                    value("limit", "Type recruitment limit", r.limit),
                    value("group_max", "Maximum group size", r.group_max),
                    value("rating", "Base rating per model", r.rating),
                    boolean("counts_roster", "Counts toward roster cap", r.counts_roster),
                    boolean("counts_income", "Counts for income size", r.counts_income),
                    boolean("counts_battle", "Participates in battles", r.counts_battle),
                    boolean(
                        "leader",
                        "Leader on recruitment",
                        r.leader.unwrap_or(r.name == "Captain"),
                    ),
                    boolean(
                        "free_dagger",
                        "Free dagger on recruitment",
                        r.free_dagger.unwrap_or(!r.id.is_empty()),
                    ),
                    Field::choices(
                        "equipment_list",
                        "Equipment list",
                        choices,
                        db.equipment_list_for(&r.id).map(|l| l.id.as_str()),
                    ),
                ];
                for (key, stat) in STAT_KEYS.into_iter().zip(STATS) {
                    f.push(Field::text(key,stat,r.stat(stat),"Numeric base characteristic or dice expression (for example D6). Players resolve rolls at the table."));
                }
                f.push(Field::multiple(
                    "abilities",
                    "Learnable skills / spells",
                    abilities(),
                    db.permissions
                        .iter()
                        .filter(|p| p.type_id == r.id && p.kind == "ability")
                        .map(|p| p.target_id.clone())
                        .collect(),
                ));
                f.push(Field::multiple(
                    "starting_abilities",
                    "Innate / starting skills",
                    abilities(),
                    db.permissions
                        .iter()
                        .filter(|p| p.type_id == r.id && p.kind == "starting-ability")
                        .map(|p| p.target_id.clone())
                        .collect(),
                ));
                for (key, stat) in CAP_KEYS.into_iter().zip(STATS) {
                    f.push(Field::text(
                        key,
                        match stat {
                            "M" => "Maximum M",
                            "WS" => "Maximum WS",
                            "BS" => "Maximum BS",
                            "S" => "Maximum S",
                            "T" => "Maximum T",
                            "W" => "Maximum W",
                            "I" => "Maximum I",
                            "A" => "Maximum A",
                            _ => "Maximum Ld",
                        },
                        db.caps
                            .iter()
                            .find(|c| c.type_id == r.id && c.characteristic == stat)
                            .map(|c| c.maximum.to_string())
                            .unwrap_or_default(),
                        "Optional positive maximum; blank means no defined cap.",
                    ));
                }
                f.push(Field::text("description","Special rules / contract",&r.description,"Record recruitment conditions, contract terms, special abilities and manually resolved mechanics."));
                f
            }
            Action::RuleList => {
                let default = EquipmentList::default();
                let r = db
                    .equipment_lists
                    .iter()
                    .find(|r| r.id == key)
                    .unwrap_or(&default);
                vec![
                    context(),
                    value("name", "Equipment list name", &r.name),
                    value("description", "List rules / source", &r.description),
                    Field::multiple(
                        "types",
                        "Assigned warrior types",
                        types(),
                        db.equipment_list_assignments
                            .iter()
                            .filter(|a| {
                                a.list_id == r.id && db.types.iter().any(|t| t.id == a.entity_id)
                            })
                            .map(|a| a.entity_id.clone())
                            .collect(),
                    ),
                    Field::multiple(
                        "items",
                        "Equipment on this list",
                        items(),
                        db.equipment_list_items
                            .iter()
                            .filter(|a| a.list_id == r.id)
                            .map(|a| a.item_id.clone())
                            .collect(),
                    ),
                ]
            }
            Action::RuleItem => {
                let default = Item {
                    category: "equipment".into(),
                    ..Item::default()
                };
                let r = db.items.iter().find(|r| r.id == key).unwrap_or(&default);
                vec![
                    context(),
                    value("name", "Equipment name", &r.name),
                    value("cost", "Catalog price (gold)", r.cost),
                    Field::choices(
                        "category",
                        "Category",
                        opts(&["melee", "missile", "armour", "equipment", "consumable"]),
                        Some(&r.category),
                    ),
                    value("rarity", "Rarity (0 = common)", r.rarity),
                    Field::text(
                        "description",
                        "Equipment rules",
                        &r.description,
                        "Record range, strength, hands, restrictions, effects and other tabletop rules.",
                    ),
                    Field::multiple(
                        "lists",
                        "Equipment lists",
                        lists(),
                        db.equipment_list_items
                            .iter()
                            .filter(|a| a.item_id == r.id)
                            .map(|a| a.list_id.clone())
                            .collect(),
                    ),
                ]
            }
            Action::RuleAbility => {
                let default = Ability {
                    kind: "combat".into(),
                    ..Ability::default()
                };
                let r = db
                    .abilities
                    .iter()
                    .find(|r| r.id == key)
                    .unwrap_or(&default);
                vec![
                    context(),
                    value("name", "Skill / spell / prayer", &r.name),
                    Field::text(
                        "kind",
                        "Category / skill table",
                        &r.kind,
                        "Combat, shooting, strength, academic, speed, special, spell, prayer, or a custom category.",
                    ),
                    value("source", "Rules source", &r.source),
                    value("description", "Rules / effects", &r.description),
                    Field::multiple(
                        "types",
                        "Learnable by these types",
                        types(),
                        db.permissions
                            .iter()
                            .filter(|p| {
                                p.target_id == r.id
                                    && p.kind == "ability"
                                    && db.types.iter().any(|t| t.id == p.type_id)
                            })
                            .map(|p| p.type_id.clone())
                            .collect(),
                    ),
                    Field::multiple(
                        "starting_types",
                        "Innate to these types",
                        types(),
                        db.permissions
                            .iter()
                            .filter(|p| {
                                p.target_id == r.id
                                    && p.kind == "starting-ability"
                                    && db.types.iter().any(|t| t.id == p.type_id)
                            })
                            .map(|p| p.type_id.clone())
                            .collect(),
                    ),
                ]
            }
            Action::RuleStartingGear => {
                let row = db
                    .permissions
                    .iter()
                    .find(|r| r.id == key && r.kind == "starting-item");
                vec![
                    context(),
                    Field::choices(
                        "warrior",
                        "Warrior type",
                        types(),
                        row.map(|r| r.type_id.as_str()),
                    ),
                    Field::choices(
                        "item",
                        "Starting equipment",
                        items(),
                        row.map(|r| r.target_id.as_str()),
                    ),
                    Field::text(
                        "quantity",
                        "Copies per hired model",
                        row.map_or(1, |r| r.quantity).to_string(),
                        "Granted at zero acquisition cost on recruitment. Zero removes this rule. Group reinforcements keep the group's loadout; surplus starting items go to stash.",
                    ),
                ]
            }
            Action::RuleProgression => {
                let role = if ["hero", "henchman", "hired", "dramatis"].contains(&key) {
                    key
                } else {
                    "hero"
                };
                vec![
                    context(),
                    Field::choices(
                        "role",
                        "Progression role",
                        opts(&["hero", "henchman", "hired", "dramatis"]),
                        Some(role),
                    ),
                    Field::text(
                        "thresholds",
                        "Advance XP thresholds",
                        threshold_text(db, p.unwrap_or(""), role),
                        "Comma/space-separated XP totals (e.g. 2, 4, 6, 8). Blank defines no automatic advances. Rolls stay on the tabletop.",
                    ),
                ]
            }
            Action::RuleCopy => vec![
                Field::choices("profile", "Source profile", profiles(), p),
                value(
                    "name",
                    "New profile name",
                    profile
                        .map(|p| format!("{} copy", p.name))
                        .unwrap_or_default(),
                ),
                value("version", "New content version", "1.0"),
            ],
            Action::RuleRemove => {
                let kind = s.rule_kind.as_deref().unwrap_or("profile");
                let name = match kind {
                    "profile" => db
                        .profiles
                        .iter()
                        .find(|r| r.id == key)
                        .map(|r| r.name.as_str()),
                    "warrior" => db
                        .types
                        .iter()
                        .find(|r| r.id == key)
                        .map(|r| r.name.as_str()),
                    "equipment-list" => db
                        .equipment_lists
                        .iter()
                        .find(|r| r.id == key)
                        .map(|r| r.name.as_str()),
                    "equipment" => db
                        .items
                        .iter()
                        .find(|r| r.id == key)
                        .map(|r| r.name.as_str()),
                    "ability" => db
                        .abilities
                        .iter()
                        .find(|r| r.id == key)
                        .map(|r| r.name.as_str()),
                    "starting-gear" => db
                        .permissions
                        .iter()
                        .find(|r| r.id == key && r.kind == "starting-item")
                        .and_then(|r| db.items.iter().find(|i| i.id == r.target_id))
                        .map(|i| i.name.as_str()),
                    _ => None,
                };
                vec![
                    Field::choices(
                        "kind",
                        "Definition category",
                        vec![Choice::new(kind, kind)],
                        Some(kind),
                    ),
                    Field::choices(
                        "record",
                        "Definition to remove",
                        name.map(|n| vec![Choice::new(key, n)]).unwrap_or_default(),
                        Some(key),
                    ),
                ]
            }
            Action::RuleCreateWarband => {
                let mut camps = db
                    .campaigns
                    .iter()
                    .map(|c| Choice::new(&c.id, &c.name))
                    .collect::<Vec<_>>();
                camps.push(Choice::new("__new__", "Create a new campaign"));
                vec![
                    context(),
                    value("name", "Warband name", ""),
                    Field::choices("campaign", "Campaign", camps, None),
                    Field::text(
                        "campaign_name",
                        "New campaign name",
                        "New campaign",
                        "Used only when Create a new campaign is selected.",
                    ),
                ]
            }
            _ => unreachable!(),
        }
    }
    pub(super) fn rule_command(&self) -> Result<Command> {
        let req = |key| self.required(key);
        let text = |key| self.value(key).to_string();
        let boolean = |key| self.value(key) == "true";
        let key = self.rule_id.clone().unwrap_or_default();
        let p = self.value("profile").to_string();
        let definition = match self.action {
            Action::RuleSelect => {
                return Ok(Command::List {
                    what: "rules".into(),
                    filter: None,
                });
            }
            Action::RuleCopy => {
                return Ok(Command::Rulebook(RulebookCommand::Clone {
                    profile: req("profile")?,
                    name: req("name")?,
                    version: req("version")?,
                }));
            }
            Action::RuleRemove => {
                return Ok(Command::Rulebook(RulebookCommand::Delete {
                    kind: req("kind")?,
                    id: req("record")?,
                }));
            }
            Action::RuleCreateWarband => {
                return Ok(Command::Rulebook(RulebookCommand::CreateWarband {
                    profile: req("profile")?,
                    campaign: if self.value("campaign") == "__new__" {
                        req("campaign_name")?
                    } else {
                        req("campaign")?
                    },
                    name: req("name")?,
                    new_campaign: self.value("campaign") == "__new__",
                }));
            }
            Action::RuleProfile => Definition::Profile(Profile {
                id: key,
                schema_version: SCHEMA,
                name: req("name")?,
                faction: text("faction"),
                version: req("version")?,
                source: text("source"),
                classification: req("classification")?,
                starting_gold: self.num("starting_gold")?,
                bonus_gold: self.num("bonus_gold")?,
                max_members: self.num("max_members")?,
                max_heroes: self.num("max_heroes")?,
                allow_debt: boolean("allow_debt"),
                initial_advance_cost: self.num("initial_advance_cost")?,
                description: text("description"),
            }),
            Action::RuleWarrior => {
                let mut caps = BTreeMap::new();
                for (key, stat) in CAP_KEYS.into_iter().zip(STATS) {
                    if let Some(max) = self.optional_num::<i16>(key)? {
                        caps.insert(stat.to_string(), max);
                    }
                }
                Definition::Warrior(Box::new(WarriorDefinition {
                    warrior: WarriorType {
                        id: key,
                        profile_id: req("profile")?,
                        name: req("name")?,
                        role: req("role")?,
                        cost: self.num("cost")?,
                        upkeep: self.num("upkeep")?,
                        starting_xp: self.num("starting_xp")?,
                        gains_xp: boolean("gains_xp"),
                        limit: self.num("limit")?,
                        group_max: self.num("group_max")?,
                        rating: self.num("rating")?,
                        counts_roster: boolean("counts_roster"),
                        counts_income: boolean("counts_income"),
                        counts_battle: boolean("counts_battle"),
                        leader: Some(boolean("leader")),
                        free_dagger: Some(boolean("free_dagger")),
                        m: req("m")?,
                        ws: req("ws")?,
                        bs: req("bs")?,
                        s: req("s")?,
                        t: req("t")?,
                        w: req("w")?,
                        i: req("i")?,
                        a: req("a")?,
                        ld: req("ld")?,
                        description: text("description"),
                    },
                    equipment_list: self.optional("equipment_list"),
                    abilities: self.multiple("abilities", false)?,
                    starting_abilities: self.multiple("starting_abilities", false)?,
                    caps,
                }))
            }
            Action::RuleList => Definition::EquipmentList(ListDefinition {
                list: EquipmentList {
                    id: key,
                    profile_id: req("profile")?,
                    name: req("name")?,
                    description: text("description"),
                },
                types: self.multiple("types", false)?,
                items: self.multiple("items", false)?,
            }),
            Action::RuleItem => Definition::Equipment(ItemDefinition {
                item: Item {
                    id: key,
                    profile_id: req("profile")?,
                    name: req("name")?,
                    cost: self.num("cost")?,
                    category: req("category")?,
                    rarity: self.num("rarity")?,
                    description: text("description"),
                },
                lists: self.multiple("lists", false)?,
            }),
            Action::RuleAbility => Definition::Ability(AbilityDefinition {
                ability: Ability {
                    id: key,
                    profile_id: req("profile")?,
                    name: req("name")?,
                    kind: req("kind")?,
                    description: text("description"),
                    source: text("source"),
                },
                types: self.multiple("types", false)?,
                starting_types: self.multiple("starting_types", false)?,
            }),
            Action::RuleStartingGear => Definition::StartingGear(StartingGearDefinition {
                id: key,
                profile: req("profile")?,
                warrior: req("warrior")?,
                item: req("item")?,
                quantity: self.num("quantity")?,
            }),
            Action::RuleProgression => Definition::Progression(ProgressionDefinition {
                profile: req("profile")?,
                role: req("role")?,
                thresholds: self
                    .value("thresholds")
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|s| !s.is_empty())
                    .map(str::parse)
                    .collect::<std::result::Result<Vec<u32>, _>>()
                    .context("Use XP totals separated by commas or spaces")?,
            }),
            _ => unreachable!(),
        };
        if !matches!(self.action, Action::RuleProfile) {
            ensure!(!p.is_empty(), "Create or select a rules profile first");
        }
        Ok(Command::Rulebook(RulebookCommand::Save {
            definition: serde_json::to_string(&definition)?,
        }))
    }
    pub(super) fn rule_impact(&self, store: &Store) -> Result<String> {
        if self.action == Action::RuleSelect {
            return Ok("Choose the profile shown in the Rules editor. This does not change a warband's pinned profile.".into());
        }
        if self.action == Action::RuleRemove {
            return Ok("Remove this unused definition. Referenced warrior types, learned skills, assigned lists and inventory items cannot be deleted. Previous generations remain available.".into());
        }
        if self.action == Action::RuleCopy {
            return Ok("Create an independent copy of the complete profile, lists, warrior definitions, equipment, skills, access, caps and XP thresholds. Existing warbands keep their profile.".into());
        }
        if self.action == Action::RuleCreateWarband {
            return Ok("Create an empty warband from this stored profile, record starting funding plus faction bonus, and select it. A new campaign is created only when explicitly selected. Hire models afterward.".into());
        }
        let p = if self.action == Action::RuleProfile {
            self.rule_id.as_deref().unwrap_or("")
        } else {
            self.value("profile")
        };
        let used = store
            .db
            .warbands
            .iter()
            .filter(|w| w.profile_id == p)
            .count();
        Ok(format!(
            "Save this definition to the selected profile ({used} existing warband(s)). Shared base profiles and access changes affect current users; starting XP, innate abilities and funding apply to future hires/warbands. Existing gear, transactions, earned advances and historical references remain. Unchecking a type on an equipment list gives it an empty default list. Descriptive rules are adjudicated by the player."
        ))
    }
}
pub(super) fn threshold_text(db: &Db, profile: &str, role: &str) -> String {
    let mut values = db
        .thresholds
        .iter()
        .filter(|t| t.profile_id == profile && t.role == role)
        .map(|t| t.xp)
        .collect::<Vec<_>>();
    values.sort();
    values
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
