use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_run_commands::{
    cancel_workflow_run_with_state, start_workflow_run_with_state,
};
use oh_my_dream_tauri::workflow_run_dto::{
    CancelWorkflowRunResultDto, WorkflowRunEventDto, WorkflowRunResultDto,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody};
use tempfile::tempdir;

#[test]
fn scoped_run_emits_started_before_run_scoped_progress() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let project =
        state.store.lock().expect("store lock").create_project("Launch").expect("project");
    let events = Arc::new(Mutex::new(Vec::new()));
    let channel = recording_channel(Arc::clone(&events));

    let result = start_workflow_run_with_state(
        "run-01".to_owned(),
        image_workflow(&project.id),
        channel,
        &state,
    )
    .expect("scoped workflow run");

    assert!(
        matches!(result, WorkflowRunResultDto::Succeeded { ref run_id, .. } if run_id == "run-01")
    );
    let events = events.lock().expect("event lock");
    assert!(matches!(
        events.first(),
        Some(WorkflowRunEventDto::Started { run_id, project_id })
            if run_id == "run-01" && project_id == &project.id
    ));
    assert!(events.iter().skip(1).all(|event| {
        matches!(event, WorkflowRunEventDto::Progress { run_id, .. } if run_id == "run-01")
    }));
    assert!(events.iter().any(|event| matches!(
        event,
        WorkflowRunEventDto::Progress { node, .. }
            if node.node_id == "image" && node.progress == Some(0.25)
    )));
}

#[test]
fn unknown_project_is_rejected_before_run_registration() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let events = Arc::new(Mutex::new(Vec::new()));

    let error = start_workflow_run_with_state(
        "unknown-project-run".to_owned(),
        image_workflow("missing-project"),
        recording_channel(Arc::clone(&events)),
        &state,
    )
    .expect_err("unknown project should fail before Started");

    assert!(error.contains("validate project"));
    assert!(events.lock().expect("event lock").is_empty());
    assert_eq!(
        cancel_workflow_run_with_state("unknown-project-run".to_owned(), &state)
            .expect("cancel lookup"),
        CancelWorkflowRunResultDto::NotActive { run_id: "unknown-project-run".to_owned() }
    );
}

#[test]
fn cancellation_command_is_idempotent_until_terminal_result() {
    let root = tempdir().expect("create temp asset root");
    let state = Arc::new(AppState::from_asset_root(root.path()).expect("build app state"));
    let project =
        state.store.lock().expect("store lock").create_project("Launch").expect("project");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let channel_state = Arc::clone(&state);
    let channel_requests = Arc::clone(&requests);
    let channel = Channel::new(move |body| {
        let event = decode_event(body);
        if matches!(event, WorkflowRunEventDto::Started { .. }) {
            let first = cancel_workflow_run_with_state("cancel-me".to_owned(), &channel_state)
                .expect("first cancel");
            let second = cancel_workflow_run_with_state("cancel-me".to_owned(), &channel_state)
                .expect("second cancel");
            channel_requests.lock().expect("request lock").extend([first, second]);
        }
        Ok(())
    });

    let result = start_workflow_run_with_state(
        "cancel-me".to_owned(),
        image_workflow(&project.id),
        channel,
        &state,
    )
    .expect("cancelled workflow command");

    assert_eq!(result, WorkflowRunResultDto::Cancelled { run_id: "cancel-me".to_owned() });
    assert_eq!(
        *requests.lock().expect("request lock"),
        vec![
            CancelWorkflowRunResultDto::Requested { run_id: "cancel-me".to_owned() },
            CancelWorkflowRunResultDto::Requested { run_id: "cancel-me".to_owned() },
        ]
    );
    assert_eq!(
        cancel_workflow_run_with_state("cancel-me".to_owned(), &state).expect("late cancel"),
        CancelWorkflowRunResultDto::NotActive { run_id: "cancel-me".to_owned() }
    );
}

fn recording_channel(events: Arc<Mutex<Vec<WorkflowRunEventDto>>>) -> Channel<WorkflowRunEventDto> {
    Channel::new(move |body| {
        events.lock().expect("event lock").push(decode_event(body));
        Ok(())
    })
}

fn decode_event(body: InvokeResponseBody) -> WorkflowRunEventDto {
    let InvokeResponseBody::Json(json) = body else {
        panic!("workflow run event should use JSON IPC");
    };
    serde_json::from_str(&json).expect("decode workflow run event")
}

fn image_workflow(project_id: &str) -> String {
    json!({
        "version": "1.0",
        "project_id": project_id,
        "nodes": [
            { "id": "prompt", "type": "TextPrompt", "params": { "text": "a red fox" }, "inputs": {} },
            { "id": "image", "type": "TextToImage", "params": {}, "inputs": { "prompt": ["prompt", "text"] } }
        ]
    })
    .to_string()
}
