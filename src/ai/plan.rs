//! Models propose CLI actions. Only a player-approved, generation-bound preview can publish them.
use crate::{
    cli::{Cli, Command},
    engine,
    model::Db,
    store::{Bundle, Store},
};
use anyhow::{Context, Result, ensure};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedAction {
    pub description: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub summary: String,
    pub actions: Vec<PlannedAction>,
}
impl Plan {
    pub fn parse(response: &str) -> Result<Self> {
        let response = response.trim();
        let json = if response.starts_with("```") {
            response
                .split_once('\n')
                .context("Invalid fenced proposal")?
                .1
                .trim_end()
                .strip_suffix("```")
                .context("Unclosed proposal fence")?
                .trim()
        } else {
            response
        };
        let plan: Self =
            serde_json::from_str(json).context("AI must return a JSON action proposal")?;
        ensure!(
            plan.actions.len() <= 256,
            "Split proposals longer than 256 actions into smaller plans"
        );
        for action in &plan.actions {
            ensure!(
                !action.args.is_empty() && !action.description.trim().is_empty(),
                "Each action needs a description and argument list"
            );
            parsed(action, None)?;
        }
        Ok(plan)
    }
    pub fn description(&self) -> String {
        let mut text = format!("AI PROPOSAL — nothing applied\n{}\n", self.summary);
        for (i, action) in self.actions.iter().enumerate() {
            text.push_str(&format!(
                "\n{}. {}\n   Arguments: {}\n   Reason: {}\n",
                i + 1,
                action.description,
                serde_json::to_string(&action.args).unwrap_or_default(),
                action.reason
            ));
        }
        text
    }
}
fn parsed(action: &PlannedAction, warband: Option<&str>) -> Result<Cli> {
    let mut cli = Cli::try_parse_from(
        std::iter::once("mordheim").chain(action.args.iter().map(String::as_str)),
    )?;
    ensure!(
        !action
            .args
            .iter()
            .any(|a| a == "--db" || a.starts_with("--db=")),
        "A proposal cannot select another database folder"
    );
    ensure!(
        !matches!(
            cli.command,
            None | Some(Command::Ai { .. } | Command::Mcp | Command::Tui)
        ),
        "Proposals contain player actions, not recursive AI requests or terminal sessions"
    );
    if cli.warband.is_none() && cli.campaign.is_none() {
        cli.warband = warband.map(str::to_string);
    }
    cli.reason = format!(
        "AI-assisted: {}",
        if action.reason.trim().is_empty() {
            if cli.reason.trim().is_empty() {
                action.description.as_str()
            } else {
                cli.reason.as_str()
            }
        } else {
            action.reason.as_str()
        }
    );
    Ok(cli)
}
struct Artifact {
    source: PathBuf,
    destination: PathBuf,
}
pub struct ReviewedPlan {
    base: String,
    bundle: Bundle,
    artifacts: Vec<Artifact>,
    _sandbox: tempfile::TempDir,
    pub report: String,
    selected_warband: Option<String>,
}
impl ReviewedPlan {
    pub fn selected_warband(&self) -> Option<&str> {
        self.selected_warband.as_deref()
    }
    pub fn is_current(&self, store: &Store) -> bool {
        self.base == store.generation
    }
    /// Called only by the explicit Approve control. Preview construction never publishes.
    pub fn approve(self, store: &mut Store) -> Result<()> {
        ensure!(
            self.is_current(store),
            "Campaign changed since review; request/review a new plan"
        );
        let mut created = vec![];
        let result = (|| {
            for artifact in &self.artifacts {
                copy_new(&artifact.source, &artifact.destination, &mut created)?;
            }
            store.adopt_reviewed(self.bundle, &self.base)
        })();
        if result.is_err() {
            for path in created.into_iter().rev() {
                if path.is_dir() {
                    let _ = fs::remove_dir(path);
                } else {
                    let _ = fs::remove_file(path);
                }
            }
        }
        result
    }
}
pub fn review(store: &Store, warband: Option<&str>, plan: &Plan) -> Result<ReviewedPlan> {
    ensure!(
        !plan.actions.is_empty(),
        "This proposal contains advice only; there are no actions to approve"
    );
    let sandbox = tempfile::tempdir()?;
    let mut staged = Store::open(sandbox.path().join("database"))?;
    if !store.generation.is_empty() {
        staged.restore_bundle(store.bundle()?)?;
    }
    let mut artifacts: Vec<Artifact> = vec![];
    let mut report = plan.description();
    let mut active_warband = warband.map(str::to_string);
    for (i, action) in plan.actions.iter().enumerate() {
        let mut cli = parsed(action, active_warband.as_deref())?;
        cli.db = staged.root.clone();
        if let Some(Command::Export { file, .. }) = &mut cli.command {
            let destination = destination(file)?;
            ensure!(
                !destination.starts_with(fs::canonicalize(&store.root)?),
                "Exports must be outside the database directory"
            );
            ensure!(
                !destination.exists() && !artifacts.iter().any(|a| a.destination == destination),
                "Export destination already exists or is duplicated"
            );
            let source = sandbox.path().join(format!("export-{i}"));
            *file = source.clone();
            artifacts.push(Artifact {
                source,
                destination,
            });
        } else if let Some(Command::Import { file, .. } | Command::Recover { file }) =
            &mut cli.command
        {
            let target = destination(file)?;
            if let Some(artifact) = artifacts.iter().find(|a| a.destination == target) {
                *file = artifact.source.clone();
            }
        }
        let before = staged.db.clone();
        let mut result = engine::execute(&mut staged, &cli)
            .with_context(|| format!("Action {} failed: {}", i + 1, action.description))?;
        if matches!(cli.command, Some(Command::Select { .. })) {
            active_warband = result["id"].as_str().map(str::to_string);
        }
        if active_warband
            .as_ref()
            .is_none_or(|id| !staged.db.warbands.iter().any(|w| w.id == *id))
        {
            let selection = Cli::try_parse_from(["mordheim"])?;
            active_warband = engine::selected(&staged.db, &selection).ok();
        }
        if matches!(cli.command, Some(Command::Export { .. })) {
            result["exported"] = artifacts
                .last()
                .unwrap()
                .destination
                .display()
                .to_string()
                .into();
        }
        report.push_str(&format!(
            "\nVALIDATED ACTION {}: {}\n{}\n",
            i + 1,
            action.description,
            serde_json::to_string_pretty(&result)?
        ));
        for change in changes(&before, &staged.db)? {
            report.push_str(&format!("  {change}\n"));
        }
        if matches!(cli.command, Some(Command::Export { .. })) {
            let artifact = artifacts.last().unwrap();
            report.push_str(&format!(
                "  On approval: write {} to {}\n",
                if artifact.source.is_dir() {
                    "CSV tables"
                } else {
                    "export file"
                },
                artifact.destination.display()
            ));
        }
    }
    report.push_str("\nALL ACTIONS VALIDATED. Nothing has been applied to your campaign.\nY opens the explicit approval confirmation; D rejects this plan.\n");
    Ok(ReviewedPlan {
        base: store.generation.clone(),
        bundle: staged.bundle()?,
        artifacts,
        _sandbox: sandbox,
        report,
        selected_warband: active_warband,
    })
}
fn destination(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            c => normalized.push(c),
        }
    }
    let mut ancestor = normalized.as_path();
    let mut tail = vec![];
    while !ancestor.exists() {
        tail.push(
            ancestor
                .file_name()
                .context("Choose an export filename")?
                .to_os_string(),
        );
        ancestor = ancestor.parent().context("Invalid output path")?;
    }
    let mut resolved = fs::canonicalize(ancestor)?;
    for component in tail.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}
fn copy_new(source: &Path, target: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    ensure!(
        !target.exists(),
        "Approved export destination already exists: {}",
        target.display()
    );
    if let Some(parent) = target.parent().filter(|p| !p.exists()) {
        create_parents(parent, created)?;
    }
    if source.is_dir() {
        fs::create_dir(target)?;
        created.push(target.into());
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_new(&entry.path(), &target.join(entry.file_name()), created)?;
        }
    } else {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
        created.push(target.into());
        std::io::copy(&mut fs::File::open(source)?, &mut file)?;
        file.sync_all()?;
    }
    Ok(())
}
fn create_parents(path: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        create_parents(parent, created)?;
    }
    fs::create_dir(path)?;
    created.push(path.into());
    Ok(())
}
pub fn changes(before: &Db, after: &Db) -> Result<Vec<String>> {
    let (before, after) = (serde_json::to_value(before)?, serde_json::to_value(after)?);
    let mut changes = vec![];
    for (table, rows) in after.as_object().context("Invalid state")? {
        if table == "events" {
            continue;
        }
        let old = before[table].as_array().context("Invalid table")?;
        let new = rows.as_array().context("Invalid table")?;
        for row in new {
            match old.iter().find(|r| r["id"] == row["id"]) {
                None => changes.push(format!("ADD {table}: {row}")),
                Some(previous) if previous != row => {
                    changes.push(format!("CHANGE {table}: {previous} → {row}"))
                }
                _ => {}
            }
        }
        for row in old
            .iter()
            .filter(|r| !new.iter().any(|n| n["id"] == r["id"]))
        {
            changes.push(format!("REMOVE {table}: {row}"));
        }
    }
    if changes.is_empty() {
        changes.push("No campaign data changed (read-only action).".into());
    }
    Ok(changes)
}
