use engine::{NodeExecutionState, NodeProgressEvent, RunOutputs, Value, ValueMap, Workflow};
use oh_my_dream_tauri::dto::{
    AssetDto, NodeProgressEventDto, ProjectDto, RunOutputDto, RunWorkflowResultDto,
};

#[test]
fn converts_engine_outputs_to_named_run_output_dto() {
    let outputs = RunOutputs::from([(
        "prompt".to_owned(),
        ValueMap::from([("text".to_owned(), Value::String("hello".to_owned()))]),
    )]);

    let dto = RunWorkflowResultDto::from_outputs(&outputs);

    assert_eq!(
        dto.outputs.get("prompt").and_then(|values| values.get("text")),
        Some(&RunOutputDto { kind: "string".to_owned(), value: "hello".to_owned() })
    );
}

#[test]
fn preserves_every_engine_value_kind_in_run_output_dto() {
    let values = ValueMap::from([
        ("audio".to_owned(), Value::Audio("audio-ref".to_owned())),
        ("float".to_owned(), Value::Float(1.5)),
        ("image".to_owned(), Value::Image("image-ref".to_owned())),
        ("int".to_owned(), Value::Int(42)),
        ("model".to_owned(), Value::Model("model-id".to_owned())),
        ("string".to_owned(), Value::String("hello".to_owned())),
        ("video".to_owned(), Value::Video("video-ref".to_owned())),
    ]);
    let outputs = RunOutputs::from([("node".to_owned(), values)]);

    let dto = RunWorkflowResultDto::from_outputs(&outputs);
    let kinds = dto.outputs["node"]
        .iter()
        .map(|(name, output)| (name.as_str(), output.kind.as_str()))
        .collect::<Vec<_>>();

    assert_eq!(
        kinds,
        vec![
            ("audio", "audio"),
            ("float", "float"),
            ("image", "image"),
            ("int", "int"),
            ("model", "model"),
            ("string", "string"),
            ("video", "video"),
        ]
    );
}

#[test]
fn asset_dto_serializes_asset_kind_as_frontend_string() {
    let asset = test_asset();

    let dto = AssetDto::from(asset);
    let json = serde_json::to_value(dto).expect("asset dto should serialize");

    assert_eq!(json["kind"], "audio");
    assert_eq!(json["prompt"], "ocean at night");
    assert_eq!(json["project_id"], "project-1");
    assert_eq!(json["project_name"], "Launch");
    assert_eq!(json["source_node_id"], "video");
    assert_eq!(json["source_node_type"], "TextToAudio");
    assert_eq!(json["model"], "mock-audio");
    assert_eq!(json["seed"], "42");
    assert_eq!(json["cost"], 1250);
}

fn test_asset() -> assets::Asset {
    assets::Asset {
        id: "asset-1".to_owned(),
        kind: assets::AssetKind::Audio,
        file_path: "/tmp/audio.wav".to_owned(),
        thumbnail_path: Some("/tmp/thumb.png".to_owned()),
        workflow_snapshot: serde_json::json!({"version": "1.0"}),
        prompt: Some("ocean at night".to_owned()),
        project_id: Some("project-1".to_owned()),
        project_name: Some("Launch".to_owned()),
        source_node_id: Some("video".to_owned()),
        source_node_type: Some("TextToAudio".to_owned()),
        model: Some("mock-audio".to_owned()),
        seed: Some(42),
        cost: Some(1250),
        tags: vec!["saved".to_owned()],
        created_at: 123,
    }
}

#[test]
fn asset_dto_serializes_seed_without_javascript_precision_loss() {
    let mut asset = test_asset();
    asset.seed = Some(u64::MAX);

    let json = serde_json::to_value(AssetDto::from(asset)).expect("asset dto should serialize");

    assert_eq!(json["seed"], u64::MAX.to_string());
}

#[test]
fn serializes_project_and_node_progress_contracts() {
    let project =
        ProjectDto { id: "project-1".to_owned(), name: "Launch".to_owned(), created_at: 456 };
    let progress = NodeProgressEventDto::from(NodeProgressEvent {
        node_id: "audio".to_owned(),
        state: NodeExecutionState::Running,
        progress: Some(0.75),
        cost: Some(1250),
    });

    assert_eq!(
        serde_json::to_value(project).expect("project dto serializes"),
        serde_json::json!({
            "id": "project-1",
            "name": "Launch",
            "created_at": 456
        })
    );
    assert_eq!(
        serde_json::to_value(progress).expect("progress dto serializes"),
        serde_json::json!({
            "node_id": "audio",
            "state": "running",
            "progress": 0.75,
            "cost": 1250
        })
    );
}

#[test]
fn workflow_deserializes_project_id() {
    let workflow: Workflow = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "project_id": "project-1",
        "nodes": []
    }))
    .expect("workflow should deserialize with project id");

    assert_eq!(workflow.project_id, "project-1");
}
