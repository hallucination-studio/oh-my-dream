//! Frozen Assistant command DTOs and domain projections.

use assistant::domain::{
    AssistantWorkflowChangeAggregate, AssistantWorkflowChangeLineage, AssistantWorkflowChangeState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantSendMessageRequestDto {
    pub project_id: String,
    pub workflow_present: bool,
    pub workflow_revision: Option<String>,
    #[serde(default)]
    pub selected_node_ids: Vec<String>,
    #[serde(default)]
    pub selected_asset_ids: Vec<String>,
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantGetPendingWorkflowChangeRequestDto {
    pub project_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantDecideWorkflowChangeRequestDto {
    pub project_id: String,
    pub workflow_change_id: String,
    pub approval_scope_id: String,
    pub mutation_digest_hex: String,
    pub decision: AssistantWorkflowChangeDecisionDto,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantWorkflowChangeDecisionDto {
    Approve,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AssistantSendMessageResultDto {
    pub invocation_id: String,
    pub final_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AssistantWorkflowChangeDecisionResultDto {
    pub workflow_change_id: String,
    pub state: AssistantWorkflowChangeStateDto,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AssistantPendingWorkflowChangeDto {
    pub workflow_change_id: String,
    pub project_id: String,
    pub base_workflow_revision: String,
    pub mutation_digest_hex: String,
    pub approval_scope_id: String,
    pub expires_at_epoch_ms: String,
    pub state: AssistantWorkflowChangeStateDto,
    pub lineage: AssistantWorkflowChangeLineageDto,
    pub mutations: Vec<serde_json::Value>,
    pub readiness_issues: Vec<serde_json::Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantWorkflowChangeStateDto {
    Proposed,
    ReviewRejected,
    AwaitingApproval,
    Rejected,
    Applying,
    Applied,
    ApplyFailed,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssistantWorkflowChangeLineageDto {
    UserMessage { invocation_id: String, intent: String },
    ReviewedRepair { activation_id: String, failed_workflow_run_id: String },
}

pub fn pending_workflow_change_dto(
    change: AssistantWorkflowChangeAggregate,
) -> Result<AssistantPendingWorkflowChangeDto, serde_json::Error> {
    Ok(AssistantPendingWorkflowChangeDto {
        workflow_change_id: change.id().as_uuid().to_string(),
        project_id: change.project_id().as_uuid().to_string(),
        base_workflow_revision: change.base_workflow_revision().get().to_string(),
        mutation_digest_hex: hex(&change.mutation_digest().as_bytes()),
        approval_scope_id: change.approval_scope_id().as_uuid().to_string(),
        expires_at_epoch_ms: change.expires_at().epoch_ms().to_string(),
        state: state(change.state()),
        lineage: lineage(change.lineage()),
        mutations: change
            .ordered_mutations()
            .iter()
            .map(|value| serde_json::from_slice(value.canonical_bytes()))
            .collect::<Result<_, _>>()?,
        readiness_issues: change
            .readiness_issues()
            .iter()
            .map(|value| serde_json::from_slice(value.canonical_bytes()))
            .collect::<Result<_, _>>()?,
    })
}

pub fn workflow_change_decision_result_dto(
    change: AssistantWorkflowChangeAggregate,
) -> AssistantWorkflowChangeDecisionResultDto {
    AssistantWorkflowChangeDecisionResultDto {
        workflow_change_id: change.id().as_uuid().to_string(),
        state: state(change.state()),
    }
}

fn state(value: AssistantWorkflowChangeState) -> AssistantWorkflowChangeStateDto {
    match value {
        AssistantWorkflowChangeState::AwaitingApproval => {
            AssistantWorkflowChangeStateDto::AwaitingApproval
        }
        AssistantWorkflowChangeState::Rejected => AssistantWorkflowChangeStateDto::Rejected,
        AssistantWorkflowChangeState::Applying => AssistantWorkflowChangeStateDto::Applying,
        AssistantWorkflowChangeState::Applied => AssistantWorkflowChangeStateDto::Applied,
        AssistantWorkflowChangeState::ApplyFailed => AssistantWorkflowChangeStateDto::ApplyFailed,
        AssistantWorkflowChangeState::Expired => AssistantWorkflowChangeStateDto::Expired,
        AssistantWorkflowChangeState::Proposed => AssistantWorkflowChangeStateDto::Proposed,
        AssistantWorkflowChangeState::ReviewRejected => {
            AssistantWorkflowChangeStateDto::ReviewRejected
        }
    }
}

fn lineage(value: &AssistantWorkflowChangeLineage) -> AssistantWorkflowChangeLineageDto {
    match value {
        AssistantWorkflowChangeLineage::UserMessage { invocation_id, intent } => {
            AssistantWorkflowChangeLineageDto::UserMessage {
                invocation_id: invocation_id.as_uuid().to_string(),
                intent: intent.as_str().to_owned(),
            }
        }
        AssistantWorkflowChangeLineage::ReviewedRepair {
            activation_id,
            failed_workflow_run_id,
        } => AssistantWorkflowChangeLineageDto::ReviewedRepair {
            activation_id: activation_id.as_uuid().to_string(),
            failed_workflow_run_id: uuid::Uuid::from_bytes(*failed_workflow_run_id).to_string(),
        },
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(DIGITS[usize::from(byte >> 4)]));
        value.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    value
}
