use engine::workflow_graph::{
    WorkflowAddNodeAction, WorkflowApplyMutationCommand, WorkflowInputBinding,
    WorkflowMutationAction, WorkflowMutationReceipt, WorkflowMutationRequestId,
    WorkflowOrderedInputItems, WorkflowRevision, WorkflowUpdatedAt,
};

mod workflow_mutation_apply_support;
use workflow_mutation_apply_support::*;

#[test]
fn empty_workflow_result_fingerprint_matches_the_frozen_bytes() {
    let registry = registry();
    let command = command(vec![WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
        new_node_id: node_id(3),
        capability_contract: capability_ref("image.source"),
        parameter_set: empty_parameters(),
        canvas_position: position(0.0),
    })]);
    let receipt = WorkflowMutationReceipt::new(&command, empty_aggregate(&registry));
    assert_eq!(
        receipt.result_fingerprint().as_bytes(),
        [
            71, 206, 111, 5, 105, 134, 81, 79, 92, 178, 110, 30, 128, 197, 89, 108, 29, 254, 248,
            186, 116, 15, 154, 109, 138, 90, 158, 144, 113, 40, 37, 210,
        ]
    );
}

#[test]
fn matching_replay_returns_the_exact_committed_snapshot() {
    let registry = registry();
    let aggregate = empty_aggregate(&registry);
    let command = command(vec![WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
        new_node_id: node_id(3),
        capability_contract: capability_ref("image.source"),
        parameter_set: empty_parameters(),
        canvas_position: position(0.0),
    })]);
    let committed = aggregate
        .apply_mutation_command(
            &command,
            WorkflowUpdatedAt::from_utc_milliseconds(100).unwrap(),
            &registry,
        )
        .unwrap();
    let receipt = WorkflowMutationReceipt::new(&command, committed.clone());

    assert_eq!(receipt.replay_matching_command(&command).unwrap(), &committed);
    assert_eq!(receipt.committed_workflow().revision.get(), 2);
}

#[test]
fn replay_rejects_reused_request_with_different_content() {
    let registry = registry();
    let aggregate = empty_aggregate(&registry);
    let first = command(vec![WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
        new_node_id: node_id(3),
        capability_contract: capability_ref("image.source"),
        parameter_set: empty_parameters(),
        canvas_position: position(0.0),
    })]);
    let different = WorkflowApplyMutationCommand::try_new(
        first.request_id(),
        first.workflow_id(),
        WorkflowRevision::new(1).unwrap(),
        vec![WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
            new_node_id: node_id(4),
            capability_contract: capability_ref("image.source"),
            parameter_set: empty_parameters(),
            canvas_position: position(0.0),
        })],
    )
    .unwrap();
    let committed = aggregate
        .apply_mutation_command(
            &first,
            WorkflowUpdatedAt::from_utc_milliseconds(100).unwrap(),
            &registry,
        )
        .unwrap();
    let receipt = WorkflowMutationReceipt::new(&first, committed);

    assert!(receipt.replay_matching_command(&different).is_err());
}

#[test]
fn restoring_a_corrupt_result_fingerprint_is_a_persistence_failure() {
    let registry = registry();
    let aggregate = empty_aggregate(&registry);
    let command = command(vec![WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
        new_node_id: node_id(3),
        capability_contract: capability_ref("image.source"),
        parameter_set: empty_parameters(),
        canvas_position: position(0.0),
    })]);
    let committed = aggregate
        .apply_mutation_command(
            &command,
            WorkflowUpdatedAt::from_utc_milliseconds(100).unwrap(),
            &registry,
        )
        .unwrap();

    assert!(
        WorkflowMutationReceipt::try_restore(
            WorkflowMutationRequestId::from_uuid(command.request_id().as_uuid()).unwrap(),
            command.command_hash(),
            committed,
            [0; 32],
        )
        .is_err()
    );
}

#[test]
fn ordered_reference_item_order_changes_the_result_fingerprint() {
    let registry = registry();
    let first_source = node(10, "image.source");
    let second_source = node(11, "image.source");
    let reference_target = node(13, "image.references");
    let first_item = input_item(20, first_source.id, Some("subject"));
    let second_item = input_item(21, second_source.id, Some("style"));
    let nodes = vec![first_source, second_source, reference_target.clone()];
    let first_order = restore_aggregate(
        &registry,
        nodes.clone(),
        vec![(
            target(reference_target.id, "references"),
            WorkflowInputBinding::ordered_references(
                WorkflowOrderedInputItems::try_new(vec![first_item.clone(), second_item.clone()])
                    .unwrap(),
            ),
        )],
    );
    let second_order = restore_aggregate(
        &registry,
        nodes,
        vec![(
            target(reference_target.id, "references"),
            WorkflowInputBinding::ordered_references(
                WorkflowOrderedInputItems::try_new(vec![second_item, first_item]).unwrap(),
            ),
        )],
    );
    let command = command(vec![WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
        new_node_id: node_id(3),
        capability_contract: capability_ref("image.source"),
        parameter_set: empty_parameters(),
        canvas_position: position(0.0),
    })]);

    assert_ne!(
        WorkflowMutationReceipt::new(&command, first_order).result_fingerprint(),
        WorkflowMutationReceipt::new(&command, second_order).result_fingerprint()
    );
}
