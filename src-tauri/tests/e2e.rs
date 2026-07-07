// End-to-end: drive the full text-to-image -> image-to-video -> save pipeline
// through the same code path `run_workflow` uses (enrich + engine executor +
// asset store), starting from a workflow JSON string. This is the integration
// proof that the backend the frontend calls actually produces and persists a
// video asset.

use engine::{Executor, ResultCache, Workflow};
use oh_my_dream_tauri::commands::enrich_save_asset_params;
use oh_my_dream_tauri::dto::RunWorkflowResultDto;
use oh_my_dream_tauri::state::AppState;
use tempfile::tempdir;

const WORKFLOW_JSON: &str = r#"{
  "version": "1.0",
  "nodes": [
    { "id": "prompt", "type": "TextPrompt", "params": { "text": "a red fox" }, "inputs": {} },
    { "id": "image", "type": "TextToImage", "params": {}, "inputs": { "prompt": ["prompt", "text"] } },
    { "id": "video", "type": "ImageToVideo", "params": {}, "inputs": { "image": ["image", "image"] } },
    { "id": "save", "type": "SaveAsset", "params": {}, "inputs": { "media": ["video", "video"] } }
  ]
}"#;

#[test]
fn runs_full_pipeline_and_persists_video_asset() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let workflow: Workflow = serde_json::from_str(WORKFLOW_JSON).expect("parse workflow json");
    let workflow = enrich_save_asset_params(&workflow).expect("enrich workflow");

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

    // The SaveAsset node persisted exactly one video asset, retrievable back.
    let assets = state.store.lock().expect("lock store").list(None).expect("list assets");
    assert_eq!(assets.len(), 1, "one asset should be stored");
    assert_eq!(assets[0].kind, assets::AssetKind::Video);
}
