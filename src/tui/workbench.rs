use super::*;
use crate::ai::{
    exchange::{self, Proposal},
    plan::{Plan, ReviewedPlan},
};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Workbench {
    pub prompt: String,
    pub report: String,
    pub scroll: u16,
    pub max_scroll: u16,
    pub plan: Option<Plan>,
    pub reviewed: Option<ReviewedPlan>,
    pub generation: String,
    pub proposal: Option<Proposal>,
    pub inbox: Vec<Proposal>,
    last_scan: Option<Instant>,
}
impl App {
    pub(super) fn ask_ai(&mut self, store: &Store, prompt: String) {
        match exchange::request(&store.root, &store.generation, self.wb.as_deref(), &prompt) {
            Ok(request) => {
                self.ai.prompt = prompt;
                self.message = format!(
                    "Local task {} queued for your external agent. No changes applied.",
                    request.id
                );
                if self.ai.proposal.is_none() {
                    self.ai.report.clear();
                }
                self.tab = AI_TAB;
                self.overlay = Overlay::None;
            }
            Err(error) => self.message = format!("Could not queue agent task: {error:#}"),
        }
    }
    pub(super) fn poll_ai(&mut self, store: &Store) {
        if self
            .ai
            .last_scan
            .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        self.ai.last_scan = Some(Instant::now());
        match exchange::pending(&store.root) {
            Ok(inbox) => self.ai.inbox = inbox,
            Err(error) => {
                self.message = format!("Agent inbox could not be read: {error:#}");
                return;
            }
        }
        if self.ai.proposal.is_none()
            && self.ai.plan.is_none()
            && self.ai.report.is_empty()
            && matches!(self.overlay, Overlay::None)
            && !self.ai.inbox.is_empty()
        {
            self.next_ai(store);
        }
    }
    pub(super) fn next_ai(&mut self, store: &Store) {
        let inbox = match exchange::pending(&store.root) {
            Ok(inbox) => inbox,
            Err(error) => {
                self.message = format!("Agent inbox error: {error:#}");
                return;
            }
        };
        let index = self
            .ai
            .proposal
            .as_ref()
            .and_then(|p| inbox.iter().position(|r| r.id == p.id))
            .map_or(0, |i| (i + 1) % inbox.len());
        self.ai.inbox = inbox;
        let Some(row) = self.ai.inbox.get(index) else {
            self.message = "No pending proposals. Connect your external agent over MCP.".into();
            return;
        };
        match exchange::proposal(&store.root, &row.id) {
            Ok(proposal) => {
                self.ai.generation = proposal.generation.clone();
                self.ai.report = proposal.plan.description();
                self.ai.plan = Some(proposal.plan.clone());
                self.ai.proposal = Some(proposal);
                self.ai.reviewed = None;
                self.ai.scroll = 0;
                self.message =
                    "External agent proposal loaded on tab 8. R reviews; approval is separate."
                        .into();
            }
            Err(error) => self.message = format!("Invalid agent proposal: {error:#}"),
        }
    }
    pub(super) fn review_ai(&mut self, store: &Store) {
        if self.ai.generation != store.generation {
            self.message = "Campaign changed since the proposal. Ask the agent for a fresh plan; D dismisses this one.".into();
            self.ai.reviewed = None;
            return;
        }
        let Some(plan) = &self.ai.plan else {
            self.message = "Load a proposal with P first.".into();
            return;
        };
        let warband = self.ai.proposal.as_ref().and_then(|p| p.warband.as_deref());
        match crate::ai::plan::review(store, warband, plan) {
            Ok(reviewed) => {
                self.ai.report = reviewed.report.clone();
                self.ai.reviewed = Some(reviewed);
                self.ai.scroll = 0;
                self.message = "Every proposed action validated in a disposable copy. Nothing applied. Y opens approval.".into();
            }
            Err(error) => {
                self.ai.reviewed = None;
                self.ai.report = format!(
                    "{}\n\nPLAN REJECTED BY VALIDATION:\n{error:#}\nNo campaign changes applied.",
                    plan.description()
                );
                self.message = "Plan could not be validated. Ask the external agent to revise it; D dismisses.".into();
            }
        }
        if let Some(proposal) = &self.ai.proposal
            && let Err(error) = exchange::record_review(
                &store.root,
                proposal,
                self.ai.reviewed.is_some(),
                &self.ai.report,
            )
        {
            self.message.push_str(&format!(
                " Agent review feedback could not be saved: {error:#}"
            ));
        }
    }
    pub(super) fn confirm_ai(&mut self, store: &Store) {
        let Some(reviewed) = &self.ai.reviewed else {
            self.message = "Press R to validate and review all proposed actions first.".into();
            return;
        };
        if !reviewed.is_current(store) {
            self.ai.reviewed = None;
            self.message = "Campaign changed since review. Request a revised plan.".into();
            return;
        }
        self.overlay = Overlay::AiApproval { scroll: 0 };
    }
    pub(super) fn apply_ai(&mut self, store: &mut Store) {
        if !matches!(self.overlay, Overlay::AiApproval { .. }) {
            return;
        }
        let Some(reviewed) = self.ai.reviewed.take() else {
            return;
        };
        let selected = reviewed.selected_warband().map(str::to_string);
        match reviewed.approve(store) {
            Ok(()) => {
                self.ai.report.push_str("\nPLAYER APPROVED: all reviewed actions applied. Approval consumed; this plan cannot run again.\n");
                self.ai.plan = None;
                self.ai.generation = store.generation.clone();
                self.wb = selected.or_else(|| {
                    self.wb
                        .take()
                        .filter(|w| store.db.warbands.iter().any(|r| r.id == *w))
                        .or_else(|| store.db.warbands.first().map(|w| w.id.clone()))
                });
                self.selected = [0; 8];
                self.filter.clear();
                self.selected[4] = self
                    .steps(&store.db)
                    .iter()
                    .position(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
                    .unwrap_or(0);
                self.message =
                    "Approved external-agent plan applied. Journal records AI-assisted changes."
                        .into();
                if let Some(proposal) = self.ai.proposal.take() {
                    self.record_decision(
                        store,
                        &proposal,
                        "applied",
                        "Player reviewed and explicitly approved all actions",
                    );
                }
            }
            Err(error) => {
                self.ai
                    .report
                    .push_str(&format!("\nApproval failed: {error:#}\n"));
                self.message = format!("AI plan not applied: {error}");
                self.ai.plan = None;
                if let Some(proposal) = self.ai.proposal.take() {
                    self.record_decision(store, &proposal, "failed", &format!("{error:#}"));
                }
            }
        }
        self.overlay = Overlay::None;
    }
    fn record_decision(&mut self, store: &Store, proposal: &Proposal, status: &str, message: &str) {
        if let Err(error) =
            exchange::decide(&store.root, proposal, status, message, &store.generation)
        {
            self.message
                .push_str(&format!(" Decision receipt could not be saved: {error:#}"));
        }
        self.ai.inbox.retain(|p| p.id != proposal.id);
    }
    pub(super) fn reject_ai(&mut self, store: &Store) {
        if let Some(proposal) = self.ai.proposal.take() {
            self.record_decision(
                store,
                &proposal,
                "rejected",
                "Player rejected the proposal; no actions applied",
            );
            self.ai.report.push_str(
                "\nPLAYER REJECTED this proposal. No actions from this proposal applied.\n",
            );
            self.message = "External-agent proposal rejected.".into();
        } else {
            self.message = "No loaded proposal to reject.".into();
        }
        self.ai.plan = None;
        self.ai.reviewed = None;
        self.overlay = Overlay::None;
    }
}
