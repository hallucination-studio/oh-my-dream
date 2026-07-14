use assets::{AssetKind, NewAsset};
use engine::{CapabilityRef, WorkflowPatchOperation};
use oh_my_dream_tauri::assistant_operations::{OperationDispatchError, RequestContext};
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_patch_operation::{WorkflowApplyPatchInput, WorkflowPatchService};
use oh_my_dream_tauri::workspace_snapshot::{
    MAX_WORKSPACE_ASSET_SUMMARIES, MAX_WORKSPACE_SELECTIONS, WorkspaceSnapshotInput,
    WorkspaceSnapshotService,
};
use serde_json::{Map, json};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

fn context(project_id: &str, request_id: &str) -> RequestContext {
    RequestContext::new(project_id, format!("session-{project_id}"), request_id, 1, None)
}

fn context_with_selection(
    project_id: &str,
    request_id: &str,
    selected_node_ids: Vec<String>,
    selected_asset_ids: Vec<String>,
) -> RequestContext {
    context(project_id, request_id).with_workspace_selection(selected_node_ids, selected_asset_ids)
}

fn state_with_projects() -> (tempfile::TempDir, AppState) {
    let root = tempdir().expect("asset root");
    let state = AppState::from_asset_root(root.path()).expect("app state");
    let store = state.store.lock().expect("store lock");
    store.create_project_with_id("project-a", "Project A").expect("project A");
    store.create_project_with_id("project-b", "Project B").expect("project B");
    drop(store);
    (root, state)
}

fn source_image() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons/32x32.png")
}

fn insert_asset(state: &AppState, project_id: &str, prompt: &str) -> String {
    state
        .store
        .lock()
        .expect("store lock")
        .insert(NewAsset {
            kind: AssetKind::Image,
            file_path: source_image().to_string_lossy().into_owned(),
            workflow_snapshot: json!({}),
            prompt: Some(prompt.to_owned()),
            project_id: Some(project_id.to_owned()),
            project_name: None,
            source_node_id: None,
            source_node_type: None,
            model: Some("fixture".to_owned()),
            seed: None,
            cost: None,
            tags: Vec::new(),
        })
        .expect("insert asset")
        .id
}

fn add_node(state: &AppState, project_id: &str, capability: &str) {
    WorkflowPatchService::from_state(state)
        .apply(
            &context(project_id, &format!("patch-{project_id}")),
            WorkflowApplyPatchInput {
                expected_revision: None,
                operations: vec![WorkflowPatchOperation::AddNode {
                    alias: "selected".to_owned(),
                    capability: CapabilityRef::new(capability, "1.0"),
                    params: Map::new(),
                    position: None,
                }],
            },
        )
        .expect("add node");
}

#[test]
fn workspace_snapshot_read_only_call_preserves_absent_workflow() {
    let (_root, state) = state_with_projects();
    let service = WorkspaceSnapshotService::from_state(&state);

    let snapshot = service
        .get_snapshot(&context("project-a", "snapshot-empty"), WorkspaceSnapshotInput {})
        .expect("read empty workspace");

    assert_eq!(snapshot.scope.project_id, "project-a");
    assert_eq!(snapshot.scope.session_id, "session-project-a");
    assert_eq!(snapshot.scope.request_id, "snapshot-empty");
    assert!(snapshot.workflow_head.is_none());
    assert!(snapshot.readiness_blockers.is_empty());
    assert!(state.workflow_authority.load_head("project-a").expect("head").is_none());
}

#[test]
fn workspace_snapshot_uses_context_scope_and_rejects_foreign_selection() {
    let (_root, state) = state_with_projects();
    add_node(&state, "project-a", "ImageToVideo");
    add_node(&state, "project-b", "TextPrompt");
    let local_asset = insert_asset(&state, "project-a", "local");
    let foreign_asset = insert_asset(&state, "project-b", "foreign");
    let service = WorkspaceSnapshotService::from_state(&state);

    let snapshot = service
        .get_snapshot(
            &context_with_selection(
                "project-a",
                "snapshot-local",
                vec!["n1".to_owned()],
                vec![local_asset.clone()],
            ),
            WorkspaceSnapshotInput {},
        )
        .expect("read project A");

    assert_eq!(snapshot.project.id, "project-a");
    assert_eq!(snapshot.workflow_head.as_ref().expect("head").project_id, "project-a");
    assert_eq!(snapshot.selected_assets[0].id, local_asset);
    assert_eq!(snapshot.selected_nodes[0].id, "n1");
    assert_eq!(snapshot.selected_nodes[0].capability.id, "ImageToVideo");
    assert!(snapshot.assets.iter().all(|asset| asset.prompt.as_deref() != Some("foreign")));
    assert!(snapshot.readiness_blockers.iter().any(|blocker| {
        blocker.code == "REQUIRED_INPUT_MISSING" && blocker.pointer.contains("image")
    }));

    let error = service
        .get_snapshot(
            &context_with_selection(
                "project-a",
                "snapshot-foreign",
                Vec::new(),
                vec![foreign_asset],
            ),
            WorkspaceSnapshotInput {},
        )
        .expect_err("foreign selected asset must be rejected");
    assert_eq!(error.code, "SELECTED_ASSET_OUT_OF_SCOPE");
    assert_eq!(error.pointer, "/selected_asset_ids/0");

    let switched = service
        .get_snapshot(
            &context_with_selection(
                "project-b",
                "snapshot-switched",
                vec!["n1".to_owned()],
                Vec::new(),
            ),
            WorkspaceSnapshotInput {},
        )
        .expect("read project B after switching");
    assert_eq!(switched.project.id, "project-b");
    assert_eq!(switched.selected_nodes[0].capability.id, "TextPrompt");
    assert!(switched.readiness_blockers.is_empty());
    assert!(switched.assets.iter().all(|asset| asset.project_id.as_deref() == Some("project-b")));
    assert!(switched.assets.iter().all(|asset| asset.prompt.as_deref() != Some("local")));
}

#[test]
fn workspace_snapshot_bounds_asset_summaries_and_exposes_a_strict_operation() {
    let (_root, state) = state_with_projects();
    for index in 0..=MAX_WORKSPACE_ASSET_SUMMARIES {
        insert_asset(&state, "project-a", &format!("asset-{index}"));
    }
    insert_asset(&state, "project-b", "foreign");
    let service = WorkspaceSnapshotService::from_state(&state);
    let snapshot = service
        .get_snapshot(&context("project-a", "snapshot-bounded"), WorkspaceSnapshotInput {})
        .expect("bounded snapshot");

    assert_eq!(snapshot.assets.len(), MAX_WORKSPACE_ASSET_SUMMARIES);
    assert!(snapshot.assets.iter().all(|asset| asset.project_id.as_deref() == Some("project-a")));

    let error = service
        .get_snapshot(
            &context_with_selection(
                "project-a",
                "snapshot-over-limit",
                Vec::new(),
                vec!["asset".to_owned(); MAX_WORKSPACE_SELECTIONS + 1],
            ),
            WorkspaceSnapshotInput {},
        )
        .expect_err("direct service calls enforce the selection limit");
    assert_eq!(error.code, "SNAPSHOT_SELECTION_LIMIT");

    let registration = Arc::new(service).operation_registration().expect("snapshot operation");
    assert_eq!(registration.id(), "workspace_get_snapshot");
    assert!(registration.sdk_strict_json_schema());
    assert_eq!(registration.input_schema()["additionalProperties"], json!(false));
    assert!(registration.input_schema()["properties"].get("project_id").is_none());
    assert!(registration.input_schema().get("required").is_none());
    assert!(registration.input_schema().get("properties").is_none());
}

#[tokio::test]
async fn workspace_snapshot_registration_dispatches_with_trusted_scope() {
    let (_root, state) = state_with_projects();
    add_node(&state, "project-a", "ImageToVideo");
    let service = Arc::new(WorkspaceSnapshotService::from_state(&state));
    let registration = service.operation_registration().expect("snapshot operation");
    let context =
        context_with_selection("project-a", "snapshot-dispatch", vec!["n1".to_owned()], Vec::new());

    let output = registration
        .dispatch(&context, json!({}))
        .await
        .expect("empty model input should dispatch");
    assert_eq!(output["scope"]["project_id"], json!("project-a"));
    assert_eq!(output["scope"]["session_id"], json!("session-project-a"));
    assert_eq!(output["selected_nodes"][0]["id"], json!("n1"));
    assert_eq!(output["selected_nodes"][0]["capability"]["id"], json!("ImageToVideo"));

    let error = registration
        .dispatch(&context, json!({ "project_id": "project-b" }))
        .await
        .expect_err("model input must not select a Project");
    assert!(matches!(error, OperationDispatchError::SchemaValidation { operation_id, .. }
        if operation_id == "workspace_get_snapshot"));
}
