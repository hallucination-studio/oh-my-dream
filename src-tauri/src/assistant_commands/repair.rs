use super::{AssistantApprovalDecisionInput, finish_outcome, next_assistant_id};
use crate::assistant_repair::{ApprovedWorkflowAction, AssistantRepairService};
use crate::assistant_runtime::{
    AssistantEventSink, AssistantInvocation, AssistantRuntime, AssistantRuntimeOutcome,
    TrustedInvocationContext,
};
use crate::dto::{NodeProgressEventDto, WorkflowHeadDto};
use crate::state::AppState;
use crate::workflow_runs::{WorkflowRunEvent, WorkflowRunEventError, WorkflowRunEventSink};
use serde_json::json;
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
    let head = finish_outcome(outcome, &input.project_id, state)?;
    let Some(head) = head else { return Ok(None) };
    if !input.approved {
        return Ok(Some(head));
    }
    let action =
        ApprovedWorkflowAction::new(&input.project_id, &input.approval_scope_id, head.revision);
    let run = AssistantRepairService::from_state(state)
        .execute_with_events(&action, &mut AssistantRunEvents { sink })
        .map_err(|error| error.to_string())?;
    emit_terminal(sink, &run)?;
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
    finish_outcome(repair_outcome, &input.project_id, state)
        .map(|repair_head| repair_head.or(Some(head)))
}

struct AssistantRunEvents<'a> {
    sink: &'a mut dyn AssistantEventSink,
}

impl WorkflowRunEventSink for AssistantRunEvents<'_> {
    fn send(&mut self, event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError> {
        let value = match event {
            WorkflowRunEvent::Started { run_id, project_id } => json!({
                "type": "assistant.workflow_run.started",
                "run_id": run_id.as_str(),
                "project_id": project_id,
            }),
            WorkflowRunEvent::Progress { run_id, node } => {
                let node = NodeProgressEventDto::from(node);
                json!({
                    "type": "assistant.workflow_run.progress",
                    "run_id": run_id.as_str(),
                    "node_id": node.node_id,
                    "node_state": node.state,
                    "progress": node.progress,
                    "cost": node.cost,
                })
            }
        };
        self.sink.emit(value).map_err(|error| Box::new(error) as WorkflowRunEventError)
    }
}

fn emit_terminal(
    sink: &mut dyn AssistantEventSink,
    run: &crate::assistant_repair::AssistantRepairRun,
) -> Result<(), String> {
    let value = match &run.outcome {
        crate::workflow_run_dto::WorkflowRunResultDto::Succeeded { .. } => json!({
            "type": "assistant.workflow_run.succeeded",
            "run_id": run.run_id,
        }),
        crate::workflow_run_dto::WorkflowRunResultDto::Cancelled { .. } => json!({
            "type": "assistant.workflow_run.cancelled",
            "run_id": run.run_id,
        }),
        crate::workflow_run_dto::WorkflowRunResultDto::Failed { reason, .. } => json!({
            "type": "assistant.workflow_run.failed",
            "run_id": run.run_id,
            "reason": reason,
        }),
    };
    sink.emit(value).map_err(|error| error.to_string())
}
