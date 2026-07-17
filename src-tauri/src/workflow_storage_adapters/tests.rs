use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use engine::{
    node_capability::{
        NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
        NodeCapabilityContractVersion, NodeCapabilityExecutionKind, NodeCapabilityOutputContract,
        NodeCapabilityOutputKey, NodeCapabilityParameterSet, WorkflowDataType,
        WorkflowNodeCapabilityInterface, WorkflowNodeCapabilityRegistry, WorkflowNodeExecutionId,
        WorkflowNodeOutputSet, WorkflowRunId, WorkflowRuntimeValue, WorkflowTextPart,
        WorkflowTextValue,
    },
    workflow::{
        WorkflowAggregateRepositoryInterface, WorkflowCreateCommandHash, WorkflowCreateReceipt,
        WorkflowCreateRequestId, WorkflowCreationCommit, WorkflowExecuteRunEffect,
        WorkflowExecutionPlan, WorkflowLoadKey, WorkflowMutationCommit, WorkflowPlannedNode,
        WorkflowRunAdmissionCommit, WorkflowRunAggregate, WorkflowRunLoadKey,
        WorkflowRunRepositoryInterface, WorkflowRunRequestId, WorkflowRunScope, WorkflowRunState,
        WorkflowRunTime, WorkflowStartRunCommand,
    },
    workflow_graph::{
        WorkflowAddNodeAction, WorkflowAggregate, WorkflowAggregateRestoreData,
        WorkflowApplyMutationCommand, WorkflowCanvasPosition, WorkflowCreatedAt, WorkflowId,
        WorkflowMutationAction, WorkflowMutationReceipt, WorkflowMutationRequestId, WorkflowNodeId,
        WorkflowRevision, WorkflowSchemaVersion, WorkflowUpdatedAt,
    },
};
use nodes::ProvideLiteralTextCapabilityImpl;
use projects::project::domain::ProjectId;
use rusqlite::Connection;
use uuid::Uuid;

use super::SqliteWorkflowRunRepositoryAdapterImpl;
use crate::post_commit_effect::SqliteDesktopPostCommitEffectOutboxAdapterImpl;

#[tokio::test]
async fn atomically_creates_and_loads_exact_workflow_snapshot_and_receipt() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let capabilities = Arc::new(WorkflowNodeCapabilityRegistry::try_new([]).unwrap());
    let adapter =
        SqliteWorkflowRunRepositoryAdapterImpl::try_new(connection, capabilities).unwrap();
    let workflow = empty_workflow();
    let request_id = WorkflowCreateRequestId::from_uuid(id(3)).unwrap();
    let receipt = WorkflowCreateReceipt::new(
        request_id,
        WorkflowCreateCommandHash::from_bytes([7; 32]),
        workflow.clone(),
    )
    .unwrap();

    let committed = adapter
        .commit_workflow_creation(
            WorkflowCreationCommit::try_new(workflow.clone(), receipt.clone()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(committed, workflow);
    assert_eq!(
        adapter.load_workflow(WorkflowLoadKey::Project(workflow.project_id)).await.unwrap(),
        Some(workflow)
    );
    assert_eq!(adapter.load_workflow_creation_receipt(request_id).await.unwrap(), Some(receipt));
}

#[tokio::test]
async fn compare_and_swaps_workflow_snapshot_with_exact_mutation_receipt() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let capability = Arc::new(ProvideLiteralTextCapabilityImpl::try_new().unwrap());
    let capabilities =
        Arc::new(WorkflowNodeCapabilityRegistry::try_new([capability.clone() as Arc<_>]).unwrap());
    let adapter =
        SqliteWorkflowRunRepositoryAdapterImpl::try_new(connection, Arc::clone(&capabilities))
            .unwrap();
    let workflow = empty_workflow();
    let creation_receipt = WorkflowCreateReceipt::new(
        WorkflowCreateRequestId::from_uuid(id(30)).unwrap(),
        WorkflowCreateCommandHash::from_bytes([9; 32]),
        workflow.clone(),
    )
    .unwrap();
    adapter
        .commit_workflow_creation(
            WorkflowCreationCommit::try_new(workflow.clone(), creation_receipt).unwrap(),
        )
        .await
        .unwrap();
    let command = WorkflowApplyMutationCommand::try_new(
        WorkflowMutationRequestId::from_uuid(id(31)).unwrap(),
        workflow.id,
        workflow.revision,
        vec![WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
            new_node_id: WorkflowNodeId::from_uuid(id(32)).unwrap(),
            capability_contract: capability.node_capability_contract().contract_ref().clone(),
            parameter_set: NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
                engine::node_capability::NodeCapabilityParameterKey::new("text").unwrap(),
                engine::node_capability::NodeCapabilityParameterValue::Text("hello".into()),
            )]))
            .unwrap(),
            canvas_position: WorkflowCanvasPosition::try_new(1.0, 2.0).unwrap(),
        })],
    )
    .unwrap();
    let mutated = workflow
        .apply_mutation_command(
            &command,
            WorkflowUpdatedAt::from_utc_milliseconds(11).unwrap(),
            &capabilities,
        )
        .unwrap();
    let receipt = WorkflowMutationReceipt::new(&command, mutated.clone());

    let committed = adapter
        .commit_workflow_mutation(
            WorkflowMutationCommit::try_new(mutated.clone(), workflow.revision, receipt.clone())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(committed, receipt);
    assert_eq!(
        adapter.load_workflow(WorkflowLoadKey::Workflow(workflow.id)).await.unwrap(),
        Some(mutated)
    );
}

#[tokio::test]
async fn atomically_admits_transitions_and_restores_complete_run_outputs_and_events() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let capabilities = Arc::new(WorkflowNodeCapabilityRegistry::try_new([]).unwrap());
    let adapter =
        SqliteWorkflowRunRepositoryAdapterImpl::try_new(Arc::clone(&connection), capabilities)
            .unwrap();
    let contract = output_contract();
    let execution_id = WorkflowNodeExecutionId::from_uuid(id(10)).unwrap();
    let run_id = WorkflowRunId::from_uuid(id(11)).unwrap();
    let node_id = engine::workflow_graph::WorkflowNodeId::from_uuid(id(13)).unwrap();
    let plan = WorkflowExecutionPlan::try_new(
        WorkflowId::from_uuid(id(12)).unwrap(),
        WorkflowRevision::new(1).unwrap(),
        WorkflowRunScope::WholeWorkflow,
        vec![WorkflowPlannedNode {
            node_id,
            node_execution_id: execution_id,
            capability_contract: contract.contract_ref().clone(),
            normalized_parameters: contract
                .normalize_node_parameters(
                    &NodeCapabilityParameterSet::try_from_map(BTreeMap::new()).unwrap(),
                )
                .unwrap(),
            input_bindings: vec![],
        }],
    )
    .unwrap();
    let mut run = WorkflowRunAggregate::try_new_queued(
        run_id,
        ProjectId::from_uuid(id(14)).unwrap(),
        plan,
        WorkflowRunTime::from_utc_milliseconds(1).unwrap(),
    )
    .unwrap();
    let request_id = WorkflowRunRequestId::from_uuid(id(15)).unwrap();
    let command = WorkflowStartRunCommand::new(
        request_id,
        run.workflow_id(),
        run.workflow_revision(),
        run.scope(),
    );
    let receipt = engine::workflow::WorkflowRunAdmissionReceipt::new(command, run_id);
    adapter
        .admit_workflow_run(
            WorkflowRunAdmissionCommit::try_new(
                run.clone(),
                receipt.clone(),
                WorkflowExecuteRunEffect { workflow_run_id: run_id },
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        adapter.list_active_project_workflow_runs(run.project_id(), 32).await.unwrap().len(),
        1
    );
    assert_eq!(adapter.list_active_workflow_run_ids_after(None, 100).await.unwrap(), vec![run_id]);
    let queued_event = adapter.list_undelivered_workflow_run_events(10).await.unwrap();
    assert_eq!(queued_event.len(), 1);
    adapter
        .record_workflow_run_event_delivery_attempt(run_id, queued_event[0].sequence(), true)
        .await
        .unwrap();

    run.start(WorkflowRunTime::from_utc_milliseconds(2).unwrap()).unwrap();
    run.start_node(execution_id, WorkflowRunTime::from_utc_milliseconds(3).unwrap()).unwrap();
    run.wait_node_for_external_completion(
        execution_id,
        WorkflowRunTime::from_utc_milliseconds(4).unwrap(),
    )
    .unwrap();
    adapter.commit_workflow_run_transition(run.clone(), 1).await.unwrap();
    let waiting =
        adapter.load_workflow_run(WorkflowRunLoadKey::Run(run_id)).await.unwrap().unwrap();
    assert_eq!(
        waiting.node_executions()[0].state(),
        engine::workflow::WorkflowNodeExecutionState::WaitingForExternalCompletion
    );
    let outputs = WorkflowNodeOutputSet::try_new(
        &contract,
        BTreeMap::from([(
            NodeCapabilityOutputKey::new("text").unwrap(),
            WorkflowRuntimeValue::Text(
                WorkflowTextValue::try_new([WorkflowTextPart::Literal("done".into())]).unwrap(),
            ),
        )]),
    )
    .unwrap();
    run.succeed_node(execution_id, outputs, WorkflowRunTime::from_utc_milliseconds(5).unwrap())
        .unwrap();
    run.finish(WorkflowRunTime::from_utc_milliseconds(6).unwrap()).unwrap();

    adapter.commit_workflow_run_transition(run.clone(), 4).await.unwrap();
    assert!(
        adapter.list_active_project_workflow_runs(run.project_id(), 32).await.unwrap().is_empty()
    );
    assert!(adapter.list_active_workflow_run_ids_after(None, 100).await.unwrap().is_empty());
    let restored =
        adapter.load_workflow_run(WorkflowRunLoadKey::Run(run_id)).await.unwrap().unwrap();
    let latest = adapter
        .load_latest_workflow_run_for_node(run.project_id(), run.workflow_id(), node_id)
        .await
        .unwrap()
        .unwrap();
    let events = adapter.list_workflow_run_events_after(run_id, None, 10).await.unwrap();
    let events_after_three = adapter
        .list_workflow_run_events_after(
            run_id,
            Some(engine::workflow::WorkflowRunEventSequence::new(3).unwrap()),
            10,
        )
        .await
        .unwrap();

    assert_eq!(restored.state(), WorkflowRunState::Succeeded);
    assert_eq!(latest.run_id(), run_id);
    assert_eq!(restored.node_executions()[0].outputs(), run.node_executions()[0].outputs());
    assert_eq!(events, run.events());
    assert_eq!(events_after_three, run.events()[3..]);
    let pending_events = adapter.list_undelivered_workflow_run_events(10).await.unwrap();
    assert_eq!(pending_events.len(), 5);
    adapter
        .record_workflow_run_event_delivery_attempt(run_id, pending_events[0].sequence(), false)
        .await
        .unwrap();
    assert_eq!(adapter.list_undelivered_workflow_run_events(10).await.unwrap().len(), 5);
    adapter
        .record_workflow_run_event_delivery_attempt(run_id, pending_events[0].sequence(), true)
        .await
        .unwrap();
    assert_eq!(adapter.list_undelivered_workflow_run_events(10).await.unwrap().len(), 4);
    let output_row_count: i64 = connection
        .lock()
        .unwrap()
        .query_row("SELECT count(*) FROM workflow_node_execution_outputs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(output_row_count, 1);
    assert_eq!(
        adapter.load_workflow_run_admission_receipt(request_id).await.unwrap(),
        Some(receipt)
    );
    assert_eq!(
        adapter.commit_workflow_run_transition(run, 1).await.unwrap_err(),
        engine::workflow::WorkflowApplicationError::WorkflowPersistenceFailure,
    );
    connection.lock().unwrap().execute("DELETE FROM workflow_node_execution_outputs", []).unwrap();
    assert_eq!(
        adapter.load_workflow_run(WorkflowRunLoadKey::Run(run_id)).await.unwrap_err(),
        engine::workflow::WorkflowApplicationError::WorkflowDomain(
            engine::workflow::WorkflowDomainError::InvalidWorkflowRunValue,
        )
    );
}

#[tokio::test]
async fn rolls_back_run_and_receipt_when_atomic_effect_insert_fails() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let capabilities = Arc::new(WorkflowNodeCapabilityRegistry::try_new([]).unwrap());
    let adapter =
        SqliteWorkflowRunRepositoryAdapterImpl::try_new(connection, capabilities).unwrap();
    let contract = output_contract();
    let execution_id = WorkflowNodeExecutionId::from_uuid(id(20)).unwrap();
    let run_id = WorkflowRunId::from_uuid(id(21)).unwrap();
    let plan = WorkflowExecutionPlan::try_new(
        WorkflowId::from_uuid(id(22)).unwrap(),
        WorkflowRevision::new(1).unwrap(),
        WorkflowRunScope::WholeWorkflow,
        vec![WorkflowPlannedNode {
            node_id: engine::workflow_graph::WorkflowNodeId::from_uuid(id(23)).unwrap(),
            node_execution_id: execution_id,
            capability_contract: contract.contract_ref().clone(),
            normalized_parameters: contract
                .normalize_node_parameters(
                    &NodeCapabilityParameterSet::try_from_map(BTreeMap::new()).unwrap(),
                )
                .unwrap(),
            input_bindings: vec![],
        }],
    )
    .unwrap();
    let run = WorkflowRunAggregate::try_new_queued(
        run_id,
        ProjectId::from_uuid(id(24)).unwrap(),
        plan,
        WorkflowRunTime::from_utc_milliseconds(1).unwrap(),
    )
    .unwrap();
    let request_id = WorkflowRunRequestId::from_uuid(id(25)).unwrap();
    let command = WorkflowStartRunCommand::new(
        request_id,
        run.workflow_id(),
        run.workflow_revision(),
        run.scope(),
    );
    let receipt = engine::workflow::WorkflowRunAdmissionReceipt::new(command, run_id);

    let error = adapter
        .admit_workflow_run(
            WorkflowRunAdmissionCommit::try_new(
                run,
                receipt,
                WorkflowExecuteRunEffect { workflow_run_id: run_id },
            )
            .unwrap(),
        )
        .await
        .unwrap_err();

    assert_eq!(error, engine::workflow::WorkflowApplicationError::WorkflowPersistenceFailure);
    assert!(adapter.load_workflow_run(WorkflowRunLoadKey::Run(run_id)).await.unwrap().is_none());
    assert!(adapter.load_workflow_run_admission_receipt(request_id).await.unwrap().is_none());
}

fn empty_workflow() -> WorkflowAggregate {
    WorkflowAggregate::try_restore(
        WorkflowAggregateRestoreData {
            schema_version: WorkflowSchemaVersion::CURRENT,
            id: WorkflowId::from_uuid(id(1)).unwrap(),
            project_id: ProjectId::from_uuid(id(2)).unwrap(),
            revision: WorkflowRevision::new(1).unwrap(),
            created_at: WorkflowCreatedAt::from_utc_milliseconds(10).unwrap(),
            updated_at: WorkflowUpdatedAt::from_utc_milliseconds(10).unwrap(),
            nodes: vec![],
            input_bindings: vec![],
        },
        &WorkflowNodeCapabilityRegistry::try_new([]).unwrap(),
    )
    .unwrap()
}

fn output_contract() -> NodeCapabilityContract {
    NodeCapabilityContract::try_new(
        NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new("test.output").unwrap(),
            NodeCapabilityContractVersion::new(1, 0).unwrap(),
        ),
        vec![],
        vec![],
        vec![NodeCapabilityOutputContract::new(
            NodeCapabilityOutputKey::new("text").unwrap(),
            WorkflowDataType::Text,
            true,
        )],
        NodeCapabilityExecutionKind::PureValue,
    )
    .unwrap()
}

fn id(value: u128) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{value:012x}")).unwrap()
}
