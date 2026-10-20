use crate::model::*;
use anyhow::{Result, ensure};
use serde::de::DeserializeOwned;
use std::path::Path;
fn read<T: DeserializeOwned>(dir: &Path, name: &str) -> Result<Vec<T>> {
    Ok(csv::Reader::from_path(dir.join(format!("{name}.csv")))?
        .deserialize()
        .collect::<std::result::Result<Vec<T>, _>>()?)
}
pub fn load(dir: &Path) -> Result<Db> {
    let mut db = Db {
        profiles: read(dir, "profiles")?,
        types: read(dir, "types")?,
        items: read(dir, "items")?,
        permissions: read(dir, "permissions")?,
        abilities: read(dir, "abilities")?,
        caps: read(dir, "caps")?,
        thresholds: read(dir, "thresholds")?,
        ..Db::default()
    };
    if dir.join("equipment_lists.csv").exists() {
        db.equipment_lists = read(dir, "equipment_lists")?;
    }
    if dir.join("equipment_list_items.csv").exists() {
        db.equipment_list_items = read(dir, "equipment_list_items")?;
    }
    if dir.join("equipment_list_assignments.csv").exists() {
        db.equipment_list_assignments = read(dir, "equipment_list_assignments")?;
    }
    db.normalize_equipment_lists()?;
    db.validate()?;
    ensure!(!db.profiles.is_empty(), "No profiles defined");
    Ok(db)
}
pub fn merge(db: &mut Db, rules: Db) -> Result<()> {
    for p in &rules.profiles {
        ensure!(
            !db.profiles.iter().any(|a| a.id == p.id),
            "Profile ID {} already pinned; use new IDs/version for migration",
            p.id
        );
    }
    db.profiles.extend(rules.profiles);
    db.types.extend(rules.types);
    db.items.extend(rules.items);
    db.permissions.extend(rules.permissions);
    db.equipment_lists.extend(rules.equipment_lists);
    db.equipment_list_items.extend(rules.equipment_list_items);
    db.equipment_list_assignments
        .extend(rules.equipment_list_assignments);
    db.abilities.extend(rules.abilities);
    db.caps.extend(rules.caps);
    db.thresholds.extend(rules.thresholds);
    db.normalize_equipment_lists()?;
    db.validate()
}
