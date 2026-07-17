use engine::{
    node_capability::{NodeCapabilityInputRoleKey, NodeCapabilityOutputKey, WorkflowInputItemId},
    workflow_graph::{
        WorkflowCanvasPosition, WorkflowId, WorkflowInputBinding, WorkflowInputItemEntity,
        WorkflowNodeId, WorkflowOrderedInputItems, WorkflowRevision, WorkflowSchemaVersion,
    },
};
use uuid::Uuid;

#[test]
fn frozen_graph_values_reject_invalid_identity_revision_schema_and_position() {
    assert!(WorkflowId::from_uuid(Uuid::nil()).is_err());
    assert!(WorkflowRevision::new(0).is_err());
    assert!(WorkflowSchemaVersion::new(2).is_err());
    assert!(WorkflowCanvasPosition::try_new(f64::NAN, 0.0).is_err());
    assert!(WorkflowCanvasPosition::try_new(1_000_000.1, 0.0).is_err());

    let position = WorkflowCanvasPosition::try_new(-0.0, -0.0).unwrap();
    assert_eq!(position.x().to_bits(), 0.0_f64.to_bits());
    assert_eq!(position.y().to_bits(), 0.0_f64.to_bits());
}

#[test]
fn binding_shape_owns_role_presence_and_preserves_stable_item_order() {
    let first = item(1, Some("subject"));
    let second = item(2, Some("style"));
    let ordered = WorkflowOrderedInputItems::try_new(vec![first.clone(), second.clone()]).unwrap();
    let binding = WorkflowInputBinding::ordered_references(ordered);
    let ids = binding.items().map(|item| item.id).collect::<Vec<_>>();
    assert_eq!(ids, [first.id, second.id]);

    assert!(WorkflowInputBinding::try_single(first).is_err());
    assert!(WorkflowOrderedInputItems::try_new(vec![item(3, None)]).is_err());
    assert!(WorkflowOrderedInputItems::try_new(Vec::new()).is_err());
}

fn item(seed: u8, role: Option<&str>) -> WorkflowInputItemEntity {
    WorkflowInputItemEntity {
        id: WorkflowInputItemId::from_uuid(Uuid::from_bytes(uuid_v4_bytes(seed))).unwrap(),
        source_node_id: WorkflowNodeId::from_uuid(Uuid::from_bytes(uuid_v4_bytes(seed + 10)))
            .unwrap(),
        source_output_key: NodeCapabilityOutputKey::new("output").unwrap(),
        input_role_key: role.map(|value| NodeCapabilityInputRoleKey::new(value).unwrap()),
    }
}

fn uuid_v4_bytes(seed: u8) -> [u8; 16] {
    [seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed]
}
