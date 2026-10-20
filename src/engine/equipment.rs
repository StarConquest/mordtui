use super::*;

pub(super) fn resolve_lists(
    db: &Db,
    w: &str,
    names: &[String],
    all_if_empty: bool,
) -> Result<Vec<String>> {
    let rows: Vec<_> = db
        .equipment_lists
        .iter()
        .filter(|l| l.profile_id == wb(db, w).profile_id)
        .cloned()
        .collect();
    let mut lists = if names.is_empty() && all_if_empty {
        rows.iter().map(|l| l.id.clone()).collect()
    } else {
        names
            .iter()
            .map(|name| resolve(&rows, name).map(|l| l.id.clone()))
            .collect::<Result<Vec<_>>>()?
    };
    lists.sort();
    lists.dedup();
    Ok(lists)
}

fn targets(db: &Db, w: &str, names: &[String]) -> Result<Vec<String>> {
    let profile = &wb(db, w).profile_id;
    let rows: Vec<_> = db
        .types
        .iter()
        .filter(|t| t.profile_id == *profile)
        .map(|t| (&t.id, &t.name))
        .chain(
            db.units
                .iter()
                .filter(|u| u.warband_id == w)
                .map(|u| (&u.id, &u.name)),
        )
        .chain(db.members_of(w).map(|m| (&m.id, &m.name)))
        .collect();
    let mut result = vec![];
    for name in names {
        if let Some((id, _)) = rows.iter().find(|(id, _)| *id == name) {
            result.push((*id).clone());
            continue;
        }
        let found: Vec<_> = rows
            .iter()
            .filter(|(id, n)| {
                n.eq_ignore_ascii_case(name) || name.len() >= 8 && id.starts_with(name)
            })
            .collect();
        match found.as_slice() {
            [(id, _)] => result.push((*id).clone()),
            [] => bail!("No warrior type, unit or model matches {name:?}"),
            _ => bail!("Ambiguous equipment-list target {name:?}; choose a stable ID"),
        }
    }
    result.sort();
    result.dedup();
    ensure!(
        !result.is_empty(),
        "Select at least one warrior type, unit or model"
    );
    Ok(result)
}

pub(super) fn execute(
    db: &mut Db,
    c: &mut ContextData,
    command: &EquipmentListCommand,
) -> Result<Value> {
    let profile = wb(db, &c.wb).profile_id.clone();
    match command {
        EquipmentListCommand::Create {
            name,
            description,
            types,
            items,
        } => {
            ensure!(!name.trim().is_empty(), "Enter an equipment list name");
            ensure!(
                !db.equipment_lists
                    .iter()
                    .any(|l| l.profile_id == profile && l.name.eq_ignore_ascii_case(name.trim())),
                "Equipment list already exists"
            );
            let rows: Vec<_> = db
                .types
                .iter()
                .filter(|t| t.profile_id == profile)
                .cloned()
                .collect();
            let types = types
                .iter()
                .map(|name| resolve(&rows, name).map(|t| t.id.clone()))
                .collect::<Result<Vec<_>>>()?;
            let items = items
                .iter()
                .map(|name| item(db, &c.wb, name).map(|i| i.id))
                .collect::<Result<Vec<_>>>()?;
            let list = EquipmentList {
                id: id(),
                profile_id: profile,
                name: name.trim().into(),
                description: description.clone(),
            };
            for item in items {
                db.add_to_equipment_list(&list.id, &item);
            }
            for t in types {
                db.assign_equipment_list(&t, &list.id);
            }
            db.equipment_lists.push(list.clone());
            Ok(json!({"equipment_list":list}))
        }
        EquipmentListCommand::Assign {
            list,
            targets: names,
        } => {
            let entities = targets(db, &c.wb, names)?;
            if list == "inherit" {
                ensure!(
                    entities
                        .iter()
                        .all(|entity| !db.types.iter().any(|t| t.id == *entity)),
                    "Warrior types need a default equipment list; only models and units can inherit"
                );
                db.equipment_list_assignments
                    .retain(|a| !entities.contains(&a.entity_id));
            } else {
                let list = resolve_lists(db, &c.wb, std::slice::from_ref(list), false)?.remove(0);
                for entity in &entities {
                    db.assign_equipment_list(entity, &list);
                }
            }
            Ok(json!({"assigned":entities,"list":list}))
        }
        EquipmentListCommand::Item { item: name, lists } => {
            let item = item(db, &c.wb, name)?;
            let lists = resolve_lists(db, &c.wb, lists, false)?;
            db.equipment_list_items.retain(|r| r.item_id != item.id);
            for list in &lists {
                db.add_to_equipment_list(list, &item.id);
            }
            Ok(json!({"item":item,"equipment_lists":lists}))
        }
    }
}

pub(super) fn list_view(db: &Db, w: &str) -> Value {
    json!(db.equipment_lists.iter().filter(|l| l.profile_id == wb(db, w).profile_id).map(|l| json!({
        "list": l,
        "items": db.equipment_list_items.iter().filter(|r| r.list_id == l.id).filter_map(|r| db.items.iter().find(|i| i.id == r.item_id)).collect::<Vec<_>>(),
        "assigned": db.equipment_list_assignments.iter().filter(|a| a.list_id == l.id).collect::<Vec<_>>(),
        "models": db.members_of(w).filter(|m| db.equipment_list_for(&m.id).is_some_and(|list| list.id == l.id)).map(|m| &m.id).collect::<Vec<_>>()
    })).collect::<Vec<_>>())
}

/// Preserve individual/group list overrides when changing campaign profiles.
pub(super) fn migrate_overrides(
    next: &mut Db,
    old: &Db,
    warband: &str,
    target: &str,
    items: &std::collections::HashMap<String, String>,
) -> Result<()> {
    let entities: Vec<_> = old
        .units
        .iter()
        .filter(|u| u.warband_id == warband)
        .map(|u| u.id.clone())
        .chain(old.members_of(warband).map(|m| m.id.clone()))
        .collect();
    for assignment in old
        .equipment_list_assignments
        .iter()
        .filter(|a| entities.contains(&a.entity_id))
    {
        let old_list = old
            .equipment_lists
            .iter()
            .find(|l| l.id == assignment.list_id)
            .context("Missing equipment list")?;
        let list = format!("migrated-list:{target}:{}", old_list.id);
        if !next.equipment_lists.iter().any(|l| l.id == list) {
            next.equipment_lists.push(EquipmentList {
                id: list.clone(),
                profile_id: target.into(),
                name: format!("{} ({})", old_list.name, old_list.id),
                description: old_list.description.clone(),
            });
            for row in old
                .equipment_list_items
                .iter()
                .filter(|r| r.list_id == old_list.id)
            {
                let item = items
                    .get(&row.item_id)
                    .context("New profile lacks an item in an assigned equipment list")?;
                next.add_to_equipment_list(&list, item);
            }
        }
        next.assign_equipment_list(&assignment.entity_id, &list);
    }
    Ok(())
}
