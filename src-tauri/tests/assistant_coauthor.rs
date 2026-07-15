use std::path::{Path, PathBuf};
use std::sync::Arc;

use engine::Workflow;
use oh_my_dream_tauri::assistant_operations::OperationRegistration;
use oh_my_dream_tauri::assistant_runtime::{
    AssistantInvocation, AssistantRuntime, AssistantRuntimeOutcome, AssistantSidecarCommand,
    TrustedInvocationContext,
};
use oh_my_dream_tauri::capability_discovery::CapabilityDiscovery;
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_patch_operation::WorkflowPatchService;
use oh_my_dream_tauri::workspace_snapshot::WorkspaceSnapshotService;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

#[tokio::test]
async fn coauthor_agent_creates_one_connected_twelve_second_workflow() {
    let (root, state) = state_with_project();
    let runtime = AssistantRuntime::new(fixture_command(), operation_registrations(&state))
        .expect("coauthor runtime should accept the four M3 operations");
    let session_path = root.path().join("assistant-session.sqlite3");

    let outcome = runtime
        .invoke(
            AssistantInvocation::new(
                "invoke-coauthor",
                "project:project",
                &session_path,
                Some(
                    "Create a 12-second, three-shot video: sunrise over a city, a cyclist crossing a bridge, and coffee steaming by a window."
                        .to_owned(),
                ),
            ),
            TrustedInvocationContext::new("project", "request-coauthor"),
        )
        .await
        .expect("coauthor invocation should complete");

    let AssistantRuntimeOutcome::Completed(completed) = outcome else {
        panic!("coauthor invocation should not pause for approval");
    };
    assert_eq!(completed.final_output(), &Value::String("Workflow created.".to_owned()));
    assert_eq!(
        completed.operation_calls().iter().map(|call| call.operation_id()).collect::<Vec<_>>(),
        vec![
            "workspace_get_snapshot",
            "capability_search",
            "capability_search",
            "capability_describe",
            "capability_describe",
            "workflow_apply_patch",
        ]
    );

    let patch_call =
        completed.operation_calls().last().expect("the last operation should be the patch");
    let patch_output: Value =
        serde_json::from_str(patch_call.output_json()).expect("patch output should be JSON");
    assert_eq!(patch_output["changed"], true);
    assert_eq!(patch_output["deduplicated"], false);
    assert_eq!(patch_output["readiness_blockers"], serde_json::json!([]));
    assert_eq!(patch_output["undo_id"], "workflow:project:1");
    assert_eq!(patch_output["workflow_head"]["revision"], 1);

    let workflow: Workflow =
        serde_json::from_value(patch_output["workflow_head"]["workflow"].clone())
            .expect("patch output should contain a canonical Workflow");
    assert_eq!(workflow.nodes.len(), 10);
    assert_eq!(nodes_with_selector(&workflow, "Text", "literal").count(), 3);
    assert_eq!(nodes_with_selector(&workflow, "Image", "text").count(), 3);
    assert_eq!(nodes_with_selector(&workflow, "Video", "image").count(), 3);
    assert_eq!(nodes_with_selector(&workflow, "Video", "concat").count(), 1);
    let duration_total: f64 = workflow
        .nodes
        .iter()
        .filter(|node| node.type_id == "Video" && node.params["mode"] == "image")
        .map(|node| node.params["duration"].as_f64().expect("duration should be numeric"))
        .sum();
    assert_eq!(duration_total, 12.0);

    let concat = workflow
        .nodes
        .iter()
        .find(|node| node.type_id == "Video" && node.params["mode"] == "concat")
        .expect("concat node should exist");
    let clips = serde_json::to_value(&concat.inputs["clips"]).expect("clips binding JSON");
    assert_eq!(
        clips["sources"]
            .as_array()
            .expect("ordered clips")
            .iter()
            .map(|source| source["node_id"].as_str().expect("source id"))
            .collect::<Vec<_>>(),
        vec!["n3", "n6", "n9"]
    );

    let persisted = state
        .workflow_authority
        .load_head("project")
        .expect("load persisted head")
        .expect("coauthor patch should persist a head");
    assert_eq!(persisted.revision, 1);
    assert_eq!(persisted.workflow, workflow);

    let database = Connection::open(state.config_root.join("workflow.sqlite"))
        .expect("open Workflow authority database");
    for table in ["workflow_heads", "workflow_undo", "workflow_receipts"] {
        let count: i64 = database
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .expect("count authority rows");
        assert_eq!(count, 1, "coauthor should create one {table} row");
    }
}

fn nodes_with_selector<'a>(
    workflow: &'a Workflow,
    type_id: &'a str,
    mode: &'a str,
) -> impl Iterator<Item = &'a engine::WorkflowNode> {
    workflow.nodes.iter().filter(move |node| node.type_id == type_id && node.params["mode"] == mode)
}

fn state_with_project() -> (TempDir, AppState) {
    let root = tempfile::tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    state
        .store
        .lock()
        .expect("store lock")
        .create_project_with_id("project", "Project")
        .expect("create project");
    (root, state)
}

fn operation_registrations(state: &AppState) -> Vec<OperationRegistration> {
    let snapshot = Arc::new(WorkspaceSnapshotService::from_state(state))
        .operation_registration()
        .expect("register snapshot");
    let patch = Arc::new(WorkflowPatchService::from_state(state))
        .operation_registration()
        .expect("register patch");
    let discovery = Arc::new(CapabilityDiscovery::from_state(state))
        .operation_registrations()
        .expect("register discovery");
    let mut registrations = vec![snapshot, patch];
    registrations.extend(discovery);
    registrations
}

fn fixture_command() -> AssistantSidecarCommand {
    AssistantSidecarCommand::new("python3")
        .args(["-m", "assistant.tests.agent_transport_fixture", "coauthor"])
        .current_dir(repository_root())
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri should have a repository parent")
        .to_owned()
}
