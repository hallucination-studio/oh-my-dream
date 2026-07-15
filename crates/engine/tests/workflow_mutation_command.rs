use std::collections::BTreeMap;

use engine::{
    node_capability::{
        NodeCapabilityInputKey, NodeCapabilityInputRoleKey, NodeCapabilityOutputKey,
        NodeCapabilityParameterSet, WorkflowInputItemId,
    },
    workflow_graph::{
        WorkflowAddNodeAction, WorkflowApplyMutationCommand, WorkflowBindSingleInputAction,
        WorkflowCanvasPosition, WorkflowId, WorkflowInputItemEntity, WorkflowInputTarget,
        WorkflowMutationAction, WorkflowMutationRequestId, WorkflowNodeId, WorkflowRevision,
    },
};
use uuid::Uuid;

#[test]
fn command_requires_between_one_and_one_hundred_twenty_eight_actions() {
    assert!(
        WorkflowApplyMutationCommand::try_new(
            request_id(1),
            workflow_id(2),
            WorkflowRevision::new(1).unwrap(),
            Vec::new(),
        )
        .is_err()
    );

    let too_many = (0..129).map(|_| add_node_action(3)).collect::<Vec<_>>();
    assert!(
        WorkflowApplyMutationCommand::try_new(
            request_id(1),
            workflow_id(2),
            WorkflowRevision::new(1).unwrap(),
            too_many,
        )
        .is_err()
    );
}

#[test]
fn command_rejects_a_role_bearing_single_input_item() {
    let action = WorkflowMutationAction::BindSingleInput(WorkflowBindSingleInputAction {
        target: WorkflowInputTarget {
            node_id: node_id(3),
            input_key: NodeCapabilityInputKey::new("input").unwrap(),
        },
        new_item: WorkflowInputItemEntity {
            id: WorkflowInputItemId::from_uuid(uuid(4)).unwrap(),
            source_node_id: node_id(5),
            source_output_key: NodeCapabilityOutputKey::new("output").unwrap(),
            input_role_key: Some(NodeCapabilityInputRoleKey::new("subject").unwrap()),
        },
    });
    assert!(
        WorkflowApplyMutationCommand::try_new(
            request_id(1),
            workflow_id(2),
            WorkflowRevision::new(1).unwrap(),
            vec![action],
        )
        .is_err()
    );
}

#[test]
fn command_hash_excludes_request_id_but_includes_action_order() {
    let first = WorkflowApplyMutationCommand::try_new(
        request_id(1),
        workflow_id(2),
        WorkflowRevision::new(1).unwrap(),
        vec![add_node_action(3), add_node_action(4)],
    )
    .unwrap();
    let replay = WorkflowApplyMutationCommand::try_new(
        request_id(9),
        workflow_id(2),
        WorkflowRevision::new(1).unwrap(),
        vec![add_node_action(3), add_node_action(4)],
    )
    .unwrap();
    let reordered = WorkflowApplyMutationCommand::try_new(
        request_id(1),
        workflow_id(2),
        WorkflowRevision::new(1).unwrap(),
        vec![add_node_action(4), add_node_action(3)],
    )
    .unwrap();

    assert_eq!(first.command_hash(), replay.command_hash());
    assert_ne!(first.command_hash(), reordered.command_hash());
}

fn add_node_action(seed: u8) -> WorkflowMutationAction {
    WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
        new_node_id: WorkflowNodeId::from_uuid(uuid(seed)).unwrap(),
        capability_contract: engine::node_capability::NodeCapabilityContractRef::new(
            engine::node_capability::NodeCapabilityContractId::new("image.generate").unwrap(),
            engine::node_capability::NodeCapabilityContractVersion::new(1, 0).unwrap(),
        ),
        parameter_set: NodeCapabilityParameterSet::try_from_map(BTreeMap::new()).unwrap(),
        canvas_position: WorkflowCanvasPosition::try_new(seed.into(), 0.0).unwrap(),
    })
}

fn request_id(seed: u8) -> WorkflowMutationRequestId {
    WorkflowMutationRequestId::from_uuid(uuid(seed)).unwrap()
}

fn workflow_id(seed: u8) -> WorkflowId {
    WorkflowId::from_uuid(uuid(seed)).unwrap()
}

fn node_id(seed: u8) -> WorkflowNodeId {
    WorkflowNodeId::from_uuid(uuid(seed)).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
