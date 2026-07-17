use std::collections::BTreeMap;

use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
    NodeCapabilityContractVersion, NodeCapabilityExecutionError, NodeCapabilityExecutionKind,
    NodeCapabilityInputBindingContract, NodeCapabilityInputContract, NodeCapabilityInputKey,
    NodeCapabilityOutputContract, NodeCapabilityOutputKey, NodeCapabilityParameterConstraint,
    NodeCapabilityParameterContract, NodeCapabilityParameterKey, NodeCapabilityParameterSet,
    NodeCapabilityParameterValue, WorkflowDataType, WorkflowNodeExecutionId, WorkflowNodeOutputSet,
    WorkflowRunId, WorkflowRuntimeValue, WorkflowTextPart, WorkflowTextValue,
};
use engine::workflow::{
    WorkflowDomainError, WorkflowExecutionPlan, WorkflowNodeExecutionFailure,
    WorkflowNodeExecutionRestoreData, WorkflowNodeExecutionState, WorkflowPlannedNode,
    WorkflowReadinessIssue, WorkflowReadinessPolicy, WorkflowReadinessResult, WorkflowRunAggregate,
    WorkflowRunRestoreData, WorkflowRunScope, WorkflowRunState, WorkflowRunTime,
    WorkflowStructuralReadinessNode,
};
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use projects::project::domain::ProjectId;
use uuid::Uuid;

#[test]
fn run_scope_keeps_whole_and_exact_through_node_variants() {
    let node_id = workflow_node_id(1);
    assert_eq!(WorkflowRunScope::WholeWorkflow.selected_node_id(), None);
    assert_eq!(WorkflowRunScope::ThroughNode(node_id).selected_node_id(), Some(node_id));
}

#[test]
fn queued_run_starts_nodes_and_cancels_with_monotonic_events() {
    let node_execution_id = node_execution_id(2);
    let mut run = queued_run(
        run_id(3),
        project_id(4),
        workflow_id(5),
        WorkflowRunScope::WholeWorkflow,
        vec![(workflow_node_id(6), node_execution_id)],
        time(100),
    )
    .unwrap();

    assert_eq!(run.state(), WorkflowRunState::Queued);
    assert_eq!(run.events().len(), 1);
    assert_eq!(run.events()[0].sequence().get(), 1);
    run.start(time(101)).unwrap();
    run.start_node(node_execution_id, time(102)).unwrap();
    run.progress_node(node_execution_id, 4_000, time(103)).unwrap();
    run.cancel(time(104)).unwrap();

    assert_eq!(run.state(), WorkflowRunState::Cancelled);
    assert_eq!(run.node_executions()[0].state(), WorkflowNodeExecutionState::Cancelled);
    assert_eq!(run.events().last().unwrap().sequence().get(), 6);
}

#[test]
fn run_rejects_illegal_and_regressing_transitions() {
    let node_execution_id = node_execution_id(10);
    let mut run = queued_run(
        run_id(11),
        project_id(12),
        workflow_id(13),
        WorkflowRunScope::WholeWorkflow,
        vec![(workflow_node_id(14), node_execution_id)],
        time(100),
    )
    .unwrap();

    assert_eq!(
        run.start_node(node_execution_id, time(101)).unwrap_err(),
        WorkflowDomainError::WorkflowIllegalNodeExecutionTransition
    );
    run.start(time(102)).unwrap();
    run.start_node(node_execution_id, time(103)).unwrap();
    run.progress_node(node_execution_id, 4_000, time(104)).unwrap();
    assert_eq!(
        run.progress_node(node_execution_id, 3_999, time(105)).unwrap_err(),
        WorkflowDomainError::WorkflowProgressRegression
    );
    run.cancel(time(106)).unwrap();
    assert_eq!(
        run.start(time(107)).unwrap_err(),
        WorkflowDomainError::WorkflowTerminalStateImmutable
    );
}

#[test]
fn readiness_reports_missing_structure_in_frozen_order() {
    let contract = required_contract();
    let parameters = NodeCapabilityParameterSet::default();
    let result = WorkflowReadinessPolicy::check(&[WorkflowStructuralReadinessNode {
        node_id: workflow_node_id(20),
        contract: &contract,
        parameters: &parameters,
        input_bindings: &[],
    }]);

    let WorkflowReadinessResult::Blocked { issues } = result else { panic!("expected blocked") };
    assert!(matches!(issues[0], WorkflowReadinessIssue::WorkflowRequiredParameterMissing { .. }));
    assert!(matches!(issues[1], WorkflowReadinessIssue::WorkflowRequiredInputMissing { .. }));
}

#[test]
fn complete_outputs_are_required_by_type_before_node_and_run_success() {
    let contract = required_contract();
    let execution_id = node_execution_id(21);
    let mut run = queued_run(
        run_id(22),
        project_id(23),
        workflow_id(24),
        WorkflowRunScope::WholeWorkflow,
        vec![(workflow_node_id(25), execution_id)],
        time(1),
    )
    .unwrap();
    run.start(time(2)).unwrap();
    run.start_node(execution_id, time(3)).unwrap();
    let outputs = WorkflowNodeOutputSet::try_new(
        &contract,
        BTreeMap::from([(
            output_key(),
            WorkflowRuntimeValue::Text(
                WorkflowTextValue::try_new([WorkflowTextPart::Literal("ok".into())]).unwrap(),
            ),
        )]),
    )
    .unwrap();
    run.succeed_node(execution_id, outputs, time(4)).unwrap();
    run.finish(time(5)).unwrap();

    assert_eq!(run.state(), WorkflowRunState::Succeeded);
    assert!(run.node_executions()[0].outputs().is_some());
    assert_eq!(run.events().last().unwrap().sequence().get(), 5);
}

#[test]
fn execution_plan_rejects_non_deterministic_ready_node_order() {
    let contract = required_contract();
    let parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new("prompt").unwrap(),
        NodeCapabilityParameterValue::Text("hello".into()),
    )]))
    .unwrap();
    let normalized = contract.normalize_node_parameters(&parameters).unwrap();
    let node = |seed| WorkflowPlannedNode {
        node_id: workflow_node_id(seed),
        node_execution_id: node_execution_id(seed + 20),
        capability_contract: contract.contract_ref().clone(),
        normalized_parameters: normalized.clone(),
        input_bindings: Vec::new(),
    };

    let error = WorkflowExecutionPlan::try_new(
        workflow_id(30),
        WorkflowRevision::new(1).unwrap(),
        WorkflowRunScope::WholeWorkflow,
        vec![node(2), node(1)],
    )
    .unwrap_err();
    assert_eq!(error, WorkflowDomainError::InvalidWorkflowExecutionPlan);
    assert!(
        WorkflowExecutionPlan::try_new(
            workflow_id(30),
            WorkflowRevision::new(1).unwrap(),
            WorkflowRunScope::WholeWorkflow,
            vec![node(1), node(2)],
        )
        .is_ok()
    );
}

#[test]
fn failed_nodes_block_descendants_and_finish_with_sorted_failure_ids() {
    let contract = required_contract();
    let failed_node_id = workflow_node_id(40);
    let failed_execution_id = node_execution_id(41);
    let blocked_execution_id = node_execution_id(42);
    let mut run = queued_run(
        run_id(43),
        project_id(44),
        workflow_id(45),
        WorkflowRunScope::WholeWorkflow,
        vec![(failed_node_id, failed_execution_id), (workflow_node_id(46), blocked_execution_id)],
        time(1),
    )
    .unwrap();
    run.start(time(2)).unwrap();
    run.start_node(failed_execution_id, time(3)).unwrap();
    run.fail_node(
        failed_execution_id,
        WorkflowNodeExecutionFailure {
            capability_error: NodeCapabilityExecutionError::invalid_capability_invocation(
                contract.contract_ref().clone(),
                failed_execution_id,
            ),
        },
        time(4),
    )
    .unwrap();
    run.block_node(blocked_execution_id, vec![failed_node_id], time(5)).unwrap();
    run.finish(time(6)).unwrap();

    assert_eq!(run.state(), WorkflowRunState::Failed);
    assert!(run.node_executions()[1].block_reason().is_some());
}

#[test]
fn rejected_backdated_transition_does_not_mutate_run_state() {
    let mut run = queued_run(
        run_id(50),
        project_id(51),
        workflow_id(52),
        WorkflowRunScope::WholeWorkflow,
        vec![(workflow_node_id(53), node_execution_id(54))],
        time(100),
    )
    .unwrap();

    assert_eq!(run.start(time(99)).unwrap_err(), WorkflowDomainError::InvalidWorkflowRunValue);
    assert_eq!(run.state(), WorkflowRunState::Queued);
    assert_eq!(run.events().len(), 1);
}

#[test]
fn external_completion_handoff_is_idempotent_and_rejected_for_mismatched_or_terminal_nodes() {
    let execution_id = node_execution_id(57);
    let mut run = queued_run(
        run_id(55),
        project_id(56),
        workflow_id(57),
        WorkflowRunScope::WholeWorkflow,
        vec![(workflow_node_id(58), execution_id)],
        time(100),
    )
    .unwrap();
    run.start(time(101)).unwrap();
    run.start_node(execution_id, time(102)).unwrap();

    run.wait_node_for_external_completion(execution_id, time(103)).unwrap();
    let event_count = run.events().len();
    run.wait_node_for_external_completion(execution_id, time(103)).unwrap();

    assert_eq!(run.events().len(), event_count);
    assert_eq!(
        run.node_executions()[0].state(),
        WorkflowNodeExecutionState::WaitingForExternalCompletion
    );
    assert_eq!(
        run.wait_node_for_external_completion(node_execution_id(59), time(104)),
        Err(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition)
    );
    assert_eq!(run.finish(time(104)), Err(WorkflowDomainError::WorkflowIllegalRunTransition));

    run.cancel(time(105)).unwrap();
    assert_eq!(
        run.wait_node_for_external_completion(execution_id, time(106)),
        Err(WorkflowDomainError::WorkflowTerminalStateImmutable)
    );
}

#[test]
fn restore_rejects_outcome_fields_inconsistent_with_node_state() {
    let run = queued_run(
        run_id(60),
        project_id(61),
        workflow_id(62),
        WorkflowRunScope::WholeWorkflow,
        vec![(workflow_node_id(63), node_execution_id(64))],
        time(100),
    )
    .unwrap();
    let restored = WorkflowRunAggregate::try_restore(WorkflowRunRestoreData {
        run_id: run.run_id(),
        project_id: run.project_id(),
        plan: run.plan().clone(),
        state: run.state(),
        node_executions: vec![WorkflowNodeExecutionRestoreData {
            node_id: workflow_node_id(63),
            execution_id: node_execution_id(64),
            state: WorkflowNodeExecutionState::Pending,
            progress_basis_points: Some(1),
            started_at: None,
            finished_at: None,
            outputs: None,
            failure: None,
            block_reason: None,
        }],
        events: run.events().to_vec(),
        created_at: run.created_at(),
        updated_at: run.updated_at(),
        failure: None,
    });

    assert!(matches!(restored, Err(WorkflowDomainError::InvalidWorkflowRunValue)));
}

pub(crate) fn queued_run(
    run_id: WorkflowRunId,
    project_id: ProjectId,
    workflow_id: WorkflowId,
    scope: WorkflowRunScope,
    planned_nodes: Vec<(WorkflowNodeId, WorkflowNodeExecutionId)>,
    created_at: WorkflowRunTime,
) -> Result<WorkflowRunAggregate, WorkflowDomainError> {
    let contract = required_contract();
    let supplied = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new("prompt").unwrap(),
        NodeCapabilityParameterValue::Text("planned".into()),
    )]))
    .unwrap();
    let normalized = contract.normalize_node_parameters(&supplied).unwrap();
    let nodes = planned_nodes
        .into_iter()
        .map(|(node_id, node_execution_id)| WorkflowPlannedNode {
            node_id,
            node_execution_id,
            capability_contract: contract.contract_ref().clone(),
            normalized_parameters: normalized.clone(),
            input_bindings: Vec::new(),
        })
        .collect();
    let plan = WorkflowExecutionPlan::try_new(
        workflow_id,
        WorkflowRevision::new(1).unwrap(),
        scope,
        nodes,
    )?;
    WorkflowRunAggregate::try_new_queued(run_id, project_id, plan, created_at)
}

fn required_contract() -> NodeCapabilityContract {
    NodeCapabilityContract::try_new(
        NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new("test.required").unwrap(),
            NodeCapabilityContractVersion::new(1, 0).unwrap(),
        ),
        vec![NodeCapabilityParameterContract::required(
            NodeCapabilityParameterKey::new("prompt").unwrap(),
            NodeCapabilityParameterConstraint::text_utf8_bytes(1, 100).unwrap(),
        )],
        vec![
            NodeCapabilityInputContract::new(
                NodeCapabilityInputKey::new("source").unwrap(),
                NodeCapabilityInputBindingContract::RequiredSingleValue {
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

fn output_key() -> NodeCapabilityOutputKey {
    NodeCapabilityOutputKey::new("result").unwrap()
}

fn time(value: i64) -> WorkflowRunTime {
    WorkflowRunTime::from_utc_milliseconds(value).unwrap()
}

fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}

fn workflow_id(seed: u8) -> WorkflowId {
    WorkflowId::from_uuid(uuid(seed)).unwrap()
}

fn workflow_node_id(seed: u8) -> WorkflowNodeId {
    WorkflowNodeId::from_uuid(uuid(seed)).unwrap()
}

fn run_id(seed: u8) -> WorkflowRunId {
    WorkflowRunId::from_uuid(uuid(seed)).unwrap()
}

fn node_execution_id(seed: u8) -> WorkflowNodeExecutionId {
    WorkflowNodeExecutionId::from_uuid(uuid(seed)).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
