use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl Db {
    /// An explicit individual assignment overrides its group, then its type.
    pub fn equipment_list_for(&self, entity: &str) -> Option<&EquipmentList> {
        if let Some(a) = self
            .equipment_list_assignments
            .iter()
            .find(|a| a.entity_id == entity)
        {
            return self.equipment_lists.iter().find(|l| l.id == a.list_id);
        }
        if let Some(m) = self.members.iter().find(|m| m.id == entity) {
            return self.equipment_list_for(&m.unit_id);
        }
        if let Some(u) = self.units.iter().find(|u| u.id == entity) {
            return self.equipment_list_for(&u.type_id);
        }
        None
    }
    pub fn equipment_allowed(&self, member: &str, item: &str) -> bool {
        self.equipment_list_for(member).is_some_and(|l| {
            self.equipment_list_items
                .iter()
                .any(|r| r.list_id == l.id && r.item_id == item)
        })
    }
    pub fn assign_equipment_list(&mut self, entity: &str, list: &str) {
        self.equipment_list_assignments
            .retain(|a| a.entity_id != entity);
        self.equipment_list_assignments
            .push(EquipmentListAssignment {
                id: format!("equipment-list-assignment:{entity}"),
                entity_id: entity.into(),
                list_id: list.into(),
            });
    }
    pub fn add_to_equipment_list(&mut self, list: &str, item: &str) {
        if !self
            .equipment_list_items
            .iter()
            .any(|r| r.list_id == list && r.item_id == item)
        {
            self.equipment_list_items.push(EquipmentListItem {
                id: format!("equipment-list-item:{list}:{item}"),
                list_id: list.into(),
                item_id: item.into(),
            });
        }
    }
    /// Deterministic, read-compatible upgrade. Original generation files are
    /// never changed. Types with identical legacy access share an equipment list.
    pub fn normalize_equipment_lists(&mut self) -> Result<()> {
        self.validate()?;
        let legacy: Vec<_> = self
            .permissions
            .iter()
            .filter(|p| p.kind == "item")
            .cloned()
            .collect();
        for p in &legacy {
            let profile = self.equipment_entity_profile(&p.type_id).ok_or_else(|| {
                anyhow::anyhow!("Unknown equipment permission owner {}", p.type_id)
            })?;
            ensure!(
                self.items
                    .iter()
                    .any(|i| i.id == p.target_id && i.profile_id == profile),
                "Unknown or mismatched equipment permission {}",
                p.id
            );
        }
        let mut shared: BTreeMap<(String, BTreeSet<String>), String> = BTreeMap::new();
        let types = self.types.clone();
        for t in types {
            if self
                .equipment_list_assignments
                .iter()
                .any(|a| a.entity_id == t.id)
            {
                continue;
            }
            let items: BTreeSet<_> = legacy
                .iter()
                .filter(|p| p.type_id == t.id)
                .map(|p| p.target_id.clone())
                .collect();
            let key = (t.profile_id.clone(), items.clone());
            let list = if let Some(list) = shared.get(&key) {
                list.clone()
            } else {
                let list = format!("equipment-list:{}", t.id);
                ensure!(
                    !self.equipment_lists.iter().any(|l| l.id == list),
                    "Equipment list migration ID conflict"
                );
                let mut name = format!("{} equipment", t.name);
                if self
                    .equipment_lists
                    .iter()
                    .any(|l| l.profile_id == t.profile_id && l.name.eq_ignore_ascii_case(&name))
                {
                    name = format!("{name} ({})", t.id);
                }
                self.equipment_lists.push(EquipmentList { id: list.clone(), profile_id: t.profile_id.clone(), name, description: "Imported equipment access; shared by warrior types with the same original list.".into() });
                for item in items {
                    self.add_to_equipment_list(&list, &item);
                }
                shared.insert(key, list.clone());
                list
            };
            self.assign_equipment_list(&t.id, &list);
        }
        // Preserve legacy unit-specific additions without broadening other units.
        let units: BTreeSet<_> = legacy
            .iter()
            .filter(|p| self.units.iter().any(|u| u.id == p.type_id))
            .map(|p| p.type_id.clone())
            .collect();
        for unit in units {
            if self
                .equipment_list_assignments
                .iter()
                .any(|a| a.entity_id == unit)
            {
                continue;
            }
            let u = self.unit(&unit).clone();
            let base = self
                .equipment_list_for(&u.type_id)
                .map(|l| l.id.clone())
                .unwrap_or_default();
            let mut items: BTreeSet<_> = self
                .equipment_list_items
                .iter()
                .filter(|r| r.list_id == base)
                .map(|r| r.item_id.clone())
                .collect();
            items.extend(
                legacy
                    .iter()
                    .filter(|p| p.type_id == unit)
                    .map(|p| p.target_id.clone()),
            );
            let list = format!("equipment-list:{unit}");
            self.equipment_lists.push(EquipmentList {
                id: list.clone(),
                profile_id: self.typ(&u.type_id).profile_id.clone(),
                name: format!("{} equipment ({})", u.name, u.id),
                description: "Imported group-specific equipment access.".into(),
            });
            for item in items {
                self.add_to_equipment_list(&list, &item);
            }
            self.assign_equipment_list(&unit, &list);
        }
        for p in &legacy {
            ensure!(
                self.equipment_list_for(&p.type_id).is_some_and(|l| self
                    .equipment_list_items
                    .iter()
                    .any(|r| r.list_id == l.id && r.item_id == p.target_id)),
                "Legacy equipment permissions conflict with an assigned equipment list"
            );
        }
        self.permissions.retain(|p| p.kind != "item");
        Ok(())
    }
    pub fn equipment_entity_profile(&self, entity: &str) -> Option<&str> {
        if let Some(t) = self.types.iter().find(|t| t.id == entity) {
            return Some(&t.profile_id);
        }
        let unit = self
            .members
            .iter()
            .find(|m| m.id == entity)
            .map_or(entity, |m| m.unit_id.as_str());
        self.units
            .iter()
            .find(|u| u.id == unit)
            .and_then(|u| self.types.iter().find(|t| t.id == u.type_id))
            .map(|t| t.profile_id.as_str())
    }
    pub(super) fn validate_equipment_lists(&self) -> Result<()> {
        let mut names = BTreeSet::new();
        for list in &self.equipment_lists {
            ensure!(
                !list.name.trim().is_empty()
                    && self.profiles.iter().any(|p| p.id == list.profile_id),
                "Invalid equipment list {}",
                list.id
            );
            ensure!(
                names.insert((&list.profile_id, list.name.trim().to_lowercase())),
                "Duplicate equipment list name"
            );
        }
        let mut pairs = BTreeSet::new();
        for row in &self.equipment_list_items {
            let list = self
                .equipment_lists
                .iter()
                .find(|l| l.id == row.list_id)
                .ok_or_else(|| anyhow::anyhow!("Unknown equipment list {}", row.list_id))?;
            ensure!(
                self.items
                    .iter()
                    .any(|i| i.id == row.item_id && i.profile_id == list.profile_id),
                "Unknown or mismatched equipment list item"
            );
            ensure!(
                pairs.insert((&row.list_id, &row.item_id)),
                "Duplicate equipment list membership"
            );
        }
        let mut entities = BTreeSet::new();
        for row in &self.equipment_list_assignments {
            let list = self
                .equipment_lists
                .iter()
                .find(|l| l.id == row.list_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("Unknown assigned equipment list {}", row.list_id)
                })?;
            ensure!(
                self.equipment_entity_profile(&row.entity_id) == Some(list.profile_id.as_str()),
                "Unknown or mismatched equipment list assignment"
            );
            ensure!(
                entities.insert(&row.entity_id),
                "Multiple equipment lists assigned to one entity"
            );
        }
        Ok(())
    }
}
