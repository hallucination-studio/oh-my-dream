use engine::{
    node_capability::NodeCapabilityInputRoleKey,
    workflow_graph::{
        WorkflowAddNodeAction, WorkflowBindSingleInputAction, WorkflowInputBinding,
        WorkflowInsertReferenceItemAction, WorkflowMoveNodeAction, WorkflowMoveReferenceItemAction,
        WorkflowMutationAction, WorkflowOrderedInputItems, WorkflowRemoveInputItemAction,
        WorkflowRemoveNodeAction, WorkflowReplaceNodeParametersAction,
        WorkflowSelectNodeCapabilityAction, WorkflowSetInputItemRoleAction, WorkflowUpdatedAt,
    },
};

mod workflow_mutation_apply_support;
use workflow_mutation_apply_support::*;

#[test]
fn successful_actions_advance_revision_and_monotonic_updated_time_once() {
    let registry = registry();
    let aggregate = empty_aggregate(&registry);
    let new_node_id = node_id(3);
    let command = command(vec![
        WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
            new_node_id,
            capability_contract: capability_ref("image.source"),
            parameter_set: empty_parameters(),
            canvas_position: position(0.0),
        }),
        WorkflowMutationAction::MoveNode(WorkflowMoveNodeAction {
            node_id: new_node_id,
            canvas_position: position(42.0),
        }),
    ]);

    let updated = aggregate
        .apply_mutation_command(
            &command,
            WorkflowUpdatedAt::from_utc_milliseconds(99).unwrap(),
            &registry,
        )
        .unwrap();

    assert_eq!(updated.revision.get(), 2);
    assert_eq!(updated.updated_at.as_utc_milliseconds(), 101);
    assert_eq!(updated.nodes()[&new_node_id].canvas_position.x(), 42.0);
}

#[test]
fn a_later_failed_action_leaves_the_original_aggregate_unchanged() {
    let registry = registry();
    let aggregate = empty_aggregate(&registry);
    let before = aggregate.clone();
    let command = command(vec![
        WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
            new_node_id: node_id(3),
            capability_contract: capability_ref("image.source"),
            parameter_set: empty_parameters(),
            canvas_position: position(0.0),
        }),
        WorkflowMutationAction::RemoveNode(WorkflowRemoveNodeAction { node_id: node_id(9) }),
    ]);

    assert!(
        aggregate
            .apply_mutation_command(
                &command,
                WorkflowUpdatedAt::from_utc_milliseconds(200).unwrap(),
                &registry,
            )
            .is_err()
    );
    assert_eq!(aggregate, before);
}

#[test]
fn complete_candidate_validation_rejects_a_cycle_without_changing_the_aggregate() {
    let registry = registry();
    let first = node(12, "image.single");
    let second = node(15, "image.single");
    let aggregate = restore_aggregate(&registry, vec![first.clone(), second.clone()], Vec::new());
    let before = aggregate.clone();
    let command = command(vec![
        WorkflowMutationAction::BindSingleInput(WorkflowBindSingleInputAction {
            target: target(first.id, "input"),
            new_item: input_item(30, second.id, None),
        }),
        WorkflowMutationAction::BindSingleInput(WorkflowBindSingleInputAction {
            target: target(second.id, "input"),
            new_item: input_item(31, first.id, None),
        }),
    ]);

    let result = aggregate.apply_mutation_command(
        &command,
        WorkflowUpdatedAt::from_utc_milliseconds(101).unwrap(),
        &registry,
    );

    assert_eq!(result.unwrap_err(), engine::workflow_graph::WorkflowGraphError::Cycle);
    assert_eq!(aggregate, before);
}

#[test]
fn capability_selection_never_silently_removes_an_existing_binding() {
    let registry = registry();
    let source = node(10, "image.source");
    let target_node = node(12, "image.single");
    let aggregate = restore_aggregate(
        &registry,
        vec![source.clone(), target_node.clone()],
        vec![(
            target(target_node.id, "input"),
            WorkflowInputBinding::try_single(input_item(23, source.id, None)).unwrap(),
        )],
    );
    let before = aggregate.clone();
    let command = command(vec![WorkflowMutationAction::SelectNodeCapability(
        WorkflowSelectNodeCapabilityAction {
            node_id: target_node.id,
            capability_contract: capability_ref("image.source"),
            parameter_set: empty_parameters(),
        },
    )]);

    assert_eq!(
        aggregate
            .apply_mutation_command(
                &command,
                WorkflowUpdatedAt::from_utc_milliseconds(101).unwrap(),
                &registry,
            )
            .unwrap_err(),
        engine::workflow_graph::WorkflowGraphError::InputNotFound
    );
    assert_eq!(aggregate, before);
}

#[test]
fn all_ten_actions_apply_in_order_and_preserve_reference_identity() {
    let registry = registry();
    let source = node(10, "image.source");
    let second_source = node(11, "image.source");
    let single_target = node(12, "image.single");
    let reference_target = node(13, "image.references");
    let original_reference = input_item(20, source.id, Some("subject"));
    let aggregate = restore_aggregate(
        &registry,
        vec![
            source.clone(),
            second_source.clone(),
            single_target.clone(),
            reference_target.clone(),
        ],
        vec![(
            target(reference_target.id, "references"),
            WorkflowInputBinding::ordered_references(
                WorkflowOrderedInputItems::try_new(vec![original_reference.clone()]).unwrap(),
            ),
        )],
    );
    let temporary_node_id = node_id(14);
    let inserted_reference = input_item(22, second_source.id, Some("subject"));
    let actions = vec![
        WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
            new_node_id: temporary_node_id,
            capability_contract: capability_ref("image.source"),
            parameter_set: empty_parameters(),
            canvas_position: position(0.0),
        }),
        WorkflowMutationAction::ReplaceNodeParameters(WorkflowReplaceNodeParametersAction {
            node_id: temporary_node_id,
            parameter_set: empty_parameters(),
        }),
        WorkflowMutationAction::SelectNodeCapability(WorkflowSelectNodeCapabilityAction {
            node_id: temporary_node_id,
            capability_contract: capability_ref("image.source"),
            parameter_set: empty_parameters(),
        }),
        WorkflowMutationAction::MoveNode(WorkflowMoveNodeAction {
            node_id: temporary_node_id,
            canvas_position: position(8.0),
        }),
        WorkflowMutationAction::BindSingleInput(WorkflowBindSingleInputAction {
            target: target(single_target.id, "input"),
            new_item: input_item(21, source.id, None),
        }),
        WorkflowMutationAction::InsertReferenceItem(WorkflowInsertReferenceItemAction {
            target: target(reference_target.id, "references"),
            new_item: inserted_reference.clone(),
            insertion_index: 1,
        }),
        WorkflowMutationAction::MoveReferenceItem(WorkflowMoveReferenceItemAction {
            target: target(reference_target.id, "references"),
            input_item_id: inserted_reference.id,
            insertion_index_after_removal: 0,
        }),
        WorkflowMutationAction::SetInputItemRole(WorkflowSetInputItemRoleAction {
            target: target(reference_target.id, "references"),
            input_item_id: inserted_reference.id,
            input_role_key: NodeCapabilityInputRoleKey::new("style").unwrap(),
        }),
        WorkflowMutationAction::RemoveInputItem(WorkflowRemoveInputItemAction {
            target: target(reference_target.id, "references"),
            input_item_id: original_reference.id,
        }),
        WorkflowMutationAction::RemoveNode(WorkflowRemoveNodeAction { node_id: temporary_node_id }),
    ];

    let updated = aggregate
        .apply_mutation_command(
            &command(actions),
            WorkflowUpdatedAt::from_utc_milliseconds(100).unwrap(),
            &registry,
        )
        .unwrap();

    assert!(!updated.nodes().contains_key(&temporary_node_id));
    let reference_binding = &updated.input_bindings()[&target(reference_target.id, "references")];
    let remaining = reference_binding.items().collect::<Vec<_>>();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, inserted_reference.id);
    assert_eq!(remaining[0].input_role_key.as_ref().unwrap().as_str(), "style");
    assert!(updated.input_bindings().contains_key(&target(single_target.id, "input")));
}
