use oh_my_dream_tauri::assistant_commands::AssistantPendingApprovalDto;
use serde_json::json;

pub fn fixture() -> AssistantPendingApprovalDto {
    AssistantPendingApprovalDto {
        project_id: "project-1".to_owned(),
        user_intent: "Build a film".to_owned(),
        candidate_digest: "sha256:candidate".to_owned(),
        reviewer_version: "reviewer-v1".to_owned(),
        evidence_hash: "sha256:evidence".to_owned(),
        workflow: json!({"version":"1.0","project_id":"project-1","nodes":[]}),
        readiness_blockers: json!([]),
    }
}
