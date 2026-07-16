use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
    NodeCapabilityContractVersion, NodeCapabilityExecutionError, NodeCapabilityExecutionKind,
    NodeCapabilityExecutionRequest, NodeCapabilityNormalizedParameters,
    NodeCapabilityOutputContract, NodeCapabilityOutputKey, NodeCapabilityParameterError,
    NodeCapabilityParameterSet, NodeCapabilityReadinessIssue, NodeCapabilityReadinessRequest,
    WorkflowDataType, WorkflowNodeCapabilityInterface, WorkflowNodeCapabilityRegistry,
    WorkflowNodeOutputSet, WorkflowRuntimeValue, WorkflowTextPart, WorkflowTextValue,
};
use engine::workflow::{
    WorkflowApplicationError, WorkflowApplyMutationUseCase, WorkflowCheckReadinessUseCase,
    WorkflowCreateCommand, WorkflowCreateRequestId, WorkflowCreateUseCase,
    WorkflowGetCurrentUseCase, WorkflowReadinessIssue, WorkflowReadinessResult,
};
use engine::workflow_graph::{
    WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowApplyMutationCommand,
    WorkflowCanvasPosition, WorkflowCreatedAt, WorkflowId, WorkflowMoveNodeAction,
    WorkflowMutationAction, WorkflowMutationRequestId, WorkflowNodeEntity, WorkflowNodeId,
    WorkflowRevision, WorkflowSchemaVersion, WorkflowUpdatedAt,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::workflow_interfaces::WorkflowContractFakeImpl;

pub(crate) struct ReadinessCapabilityImpl {
    pub(crate) contract: NodeCapabilityContract,
    pub(crate) issues: Vec<NodeCapabilityReadinessIssue>,
}

#[async_trait]
impl WorkflowNodeCapabilityInterface for ReadinessCapabilityImpl {
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
        self.issues.clone()
    }
    async fn execute_node_capability(
        &self,
        request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
        WorkflowNodeOutputSet::try_new(
            &self.contract,
            BTreeMap::from([(
                output_key(),
                WorkflowRuntimeValue::Text(
                    WorkflowTextValue::try_new([WorkflowTextPart::Literal("ok".into())]).unwrap(),
                ),
            )]),
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
async fn create_replays_exact_snapshot_and_get_current_reads_it() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capabilities = Arc::new(WorkflowNodeCapabilityRegistry::try_new([]).unwrap());
    let use_case = WorkflowCreateUseCase::new(
        repository.clone(),
        repository.clone(),
        repository.clone(),
        capabilities,
    );
    let command = WorkflowCreateCommand::new(create_request_id(1), project_id(2));
    let created = use_case.create_workflow(command).await.unwrap();
    let replayed = use_case.create_workflow(command).await.unwrap();

    assert_eq!(created, replayed);
    assert!(created.nodes().is_empty());
    let loaded = WorkflowGetCurrentUseCase::new(repository)
        .get_current_workflow(project_id(2))
        .await
        .unwrap();
    assert_eq!(loaded, created);
}

#[test]
fn creation_hash_excludes_request_identity_but_includes_project() {
    let first = WorkflowCreateCommand::new(create_request_id(3), project_id(4));
    let replay = WorkflowCreateCommand::new(create_request_id(5), project_id(4));
    let other_project = WorkflowCreateCommand::new(create_request_id(3), project_id(6));
    assert_eq!(first.command_hash(), replay.command_hash());
    assert_ne!(first.command_hash(), other_project.command_hash());
}

#[tokio::test]
async fn readiness_preserves_capability_owned_external_issue() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability = Arc::new(ReadinessCapabilityImpl {
        contract: contract(),
        issues: vec![NodeCapabilityReadinessIssue::invalid_capability_invocation()],
    });
    let restore_registry = WorkflowNodeCapabilityRegistry::try_new([
        capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
    ])
    .unwrap();
    let workflow = workflow_with_node(&restore_registry, capability.node_capability_contract());
    repository.seed_workflow(workflow.clone());
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );

    let result = WorkflowCheckReadinessUseCase::new(repository, registry)
        .check_workflow_readiness(workflow.id)
        .await
        .unwrap();
    let WorkflowReadinessResult::Blocked { issues } = result else { panic!("expected blocked") };
    assert!(matches!(
        issues.as_slice(),
        [WorkflowReadinessIssue::WorkflowCapabilityExternalReadinessIssue { .. }]
    ));
}

#[tokio::test]
async fn mutation_commits_once_and_replays_with_current_readiness() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability = Arc::new(ReadinessCapabilityImpl { contract: contract(), issues: Vec::new() });
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let workflow = workflow_with_node(&registry, capability.node_capability_contract());
    repository.seed_workflow(workflow.clone());
    let command = WorkflowApplyMutationCommand::try_new(
        mutation_request_id(10),
        workflow.id,
        workflow.revision,
        vec![WorkflowMutationAction::MoveNode(WorkflowMoveNodeAction {
            node_id: node_id(11),
            canvas_position: WorkflowCanvasPosition::try_new(12.0, 13.0).unwrap(),
        })],
    )
    .unwrap();
    let use_case = WorkflowApplyMutationUseCase::new(repository.clone(), repository, registry);
    let committed = use_case.apply_workflow_mutation(command.clone()).await.unwrap();
    let replayed = use_case.apply_workflow_mutation(command).await.unwrap();

    assert_eq!(committed.workflow.revision.get(), 2);
    assert_eq!(replayed.workflow, committed.workflow);
    assert_eq!(committed.readiness, WorkflowReadinessResult::Ready);
}

#[tokio::test]
async fn mutation_rejects_a_stale_base_before_commit() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability = Arc::new(ReadinessCapabilityImpl { contract: contract(), issues: Vec::new() });
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let workflow = workflow_with_node(&registry, capability.node_capability_contract());
    repository.seed_workflow(workflow.clone());
    let command = WorkflowApplyMutationCommand::try_new(
        mutation_request_id(14),
        workflow.id,
        WorkflowRevision::new(2).unwrap(),
        vec![WorkflowMutationAction::MoveNode(WorkflowMoveNodeAction {
            node_id: node_id(11),
            canvas_position: WorkflowCanvasPosition::try_new(1.0, 1.0).unwrap(),
        })],
    )
    .unwrap();
    let error = WorkflowApplyMutationUseCase::new(repository.clone(), repository, registry)
        .apply_workflow_mutation(command)
        .await
        .unwrap_err();
    assert_eq!(error, WorkflowApplicationError::WorkflowRevisionConflict);
}

#[tokio::test]
async fn get_current_returns_typed_missing_project_identity() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let error = WorkflowGetCurrentUseCase::new(repository)
        .get_current_workflow(project_id(20))
        .await
        .unwrap_err();
    assert!(matches!(error, WorkflowApplicationError::WorkflowNotFound { .. }));
}

fn workflow_with_node(
    registry: &WorkflowNodeCapabilityRegistry,
    contract: &NodeCapabilityContract,
) -> WorkflowAggregate {
    WorkflowAggregate::try_restore(
        WorkflowAggregateRestoreData {
            schema_version: WorkflowSchemaVersion::CURRENT,
            id: workflow_id(21),
            project_id: project_id(22),
            revision: WorkflowRevision::new(1).unwrap(),
            created_at: WorkflowCreatedAt::from_utc_milliseconds(1).unwrap(),
            updated_at: WorkflowUpdatedAt::from_utc_milliseconds(1).unwrap(),
            nodes: vec![WorkflowNodeEntity {
                id: node_id(11),
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

fn contract() -> NodeCapabilityContract {
    NodeCapabilityContract::try_new(
        NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new("test.readiness").unwrap(),
            NodeCapabilityContractVersion::new(1, 0).unwrap(),
        ),
        Vec::new(),
        Vec::new(),
        vec![NodeCapabilityOutputContract::new(output_key(), WorkflowDataType::Text, true)],
        NodeCapabilityExecutionKind::PureValue,
    )
    .unwrap()
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
fn create_request_id(seed: u8) -> WorkflowCreateRequestId {
    WorkflowCreateRequestId::from_uuid(uuid(seed)).unwrap()
}
fn mutation_request_id(seed: u8) -> WorkflowMutationRequestId {
    WorkflowMutationRequestId::from_uuid(uuid(seed)).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
