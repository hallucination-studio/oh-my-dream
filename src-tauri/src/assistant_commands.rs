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
use serde::Deserialize;
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
#[derive(Debug, Clone, Deserialize)]
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
    validate_send(&input, state)?;
    let runtime = runtime_for_state(state)?;
    let (invocation, trusted) = build_invocation(&input, &state.config_root)?;
    let mut sink = ChannelAssistantSink { channel: on_event };
    let outcome = runtime
        .invoke_streamed(invocation, trusted, &mut sink)
        .await
        .map_err(|error| error.to_string())?;
    finish_outcome(outcome)
}

fn runtime_for_state(state: &AppState) -> Result<AssistantRuntime, String> {
    let launcher = state
        .assistant_sidecar_command
        .clone()
        .env("OH_MY_DREAM_CONFIG_ROOT", state.config_root.as_os_str().to_owned());
    AssistantRuntime::new(launcher, operation_registrations(state)?)
        .map_err(|error| error.to_string())
}

fn operation_registrations(state: &AppState) -> Result<Vec<OperationRegistration>, String> {
    let snapshot = Arc::new(WorkspaceSnapshotService::from_state(state))
        .operation_registration()
        .map_err(|error| error.to_string())?;
    let patch_service = Arc::new(WorkflowPatchService::from_state(state));
    let evaluate =
        patch_service.evaluation_operation_registration().map_err(|error| error.to_string())?;
    let discovery = Arc::new(CapabilityDiscovery::from_state(state))
        .operation_registrations()
        .map_err(|error| error.to_string())?;
    let plan = ProductionPlanOperations::new(Arc::clone(&state.production_plan))
        .registrations()
        .map_err(|error| error.to_string())?;
    let candidates = ReviewedChangeOperations::new(Arc::clone(&state.reviewed_change))
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

fn finish_outcome(outcome: AssistantRuntimeOutcome) -> Result<Option<WorkflowHeadDto>, String> {
    match outcome {
        AssistantRuntimeOutcome::Completed(completed) => completed
            .operation_calls()
            .iter()
            .rev()
            .find(|call| call.operation_id() == "workflow_apply_patch")
            .map_or(Ok(None), |call| workflow_head_from_patch_output(call.output_json())),
        AssistantRuntimeOutcome::WaitingApproval(_) => {
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
mod tests {
    use super::{
        AssistantSendInput, ChannelAssistantSink, assistant_identity, build_invocation,
        operation_registrations, workflow_head_from_patch_output,
    };
    use crate::assistant_runtime::AssistantEventSink;
    use crate::state::AppState;
    use serde_json::{Value, json};
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tauri::ipc::{Channel, InvokeResponseBody};
    use tempfile::tempdir;

    #[test]
    fn production_runtime_exposes_plan_memory_without_scheduler_tools() {
        let root = tempdir().expect("create app root");
        let state = AppState::from_roots(root.path().join("assets"), root.path().join("config"))
            .expect("build app state");

        let ids = operation_registrations(&state)
            .expect("build production registrations")
            .into_iter()
            .map(|registration| registration.id().to_owned())
            .collect::<Vec<_>>();

        for expected in [
            "workflow_evaluate_patch",
            "workflow_prepare_patch",
            "workflow_candidate_get",
            "production_plan_get",
            "production_plan_create",
            "production_plan_replace",
            "production_plan_update_item",
        ] {
            assert!(ids.iter().any(|id| id == expected), "missing {expected}");
        }
        assert!(ids.iter().all(|id| !id.contains("next") && !id.contains("claim")));
        assert!(!ids.iter().any(|id| id == "workflow_apply_patch"));
    }

    #[test]
    fn channel_sink_forwards_native_response_value_unchanged() {
        let events = Arc::new(Mutex::new(Vec::<Value>::new()));
        let channel = Channel::new({
            let events = Arc::clone(&events);
            move |body| {
                let InvokeResponseBody::Json(encoded) = body else {
                    panic!("assistant event should use JSON IPC");
                };
                let event = serde_json::from_str(&encoded).expect("decode assistant event");
                events.lock().expect("event lock").push(event);
                Ok(())
            }
        });
        let mut sink = ChannelAssistantSink { channel };
        let event = json!({
            "type": "response.output_text.delta",
            "delta": "native",
            "sequence_number": 4,
        });

        sink.emit(event.clone()).expect("channel should accept event");

        assert_eq!(*events.lock().expect("event lock"), vec![event]);
    }

    #[test]
    fn patch_output_returns_the_canonical_workflow_head() {
        let output = json!({
            "workflow_head": {
                "project_id": "project-1",
                "revision": 3,
                "workflow": {
                    "version": "1.0",
                    "project_id": "project-1",
                    "nodes": []
                }
            }
        });

        let head = workflow_head_from_patch_output(&output.to_string())
            .expect("patch output should decode")
            .expect("patch output should contain a head");

        assert_eq!(head.project_id, "project-1");
        assert_eq!(head.revision, 3);
        assert_eq!(head.workflow["nodes"], json!([]));
    }

    #[test]
    fn patch_output_keeps_an_absent_workflow_head_absent() {
        let output = json!({ "workflow_head": null });

        assert_eq!(workflow_head_from_patch_output(&output.to_string()), Ok(None));
    }

    #[test]
    fn project_session_is_stable_while_turn_ids_are_rust_owned() {
        let root = tempdir().expect("create assistant config root");
        let first = assistant_identity(root.path(), "project-1").expect("first identity");
        let second = assistant_identity(root.path(), "project-1").expect("second identity");

        assert_eq!(first.session_id, "project:project-1");
        assert_eq!(first.session_id, second.session_id);
        assert_eq!(first.session_path, second.session_path);
        assert_ne!(first.invocation_id, second.invocation_id);
        assert_ne!(first.request_id, second.request_id);
        assert!(first.session_path.starts_with(root.path()));
        assert!(!first.session_path.starts_with(Path::new("project-1")));
    }

    #[test]
    fn invocation_keeps_user_text_and_selection_in_trusted_scope() {
        let root = tempdir().expect("create assistant config root");
        let input = AssistantSendInput {
            project_id: "project-1".to_owned(),
            workflow_present: true,
            workflow_revision: Some(4),
            selected_node_ids: vec!["node-1".to_owned()],
            selected_asset_ids: vec!["asset-1".to_owned()],
            text: "  preserve this exact text  ".to_owned(),
        };

        let (invocation, trusted) = build_invocation(&input, root.path()).expect("build turn");

        assert_eq!(invocation.input(), Some("  preserve this exact text  "));
        assert_eq!(trusted.project_id(), "project-1");
        assert_eq!(trusted.selected_node_ids(), ["node-1"]);
        assert_eq!(trusted.selected_asset_ids(), ["asset-1"]);
        assert!(trusted.request_id().starts_with("assistant-request-"));
    }
}
