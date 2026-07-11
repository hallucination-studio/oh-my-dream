use engine::{EngineError, NodeExecutionState, NodeProgressEvent, RunOutputs, Value, ValueMap};
use oh_my_dream_tauri::workflow_run_dto::{
    CancelWorkflowRunResultDto, WorkflowRunEventDto, WorkflowRunResultDto,
};
use oh_my_dream_tauri::workflow_runs::{
    CancellationRequest, RunId, WorkflowRunEvent, WorkflowRunOutcome,
};

#[test]
fn serializes_scoped_run_events_as_discriminated_unions() {
    let run_id = RunId::parse("run-01").expect("run id");
    let started = WorkflowRunEventDto::from(WorkflowRunEvent::Started {
        run_id: run_id.clone(),
        project_id: "project-01".to_owned(),
    });
    let progress = WorkflowRunEventDto::from(WorkflowRunEvent::Progress {
        run_id,
        node: NodeProgressEvent {
            node_id: "image".to_owned(),
            state: NodeExecutionState::Running,
            progress: Some(0.25),
            cost: None,
        },
    });

    assert_eq!(
        serde_json::to_value(started).expect("serialize Started"),
        serde_json::json!({
            "event": "started",
            "run_id": "run-01",
            "project_id": "project-01"
        })
    );
    assert_eq!(
        serde_json::to_value(progress).expect("serialize Progress"),
        serde_json::json!({
            "event": "progress",
            "run_id": "run-01",
            "node": {
                "node_id": "image",
                "state": "running",
                "progress": 0.25,
                "cost": null
            }
        })
    );
}

#[test]
fn serializes_all_terminal_run_results() {
    let outputs = RunOutputs::from([(
        "image".to_owned(),
        ValueMap::from([("image".to_owned(), Value::Image("asset-01".to_owned()))]),
    )]);

    let succeeded =
        WorkflowRunResultDto::from_outcome("run-01", WorkflowRunOutcome::Succeeded(outputs));
    let cancelled = WorkflowRunResultDto::from_outcome("run-02", WorkflowRunOutcome::Cancelled);
    let failed = WorkflowRunResultDto::from_outcome(
        "run-03",
        WorkflowRunOutcome::Failed(EngineError::InvalidWorkflow {
            message: "broken graph".to_owned(),
        }),
    );

    assert_eq!(
        serde_json::to_value(succeeded).expect("succeeded"),
        serde_json::json!({
            "status": "succeeded",
            "run_id": "run-01",
            "outputs": { "image": { "image": { "kind": "image", "value": "asset-01" } } }
        })
    );
    assert_eq!(
        serde_json::to_value(cancelled).expect("cancelled"),
        serde_json::json!({
            "status": "cancelled",
            "run_id": "run-02"
        })
    );
    assert_eq!(
        serde_json::to_value(failed).expect("failed"),
        serde_json::json!({
            "status": "failed",
            "run_id": "run-03",
            "reason": "invalid workflow: broken graph"
        })
    );
}

#[test]
fn serializes_both_cancellation_command_outcomes() {
    assert_eq!(
        CancelWorkflowRunResultDto::from_request("run-01", CancellationRequest::Requested),
        CancelWorkflowRunResultDto::Requested { run_id: "run-01".to_owned() }
    );
    assert_eq!(
        CancelWorkflowRunResultDto::from_request("run-02", CancellationRequest::NotActive),
        CancelWorkflowRunResultDto::NotActive { run_id: "run-02".to_owned() }
    );
}
