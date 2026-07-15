use std::collections::BTreeMap;

use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
    NodeCapabilityContractVersion, NodeCapabilityExecutionKind, NodeCapabilityInputBindingContract,
    NodeCapabilityInputContract, NodeCapabilityInputKey, NodeCapabilityInputRoleKey,
    NodeCapabilityOutputContract, NodeCapabilityOutputKey, WorkflowAcceptedDataTypeSet,
    WorkflowDataType, WorkflowInputItemId, WorkflowManagedAssetIdBoundaryValue,
    WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowNodeInputSet,
    WorkflowNodeInputValue, WorkflowNodeOutputSet, WorkflowRuntimeInputItem, WorkflowRuntimeValue,
    WorkflowTextPart, WorkflowTextValue,
};
use uuid::Uuid;

#[test]
fn structured_text_normalizes_literals_and_enforces_part_limit() {
    let reference_id = workflow_input_item_id(7);
    let text = WorkflowTextValue::try_new([
        WorkflowTextPart::Literal(String::new()),
        WorkflowTextPart::Literal("hello".to_owned()),
        WorkflowTextPart::Literal(" world".to_owned()),
        WorkflowTextPart::InputItemReference(reference_id),
    ])
    .unwrap();

    assert_eq!(
        text.parts(),
        &[
            WorkflowTextPart::Literal("hello world".to_owned()),
            WorkflowTextPart::InputItemReference(reference_id),
        ]
    );
    assert!(
        WorkflowTextValue::try_new((0..1_025).map(|_| WorkflowTextPart::Literal(String::new())))
            .is_err()
    );
}

#[test]
fn input_and_output_sets_require_exact_contract_shapes_and_types() {
    let input_key = NodeCapabilityInputKey::new("prompt").unwrap();
    let output_key = NodeCapabilityOutputKey::new("image").unwrap();
    let contract = NodeCapabilityContract::try_new(
        capability_ref("image.generate_from_text"),
        Vec::new(),
        vec![
            NodeCapabilityInputContract::new(
                input_key.clone(),
                NodeCapabilityInputBindingContract::RequiredSingleValue {
                    data_type: WorkflowDataType::Text,
                },
            )
            .unwrap(),
        ],
        vec![NodeCapabilityOutputContract::new(output_key.clone(), WorkflowDataType::Image, true)],
        NodeCapabilityExecutionKind::ContentGeneration,
    )
    .unwrap();
    let text = WorkflowRuntimeValue::Text(
        WorkflowTextValue::try_new([WorkflowTextPart::Literal("draw".to_owned())]).unwrap(),
    );
    let input_item = WorkflowRuntimeInputItem {
        input_item_id: workflow_input_item_id(8),
        input_role_key: None,
        value: text.clone(),
    };
    let image = WorkflowRuntimeValue::Image(WorkflowManagedImageRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid_v4_bytes(9)).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes([3; 32]),
    ));

    assert!(
        WorkflowNodeInputSet::try_new(
            &contract,
            BTreeMap::from([(input_key, WorkflowNodeInputValue::Single(input_item))]),
        )
        .is_ok()
    );
    assert!(
        WorkflowNodeOutputSet::try_new(&contract, BTreeMap::from([(output_key.clone(), image)]),)
            .is_ok()
    );
    assert!(
        WorkflowNodeOutputSet::try_new(&contract, BTreeMap::from([(output_key, text)])).is_err()
    );
}

#[test]
fn ordered_runtime_inputs_require_declared_roles_and_types() {
    let role = NodeCapabilityInputRoleKey::new("subject").unwrap();
    let input_key = NodeCapabilityInputKey::new("references").unwrap();
    let contract = NodeCapabilityContract::try_new(
        capability_ref("test.ordered_references"),
        Vec::new(),
        vec![
            NodeCapabilityInputContract::new(
                input_key.clone(),
                NodeCapabilityInputBindingContract::OrderedReferences {
                    minimum_items: 1,
                    maximum_items: Some(2),
                    accepted_data_types_by_role: BTreeMap::from([(
                        role.clone(),
                        WorkflowAcceptedDataTypeSet::try_new([WorkflowDataType::Image]).unwrap(),
                    )]),
                },
            )
            .unwrap(),
        ],
        vec![NodeCapabilityOutputContract::new(
            NodeCapabilityOutputKey::new("image").unwrap(),
            WorkflowDataType::Image,
            true,
        )],
        NodeCapabilityExecutionKind::ContentGeneration,
    )
    .unwrap();
    let item = WorkflowRuntimeInputItem {
        input_item_id: workflow_input_item_id(10),
        input_role_key: Some(role),
        value: WorkflowRuntimeValue::Image(WorkflowManagedImageRef::new(
            WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid_v4_bytes(11)).unwrap(),
            WorkflowManagedContentFingerprint::from_bytes([4; 32]),
        )),
    };

    assert!(
        WorkflowNodeInputSet::try_new(
            &contract,
            BTreeMap::from([(input_key, WorkflowNodeInputValue::OrderedReferences(vec![item]))]),
        )
        .is_ok()
    );
}

fn capability_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}

fn workflow_input_item_id(seed: u8) -> WorkflowInputItemId {
    WorkflowInputItemId::from_uuid(Uuid::from_bytes(uuid_v4_bytes(seed))).unwrap()
}

fn uuid_v4_bytes(seed: u8) -> [u8; 16] {
    [seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed]
}
