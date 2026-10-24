//! Local mailbox shared by the lock-free MCP reader and the TUI's sole writer.
//! Proposals are immutable. Decisions are written only by the player workbench.
use super::plan::Plan;
use crate::{cli::Cli, engine, model::Db, store::Store};
use anyhow::{Context, Result, ensure};
use clap::Parser;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

pub const MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub created_at: String,
    pub generation: String,
    pub warband: Option<String>,
    pub prompt: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub id: String,
    pub created_at: String,
    pub generation: String,
    pub warband: Option<String>,
    pub request_id: Option<String>,
    pub plan: Plan,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub id: String,
    pub status: String,
    pub message: String,
    pub generation: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub valid: bool,
    pub report: String,
    pub generation: String,
}

fn valid_id(id: &str) -> Result<()> {
    ensure!(
        uuid::Uuid::parse_str(id).is_ok(),
        "Expected a proposal/request UUID"
    );
    Ok(())
}
fn read<T: DeserializeOwned>(root: &Path, kind: &str, id: &str) -> Result<T> {
    valid_id(id)?;
    let path = root.join("agent").join(kind).join(format!("{id}.json"));
    let file = fs::File::open(path)?;
    ensure!(
        file.metadata()?.len() <= MAX_BYTES,
        "Agent document exceeds 8 MiB"
    );
    serde_json::from_reader(file).context("Invalid agent document")
}
fn publish<T: Serialize>(root: &Path, kind: &str, id: &str, value: &T) -> Result<()> {
    valid_id(id)?;
    let bytes = serde_json::to_vec(value)?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "Agent document exceeds 8 MiB"
    );
    let dir = root.join("agent").join(kind);
    fs::create_dir_all(&dir)?;
    let mut staging = tempfile::NamedTempFile::new_in(&dir)?;
    staging.write_all(&bytes)?;
    staging.as_file().sync_all()?;
    staging.persist_noclobber(dir.join(format!("{id}.json")))?;
    fs::File::open(dir)?.sync_all()?;
    Ok(())
}
fn document_ids(root: &Path, kind: &str) -> Result<Vec<String>> {
    let dir = root.join("agent").join(kind);
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error.into()),
    };
    let mut ids = vec![];
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json")
            && let Some(id) = path.file_stem().and_then(|id| id.to_str())
            && valid_id(id).is_ok()
        {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}
fn documents<T: DeserializeOwned>(root: &Path, kind: &str) -> Result<Vec<T>> {
    document_ids(root, kind)?
        .iter()
        .map(|id| read(root, kind, id))
        .collect()
}
pub fn selected(db: &Db, warband: Option<&str>) -> Result<Option<String>> {
    let mut cli = Cli::try_parse_from(["mordheim"])?;
    cli.warband = warband.map(str::to_string);
    if warband.is_some() {
        return engine::selected(db, &cli).map(Some);
    }
    Ok(engine::selected(db, &cli).ok())
}
pub fn request(
    root: &Path,
    generation: &str,
    warband: Option<&str>,
    prompt: &str,
) -> Result<Request> {
    ensure!(
        !prompt.trim().is_empty(),
        "Enter a request for the external agent"
    );
    let request = Request {
        id: crate::model::id(),
        created_at: chrono::Utc::now().to_rfc3339(),
        generation: generation.into(),
        warband: warband.map(str::to_string),
        prompt: prompt.into(),
    };
    publish(root, "requests", &request.id, &request)?;
    Ok(request)
}
pub fn requests(root: &Path) -> Result<Vec<Request>> {
    let mut rows: Vec<Request> = documents(root, "requests")?;
    rows.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(rows)
}
pub fn submit(
    root: &Path,
    generation: &str,
    warband: Option<&str>,
    request_id: Option<String>,
    plan: Plan,
) -> Result<Proposal> {
    // Typed deserialization alone does not validate CLI arguments or forbidden commands.
    let plan = Plan::parse(&serde_json::to_string(&plan)?)?;
    let (current, db) = Store::read_snapshot(root)?;
    ensure!(
        current == generation,
        "Campaign changed; read context and prepare a new proposal"
    );
    let request = request_id
        .as_ref()
        .map(|id| read::<Request>(root, "requests", id))
        .transpose()?;
    let warband = match (warband, &request) {
        (None, Some(request)) => request.warband.clone(),
        _ => selected(&db, warband)?,
    };
    if let Some(request) = &request {
        ensure!(
            request.warband == warband,
            "Proposal must target the request's warband"
        );
    }
    let proposal = Proposal {
        id: crate::model::id(),
        created_at: chrono::Utc::now().to_rfc3339(),
        generation: current,
        warband,
        request_id,
        plan,
    };
    publish(root, "proposals", &proposal.id, &proposal)?;
    Ok(proposal)
}
pub fn proposal(root: &Path, id: &str) -> Result<Proposal> {
    let proposal: Proposal = read(root, "proposals", id)?;
    ensure!(proposal.id == id, "Mismatched proposal ID");
    Plan::parse(&serde_json::to_string(&proposal.plan)?)?;
    Ok(proposal)
}
pub fn pending(root: &Path) -> Result<Vec<Proposal>> {
    let mut rows: Vec<Proposal> = document_ids(root, "proposals")?
        .into_iter()
        .filter(|id| {
            !root
                .join("agent/decisions")
                .join(format!("{id}.json"))
                .exists()
        })
        .map(|id| proposal(root, &id))
        .collect::<Result<_>>()?;
    rows.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    Ok(rows)
}
pub fn record_review(root: &Path, proposal: &Proposal, valid: bool, report: &str) -> Result<()> {
    if root
        .join("agent/reviews")
        .join(format!("{}.json", proposal.id))
        .exists()
    {
        return Ok(());
    }
    publish(
        root,
        "reviews",
        &proposal.id,
        &Review {
            valid,
            report: report.into(),
            generation: proposal.generation.clone(),
        },
    )
}
/// Not exposed as an agent tool. Approval is performed only by ReviewedPlan in the TUI.
pub fn decide(
    root: &Path,
    proposal: &Proposal,
    status: &str,
    message: &str,
    generation: &str,
) -> Result<()> {
    ensure!(
        ["applied", "rejected", "stale", "failed"].contains(&status),
        "Invalid decision"
    );
    publish(
        root,
        "decisions",
        &proposal.id,
        &Decision {
            id: proposal.id.clone(),
            status: status.into(),
            message: message.into(),
            generation: generation.into(),
        },
    )
}
pub fn status(root: &Path, id: &str) -> Result<Value> {
    let proposal = proposal(root, id)?;
    let review = if root
        .join("agent/reviews")
        .join(format!("{id}.json"))
        .exists()
    {
        Some(read::<Review>(root, "reviews", id)?)
    } else {
        None
    };
    let path = root.join("agent/decisions").join(format!("{id}.json"));
    if path.exists() {
        let decision: Decision = read(root, "decisions", id)?;
        return Ok(json!({"proposal":proposal,"review":review,"decision":decision}));
    }
    let (generation, _) = Store::read_snapshot(root)?;
    Ok(
        json!({"proposal":proposal,"review":review,"decision":{"id":id,"status":if proposal.generation == generation {"pending"} else {"stale"},"message":"Only the player can review, approve or reject proposals in TUI tab 8"}}),
    )
}
