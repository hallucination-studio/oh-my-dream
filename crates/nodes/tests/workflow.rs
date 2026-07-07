use assets::{AssetKind, AssetStore};
use backends::{InferenceBackend, MockBackend};
use engine::{
    EngineError, Executor, NodeParams, NodeRegistry, OutputRef, ResultCache, Value, Workflow,
    WorkflowNode,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn executes_full_generation_workflow_and_persists_video_asset() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let store = shared_store(temp_dir.path());
    let backend: Arc<dyn InferenceBackend> = Arc::new(MockBackend::new());
    let mut registry = NodeRegistry::new();
    nodes::register_all(&mut registry, backend, Arc::clone(&store));
    let workflow = full_workflow();

    let outputs = Executor::new(&registry)
        .execute(&workflow, &mut ResultCache::new())
        .expect("workflow should execute");

    let video = outputs
        .get("video")
        .and_then(|values| values.get("video"))
        .expect("video output should be produced");
    assert!(
        matches!(video, Value::Video(value) if value.starts_with("mock://mock/image-to-video/"))
    );

    let saved_assets =
        store.lock().expect("store lock should succeed").list(None).expect("assets should list");
    assert_eq!(saved_assets.len(), 1);
    assert_eq!(saved_assets[0].kind, AssetKind::Video);
    assert_eq!(saved_assets[0].workflow_snapshot, json!({"case": "nodes-workflow"}));

    let stored = store
        .lock()
        .expect("store lock should succeed")
        .get(&saved_assets[0].id)
        .expect("asset should be retrievable");
    assert_eq!(stored.source_node_id, Some("video".to_owned()));
}

#[test]
fn failed_backend_task_surfaces_as_execution_error() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let store = shared_store(temp_dir.path());
    let backend: Arc<dyn InferenceBackend> = Arc::new(MockBackend::always_fails("provider failed"));
    let mut registry = NodeRegistry::new();
    nodes::register_all(&mut registry, backend, store);

    let error = Executor::new(&registry)
        .execute(&image_workflow(), &mut ResultCache::new())
        .expect_err("backend failure should fail workflow execution");

    assert!(matches!(
        error,
        EngineError::NodeExecution {
            ref node_id,
            ref type_id,
            ..
        } if node_id == "image" && type_id == "TextToImage"
    ));
    assert!(error.to_string().contains("provider failed"));
}

fn shared_store(path: &std::path::Path) -> nodes::SharedAssetStore {
    Arc::new(Mutex::new(AssetStore::open(path).expect("asset store should open")))
}

fn full_workflow() -> Workflow {
    let mut nodes = image_workflow().nodes;
    nodes.push(WorkflowNode {
        id: "video".to_owned(),
        type_id: "ImageToVideo".to_owned(),
        params: params(json!({
            "model": "mock-video",
            "duration": 4.0,
            "fps": 24
        })),
        inputs: BTreeMap::from([(
            "image".to_owned(),
            OutputRef("image".to_owned(), "image".to_owned()),
        )]),
        position: None,
    });
    nodes.push(WorkflowNode {
        id: "save".to_owned(),
        type_id: "SaveAsset".to_owned(),
        params: params(json!({
            "workflow_snapshot": {"case": "nodes-workflow"},
            "source_node_id": "video"
        })),
        inputs: BTreeMap::from([(
            "media".to_owned(),
            OutputRef("video".to_owned(), "video".to_owned()),
        )]),
        position: None,
    });
    Workflow { version: "1.0".to_owned(), nodes }
}

fn image_workflow() -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        nodes: vec![
            WorkflowNode {
                id: "prompt".to_owned(),
                type_id: "TextPrompt".to_owned(),
                params: params(json!({"text": "a small moonlit house"})),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "image".to_owned(),
                type_id: "TextToImage".to_owned(),
                params: params(json!({
                    "model": "mock-image",
                    "steps": 28,
                    "seed": 42
                })),
                inputs: BTreeMap::from([(
                    "prompt".to_owned(),
                    OutputRef("prompt".to_owned(), "text".to_owned()),
                )]),
                position: None,
            },
        ],
    }
}

fn params(value: serde_json::Value) -> NodeParams {
    match value {
        serde_json::Value::Object(params) => params,
        _ => NodeParams::new(),
    }
}
