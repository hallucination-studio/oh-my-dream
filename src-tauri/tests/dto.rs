use engine::{RunOutputs, Value, ValueMap};
use oh_my_dream_tauri::dto::{AssetDto, RunOutputDto, RunWorkflowResultDto};

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
fn asset_dto_serializes_asset_kind_as_frontend_string() {
    let asset = assets::Asset {
        id: "asset-1".to_owned(),
        kind: assets::AssetKind::Video,
        file_path: "/tmp/video.mp4".to_owned(),
        thumbnail_path: Some("/tmp/thumb.png".to_owned()),
        workflow_snapshot: serde_json::json!({"version": "1.0"}),
        source_node_id: Some("video".to_owned()),
        tags: vec!["saved".to_owned()],
        created_at: 123,
    };

    let dto = AssetDto::from(asset);
    let json = serde_json::to_value(dto).expect("asset dto should serialize");

    assert_eq!(json["kind"], "video");
    assert_eq!(json["source_node_id"], "video");
}
