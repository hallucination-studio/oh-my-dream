use engine::{
    NodeExecutionState, NodeProgressEvent, RunOutputs, ValueMap, Workflow, WorkflowNodeValue,
};
use oh_my_dream_tauri::dto::{
    NodeProgressEventDto, ProjectDto, RunOutputDto, RunWorkflowResultDto,
};

#[test]
fn converts_engine_outputs_to_named_run_output_dto() {
    let outputs = RunOutputs::from([(
        "prompt".to_owned(),
        ValueMap::from([("text".to_owned(), WorkflowNodeValue::String("hello".to_owned()))]),
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
        ("audio".to_owned(), WorkflowNodeValue::Audio("audio-ref".to_owned())),
        ("float".to_owned(), WorkflowNodeValue::Float(1.5)),
        ("image".to_owned(), WorkflowNodeValue::Image("image-ref".to_owned())),
        ("int".to_owned(), WorkflowNodeValue::Int(42)),
        ("model".to_owned(), WorkflowNodeValue::Model("model-id".to_owned())),
        ("string".to_owned(), WorkflowNodeValue::String("hello".to_owned())),
        ("video".to_owned(), WorkflowNodeValue::Video("video-ref".to_owned())),
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
