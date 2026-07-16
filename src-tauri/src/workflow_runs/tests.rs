use super::{
    CancellationRequest, RunId, WorkflowRunEvent, WorkflowRunEventError, WorkflowRunEventSink,
    WorkflowRunOutcome, WorkflowRuns, WorkflowRunsError,
};
use engine::{
    EngineError, InputPort, NodeInterface, NodeRegistry, NodeRunContextImpl, NodeRunError,
    NodeRunResult, OutputPort, PortType, RunOutputs, Value, Workflow, WorkflowNode,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Barrier, mpsc};

#[test]
fn rejects_empty_oversized_and_invalid_run_ids() {
    for invalid in ["", "run id", "run/id", "run:id", "运行", &"x".repeat(65)] {
        assert!(RunId::parse(invalid).is_err(), "`{invalid}` should be rejected");
    }

    assert_eq!(RunId::parse("run_01-Test").expect("valid run id").as_str(), "run_01-Test");
}

#[test]
fn concurrent_registration_accepts_only_one_duplicate_run_id() {
    let service = service();
    let start = Arc::new(Barrier::new(3));
    let release = Arc::new(Barrier::new(2));
    let (sender, receiver) = mpsc::channel();
    let mut threads = Vec::new();

    for _ in 0..2 {
        let service = Arc::clone(&service);
        let start = Arc::clone(&start);
        let release = Arc::clone(&release);
        let sender = sender.clone();
        threads.push(std::thread::spawn(move || {
            start.wait();
            let registration = service.register(run_id("same-run"), "project-a");
            sender.send(registration.is_ok()).expect("send registration result");
            if let Ok(_registration) = registration {
                release.wait();
            }
        }));
    }

    start.wait();
    let accepted =
        [receiver.recv().expect("first result"), receiver.recv().expect("second result")];
    assert_eq!(accepted.into_iter().filter(|accepted| *accepted).count(), 1);
    release.wait();
    for thread in threads {
        thread.join().expect("registration thread");
    }
}

#[test]
fn rejects_a_second_active_run_for_the_same_project() {
    let service = service();
    let _first = service.register(run_id("run-a"), "project-a").expect("first registration");

    let error = service
        .register(run_id("run-b"), "project-a")
        .expect_err("project should permit one active run");

    assert!(
        matches!(error, WorkflowRunsError::ProjectBusy { project_id } if project_id == "project-a")
    );
}

#[test]
fn active_run_lookup_is_project_scoped_and_clears_with_registration() {
    let service = service();
    let registration = service.register(run_id("run-a"), "project-a").expect("register active run");

    assert_eq!(
        service.active_run_id("project-a").expect("active lookup").expect("active run").as_str(),
        "run-a"
    );
    assert!(service.active_run_id("project-b").expect("foreign lookup").is_none());

    drop(registration);
    assert!(service.active_run_id("project-a").expect("cleared lookup").is_none());
}

#[test]
fn project_caches_use_independent_mutexes() {
    let service = service();
    let first = service.cache_for("project-a").expect("first cache");
    let second = service.cache_for("project-b").expect("second cache");
    let first_again = service.cache_for("project-a").expect("same cache");

    assert!(!Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first, &first_again));
}

#[test]
fn started_event_can_cancel_the_registered_run_before_execution() {
    let service = service();
    let run_id = run_id("cancel-on-start");
    let mut sink = CancellingSink { service: Arc::clone(&service), run_id: run_id.clone() };

    let outcome = service
        .run(run_id, empty_workflow("project-a"), &mut sink)
        .expect("run should finish cleanly");

    assert!(matches!(outcome, WorkflowRunOutcome::Cancelled));
}

#[test]
fn event_sink_failure_requests_cancellation() {
    let service = service();
    let run_id = run_id("closed-channel");

    let outcome = service
        .run(run_id.clone(), empty_workflow("project-a"), &mut FailingSink)
        .expect("sink failure should become cancellation");

    assert!(matches!(outcome, WorkflowRunOutcome::Cancelled));
    assert_eq!(service.cancel(&run_id).expect("cancel lookup"), CancellationRequest::NotActive);
}

#[test]
fn progress_sink_failure_requests_cancellation() {
    let service = immediate_node_service();
    let run_id = run_id("progress-channel-closed");
    let mut sink = ProgressFailingSink::default();

    let outcome = service
        .run(run_id, immediate_workflow("project-a"), &mut sink)
        .expect("progress sink failure should be coordinated");

    assert!(matches!(outcome, WorkflowRunOutcome::Cancelled));
    assert!(sink.started);
    assert_eq!(sink.progress_events, 1);
}

#[test]
fn cancel_before_terminal_commit_returns_cancelled() {
    let service = service();
    let run_id = run_id("cancel-wins");
    let registration = service.register(run_id.clone(), "project-a").expect("register active run");

    assert_eq!(service.cancel(&run_id).expect("cancel run"), CancellationRequest::Requested);
    assert!(matches!(
        registration.finish_success(RunOutputs::new()).expect("finish run"),
        WorkflowRunOutcome::Cancelled
    ));
}

#[test]
fn completion_before_late_cancel_remains_succeeded() {
    let service = service();
    let run_id = run_id("complete-wins");
    let registration = service.register(run_id.clone(), "project-a").expect("register active run");

    assert!(matches!(
        registration.finish_success(RunOutputs::new()).expect("finish run"),
        WorkflowRunOutcome::Succeeded(_)
    ));
    assert_eq!(service.cancel(&run_id).expect("late cancel"), CancellationRequest::NotActive);
}

#[test]
fn cancellation_request_does_not_mask_execution_failure() {
    let service = service();
    let run_id = run_id("failure-wins");
    let registration = service.register(run_id.clone(), "project-a").expect("register active run");

    assert_eq!(service.cancel(&run_id).expect("cancel run"), CancellationRequest::Requested);
    let outcome = registration
        .finish_failed(EngineError::InvalidWorkflow {
            message: "provider cancel failed".to_owned(),
        })
        .expect("finish failed run");

    assert!(matches!(outcome, WorkflowRunOutcome::Failed(EngineError::InvalidWorkflow { .. })));
}

#[test]
fn repeated_cancellation_is_idempotent_while_active() {
    let service = service();
    let run_id = run_id("repeat-cancel");
    let _registration = service.register(run_id.clone(), "project-a").expect("register active run");

    assert_eq!(service.cancel(&run_id).expect("first cancel"), CancellationRequest::Requested);
    assert_eq!(service.cancel(&run_id).expect("second cancel"), CancellationRequest::Requested);
}

#[test]
fn stale_cleanup_cannot_remove_a_reused_run_id() {
    let service = service();
    let run_id = run_id("reused-run");
    let stale = service.register(run_id.clone(), "project-a").expect("first registration");
    service.cleanup(&stale.key).expect("release first registration");
    let current = service.register(run_id.clone(), "project-b").expect("reuse run id");

    drop(stale);

    assert_eq!(
        service.cancel(&run_id).expect("current run remains"),
        CancellationRequest::Requested
    );
    drop(current);
}

fn service() -> Arc<WorkflowRuns> {
    Arc::new(WorkflowRuns::new(Arc::new(NodeRegistry::new())))
}

fn immediate_node_service() -> Arc<WorkflowRuns> {
    let mut registry = NodeRegistry::new();
    registry.register(
        "Immediate",
        Box::new(|_| {
            Ok(Box::new(ImmediateNodeImpl {
                outputs: vec![OutputPort { name: "text".to_owned(), port_type: PortType::String }],
            }))
        }),
    );
    Arc::new(WorkflowRuns::new(Arc::new(registry)))
}

fn run_id(value: &str) -> RunId {
    RunId::parse(value).expect("valid test run id")
}

fn empty_workflow(project_id: &str) -> Workflow {
    Workflow { version: "1.0".to_owned(), project_id: project_id.to_owned(), nodes: Vec::new() }
}

fn immediate_workflow(project_id: &str) -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: project_id.to_owned(),
        nodes: vec![WorkflowNode {
            id: "node".to_owned(),
            type_id: "Immediate".to_owned(),
            contract_version: "1.0".to_owned(),
            params: engine::NodeParams::new(),
            inputs: BTreeMap::new(),
            position: None,
        }],
    }
}

struct CancellingSink {
    service: Arc<WorkflowRuns>,
    run_id: RunId,
}

impl WorkflowRunEventSink for CancellingSink {
    fn send(&mut self, event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError> {
        assert!(matches!(event, WorkflowRunEvent::Started { .. }));
        assert_eq!(
            self.service.cancel(&self.run_id).expect("cancel started run"),
            CancellationRequest::Requested
        );
        Ok(())
    }
}

struct FailingSink;

impl WorkflowRunEventSink for FailingSink {
    fn send(&mut self, _event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError> {
        Err(Box::new(std::io::Error::other("channel closed")))
    }
}

#[derive(Default)]
struct ProgressFailingSink {
    started: bool,
    progress_events: usize,
}

impl WorkflowRunEventSink for ProgressFailingSink {
    fn send(&mut self, event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError> {
        match event {
            WorkflowRunEvent::Started { .. } => {
                self.started = true;
                Ok(())
            }
            WorkflowRunEvent::Progress { .. } => {
                self.progress_events += 1;
                Err(Box::new(std::io::Error::other("progress channel closed")))
            }
        }
    }
}

struct ImmediateNodeImpl {
    outputs: Vec<OutputPort>,
}

impl NodeInterface for ImmediateNodeImpl {
    fn type_id(&self) -> &str {
        "Immediate"
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn run(
        &self,
        _inputs: &engine::NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, NodeRunError> {
        Ok(NodeRunResult::new(BTreeMap::from([(
            "text".to_owned(),
            Value::String("done".to_owned()),
        )])))
    }
}
