use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityExecutionError, NodeCapabilityExecutionRequest,
    NodeCapabilityNormalizedParameters, NodeCapabilityParameterError, NodeCapabilityParameterSet,
    NodeCapabilityReadinessIssue, NodeCapabilityReadinessRequest, WorkflowNodeCapabilityInterface,
    WorkflowNodeCapabilityRegistry, WorkflowNodeOutputSet,
};
use engine::workflow::{
    WorkflowCancelRunUseCase, WorkflowExecuteRunUseCase, WorkflowExecutionCancellationRegistry,
    WorkflowRunFailure, WorkflowRunRequestId, WorkflowRunScope, WorkflowRunState,
    WorkflowStartRunCommand, WorkflowStartRunUseCase,
};
use uuid::Uuid;

use super::workflow_interfaces::WorkflowContractFakeImpl;
use super::workflow_run_admission::{branching_workflow, contract};
use super::workflow_use_cases::ReadinessCapabilityImpl;

struct ToggleReadinessCapabilityImpl {
    contract: NodeCapabilityContract,
    unavailable: AtomicBool,
    execution_calls: AtomicUsize,
}

struct BlockingCapabilityImpl {
    contract: NodeCapabilityContract,
    started: AtomicBool,
}

#[async_trait]
impl WorkflowNodeCapabilityInterface for BlockingCapabilityImpl {
    fn node_capability_contract(&self) -> &NodeCapabilityContract {
        &self.contract
    }
    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError> {
        self.contract.normalize_node_parameters(parameters)
    }
    async fn check_node_external_readiness(
        &self,
        _request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue> {
        Vec::new()
    }
    async fn execute_node_capability(
        &self,
        request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
        self.started.store(true, Ordering::Release);
        while !request.context.cancellation.is_cancelled() {
            tokio::task::yield_now().await;
        }
        Err(NodeCapabilityExecutionError::cancelled_while_calling_provider(
            self.contract.contract_ref().clone(),
            request.context.node_execution_id,
        ))
    }
}

#[async_trait]
impl WorkflowNodeCapabilityInterface for ToggleReadinessCapabilityImpl {
    fn node_capability_contract(&self) -> &NodeCapabilityContract {
        &self.contract
    }
    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError> {
        self.contract.normalize_node_parameters(parameters)
    }
    async fn check_node_external_readiness(
        &self,
        _request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue> {
        if self.unavailable.load(Ordering::Acquire) {
            vec![NodeCapabilityReadinessIssue::invalid_capability_invocation()]
        } else {
            Vec::new()
        }
    }
    async fn execute_node_capability(
        &self,
        request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
        self.execution_calls.fetch_add(1, Ordering::AcqRel);
        Err(NodeCapabilityExecutionError::invalid_capability_invocation(
            self.contract.contract_ref().clone(),
            request.context.node_execution_id,
        ))
    }
}

#[tokio::test]
async fn coordinator_executes_ready_branches_and_finishes_successfully() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability = Arc::new(ReadinessCapabilityImpl { contract: contract(), issues: Vec::new() });
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let run =
        admit_run(repository.clone(), registry.clone(), capability.node_capability_contract())
            .await;
    let cancellations = Arc::new(WorkflowExecutionCancellationRegistry::default());
    let executed = WorkflowExecuteRunUseCase::try_new(
        repository.clone(),
        repository.clone(),
        repository,
        registry,
        cancellations,
        2,
    )
    .unwrap()
    .execute_workflow_run(run.run_id())
    .await
    .unwrap();

    assert_eq!(executed.state(), WorkflowRunState::Succeeded);
    assert!(
        executed.node_executions().iter().all(|node| {
            node.state() == engine::workflow::WorkflowNodeExecutionState::Succeeded
        })
    );
}

#[tokio::test]
async fn execution_rechecks_readiness_and_never_dispatches_when_it_changed() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability = Arc::new(ToggleReadinessCapabilityImpl {
        contract: contract(),
        unavailable: AtomicBool::new(false),
        execution_calls: AtomicUsize::new(0),
    });
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let run =
        admit_run(repository.clone(), registry.clone(), capability.node_capability_contract())
            .await;
    capability.unavailable.store(true, Ordering::Release);
    let executed = WorkflowExecuteRunUseCase::try_new(
        repository.clone(),
        repository.clone(),
        repository,
        registry,
        Arc::new(WorkflowExecutionCancellationRegistry::default()),
        2,
    )
    .unwrap()
    .execute_workflow_run(run.run_id())
    .await
    .unwrap();

    assert_eq!(executed.state(), WorkflowRunState::Failed);
    assert_eq!(capability.execution_calls.load(Ordering::Acquire), 0);
    assert!(matches!(executed.failure(), Some(WorkflowRunFailure::NodeExecutionFailed { .. })));
}

#[tokio::test]
async fn restart_interruption_fails_a_queued_run_without_dispatch() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability = Arc::new(ReadinessCapabilityImpl { contract: contract(), issues: Vec::new() });
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let run =
        admit_run(repository.clone(), registry.clone(), capability.node_capability_contract())
            .await;
    let interrupted = WorkflowExecuteRunUseCase::try_new(
        repository.clone(),
        repository.clone(),
        repository,
        registry,
        Arc::new(WorkflowExecutionCancellationRegistry::default()),
        1,
    )
    .unwrap()
    .interrupt_workflow_run_after_restart(run.run_id())
    .await
    .unwrap();

    assert_eq!(interrupted.state(), WorkflowRunState::Failed);
    assert_eq!(interrupted.failure(), Some(&WorkflowRunFailure::InterruptedByRestart));
}

#[tokio::test]
async fn cancellation_commits_before_signalling_and_rejects_late_results() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability =
        Arc::new(BlockingCapabilityImpl { contract: contract(), started: AtomicBool::new(false) });
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let run =
        admit_run(repository.clone(), registry.clone(), capability.node_capability_contract())
            .await;
    let cancellations = Arc::new(WorkflowExecutionCancellationRegistry::default());
    let execute = WorkflowExecuteRunUseCase::try_new(
        repository.clone(),
        repository.clone(),
        repository.clone(),
        registry,
        cancellations.clone(),
        2,
    )
    .unwrap();
    let run_id = run.run_id();
    let task = tokio::spawn(async move { execute.execute_workflow_run(run_id).await });
    while !capability.started.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    let cancelled = WorkflowCancelRunUseCase::new(
        repository.clone(),
        repository.clone(),
        repository,
        cancellations,
    )
    .cancel_workflow_run(run_id)
    .await
    .unwrap();
    let completed = task.await.unwrap().unwrap();

    assert_eq!(cancelled.state(), WorkflowRunState::Cancelled);
    assert_eq!(completed.state(), WorkflowRunState::Cancelled);
    assert!(
        completed.node_executions().iter().all(|node| {
            node.state() == engine::workflow::WorkflowNodeExecutionState::Cancelled
        })
    );
}

pub(crate) async fn admit_run(
    repository: Arc<WorkflowContractFakeImpl>,
    registry: Arc<WorkflowNodeCapabilityRegistry>,
    capability_contract: &NodeCapabilityContract,
) -> engine::workflow::WorkflowRunAggregate {
    let workflow = branching_workflow(&registry, capability_contract, 1);
    repository.seed_workflow(workflow.clone());
    WorkflowStartRunUseCase::new(
        repository.clone(),
        repository.clone(),
        repository.clone(),
        repository,
        registry,
    )
    .start_workflow_run(WorkflowStartRunCommand::new(
        run_request_id(70),
        workflow.id,
        workflow.revision,
        WorkflowRunScope::WholeWorkflow,
    ))
    .await
    .unwrap()
}

fn run_request_id(seed: u8) -> WorkflowRunRequestId {
    WorkflowRunRequestId::from_uuid(uuid(seed)).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
