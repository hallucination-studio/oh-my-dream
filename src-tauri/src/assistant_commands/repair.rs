use super::{AssistantApprovalDecisionInput, finish_outcome, next_assistant_id};
use crate::assistant_repair::{ApprovedWorkflowAction, AssistantRepairService};
use crate::assistant_runtime::{
    AssistantEventSink, AssistantInvocation, AssistantRuntime, AssistantRuntimeOutcome,
    TrustedInvocationContext,
};
use crate::dto::WorkflowHeadDto;
use crate::state::AppState;
use std::path::Path;

pub(super) async fn finish_approval_outcome(
    outcome: AssistantRuntimeOutcome,
    input: &AssistantApprovalDecisionInput,
    session_id: &str,
    session_path: &Path,
    runtime: &AssistantRuntime,
    sink: &mut dyn AssistantEventSink,
    state: &AppState,
) -> Result<Option<WorkflowHeadDto>, String> {
    let head = finish_outcome(outcome, state)?;
    let Some(head) = head else { return Ok(None) };
    if !input.approved {
        return Ok(Some(head));
    }
    let action =
        ApprovedWorkflowAction::new(&input.project_id, &input.approval_scope_id, head.revision);
    let run = AssistantRepairService::from_state(state)
        .execute(&action)
        .map_err(|error| error.to_string())?;
    let Some(activation) = run.activation else { return Ok(Some(head)) };
    let activation_input = serde_json::to_string(&activation).map_err(|error| error.to_string())?;
    let invocation = AssistantInvocation::new(
        next_assistant_id("repair")?,
        session_id,
        session_path,
        Some(activation_input),
    );
    let trusted = TrustedInvocationContext::new(&input.project_id, next_assistant_id("request")?);
    let repair_outcome = runtime
        .invoke_streamed(invocation, trusted, sink)
        .await
        .map_err(|error| error.to_string())?;
    finish_outcome(repair_outcome, state).map(|repair_head| repair_head.or(Some(head)))
}
