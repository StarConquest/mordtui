use crate::model::*;
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const TABLES: &[&str] = &[
    "campaigns",
    "warbands",
    "profiles",
    "types",
    "items",
    "permissions",
    "abilities",
    "caps",
    "thresholds",
    "units",
    "members",
    "advances",
    "effects",
    "holdings",
    "allocations",
    "ledger",
    "notes",
    "battles",
    "participants",
    "steps",
    "searches",
    "resolutions",
    "dice",
    "snapshots",
    "events",
];
const EQUIPMENT_TABLES: &[&str] = &[
    "equipment_lists",
    "equipment_list_items",
    "equipment_list_assignments",
];
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub schema: u32,
    pub current: String,
    pub generations: BTreeMap<String, Db>,
}
pub struct Store {
    pub root: PathBuf,
    pub generation: String,
    pub db: Db,
    _lock: File,
}
fn table_write<T: Serialize>(dir: &Path, name: &str, rows: &[T]) -> Result<()> {
    let path = dir.join(format!("{name}.csv"));
    let mut w = csv::Writer::from_writer(File::create(&path)?);
    for r in rows {
        w.serialize(r).with_context(|| format!("Writing {name}"))?;
    }
    w.flush()?;
    w.into_inner()?.sync_all()?;
    Ok(())
}
fn table_read<T: DeserializeOwned>(dir: &Path, name: &str) -> Result<Vec<T>> {
    csv::Reader::from_path(dir.join(format!("{name}.csv")))?
        .deserialize()
        .collect::<std::result::Result<Vec<T>, _>>()
        .with_context(|| format!("Reading {name}.csv"))
}
pub fn write_tables(dir: &Path, db: &Db) -> Result<()> {
    fs::create_dir_all(dir)?;
    macro_rules! tables {($($f:ident),*)=>{$(table_write(dir,stringify!($f),&db.$f)?;)*};}
    tables!(
        campaigns,
        warbands,
        profiles,
        types,
        items,
        permissions,
        equipment_lists,
        equipment_list_items,
        equipment_list_assignments,
        abilities,
        caps,
        thresholds,
        units,
        members,
        advances,
        effects,
        holdings,
        allocations,
        ledger,
        notes,
        battles,
        participants,
        steps,
        searches,
        resolutions,
        dice,
        snapshots,
        events
    );
    Ok(())
}
pub fn read_tables(dir: &Path) -> Result<Db> {
    let mut db = Db::default();
    macro_rules! tables {($($f:ident),*)=>{$(db.$f=table_read(dir,stringify!($f))?;)*};}
    tables!(
        campaigns,
        warbands,
        profiles,
        types,
        items,
        permissions,
        abilities,
        caps,
        thresholds,
        units,
        members,
        advances,
        effects,
        holdings,
        allocations,
        ledger,
        notes,
        battles,
        participants,
        steps,
        searches,
        resolutions,
        dice,
        snapshots,
        events
    );
    if dir.join("equipment_lists.csv").exists() {
        db.equipment_lists = table_read(dir, "equipment_lists")?;
    }
    if dir.join("equipment_list_items.csv").exists() {
        db.equipment_list_items = table_read(dir, "equipment_list_items")?;
    }
    if dir.join("equipment_list_assignments.csv").exists() {
        db.equipment_list_assignments = table_read(dir, "equipment_list_assignments")?;
    }
    db.normalize_equipment_lists()?;
    db.validate()?;
    Ok(db)
}
fn manifest(dir: &Path) -> Result<BTreeMap<String, String>> {
    TABLES
        .iter()
        .chain(
            EQUIPMENT_TABLES
                .iter()
                .filter(|name| dir.join(format!("{name}.csv")).exists()),
        )
        .map(|name| {
            let file = format!("{name}.csv");
            Ok((
                file.clone(),
                format!("{:x}", Sha256::digest(fs::read(dir.join(file))?)),
            ))
        })
        .collect()
}
fn write_generation(root: &Path, generation_id: &str, db: &Db) -> Result<()> {
    ensure!(valid_gen(generation_id), "Invalid generation ID");
    let staging = root.join(format!("staging-{generation_id}"));
    ensure!(!staging.exists(), "Staging folder exists");
    write_tables(&staging, db)?;
    table_write(
        &staging,
        "manifest",
        &manifest(&staging)?
            .into_iter()
            .map(|(file, sha256)| Manifest { file, sha256 })
            .collect::<Vec<_>>(),
    )?;
    File::open(&staging)?.sync_all()?;
    fs::rename(staging, root.join("generations").join(generation_id))?;
    File::open(root.join("generations"))?.sync_all()?;
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    file: String,
    sha256: String,
}
fn valid_gen(generation_id: &str) -> bool {
    uuid::Uuid::parse_str(generation_id).is_ok()
}
fn load_generation(root: &Path, generation_id: &str) -> Result<Db> {
    ensure!(valid_gen(generation_id), "Invalid generation reference");
    let dir = root.join("generations").join(generation_id);
    let saved: Vec<Manifest> = table_read(&dir, "manifest")?;
    let expected: BTreeMap<_, _> = saved.into_iter().map(|x| (x.file, x.sha256)).collect();
    ensure!(
        expected == manifest(&dir)?,
        "Database CSVs changed or corrupt in {generation_id}. Use import to reconcile a copied database; never overwrite history."
    );
    read_tables(&dir)
}
fn point(root: &Path, generation_id: &str) -> Result<()> {
    let path = root.join("CURRENT.new");
    let mut f = File::create(&path)?;
    writeln!(f, "{generation_id}")?;
    f.sync_all()?;
    fs::rename(path, root.join("CURRENT"))?;
    File::open(root)?.sync_all()?;
    Ok(())
}
impl Store {
    /// CURRENT points at immutable, checksummed generations. Readers need no writer lock.
    /// Keep this generation ID with the data so later proposals can reject stale state.
    pub fn read_snapshot(root: &Path) -> Result<(String, Db)> {
        match fs::read_to_string(root.join("CURRENT")) {
            Ok(value) => {
                let generation = value.trim().to_string();
                let db = load_generation(root, &generation)?;
                Ok((generation, db))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok((String::new(), Db::default()))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("LOCK"))?;
        lock.try_lock_exclusive()
            .context("Database is in use by another process")?;
        fs::create_dir_all(root.join("generations"))?;
        let (generation, db) = if root.join("CURRENT").exists() {
            let generation_id = fs::read_to_string(root.join("CURRENT"))?.trim().to_string();
            let db = load_generation(&root, &generation_id)?;
            (generation_id, db)
        } else {
            (String::new(), Db::default())
        };
        Ok(Self {
            root,
            generation,
            db,
            _lock: lock,
        })
    }
    pub fn historical(&self, generation_id: &str) -> Result<Db> {
        if generation_id.is_empty() {
            return Ok(Db::default());
        }
        load_generation(&self.root, generation_id)
    }
    pub fn commit(
        &mut self,
        mut next: Db,
        action: &str,
        date: &str,
        reason: &str,
        provenance: &str,
    ) -> Result<String> {
        ensure!(
            self.generation
                == if self.root.join("CURRENT").exists() {
                    fs::read_to_string(self.root.join("CURRENT"))?
                        .trim()
                        .to_string()
                } else {
                    String::new()
                },
            "Database changed externally"
        );
        if !self.generation.is_empty() {
            load_generation(&self.root, &self.generation)?;
        }
        let generation_id = id();
        for snap in &mut next.snapshots {
            if snap.generation == "__next__" {
                snap.generation = generation_id.clone();
            }
        }
        let e = Event {
            id: id(),
            sequence: self.db.events.len() as u64 + 1,
            recorded_at: chrono::Utc::now().to_rfc3339(),
            date: date.into(),
            reason: reason.into(),
            action: action.into(),
            related_ids: next
                .warbands
                .iter()
                .map(|w| w.id.as_str())
                .collect::<Vec<_>>()
                .join(","),
            generation: generation_id.clone(),
            parent_generation: self.generation.clone(),
            provenance: provenance.into(),
        };
        next.events = self.db.events.clone();
        next.events.push(e);
        next.normalize_equipment_lists()?;
        next.validate()?;
        write_generation(&self.root, &generation_id, &next)?;
        point(&self.root, &generation_id)?;
        self.generation = generation_id.clone();
        self.db = next;
        Ok(generation_id)
    }
    pub fn bundle(&self) -> Result<Bundle> {
        let mut generations = BTreeMap::new();
        for event in &self.db.events {
            generations.insert(
                event.generation.clone(),
                self.historical(&event.generation)?,
            );
        }
        if !self.generation.is_empty() {
            generations.insert(self.generation.clone(), self.db.clone());
        }
        Ok(Bundle {
            schema: SCHEMA,
            current: self.generation.clone(),
            generations,
        })
    }
    pub fn restore_bundle(&mut self, mut bundle: Bundle) -> Result<()> {
        for db in bundle.generations.values_mut() {
            db.normalize_equipment_lists()?;
        }
        ensure!(
            self.generation.is_empty(),
            "Restore requires an empty database folder"
        );
        ensure!(bundle.schema == SCHEMA, "Unsupported bundle schema");
        ensure!(
            bundle.generations.contains_key(&bundle.current),
            "Missing current generation"
        );
        for (generation_id, db) in &bundle.generations {
            ensure!(valid_gen(generation_id), "Invalid generation");
            db.validate()?;
            for event in &db.events {
                ensure!(
                    bundle.generations.contains_key(&event.generation),
                    "Bundle missing event generation"
                );
            }
            for snap in &db.snapshots {
                ensure!(
                    bundle.generations.contains_key(&snap.generation),
                    "Bundle missing snapshot generation"
                );
            }
        }
        for (generation_id, db) in &bundle.generations {
            write_generation(&self.root, generation_id, db)?;
        }
        point(&self.root, &bundle.current)?;
        self.generation = bundle.current.clone();
        self.db = bundle.generations[&bundle.current].clone();
        Ok(())
    }
    /// Publish a validated preview in one CURRENT switch, preserving every existing image.
    pub(crate) fn adopt_reviewed(&mut self, bundle: Bundle, expected: &str) -> Result<()> {
        ensure!(
            self.generation == expected,
            "Campaign changed since review; review the plan again"
        );
        if expected.is_empty() && bundle.current.is_empty() && bundle.generations.is_empty() {
            return Ok(());
        }
        ensure!(
            bundle.schema == SCHEMA && bundle.generations.contains_key(&bundle.current),
            "Invalid reviewed state"
        );
        for (generation, original) in self.bundle()?.generations {
            ensure!(
                bundle.generations.get(&generation) == Some(&original),
                "Reviewed plan cannot rewrite existing history"
            );
        }
        for (generation, db) in &bundle.generations {
            ensure!(valid_gen(generation), "Invalid reviewed generation");
            db.validate()?;
            for event in &db.events {
                ensure!(
                    bundle.generations.contains_key(&event.generation),
                    "Missing reviewed event history"
                );
            }
            for snapshot in &db.snapshots {
                ensure!(
                    bundle.generations.contains_key(&snapshot.generation),
                    "Missing reviewed snapshot history"
                );
            }
            let path = self.root.join("generations").join(generation);
            if path.exists() {
                ensure!(
                    self.historical(generation)? == *db,
                    "Existing generation differs from review"
                );
            }
        }
        for (generation, db) in &bundle.generations {
            if !self.root.join("generations").join(generation).exists() {
                write_generation(&self.root, generation, db)?;
            }
        }
        point(&self.root, &bundle.current)?;
        self.db = bundle.generations[&bundle.current].clone();
        self.generation = bundle.current;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_equipment_generations_upgrade_in_memory_and_new_tables_stay_checksummed() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("generations")).unwrap();
        let legacy: Db =
            serde_json::from_str(include_str!("../examples/burgomeisters.json")).unwrap();
        let generation = id();
        write_generation(dir.path(), &generation, &legacy).unwrap();
        let path = dir.path().join("generations").join(&generation);
        for table in EQUIPMENT_TABLES {
            fs::remove_file(path.join(format!("{table}.csv"))).unwrap();
        }
        table_write(
            &path,
            "manifest",
            &manifest(&path)
                .unwrap()
                .into_iter()
                .map(|(file, sha256)| Manifest { file, sha256 })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        point(dir.path(), &generation).unwrap();
        let original: BTreeMap<_, _> = fs::read_dir(&path)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect();
        let mut store = Store::open(dir.path()).unwrap();
        assert!(store.db.equipment_allowed("hans", "burgomeisters-sword"));
        assert!(store.db.permissions.iter().all(|p| p.kind != "item"));
        assert_eq!(store.db, store.historical(&generation).unwrap());
        assert_eq!(store.db.allocations, legacy.allocations);
        let mut next = store.db.clone();
        next.notes.push(Note {
            id: id(),
            entity_id: "hans".into(),
            text: "A later note".into(),
            date: "2026-10-20".into(),
        });
        store
            .commit(next, "Note", "2026-10-20", "A later note", "manual")
            .unwrap();
        for (name, bytes) in &original {
            assert_eq!(&fs::read(path.join(name)).unwrap(), bytes);
        }
        for table in EQUIPMENT_TABLES {
            assert!(!path.join(format!("{table}.csv")).exists());
        }
        let current = store.generation.clone();
        drop(store);
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(
            store.db.equipment_list_for("hans").unwrap().id,
            store
                .historical(&generation)
                .unwrap()
                .equipment_list_for("hans")
                .unwrap()
                .id
        );
        drop(store);
        fs::remove_file(
            dir.path()
                .join("generations")
                .join(&current)
                .join("equipment_list_items.csv"),
        )
        .unwrap();
        let error = Store::open(dir.path()).err().unwrap().to_string();
        assert!(error.contains("corrupt"), "{error}");
    }
    #[test]
    fn interrupted_pointer_switch_keeps_previous_complete_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        store
            .commit(Db::default(), "initial", "2026-10-19", "", "manual")
            .unwrap();
        let original = store.generation.clone();
        let mut next = store.db.clone();
        next.notes.push(Note {
            id: id(),
            entity_id: "selection".into(),
            text: "uncommitted".into(),
            date: "2026-10-19".into(),
        });
        let interrupted = id();
        write_generation(dir.path(), &interrupted, &next).unwrap();
        std::fs::write(dir.path().join("CURRENT.new"), &interrupted).unwrap();
        drop(store);
        let reopened = Store::open(dir.path()).unwrap();
        assert_eq!(reopened.generation, original);
        assert!(reopened.db.notes.is_empty());
    }
}
