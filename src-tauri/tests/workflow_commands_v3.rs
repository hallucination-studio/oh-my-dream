use std::{collections::BTreeMap, sync::Arc};

use engine::{
    node_capability::{
        NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
        NodeCapabilityParameterKey, NodeCapabilityParameterSet, NodeCapabilityParameterValue,
    },
    workflow::{
        WorkflowCreateCommand, WorkflowCreateRequestId, WorkflowRunRequestId, WorkflowRunScope,
        WorkflowStartRunCommand,
    },
    workflow_graph::{
        WorkflowAddNodeAction, WorkflowApplyMutationCommand, WorkflowCanvasPosition,
        WorkflowMutationAction, WorkflowMutationRequestId, WorkflowNodeId,
    },
};
use oh_my_dream_tauri::{
    composition::{DesktopApplicationPaths, DesktopCompositionRoot},
    workflow_run_event_publisher::{DesktopEventEmissionError, DesktopEventEmitterInterface},
};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn canonical_workflow_slice_creates_mutates_admits_queries_and_cancels() {
    tauri::async_runtime::block_on(async {
        let directory = tempdir().unwrap();
        let dependencies = DesktopCompositionRoot::compose_activated_commands_with_emitter(
            DesktopApplicationPaths::from_application_data_root(directory.path()),
            Arc::new(TestEmitter),
        )
        .await
        .unwrap();
        let project = dependencies
            .create
            .create_project(projects::project::application::ProjectCreateRequest {
                request_id: projects::project::application::ProjectMutationRequestId::from_uuid(
                    id(1),
                )
                .unwrap(),
                name: projects::project::domain::ProjectName::new("V3").unwrap(),
            })
            .await
            .unwrap();
        let project_id = project.id();
        let workflow = dependencies
            .workflow_create
            .create_workflow(WorkflowCreateCommand::new(
                WorkflowCreateRequestId::from_uuid(id(3)).unwrap(),
                project_id,
            ))
            .await
            .unwrap();
        let node_id = WorkflowNodeId::from_uuid(id(4)).unwrap();
        let mut parameters = BTreeMap::new();
        parameters.insert(
            NodeCapabilityParameterKey::new("text").unwrap(),
            NodeCapabilityParameterValue::Text("hello".to_owned()),
        );
        let mutation = WorkflowApplyMutationCommand::try_new(
            WorkflowMutationRequestId::from_uuid(id(5)).unwrap(),
            workflow.id,
            workflow.revision,
            vec![WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
                new_node_id: node_id,
                capability_contract: NodeCapabilityContractRef::new(
                    NodeCapabilityContractId::new("text.provide_literal").unwrap(),
                    NodeCapabilityContractVersion::new(1, 0).unwrap(),
                ),
                parameter_set: NodeCapabilityParameterSet::try_from_map(parameters).unwrap(),
                canvas_position: WorkflowCanvasPosition::try_new(1.0, 2.0).unwrap(),
            })],
        )
        .unwrap();
        let changed = dependencies
            .workflow_apply_mutation
            .apply_project_workflow_mutation(project_id, mutation)
            .await
            .unwrap();
        let run = dependencies
            .workflow_start_run
            .start_project_workflow_run(
                project_id,
                WorkflowStartRunCommand::new(
                    WorkflowRunRequestId::from_uuid(id(6)).unwrap(),
                    changed.workflow.id,
                    changed.workflow.revision,
                    WorkflowRunScope::ThroughNode(node_id),
                ),
            )
            .await
            .unwrap();
        let loaded =
            dependencies.workflow_get_run.get_workflow_run(project_id, run.run_id()).await.unwrap();
        let events = dependencies
            .workflow_list_run_events
            .list_workflow_run_events(project_id, run.run_id(), None, 500)
            .await
            .unwrap();
        let other_project = dependencies
            .create
            .create_project(projects::project::application::ProjectCreateRequest {
                request_id: projects::project::application::ProjectMutationRequestId::from_uuid(
                    id(7),
                )
                .unwrap(),
                name: projects::project::domain::ProjectName::new("Other").unwrap(),
            })
            .await
            .unwrap();
        assert!(
            dependencies
                .workflow_check_readiness
                .check_project_workflow_readiness(other_project.id(), changed.workflow.id)
                .await
                .is_err()
        );
        assert!(
            dependencies
                .workflow_start_run
                .start_project_workflow_run(
                    other_project.id(),
                    WorkflowStartRunCommand::new(
                        WorkflowRunRequestId::from_uuid(id(8)).unwrap(),
                        changed.workflow.id,
                        changed.workflow.revision,
                        WorkflowRunScope::WholeWorkflow,
                    ),
                )
                .await
                .is_err()
        );
        assert!(
            dependencies
                .workflow_get_run
                .get_workflow_run(other_project.id(), run.run_id())
                .await
                .is_err()
        );
        assert!(
            dependencies
                .workflow_cancel_run
                .cancel_project_workflow_run(other_project.id(), run.run_id())
                .await
                .is_err()
        );
        assert_eq!(loaded.run_id(), run.run_id());
        assert_eq!(events.events.len(), 1);
        assert_eq!(
            dependencies
                .workflow_cancel_run
                .cancel_project_workflow_run(project_id, run.run_id())
                .await
                .unwrap()
                .state(),
            engine::workflow::WorkflowRunState::Cancelled
        );
    });
}

fn id(seed: u128) -> Uuid {
    Uuid::from_u128(0x123e_4567_e89b_42d3_a456_0000_0000_0000 | seed)
}

struct TestEmitter;

impl DesktopEventEmitterInterface for TestEmitter {
    fn emit_desktop_event(
        &self,
        _event_name: &str,
        _payload: serde_json::Value,
    ) -> Result<(), DesktopEventEmissionError> {
        Ok(())
    }
}
