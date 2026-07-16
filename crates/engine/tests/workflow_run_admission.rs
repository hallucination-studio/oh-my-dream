use std::sync::Arc;

use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
    NodeCapabilityContractVersion, NodeCapabilityExecutionKind, NodeCapabilityInputBindingContract,
    NodeCapabilityInputContract, NodeCapabilityInputKey, NodeCapabilityOutputContract,
    NodeCapabilityOutputKey, NodeCapabilityParameterSet, NodeCapabilityReadinessIssue,
    WorkflowDataType, WorkflowInputItemId, WorkflowNodeCapabilityInterface,
    WorkflowNodeCapabilityRegistry,
};
use engine::workflow::{
    WorkflowApplicationError, WorkflowRunRepositoryInterface, WorkflowRunRequestId,
    WorkflowRunScope, WorkflowStartRunCommand, WorkflowStartRunUseCase,
};
use engine::workflow_graph::{
    WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowCanvasPosition, WorkflowCreatedAt,
    WorkflowId, WorkflowInputBinding, WorkflowInputItemEntity, WorkflowInputTarget,
    WorkflowNodeEntity, WorkflowNodeId, WorkflowRevision, WorkflowSchemaVersion, WorkflowUpdatedAt,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::workflow_interfaces::WorkflowContractFakeImpl;
use super::workflow_use_cases::ReadinessCapabilityImpl;

#[tokio::test]
async fn through_node_admits_only_ancestors_in_deterministic_order_and_replays() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability = Arc::new(ReadinessCapabilityImpl { contract: contract(), issues: Vec::new() });
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let workflow = branching_workflow(&registry, capability.node_capability_contract(), 1);
    repository.seed_workflow(workflow.clone());
    let use_case = WorkflowStartRunUseCase::new(
        repository.clone(),
        repository.clone(),
        repository.clone(),
        repository.clone(),
        registry,
    );
    let command = WorkflowStartRunCommand::new(
        run_request_id(10),
        workflow.id,
        workflow.revision,
        WorkflowRunScope::ThroughNode(node_id(3)),
    );
    let admitted = use_case.start_workflow_run(command).await.unwrap();

    assert_eq!(
        admitted.plan().nodes().iter().map(|node| node.node_id).collect::<Vec<_>>(),
        vec![node_id(1), node_id(3)]
    );
    assert!(repository.has_execute_effect(admitted.run_id()));
    let conflicting = WorkflowStartRunCommand::new(
        command.run_request_id(),
        command.workflow_id(),
        command.workflow_revision(),
        WorkflowRunScope::WholeWorkflow,
    );
    assert_eq!(
        use_case.start_workflow_run(conflicting).await.unwrap_err(),
        WorkflowApplicationError::WorkflowRunIdempotencyConflict
    );
    let restore_registry = WorkflowNodeCapabilityRegistry::try_new([
        capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
    ])
    .unwrap();
    repository.seed_workflow(branching_workflow(
        &restore_registry,
        capability.node_capability_contract(),
        2,
    ));
    let replayed = use_case.start_workflow_run(command).await.unwrap();
    assert_eq!(replayed.run_id(), admitted.run_id());
}

#[tokio::test]
async fn unavailable_scope_is_rejected_before_admission() {
    let repository = Arc::new(WorkflowContractFakeImpl::default());
    let capability = Arc::new(ReadinessCapabilityImpl {
        contract: contract(),
        issues: vec![NodeCapabilityReadinessIssue::invalid_capability_invocation()],
    });
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let workflow = branching_workflow(&registry, capability.node_capability_contract(), 1);
    repository.seed_workflow(workflow.clone());
    let command = WorkflowStartRunCommand::new(
        run_request_id(20),
        workflow.id,
        workflow.revision,
        WorkflowRunScope::WholeWorkflow,
    );
    let error = WorkflowStartRunUseCase::new(
        repository.clone(),
        repository.clone(),
        repository.clone(),
        repository.clone(),
        registry,
    )
    .start_workflow_run(command)
    .await
    .unwrap_err();

    assert!(matches!(error, WorkflowApplicationError::WorkflowNotReady { .. }));
    assert!(
        repository
            .load_workflow_run_admission_receipt(command.run_request_id())
            .await
            .unwrap()
            .is_none()
    );
}

#[test]
fn run_hash_excludes_request_id_and_includes_revision_and_scope() {
    let first = WorkflowStartRunCommand::new(
        run_request_id(30),
        workflow_id(31),
        WorkflowRevision::new(1).unwrap(),
        WorkflowRunScope::WholeWorkflow,
    );
    let replay = WorkflowStartRunCommand::new(
        run_request_id(32),
        workflow_id(31),
        WorkflowRevision::new(1).unwrap(),
        WorkflowRunScope::WholeWorkflow,
    );
    let through = WorkflowStartRunCommand::new(
        run_request_id(30),
        workflow_id(31),
        WorkflowRevision::new(1).unwrap(),
        WorkflowRunScope::ThroughNode(node_id(3)),
    );
    assert_eq!(first.command_hash(), replay.command_hash());
    assert_ne!(first.command_hash(), through.command_hash());
}

pub(crate) fn branching_workflow(
    registry: &WorkflowNodeCapabilityRegistry,
    contract: &NodeCapabilityContract,
    revision: u64,
) -> WorkflowAggregate {
    let nodes = [1, 2, 3].map(|seed| WorkflowNodeEntity {
        id: node_id(seed),
        capability_contract: contract.contract_ref().clone(),
        parameter_set: NodeCapabilityParameterSet::default(),
        canvas_position: WorkflowCanvasPosition::try_new(f64::from(seed), 0.0).unwrap(),
    });
    let item = WorkflowInputItemEntity {
        id: WorkflowInputItemId::from_uuid(uuid(4)).unwrap(),
        source_node_id: node_id(1),
        source_output_key: output_key(),
        input_role_key: None,
    };
    WorkflowAggregate::try_restore(
        WorkflowAggregateRestoreData {
            schema_version: WorkflowSchemaVersion::CURRENT,
            id: workflow_id(5),
            project_id: project_id(6),
            revision: WorkflowRevision::new(revision).unwrap(),
            created_at: WorkflowCreatedAt::from_utc_milliseconds(1).unwrap(),
            updated_at: WorkflowUpdatedAt::from_utc_milliseconds(i64::try_from(revision).unwrap())
                .unwrap(),
            nodes: nodes.into_iter().collect(),
            input_bindings: vec![(
                WorkflowInputTarget { node_id: node_id(3), input_key: input_key() },
                WorkflowInputBinding::try_single(item).unwrap(),
            )],
        },
        registry,
    )
    .unwrap()
}

pub(crate) fn contract() -> NodeCapabilityContract {
    NodeCapabilityContract::try_new(
        NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new("test.admission").unwrap(),
            NodeCapabilityContractVersion::new(1, 0).unwrap(),
        ),
        Vec::new(),
        vec![
            NodeCapabilityInputContract::new(
                input_key(),
                NodeCapabilityInputBindingContract::OptionalSingleValue {
                    data_type: WorkflowDataType::Text,
                },
            )
            .unwrap(),
        ],
        vec![NodeCapabilityOutputContract::new(output_key(), WorkflowDataType::Text, true)],
        NodeCapabilityExecutionKind::PureValue,
    )
    .unwrap()
}

fn input_key() -> NodeCapabilityInputKey {
    NodeCapabilityInputKey::new("source").unwrap()
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
