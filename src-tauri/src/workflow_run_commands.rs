use crate::command_error::command_error;
use crate::dto::{NodeProgressEventDto, RunWorkflowResultDto};
use crate::state::AppState;
use crate::workflow_run_dto::{
    CancelWorkflowRunResultDto, WorkflowRunEventDto, WorkflowRunResultDto,
};
use crate::workflow_runs::{
    RunId, WorkflowRunEvent, WorkflowRunEventError, WorkflowRunEventSink, WorkflowRunOutcome,
    WorkflowRuns,
};
use engine::{NodeProgressEvent, Workflow};
use nodes::SharedAssetStore;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State, ipc::Channel};
use tracing::{error, info};

/// Runs the legacy workflow command on the blocking worker pool.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_workflow(
    workflow_json: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RunWorkflowResultDto, String> {
    let context = WorkflowRunCommandContext::from_state(&state);
    tauri::async_runtime::spawn_blocking(move || {
        run_legacy_with_context(workflow_json, context, &mut LegacyAppSink { app })
    })
    .await
    .map_err(|source| command_error("join workflow worker", source))?
}

/// Starts one run-scoped workflow command on the blocking worker pool.
#[tauri::command(rename_all = "snake_case")]
pub async fn start_workflow_run(
    run_id: String,
    workflow_json: String,
    on_event: Channel<WorkflowRunEventDto>,
    state: State<'_, AppState>,
) -> Result<WorkflowRunResultDto, String> {
    let context = WorkflowRunCommandContext::from_state(&state);
    tauri::async_runtime::spawn_blocking(move || {
        run_scoped_with_context(
            run_id,
            workflow_json,
            context,
            &mut ChannelEventSink { channel: on_event },
        )
    })
    .await
    .map_err(|source| command_error("join workflow worker", source))?
}

/// Requests cancellation of one active scoped run.
#[tauri::command(rename_all = "snake_case")]
pub fn cancel_workflow_run(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<CancelWorkflowRunResultDto, String> {
    cancel_workflow_run_with_state(run_id, &state)
}

/// Runs a scoped workflow against an explicit app state.
pub fn start_workflow_run_with_state(
    run_id: String,
    workflow_json: String,
    on_event: Channel<WorkflowRunEventDto>,
    state: &AppState,
) -> Result<WorkflowRunResultDto, String> {
    run_scoped_with_context(
        run_id,
        workflow_json,
        WorkflowRunCommandContext::from_state(state),
        &mut ChannelEventSink { channel: on_event },
    )
}

/// Requests cancellation against an explicit app state.
pub fn cancel_workflow_run_with_state(
    run_id: String,
    state: &AppState,
) -> Result<CancelWorkflowRunResultDto, String> {
    let parsed =
        RunId::parse(&run_id).map_err(|source| command_error("validate run_id", source))?;
    let request = state
        .workflow_runs
        .cancel(&parsed)
        .map_err(|source| command_error("cancel workflow run", source))?;
    Ok(CancelWorkflowRunResultDto::from_request(&run_id, request))
}

/// Runs a legacy workflow against an explicit app state.
pub fn run_workflow_with_state(
    workflow_json: String,
    state: &AppState,
) -> Result<RunWorkflowResultDto, String> {
    run_legacy_with_context(
        workflow_json,
        WorkflowRunCommandContext::from_state(state),
        &mut NoopEventSink,
    )
}

/// Runs a legacy workflow with a testable progress observer.
pub fn run_workflow_with_state_and_observer(
    workflow_json: String,
    state: &AppState,
    observer: &mut (impl FnMut(&NodeProgressEvent) + Send),
) -> Result<RunWorkflowResultDto, String> {
    run_legacy_with_context(
        workflow_json,
        WorkflowRunCommandContext::from_state(state),
        &mut ObserverEventSink { observer },
    )
}

#[derive(Clone)]
struct WorkflowRunCommandContext {
    store: SharedAssetStore,
    runs: Arc<WorkflowRuns>,
}

impl WorkflowRunCommandContext {
    fn from_state(state: &AppState) -> Self {
        Self { store: Arc::clone(&state.store), runs: Arc::clone(&state.workflow_runs) }
    }
}

fn run_scoped_with_context(
    run_id: String,
    workflow_json: String,
    context: WorkflowRunCommandContext,
    sink: &mut dyn WorkflowRunEventSink,
) -> Result<WorkflowRunResultDto, String> {
    info!(run_id, "start_workflow_run command received");
    let parsed =
        RunId::parse(&run_id).map_err(|source| command_error("validate run_id", source))?;
    let workflow = parse_workflow(&workflow_json, &context)?;
    let outcome = context
        .runs
        .run(parsed, workflow, sink)
        .map_err(|source| command_error("start workflow run", source))?;
    Ok(WorkflowRunResultDto::from_outcome(&run_id, outcome))
}

fn run_legacy_with_context(
    workflow_json: String,
    context: WorkflowRunCommandContext,
    sink: &mut dyn WorkflowRunEventSink,
) -> Result<RunWorkflowResultDto, String> {
    info!("run_workflow command received");
    let workflow = parse_workflow(&workflow_json, &context)?;
    let outcome = context
        .runs
        .run_legacy(workflow, sink)
        .map_err(|source| command_error("run workflow", source))?;
    match outcome {
        WorkflowRunOutcome::Succeeded(outputs) => {
            info!(node_count = outputs.len(), "run_workflow command completed");
            Ok(RunWorkflowResultDto::from_outputs(&outputs))
        }
        WorkflowRunOutcome::Cancelled => {
            Err(command_error("run workflow", "workflow execution was cancelled"))
        }
        WorkflowRunOutcome::Failed(source) => Err(command_error("run workflow", source)),
    }
}

fn parse_workflow(
    workflow_json: &str,
    context: &WorkflowRunCommandContext,
) -> Result<Workflow, String> {
    let workflow = serde_json::from_str::<Workflow>(workflow_json)
        .map_err(|source| command_error("deserialize workflow", source))?;
    if workflow.project_id.is_empty() {
        return Err(command_error("validate workflow", "workflow project_id is empty"));
    }
    context
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .get_project(&workflow.project_id)
        .map_err(|source| command_error("validate project", source))?;
    Ok(workflow)
}

struct ChannelEventSink {
    channel: Channel<WorkflowRunEventDto>,
}

impl WorkflowRunEventSink for ChannelEventSink {
    fn send(&mut self, event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError> {
        self.channel.send(WorkflowRunEventDto::from(event)).map_err(Into::into)
    }
}

struct LegacyAppSink {
    app: AppHandle,
}

impl WorkflowRunEventSink for LegacyAppSink {
    fn send(&mut self, event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError> {
        if let WorkflowRunEvent::Progress { node, .. } = event
            && let Err(source) = self.app.emit("node_progress", NodeProgressEventDto::from(node))
        {
            error!(error = %source, "failed to emit node_progress event");
        }
        Ok(())
    }
}

struct ObserverEventSink<'a, F> {
    observer: &'a mut F,
}

impl<F> WorkflowRunEventSink for ObserverEventSink<'_, F>
where
    F: FnMut(&NodeProgressEvent) + Send,
{
    fn send(&mut self, event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError> {
        if let WorkflowRunEvent::Progress { node, .. } = event {
            (self.observer)(&node);
        }
        Ok(())
    }
}

struct NoopEventSink;

impl WorkflowRunEventSink for NoopEventSink {
    fn send(&mut self, _event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError> {
        Ok(())
    }
}
