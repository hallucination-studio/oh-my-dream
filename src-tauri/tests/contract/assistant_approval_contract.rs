use oh_my_dream_tauri::assistant_commands::{
    AssistantApprovalDecisionInput, AssistantPendingApprovalDto,
};
use serde_json::json;

#[derive(serde::Serialize)]
pub struct AssistantApprovalFixture {
    pending: AssistantPendingApprovalDto,
    decision: AssistantApprovalDecisionInput,
}

pub fn fixture() -> AssistantApprovalFixture {
    AssistantApprovalFixture {
        pending: AssistantPendingApprovalDto {
            project_id: "project-1".to_owned(),
            approval_scope_id: "scope-1".to_owned(),
            user_intent: "Build a film".to_owned(),
            candidate_digest: "sha256:candidate".to_owned(),
            reviewer_version: "reviewer-v1".to_owned(),
            evidence_hash: "sha256:evidence".to_owned(),
            review_summary: "Ready to apply".to_owned(),
            review_findings: vec!["Matches the requested production".to_owned()],
            effect: "apply_reviewed_workflow_candidate".to_owned(),
            workflow: json!({"version":"1.0","project_id":"project-1","nodes":[]}),
            readiness_blockers: json!([]),
        },
        decision: AssistantApprovalDecisionInput {
            project_id: "project-1".to_owned(),
            approval_scope_id: "scope-1".to_owned(),
            candidate_digest: "sha256:candidate".to_owned(),
            approved: true,
        },
    }
}
