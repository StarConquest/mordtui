//! Local external-agent integration. Agents propose; only the player publishes.
pub mod exchange;
pub mod mcp;
pub mod plan;
use crate::{engine::summary, model::Db};
use serde_json::{Value, json};

pub fn queue_request(cli: &crate::cli::Cli) -> anyhow::Result<Value> {
    let (generation, db) = crate::store::Store::read_snapshot(&cli.db)?;
    queue_request_from(&cli.db, &generation, &db, cli)
}
pub(crate) fn queue_request_from(
    root: &std::path::Path,
    generation: &str,
    db: &Db,
    cli: &crate::cli::Cli,
) -> anyhow::Result<Value> {
    let Some(crate::cli::Command::Ai { prompt }) = &cli.command else {
        anyhow::bail!("Expected an external-agent request");
    };
    let warband = if cli.warband.is_some() || cli.campaign.is_some() {
        Some(crate::engine::selected(db, cli)?)
    } else {
        crate::engine::selected(db, cli).ok()
    };
    let request = exchange::request(root, generation, warband.as_deref(), prompt)?;
    Ok(
        json!({"request":request,"changes_applied":false,"next":"Connect an external agent with mordheim mcp; review proposals in TUI tab 8"}),
    )
}

pub fn context(db: &Db, warband: Option<&str>) -> Value {
    let Some(w) = warband.and_then(|id| db.warbands.iter().find(|w| w.id == id)) else {
        return json!({"state":"No warband selected", "campaigns":db.campaigns, "profiles":db.profiles,"rule_definition_templates":crate::rules::editor::templates()});
    };
    let mut selected = db.clone();
    selected.warbands.retain(|r| r.id == w.id);
    selected.campaigns.retain(|r| r.id == w.campaign_id);
    selected.profiles.retain(|r| r.id == w.profile_id);
    selected.types.retain(|r| r.profile_id == w.profile_id);
    selected.items.retain(|r| r.profile_id == w.profile_id);
    selected
        .equipment_lists
        .retain(|r| r.profile_id == w.profile_id);
    selected
        .equipment_list_items
        .retain(|r| selected.equipment_lists.iter().any(|l| l.id == r.list_id));
    selected.abilities.retain(|r| r.profile_id == w.profile_id);
    selected.units.retain(|r| r.warband_id == w.id);
    selected
        .members
        .retain(|r| selected.units.iter().any(|u| u.id == r.unit_id));
    selected.permissions.retain(|r| {
        selected.types.iter().any(|t| t.id == r.type_id)
            || selected.units.iter().any(|u| u.id == r.type_id)
    });
    selected.equipment_list_assignments.retain(|r| {
        selected.types.iter().any(|t| t.id == r.entity_id)
            || selected.units.iter().any(|u| u.id == r.entity_id)
            || selected.members.iter().any(|m| m.id == r.entity_id)
    });
    selected
        .caps
        .retain(|r| selected.types.iter().any(|t| t.id == r.type_id));
    selected.thresholds.retain(|r| r.profile_id == w.profile_id);
    selected
        .advances
        .retain(|r| selected.units.iter().any(|u| u.id == r.unit_id));
    selected
        .effects
        .retain(|r| selected.members.iter().any(|m| m.id == r.member_id));
    selected.holdings.retain(|r| r.warband_id == w.id);
    selected
        .allocations
        .retain(|r| selected.members.iter().any(|m| m.id == r.member_id));
    selected.ledger.retain(|r| r.warband_id == w.id);
    selected.battles.retain(|r| r.warband_id == w.id);
    selected
        .participants
        .retain(|r| selected.battles.iter().any(|b| b.id == r.battle_id));
    selected
        .steps
        .retain(|r| selected.battles.iter().any(|b| b.id == r.battle_id));
    selected
        .searches
        .retain(|r| selected.battles.iter().any(|b| b.id == r.battle_id));
    selected
        .resolutions
        .retain(|r| selected.battles.iter().any(|b| b.id == r.battle_id));
    selected.notes.retain(|r| {
        r.entity_id == w.id
            || r.entity_id == w.campaign_id
            || selected.members.iter().any(|m| m.id == r.entity_id)
            || selected.units.iter().any(|u| u.id == r.entity_id)
            || selected.battles.iter().any(|b| b.id == r.entity_id)
    });
    selected.dice.clear();
    selected.snapshots.retain(|s| s.warband_id == w.id);
    selected.events.clear();
    json!({"summary":summary(db,&w.id),"selected_warband_and_rules":selected,"rule_definition_templates":crate::rules::editor::templates(),"privacy":"No unrelated warbands, global history, credentials or filesystem contents included"})
}
