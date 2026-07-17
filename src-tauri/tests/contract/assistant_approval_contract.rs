use oh_my_dream_tauri::assistant_command_dto::{
    AssistantDecideWorkflowChangeRequestDto, AssistantPendingWorkflowChangeDto,
    AssistantWorkflowChangeDecisionDto, AssistantWorkflowChangeLineageDto,
    AssistantWorkflowChangeStateDto,
};

#[derive(serde::Serialize)]
pub struct AssistantApprovalFixture {
    pending: AssistantPendingWorkflowChangeDto,
    decision: AssistantDecideWorkflowChangeRequestDto,
}

pub fn fixture() -> AssistantApprovalFixture {
    AssistantApprovalFixture {
        pending: AssistantPendingWorkflowChangeDto {
            workflow_change_id: "20000000-0000-4000-8000-000000000001".to_owned(),
            project_id: "10000000-0000-4000-8000-000000000001".to_owned(),
            base_workflow_revision: "1".to_owned(),
            mutation_digest_hex: "00".repeat(32),
            approval_scope_id: "30000000-0000-4000-8000-000000000001".to_owned(),
            expires_at_epoch_ms: "1000".to_owned(),
            state: AssistantWorkflowChangeStateDto::AwaitingApproval,
            lineage: AssistantWorkflowChangeLineageDto::UserMessage {
                invocation_id: "40000000-0000-4000-8000-000000000001".to_owned(),
                intent: "Build a film".to_owned(),
            },
            mutations: vec![serde_json::json!({"type":"add_node"})],
            readiness_issues: Vec::new(),
        },
        decision: AssistantDecideWorkflowChangeRequestDto {
            project_id: "10000000-0000-4000-8000-000000000001".to_owned(),
            workflow_change_id: "20000000-0000-4000-8000-000000000001".to_owned(),
            approval_scope_id: "30000000-0000-4000-8000-000000000001".to_owned(),
            mutation_digest_hex: "00".repeat(32),
            decision: AssistantWorkflowChangeDecisionDto::Approve,
        },
    }
}
