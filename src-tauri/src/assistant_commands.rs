//! Tauri commands for the Project-scoped, Rust-owned assistant stream.

use crate::assistant_operations::OperationRegistration;
use crate::assistant_runtime::{
    AssistantEventSink, AssistantInvocation, AssistantRuntime, AssistantRuntimeOutcome,
    TrustedInvocationContext,
};
use crate::capability_discovery::CapabilityDiscovery;
use crate::dto::WorkflowHeadDto;
use crate::production_plan::operations::ProductionPlanOperations;
use crate::reviewed_change::ReviewedChangeOperations;
use crate::state::AppState;
use crate::workflow_patch_operation::WorkflowPatchService;
use crate::workspace_snapshot::WorkspaceSnapshotService;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{State, ipc::Channel};

const MAX_TEXT_CHARS: usize = 32 * 1024;
const MAX_ID_CHARS: usize = 160;
static NEXT_ASSISTANT_ID: AtomicU64 = AtomicU64::new(1);

/// Model-facing input for one Project-scoped assistant turn.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantSendInput {
    /// Trusted host scope, checked against the local Project store.
    pub project_id: String,
    /// Whether the caller's displayed Workflow head is present.
    pub workflow_present: bool,
    /// Revision of the displayed Workflow head, when present.
    pub workflow_revision: Option<u64>,
    /// Selected node pointers, never a canvas snapshot.
    #[serde(default)]
    pub selected_node_ids: Vec<String>,
    /// Selected Asset pointers, never media content.
    #[serde(default)]
    pub selected_asset_ids: Vec<String>,
    /// User-authored request text.
    pub text: String,
}

/// Human decision for the one durable pending Assistant approval.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantApprovalDecisionInput {
    pub project_id: String,
    pub candidate_digest: String,
    pub approved: bool,
}

mod pending;
pub use pending::{
    AssistantPendingApprovalDto, assistant_get_pending_approval,
    assistant_get_pending_approval_with_state,
};

/// Starts one assistant turn through the inherited-stdio sidecar.
#[tauri::command(rename_all = "snake_case")]
pub async fn assistant_send(
    input: AssistantSendInput,
    on_event: Channel<Value>,
    state: State<'_, AppState>,
) -> Result<Option<WorkflowHeadDto>, String> {
    assistant_send_with_state(input, on_event, &state).await
}

/// Starts one assistant turn against explicit app state.
pub async fn assistant_send_with_state(
    input: AssistantSendInput,
    on_event: Channel<Value>,
    state: &AppState,
) -> Result<Option<WorkflowHeadDto>, String> {
    let session_id = project_session_id(&input.project_id);
    let _active = ActiveAssistantSession::acquire(state, &session_id)?;
    if state.pending_approval.load(&session_id).map_err(|error| error.to_string())?.is_some() {
        return Err("ASSISTANT_APPROVAL_PENDING".to_owned());
    }
    validate_send(&input, state)?;
    let runtime = runtime_for_state(state)?;
    let (invocation, trusted) = build_invocation(&input, &state.config_root)?;
    let mut sink = ChannelAssistantSink { channel: on_event };
    let outcome = runtime
        .invoke_streamed(invocation, trusted, &mut sink)
        .await
        .map_err(|error| error.to_string())?;
    finish_outcome(outcome, state)
}

/// Resumes the exact SDK RunState after a human approval decision.
#[tauri::command(rename_all = "snake_case")]
pub async fn assistant_decide_approval(
    input: AssistantApprovalDecisionInput,
    on_event: Channel<Value>,
    state: State<'_, AppState>,
) -> Result<Option<WorkflowHeadDto>, String> {
    assistant_decide_approval_with_state(input, on_event, &state).await
}

pub async fn assistant_decide_approval_with_state(
    input: AssistantApprovalDecisionInput,
    on_event: Channel<Value>,
    state: &AppState,
) -> Result<Option<WorkflowHeadDto>, String> {
    validate_id("project_id", &input.project_id)?;
    let session_id = project_session_id(&input.project_id);
    let _active = ActiveAssistantSession::acquire(state, &session_id)?;
    let waiting = state
        .pending_approval
        .load(&session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "ASSISTANT_APPROVAL_NOT_FOUND".to_owned())?;
    if waiting.project_id() != input.project_id {
        return Err("ASSISTANT_APPROVAL_SCOPE_MISMATCH".to_owned());
    }
    let pending = pending::pending_approval_dto(&input.project_id, &session_id, &waiting, state)?;
    if pending.candidate_digest != input.candidate_digest {
        return Err("ASSISTANT_APPROVAL_STALE".to_owned());
    }
    let invocation = AssistantInvocation::new(
        next_assistant_id("approval")?,
        waiting.session_id(),
        waiting.session_path(),
        None,
    );
    let trusted = TrustedInvocationContext::new(&input.project_id, next_assistant_id("request")?);
    let runtime = runtime_for_state(state)?;
    let mut sink = ChannelAssistantSink { channel: on_event };
    let outcome = runtime
        .resume_streamed(invocation, trusted, waiting, input.approved, &mut sink)
        .await
        .map_err(|error| error.to_string())?;
    state.pending_approval.delete(&session_id).map_err(|error| error.to_string())?;
    finish_outcome(outcome, state)
}

struct ActiveAssistantSession {
    sessions: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    session_id: String,
}

impl ActiveAssistantSession {
    fn acquire(state: &AppState, session_id: &str) -> Result<Self, String> {
        let sessions = Arc::clone(&state.active_assistant_sessions);
        let mut active = sessions.lock().map_err(|_| "assistant session lock poisoned")?;
        if !active.insert(session_id.to_owned()) {
            return Err("ASSISTANT_SESSION_ACTIVE".to_owned());
        }
        drop(active);
        Ok(Self { sessions, session_id: session_id.to_owned() })
    }
}

impl Drop for ActiveAssistantSession {
    fn drop(&mut self) {
        if let Ok(mut active) = self.sessions.lock() {
            active.remove(&self.session_id);
        }
    }
}

fn runtime_for_state(state: &AppState) -> Result<AssistantRuntime, String> {
    let launcher = state
        .assistant_sidecar_command
        .clone()
        .env("OH_MY_DREAM_CONFIG_ROOT", state.config_root.as_os_str().to_owned());
    AssistantRuntime::new(launcher, operation_registrations(state)?)
        .map(|runtime| {
            runtime.with_review_handler(crate::assistant_review_bridge::review_handler(state))
        })
        .map_err(|error| error.to_string())
}

fn operation_registrations(state: &AppState) -> Result<Vec<OperationRegistration>, String> {
    let snapshot = Arc::new(WorkspaceSnapshotService::from_state(state))
        .operation_registration()
        .map_err(|error| error.to_string())?;
    let patch_service = Arc::new(WorkflowPatchService::from_state(state));
    let evaluate = Arc::clone(&patch_service)
        .evaluation_operation_registration()
        .map_err(|error| error.to_string())?;
    let discovery = Arc::new(CapabilityDiscovery::from_state(state))
        .operation_registrations()
        .map_err(|error| error.to_string())?;
    let plan = ProductionPlanOperations::new(Arc::clone(&state.production_plan))
        .registrations()
        .map_err(|error| error.to_string())?;
    let candidates =
        ReviewedChangeOperations::new(Arc::clone(&state.reviewed_change), patch_service)
            .registrations()
            .map_err(|error| error.to_string())?;
    let mut registrations = vec![snapshot, evaluate];
    registrations.extend(discovery);
    registrations.extend(plan);
    registrations.extend(candidates);
    Ok(registrations)
}

/// Returns the exact production Assistant operation IDs for architecture tests.
pub fn production_operation_ids(state: &AppState) -> Result<Vec<String>, String> {
    operation_registrations(state).map(|registrations| {
        registrations.into_iter().map(|registration| registration.id().to_owned()).collect()
    })
}

fn validate_send(input: &AssistantSendInput, state: &AppState) -> Result<(), String> {
    validate_id("project_id", &input.project_id)?;
    if input.text.trim().is_empty() || input.text.chars().count() > MAX_TEXT_CHARS {
        return Err("assistant text must be non-empty and within the size limit".to_owned());
    }
    validate_selection(&input.selected_node_ids)?;
    validate_selection(&input.selected_asset_ids)?;
    let head =
        state.workflow_authority.load_head(&input.project_id).map_err(|error| error.to_string())?;
    if !matches_head(head.as_ref(), input.workflow_present, input.workflow_revision) {
        return Err("ASSISTANT_WORKFLOW_REVISION_CONFLICT".to_owned());
    }
    state
        .store
        .lock()
        .map_err(|_| "asset store lock was poisoned".to_owned())?
        .get_project(&input.project_id)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn validate_id(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().count() > MAX_ID_CHARS || !value.is_ascii() {
        return Err(format!("{name} is invalid"));
    }
    Ok(())
}

fn validate_selection(values: &[String]) -> Result<(), String> {
    if values.len() > 16
        || values.iter().any(|value| value.is_empty() || value.len() > MAX_ID_CHARS)
    {
        return Err("assistant selection is outside the bounded limit".to_owned());
    }
    Ok(())
}

fn matches_head(
    head: Option<&crate::workflow_authority::WorkflowHead>,
    present: bool,
    revision: Option<u64>,
) -> bool {
    match head {
        None => !present && revision.is_none(),
        Some(head) => present && revision == Some(head.revision),
    }
}

struct AssistantIdentity {
    session_id: String,
    invocation_id: String,
    request_id: String,
    session_path: PathBuf,
}

fn build_invocation(
    input: &AssistantSendInput,
    config_root: &Path,
) -> Result<(AssistantInvocation, TrustedInvocationContext), String> {
    let identity = assistant_identity(config_root, &input.project_id)?;
    let invocation = AssistantInvocation::new(
        identity.invocation_id,
        identity.session_id,
        identity.session_path,
        Some(input.text.clone()),
    );
    let trusted = TrustedInvocationContext::new(&input.project_id, identity.request_id)
        .with_selection(input.selected_node_ids.clone(), input.selected_asset_ids.clone());
    Ok((invocation, trusted))
}

fn assistant_identity(config_root: &Path, project_id: &str) -> Result<AssistantIdentity, String> {
    let root = config_root.join("assistant_sessions");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    Ok(AssistantIdentity {
        session_id: format!("project:{project_id}"),
        invocation_id: next_assistant_id("invocation")?,
        request_id: next_assistant_id("request")?,
        session_path: root
            .join(format!("project-{:016x}.sqlite3", stable_project_hash(project_id))),
    })
}

fn next_assistant_id(kind: &str) -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("assistant clock is invalid: {error}"))?
        .as_nanos();
    let sequence = NEXT_ASSISTANT_ID.fetch_add(1, Ordering::Relaxed);
    Ok(format!("assistant-{kind}-{timestamp:032x}-{sequence:016x}"))
}

fn stable_project_hash(project_id: &str) -> u64 {
    let mut hash = 14_695_981_039_346_656_037_u64;
    for byte in project_id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211_u64);
    }
    hash
}

fn project_session_id(project_id: &str) -> String {
    format!("project:{project_id}")
}

fn finish_outcome(
    outcome: AssistantRuntimeOutcome,
    state: &AppState,
) -> Result<Option<WorkflowHeadDto>, String> {
    match outcome {
        AssistantRuntimeOutcome::Completed(completed) => completed
            .operation_calls()
            .iter()
            .rev()
            .find(|call| {
                matches!(
                    call.operation_id(),
                    "workflow_apply_patch" | "workflow_apply_reviewed_candidate"
                )
            })
            .map_or(Ok(None), |call| workflow_head_from_patch_output(call.output_json())),
        AssistantRuntimeOutcome::WaitingApproval(waiting) => {
            state.pending_approval.save(&waiting).map_err(|error| error.to_string())?;
            Err("ASSISTANT_APPROVAL_DEFERRED".to_owned())
        }
    }
}

fn workflow_head_from_patch_output(output_json: &str) -> Result<Option<WorkflowHeadDto>, String> {
    let output: Value = serde_json::from_str(output_json)
        .map_err(|error| format!("assistant patch output is invalid JSON: {error}"))?;
    let Some(head) = output.get("workflow_head") else {
        return Err("assistant patch output omitted workflow_head".to_owned());
    };
    if head.is_null() {
        return Ok(None);
    }
    serde_json::from_value(head.clone())
        .map(Some)
        .map_err(|error| format!("assistant patch workflow_head is invalid: {error}"))
}

struct ChannelAssistantSink {
    channel: Channel<Value>,
}

impl AssistantEventSink for ChannelAssistantSink {
    fn emit(
        &mut self,
        event: Value,
    ) -> Result<(), crate::assistant_runtime::AssistantRuntimeError> {
        self.channel.send(event).map_err(|error| {
            crate::assistant_runtime::AssistantRuntimeError::EventSink {
                message: error.to_string(),
            }
        })
    }
}

#[cfg(test)]
mod tests;
