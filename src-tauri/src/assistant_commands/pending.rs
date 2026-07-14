use super::{project_session_id, validate_id};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

/// Exact reviewed change shown before a human approval decision.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantPendingApprovalDto {
    pub project_id: String,
    pub user_intent: String,
    pub candidate_digest: String,
    pub reviewer_version: String,
    pub evidence_hash: String,
    pub review_summary: String,
    pub review_findings: Vec<String>,
    pub effect: String,
    pub workflow: Value,
    pub readiness_blockers: Value,
}

/// Loads the exact candidate bound to the durable pending SDK approval.
#[tauri::command]
pub fn assistant_get_pending_approval(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Option<AssistantPendingApprovalDto>, String> {
    assistant_get_pending_approval_with_state(&project_id, &state)
}

pub fn assistant_get_pending_approval_with_state(
    project_id: &str,
    state: &AppState,
) -> Result<Option<AssistantPendingApprovalDto>, String> {
    validate_id("project_id", project_id)?;
    let session_id = project_session_id(project_id);
    let Some(waiting) = state.pending_approval.load(&session_id).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    pending_approval_dto(project_id, &session_id, &waiting, state).map(Some)
}

pub(super) fn pending_approval_dto(
    project_id: &str,
    session_id: &str,
    waiting: &crate::assistant_runtime::AssistantWaitingApproval,
    state: &AppState,
) -> Result<AssistantPendingApprovalDto, String> {
    let input: ReviewedApprovalInput = serde_json::from_str(waiting.pending().arguments_json())
        .map_err(|error| format!("pending approval arguments are invalid: {error}"))?;
    let (receipt, candidate) = state
        .reviewed_change
        .replay_candidate(project_id, session_id, &input.review_receipt_id)
        .map_err(|error| error.to_string())?;
    Ok(AssistantPendingApprovalDto {
        project_id: project_id.to_owned(),
        user_intent: candidate.user_intent().to_owned(),
        candidate_digest: candidate.digest().to_owned(),
        reviewer_version: receipt.reviewer_version().to_owned(),
        evidence_hash: receipt.evidence_hash().to_owned(),
        review_summary: receipt.summary().to_owned(),
        review_findings: receipt.findings().to_vec(),
        effect: "apply_reviewed_workflow_candidate".to_owned(),
        workflow: serde_json::to_value(candidate.workflow()).map_err(|e| e.to_string())?,
        readiness_blockers: serde_json::to_value(candidate.readiness_blockers())
            .map_err(|e| e.to_string())?,
    })
}

#[derive(Deserialize)]
struct ReviewedApprovalInput {
    review_receipt_id: String,
}
