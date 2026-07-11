// End-to-end: drive the full text-to-image -> image-to-video auto-save pipeline
// through the same code path `run_workflow` uses, starting from a workflow JSON
// string. This proves the backend the frontend calls produces and persists
// image/video assets without a terminal SaveAsset node.

use backends::MockBackend;
use engine::{EngineError, Executor, ResultCache, Workflow};
use oh_my_dream_tauri::commands::run_workflow_with_state;
use oh_my_dream_tauri::dto::RunWorkflowResultDto;
use oh_my_dream_tauri::state::AppState;
use std::sync::Arc;
use tempfile::tempdir;

const WORKFLOW_JSON: &str = r#"{
  "version": "1.0",
  "project_id": "project-0000000000000001",
  "nodes": [
    { "id": "prompt", "type": "TextPrompt", "params": { "text": "a red fox" }, "inputs": {} },
    { "id": "image", "type": "TextToImage", "params": {}, "inputs": { "prompt": ["prompt", "text"] } },
    { "id": "video", "type": "ImageToVideo", "params": {}, "inputs": { "image": ["image", "image"] } }
  ]
}"#;

#[test]
fn runs_full_pipeline_and_persists_video_asset() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    state.store.lock().expect("lock store").create_project("Default").expect("project");

    let workflow: Workflow = serde_json::from_str(WORKFLOW_JSON).expect("parse workflow json");

    let mut cache = ResultCache::new();
    let outputs = Executor::new(&state.registry)
        .execute(&workflow, &mut cache)
        .expect("pipeline runs to completion");

    // The nested DTO preserves the ImageToVideo node's `video` output.
    let dto = RunWorkflowResultDto::from_outputs(&outputs);
    let video = dto
        .outputs
        .get("video")
        .and_then(|node| node.get("video"))
        .expect("video node produced a `video` output");
    assert_eq!(video.kind, "video");

    // Producer nodes persisted the image and video assets, retrievable back.
    let assets = state.store.lock().expect("lock store").list(None).expect("list assets");
    assert_eq!(assets.len(), 2, "image and video assets should be stored");
    assert_eq!(assets[0].kind, assets::AssetKind::Video);
    assert_eq!(assets[0].prompt.as_deref(), Some("a red fox"));
    assert_eq!(assets[0].source_node_type.as_deref(), Some("ImageToVideo"));
}

#[test]
fn reuses_result_cache_without_resubmitting_backend_tasks() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    state.store.lock().expect("lock store").create_project("Default").expect("project");
    run_workflow_with_state(WORKFLOW_JSON.to_owned(), &state).expect("first run should complete");
    assert_eq!(state.backend.submitted_task_count(), 2);

    run_workflow_with_state(WORKFLOW_JSON.to_owned(), &state).expect("second run should complete");

    assert_eq!(state.backend.submitted_task_count(), 2);
    let assets = state.store.lock().expect("lock store").list(None).expect("list assets");
    assert_eq!(assets.len(), 2, "cached producer nodes should not persist duplicates");
}

#[test]
fn failing_backend_surfaces_readable_run_workflow_error() {
    let root = tempdir().expect("create temp asset root");
    let backend = Arc::new(MockBackend::always_fails("provider outage"));
    let state =
        AppState::from_asset_root_with_backend(root.path(), backend).expect("build app state");
    state.store.lock().expect("lock store").create_project("Default").expect("project");

    let error =
        run_workflow_with_state(WORKFLOW_JSON.to_owned(), &state).expect_err("run should fail");

    assert!(error.contains("run workflow"));
    assert!(error.contains("provider outage"));
}

#[test]
fn rejects_image_output_wired_into_string_prompt_input() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let workflow: Workflow =
        serde_json::from_str(TYPE_MISMATCH_WORKFLOW_JSON).expect("parse workflow json");
    let mut cache = ResultCache::new();

    let error = Executor::new(&state.registry)
        .execute(&workflow, &mut cache)
        .expect_err("workflow should fail wiring validation");

    assert!(matches!(
        error,
        EngineError::TypeMismatch {
            node_id,
            input,
            source_node,
            output,
            ..
        } if node_id == "bad_prompt"
            && input == "prompt"
            && source_node == "image"
            && output == "image"
    ));
}

#[test]
fn stored_asset_can_be_read_back_with_original_workflow_snapshot() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    state.store.lock().expect("lock store").create_project("Default").expect("project");
    let workflow: Workflow = serde_json::from_str(WORKFLOW_JSON).expect("parse workflow json");
    let mut cache = ResultCache::new();

    Executor::new(&state.registry)
        .execute(&workflow, &mut cache)
        .expect("pipeline runs to completion");
    let assets = state.store.lock().expect("lock store").list(None).expect("list assets");
    let stored =
        state.store.lock().expect("lock store").get(&assets[0].id).expect("get stored asset");

    assert_eq!(
        stored.workflow_snapshot,
        serde_json::to_value(&workflow).expect("serialize submitted workflow")
    );
}

const TYPE_MISMATCH_WORKFLOW_JSON: &str = r#"{
  "version": "1.0",
  "project_id": "project-0000000000000001",
  "nodes": [
    { "id": "prompt", "type": "TextPrompt", "params": { "text": "a red fox" }, "inputs": {} },
    { "id": "image", "type": "TextToImage", "params": {}, "inputs": { "prompt": ["prompt", "text"] } },
    { "id": "bad_prompt", "type": "TextToImage", "params": {}, "inputs": { "prompt": ["image", "image"] } }
  ]
}"#;
