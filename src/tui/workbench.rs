use super::*;
use crate::ai::plan::{Plan, ReviewedPlan};
use std::sync::mpsc;

#[derive(Default)]
pub(super) struct Workbench {
    pub prompt: String,
    pub report: String,
    pub scroll: u16,
    pub max_scroll: u16,
    pub plan: Option<Plan>,
    pub reviewed: Option<ReviewedPlan>,
    pub generation: String,
    pub receiver: Option<mpsc::Receiver<std::result::Result<Plan, String>>>,
    conversation: String,
}
impl App {
    pub(super) fn ask_ai(&mut self, store: &Store, prompt: String) {
        self.ai.plan = None;
        self.ai.reviewed = None;
        self.ai.scroll = 0;
        self.ai.prompt = prompt.clone();
        self.ai.generation = store.generation.clone();
        self.ai.report = "Waiting for the configured AI. You can keep using the other tabs. No campaign actions have been applied.".into();
        self.ai
            .conversation
            .push_str(&format!("\nPLAYER: {prompt}\n"));
        let request = self.ai.conversation.clone();
        let (db, warband) = (store.db.clone(), self.wb.clone());
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(
                crate::ai::propose(&db, warband.as_deref(), &request).map_err(|e| format!("{e:#}")),
            );
        });
        self.ai.receiver = Some(receiver);
        self.tab = 6;
        self.overlay = Overlay::None;
    }
    pub(super) fn poll_ai(&mut self, store: &Store) {
        let result = self.ai.receiver.as_ref().and_then(|r| match r.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                "AI request ended without a proposal. No changes applied.".into(),
            )),
        });
        if let Some(result) = result {
            self.ai.receiver = None;
            match result {
                Ok(plan) => {
                    self.ai.report = plan.description();
                    self.ai.conversation.push_str(&format!(
                        "ASSISTANT PROPOSAL (not executed): {}\n",
                        self.ai.report
                    ));
                    self.ai.plan = Some(plan);
                    self.message = if self.ai.generation == store.generation { "AI proposal ready on tab 7. R validates; approval is separate." } else { "Campaign changed during the AI request; request a revised plan before approving." }.into();
                }
                Err(error) => {
                    self.ai.report =
                        format!("AI request failed. {error}\nNo campaign changes applied.");
                    self.message = "AI request failed; see tab 7.".into();
                }
            }
        }
    }
    pub(super) fn review_ai(&mut self, store: &Store) {
        if self.ai.generation != store.generation {
            self.message = "Campaign changed since the proposal. Request a revised plan.".into();
            self.ai.reviewed = None;
            return;
        }
        let Some(plan) = &self.ai.plan else {
            self.message = "Request a proposal first.".into();
            return;
        };
        match crate::ai::plan::review(store, self.wb.as_deref(), plan) {
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
                self.message =
                    "Plan could not be validated. Ask the assistant to revise it.".into();
            }
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
                self.ai
                    .conversation
                    .push_str("PLAYER APPROVED AND APPLIED the last proposal.\n");
                self.ai.plan = None;
                self.ai.generation = store.generation.clone();
                self.wb = selected.or_else(|| {
                    self.wb
                        .take()
                        .filter(|w| store.db.warbands.iter().any(|r| r.id == *w))
                        .or_else(|| store.db.warbands.first().map(|w| w.id.clone()))
                });
                self.selected = [0; 7];
                self.filter.clear();
                self.selected[4] = self
                    .steps(&store.db)
                    .iter()
                    .position(|s| !matches!(s.status.as_str(), "completed" | "skipped"))
                    .unwrap_or(0);
                self.message =
                    "Approved AI plan applied. Journal records AI-assisted changes.".into();
            }
            Err(error) => {
                self.ai
                    .report
                    .push_str(&format!("\nApproval failed: {error:#}\n"));
                self.message = format!("AI plan not applied: {error}");
            }
        }
        self.overlay = Overlay::None;
    }
    pub(super) fn reject_ai(&mut self) {
        self.ai.receiver = None;
        self.ai.plan = None;
        self.ai.reviewed = None;
        self.ai
            .report
            .push_str("\nPLAYER REJECTED this proposal. No actions from this proposal applied.\n");
        self.ai
            .conversation
            .push_str("PLAYER REJECTED the previous proposal.\n");
        self.overlay = Overlay::None;
        self.message = "AI proposal rejected.".into();
    }
}
