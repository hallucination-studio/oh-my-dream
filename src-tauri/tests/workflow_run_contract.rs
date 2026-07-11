use engine::{NodeExecutionState, NodeProgressEvent, RunOutputs, Value, ValueMap};
use oh_my_dream_tauri::workflow_run_dto::{
    CancelWorkflowRunResultDto, WorkflowRunEventDto, WorkflowRunResultDto,
};
use oh_my_dream_tauri::workflow_runs::{
    CancellationRequest, RunId, WorkflowRunEvent, WorkflowRunOutcome,
};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn writes_scoped_workflow_run_contract_fixtures() {
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
    let succeeded = WorkflowRunResultDto::from_outcome(
        "run-01",
        WorkflowRunOutcome::Succeeded(RunOutputs::from([(
            "image".to_owned(),
            ValueMap::from([("image".to_owned(), Value::Image("asset-01".to_owned()))]),
        )])),
    );
    let cancelled = WorkflowRunResultDto::from_outcome("run-02", WorkflowRunOutcome::Cancelled);
    let failed = WorkflowRunResultDto::from_outcome(
        "run-03",
        WorkflowRunOutcome::Failed(engine::EngineError::InvalidWorkflow {
            message: "broken graph".to_owned(),
        }),
    );
    let requested =
        CancelWorkflowRunResultDto::from_request("run-01", CancellationRequest::Requested);
    let not_active =
        CancelWorkflowRunResultDto::from_request("run-02", CancellationRequest::NotActive);

    write_fixture("workflow_run_started.json", &started);
    write_fixture("workflow_run_progress.json", &progress);
    write_fixture("workflow_run_succeeded.json", &succeeded);
    write_fixture("workflow_run_cancelled.json", &cancelled);
    write_fixture("workflow_run_failed.json", &failed);
    write_fixture("cancel_workflow_run_requested.json", &requested);
    write_fixture("cancel_workflow_run_not_active.json", &not_active);
}

fn write_fixture<T: serde::Serialize>(file_name: &str, value: &T) {
    let directory = fixture_directory();
    fs::create_dir_all(&directory).expect("create frontend fixture directory");
    let json = serde_json::to_string_pretty(value).expect("serialize fixture");
    fs::write(directory.join(file_name), format!("{json}\n")).expect("write fixture");
}

fn fixture_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("ui/src/__fixtures__")
}
