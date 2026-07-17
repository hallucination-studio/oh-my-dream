use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
    NodeCapabilityContractVersion, NodeCapabilityExecutionError, NodeCapabilityExecutionKind,
    NodeCapabilityExecutionRequest, NodeCapabilityNormalizedParameters,
    NodeCapabilityOutputContract, NodeCapabilityOutputKey, NodeCapabilityParameterError,
    NodeCapabilityParameterSet, NodeCapabilityReadinessIssue, NodeCapabilityReadinessRequest,
    WorkflowDataType, WorkflowManagedAssetIdBoundaryValue, WorkflowManagedAudioRef,
    WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowManagedVideoRef,
    WorkflowNodeCapabilityInterface, WorkflowNodeCapabilityRegistry, WorkflowNodeOutputSet,
    WorkflowRuntimeValue, WorkflowTextPart, WorkflowTextValue,
};
use engine::workflow::{
    WorkflowExecuteRunUseCase, WorkflowExecutionCancellationRegistry,
    WorkflowGetNodePresentationUseCase, WorkflowGetRunUseCase, WorkflowListRunEventsUseCase,
    WorkflowNodePresentationShell, WorkflowRunEventSequence, WorkflowRunRequestId,
    WorkflowRunScope, WorkflowStartRunCommand, WorkflowStartRunUseCase,
};
use engine::workflow_graph::{
    WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowCanvasPosition, WorkflowCreatedAt,
    WorkflowId, WorkflowNodeEntity, WorkflowNodeId, WorkflowRevision, WorkflowSchemaVersion,
    WorkflowUpdatedAt,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::workflow_interfaces::WorkflowContractFakeImpl;

struct FixedOutputCapabilityImpl {
    contract: NodeCapabilityContract,
    value: WorkflowRuntimeValue,
}

#[async_trait]
impl WorkflowNodeCapabilityInterface for FixedOutputCapabilityImpl {
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
        WorkflowNodeOutputSet::try_new(
            &self.contract,
            BTreeMap::from([(output_key(), self.value.clone())]),
        )
        .map_err(|_| {
            NodeCapabilityExecutionError::invalid_capability_invocation(
                self.contract.contract_ref().clone(),
                request.context.node_execution_id,
            )
        })
    }
}

#[tokio::test]
async fn run_query_is_project_scoped_and_event_pages_use_exclusive_cursor() {
    let (repository, registry, run) =
        executed_run(WorkflowRuntimeValue::Image(image_ref()), 1).await;
    let loaded = WorkflowGetRunUseCase::new(repository.clone())
        .get_workflow_run(project_id(2), run.run_id())
        .await
        .unwrap();
    assert_eq!(loaded.run_id(), run.run_id());
    assert!(
        WorkflowGetRunUseCase::new(repository.clone())
            .get_workflow_run(project_id(99), run.run_id())
            .await
            .is_err()
    );

    let first = WorkflowListRunEventsUseCase::new(repository.clone())
        .list_workflow_run_events(project_id(2), run.run_id(), None, 2)
        .await
        .unwrap();
    assert_eq!(first.events.len(), 2);
    assert_eq!(first.next_sequence, Some(WorkflowRunEventSequence::new(2).unwrap()));
    let second = WorkflowListRunEventsUseCase::new(repository)
        .list_workflow_run_events(project_id(2), run.run_id(), first.next_sequence, 500)
        .await
        .unwrap();
    assert!(second.events.iter().all(|event| event.sequence().get() > 2));
    assert_eq!(second.next_sequence, None);
    assert!(matches!(
        WorkflowListRunEventsUseCase::new(Arc::new(WorkflowContractFakeImpl::default()))
            .list_workflow_run_events(project_id(2), run.run_id(), None, 0)
            .await,
        Err(engine::workflow::WorkflowApplicationError::WorkflowRunEventLimitOutOfBounds { .. })
    ));
    drop(registry);
}

#[tokio::test]
async fn all_four_shells_project_values_and_media_previews() {
    for (index, value) in [
        WorkflowRuntimeValue::Text(
            WorkflowTextValue::try_new([WorkflowTextPart::Literal("text".into())]).unwrap(),
        ),
        WorkflowRuntimeValue::Image(image_ref()),
        WorkflowRuntimeValue::Video(video_ref()),
        WorkflowRuntimeValue::Audio(audio_ref()),
    ]
    .into_iter()
    .enumerate()
    {
        let (repository, registry, run) =
            executed_run(value, u8::try_from(index + 10).unwrap()).await;
        let view = WorkflowGetNodePresentationUseCase::new(
            repository.clone(),
            repository.clone(),
            repository,
            registry,
        )
        .get_workflow_node_presentation(project_id(2), workflow_id(3), node_id(4))
        .await
        .unwrap();
        match view.shell {
            WorkflowNodePresentationShell::Text(shell) => assert!(shell.value.is_some()),
            WorkflowNodePresentationShell::Image(shell) => assert!(shell.preview.is_some()),
            WorkflowNodePresentationShell::Video(shell) => assert!(shell.preview.is_some()),
            WorkflowNodePresentationShell::Audio(shell) => assert!(shell.preview.is_some()),
        }
        assert!(!view.latest_execution.unwrap().is_stale);
        assert_eq!(run.state(), engine::workflow::WorkflowRunState::Succeeded);
    }
}

#[tokio::test]
async fn semantic_node_change_marks_prior_output_stale_without_rebinding_preview() {
    let (repository, registry, run) =
        executed_run(WorkflowRuntimeValue::Image(image_ref()), 30).await;
    let alternate = Arc::new(FixedOutputCapabilityImpl {
        contract: contract(WorkflowDataType::Image, "test.alternate"),
        value: WorkflowRuntimeValue::Image(image_ref()),
    });
    let combined = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            registry.resolve_node_capability(&run.plan().nodes()[0].capability_contract).unwrap(),
            alternate.clone() as Arc<dyn WorkflowNodeCapabilityInterface>,
        ])
        .unwrap(),
    );
    repository.seed_workflow(single_node_workflow(&combined, &alternate.contract, 2));
    let view = WorkflowGetNodePresentationUseCase::new(
        repository.clone(),
        repository.clone(),
        repository,
        combined,
    )
    .get_workflow_node_presentation(project_id(2), workflow_id(3), node_id(4))
    .await
    .unwrap();

    assert!(view.latest_execution.unwrap().is_stale);
    let WorkflowNodePresentationShell::Image(shell) = view.shell else { panic!("expected image") };
    assert!(shell.value.is_some());
    assert!(shell.preview.is_some());
}

async fn executed_run(
    value: WorkflowRuntimeValue,
    request_seed: u8,
) -> (
    Arc<WorkflowContractFakeImpl>,
    Arc<WorkflowNodeCapabilityRegistry>,
    engine::workflow::WorkflowRunAggregate,
) {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability = Arc::new(FixedOutputCapabilityImpl {
        contract: contract(value.data_type(), "test.presentation"),
        value,
    });
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let workflow = single_node_workflow(&registry, &capability.contract, 1);
    repository.seed_workflow(workflow.clone());
    let admitted = WorkflowStartRunUseCase::new(
        repository.clone(),
        repository.clone(),
        repository.clone(),
        repository.clone(),
        registry.clone(),
    )
    .start_workflow_run(WorkflowStartRunCommand::new(
        run_request_id(request_seed),
        workflow.id,
        workflow.revision,
        WorkflowRunScope::WholeWorkflow,
    ))
    .await
    .unwrap();
    let executed = WorkflowExecuteRunUseCase::try_new(
        repository.clone(),
        repository.clone(),
        repository.clone(),
        registry.clone(),
        Arc::new(WorkflowExecutionCancellationRegistry::default()),
        1,
    )
    .unwrap()
    .execute_workflow_run(admitted.run_id())
    .await
    .unwrap();
    (repository, registry, executed)
}

fn single_node_workflow(
    registry: &WorkflowNodeCapabilityRegistry,
    contract: &NodeCapabilityContract,
    revision: u64,
) -> WorkflowAggregate {
    WorkflowAggregate::try_restore(
        WorkflowAggregateRestoreData {
            schema_version: WorkflowSchemaVersion::CURRENT,
            id: workflow_id(3),
            project_id: project_id(2),
            revision: WorkflowRevision::new(revision).unwrap(),
            created_at: WorkflowCreatedAt::from_utc_milliseconds(1).unwrap(),
            updated_at: WorkflowUpdatedAt::from_utc_milliseconds(i64::try_from(revision).unwrap())
                .unwrap(),
            nodes: vec![WorkflowNodeEntity {
                id: node_id(4),
                capability_contract: contract.contract_ref().clone(),
                parameter_set: NodeCapabilityParameterSet::default(),
                canvas_position: WorkflowCanvasPosition::try_new(0.0, 0.0).unwrap(),
            }],
            input_bindings: Vec::new(),
        },
        registry,
    )
    .unwrap()
}

fn contract(data_type: WorkflowDataType, id: &str) -> NodeCapabilityContract {
    NodeCapabilityContract::try_new(
        NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new(id).unwrap(),
            NodeCapabilityContractVersion::new(1, 0).unwrap(),
        ),
        Vec::new(),
        Vec::new(),
        vec![NodeCapabilityOutputContract::new(output_key(), data_type, true)],
        NodeCapabilityExecutionKind::PureValue,
    )
    .unwrap()
}

fn image_ref() -> WorkflowManagedImageRef {
    WorkflowManagedImageRef::new(asset_id(), WorkflowManagedContentFingerprint::from_bytes([1; 32]))
}
fn video_ref() -> WorkflowManagedVideoRef {
    WorkflowManagedVideoRef::new(asset_id(), WorkflowManagedContentFingerprint::from_bytes([2; 32]))
}
fn audio_ref() -> WorkflowManagedAudioRef {
    WorkflowManagedAudioRef::new(asset_id(), WorkflowManagedContentFingerprint::from_bytes([3; 32]))
}
fn asset_id() -> WorkflowManagedAssetIdBoundaryValue {
    WorkflowManagedAssetIdBoundaryValue::from_bytes(*uuid(50).as_bytes()).unwrap()
}
fn output_key() -> NodeCapabilityOutputKey {
    NodeCapabilityOutputKey::new("result").unwrap()
}
fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}
fn workflow_id(seed: u8) -> WorkflowId {
    WorkflowId::from_uuid(uuid(seed)).unwrap()
}
fn node_id(seed: u8) -> WorkflowNodeId {
    WorkflowNodeId::from_uuid(uuid(seed)).unwrap()
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
