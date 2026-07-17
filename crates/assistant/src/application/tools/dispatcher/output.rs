use serde_json::Value;

use super::super::dto::{ChangeOutput, ChangeStateDto, PlanDto, PlanItemDto, PlanItemStateDto};
use crate::{
    domain::{
        AssistantPlanItemState, AssistantProductionPlanAggregate, AssistantWorkflowChangeAggregate,
        AssistantWorkflowChangeState,
    },
    interfaces::AssistantApplicationError,
};

pub(super) fn plan_value(
    plan: &AssistantProductionPlanAggregate,
) -> Result<Value, AssistantApplicationError> {
    serialize(&plan_dto(plan))
}

pub(super) fn plan_dto(plan: &AssistantProductionPlanAggregate) -> PlanDto {
    let items = plan
        .items()
        .iter()
        .map(|item| {
            let (state, note) = match item.state() {
                AssistantPlanItemState::Pending => (PlanItemStateDto::Pending, None),
                AssistantPlanItemState::InProgress => (PlanItemStateDto::InProgress, None),
                AssistantPlanItemState::Blocked { reason } => {
                    (PlanItemStateDto::Blocked, Some(reason.as_str().to_owned()))
                }
                AssistantPlanItemState::Completed { acceptance_note } => {
                    (PlanItemStateDto::Completed, Some(acceptance_note.as_str().to_owned()))
                }
            };
            PlanItemDto {
                id: item.id().as_str().to_owned(),
                goal: item.goal().as_str().to_owned(),
                state,
                note,
            }
        })
        .collect();
    PlanDto {
        id: plan.id().as_uuid().to_string(),
        revision: plan.revision().get(),
        title: plan.title().as_str().to_owned(),
        items,
    }
}

pub(super) fn change_value(
    change: &AssistantWorkflowChangeAggregate,
) -> Result<Value, AssistantApplicationError> {
    let ordered_mutations = change
        .ordered_mutations()
        .iter()
        .map(|value| {
            serde_json::from_slice(value.canonical_bytes())
                .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)
        })
        .collect::<Result<Vec<Value>, _>>()?;
    serialize(&ChangeOutput {
        change_id: change.id().as_uuid().to_string(),
        base_workflow_revision: change.base_workflow_revision().get(),
        ordered_mutations,
        mutation_digest_hex: hex(change.mutation_digest().as_bytes()),
        resulting_workflow_fingerprint_hex: hex(change.resulting_workflow_fingerprint().as_bytes()),
        approval_scope_id: change.approval_scope_id().as_uuid().to_string(),
        expires_at_epoch_ms: change.expires_at().epoch_ms(),
        state: change_state(change.state()),
    })
}

fn change_state(value: AssistantWorkflowChangeState) -> ChangeStateDto {
    match value {
        AssistantWorkflowChangeState::Proposed => ChangeStateDto::Proposed,
        AssistantWorkflowChangeState::ReviewRejected => ChangeStateDto::ReviewRejected,
        AssistantWorkflowChangeState::AwaitingApproval => ChangeStateDto::AwaitingApproval,
        AssistantWorkflowChangeState::Rejected => ChangeStateDto::Rejected,
        AssistantWorkflowChangeState::Applying => ChangeStateDto::Applying,
        AssistantWorkflowChangeState::Applied => ChangeStateDto::Applied,
        AssistantWorkflowChangeState::ApplyFailed => ChangeStateDto::ApplyFailed,
        AssistantWorkflowChangeState::Expired => ChangeStateDto::Expired,
    }
}

pub(super) fn serialize(value: &impl serde::Serialize) -> Result<Value, AssistantApplicationError> {
    serde_json::to_value(value).map_err(|_| AssistantApplicationError::ProtocolViolation)
}

fn hex(bytes: [u8; 32]) -> String {
    use std::fmt::Write;

    bytes.iter().fold(String::with_capacity(64), |mut output, byte| {
        let _ = write!(output, "{byte:02x}");
        output
    })
}
