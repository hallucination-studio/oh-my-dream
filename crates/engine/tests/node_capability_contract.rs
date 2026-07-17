use std::collections::BTreeMap;

use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
    NodeCapabilityContractVersion, NodeCapabilityExecutionKind, NodeCapabilityInputBindingContract,
    NodeCapabilityInputContract, NodeCapabilityInputKey, NodeCapabilityOutputContract,
    NodeCapabilityOutputKey, NodeCapabilityParameterConstraint, NodeCapabilityParameterContract,
    NodeCapabilityParameterKey, NodeCapabilityParameterSet, NodeCapabilityParameterValue,
    WorkflowAcceptedDataTypeSet, WorkflowDataType,
};

#[test]
fn parameter_normalization_inserts_declared_defaults() {
    let text_key = parameter_key("text");
    let duration_key = parameter_key("duration_seconds");
    let contract = NodeCapabilityContract::try_new(
        capability_ref("video.generate_from_image"),
        vec![
            NodeCapabilityParameterContract::required(
                text_key.clone(),
                NodeCapabilityParameterConstraint::text_utf8_bytes(1, 64).unwrap(),
            ),
            NodeCapabilityParameterContract::optional_with_default(
                duration_key.clone(),
                NodeCapabilityParameterConstraint::unsigned_integer_allowed_values([5, 10])
                    .unwrap(),
                NodeCapabilityParameterValue::UnsignedInteger(5),
            )
            .unwrap(),
        ],
        Vec::new(),
        vec![primary_output("video", WorkflowDataType::Video)],
        NodeCapabilityExecutionKind::MediaTransformation,
    )
    .unwrap();
    let supplied = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        text_key,
        NodeCapabilityParameterValue::Text("hello".to_owned()),
    )]))
    .unwrap();

    let normalized = contract.normalize_node_parameters(&supplied).unwrap();

    assert_eq!(
        normalized.get(&duration_key),
        Some(&NodeCapabilityParameterValue::UnsignedInteger(5))
    );
}

#[test]
fn normalized_parameter_bytes_follow_ascending_key_order() {
    let alpha = parameter_key("alpha");
    let beta = parameter_key("beta");
    let contract = NodeCapabilityContract::try_new(
        capability_ref("text.provide_literal"),
        vec![
            NodeCapabilityParameterContract::required(
                beta.clone(),
                NodeCapabilityParameterConstraint::text_utf8_bytes(1, 16).unwrap(),
            ),
            NodeCapabilityParameterContract::required(
                alpha.clone(),
                NodeCapabilityParameterConstraint::unsigned_integer_range(1, 9).unwrap(),
            ),
        ],
        Vec::new(),
        vec![primary_output("text", WorkflowDataType::Text)],
        NodeCapabilityExecutionKind::PureValue,
    )
    .unwrap();
    let supplied = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([
        (beta, NodeCapabilityParameterValue::Text("value".to_owned())),
        (alpha, NodeCapabilityParameterValue::UnsignedInteger(3)),
    ]))
    .unwrap();

    let bytes = contract.normalize_node_parameters(&supplied).unwrap().canonical_bytes();

    assert_eq!(&bytes[..4], 2_u32.to_be_bytes());
    assert_eq!(&bytes[8..13], b"alpha");
}

#[test]
fn contract_rejects_invalid_output_and_direct_constraint_definitions() {
    let invalid_output = NodeCapabilityContract::try_new(
        capability_ref("text.provide_literal"),
        Vec::new(),
        Vec::new(),
        vec![
            primary_output("text", WorkflowDataType::Text),
            primary_output("duplicate", WorkflowDataType::Text),
        ],
        NodeCapabilityExecutionKind::PureValue,
    );
    let invalid_constraint = NodeCapabilityContract::try_new(
        capability_ref("text.provide_literal"),
        vec![NodeCapabilityParameterContract::required(
            parameter_key("count"),
            NodeCapabilityParameterConstraint::UnsignedIntegerRange { minimum: 10, maximum: 5 },
        )],
        Vec::new(),
        vec![primary_output("text", WorkflowDataType::Text)],
        NodeCapabilityExecutionKind::PureValue,
    );

    assert!(invalid_output.is_err());
    assert!(invalid_constraint.is_err());
}

#[test]
fn contract_rejects_duplicate_parameter_keys() {
    let duplicate_key = parameter_key("text");
    let result = NodeCapabilityContract::try_new(
        capability_ref("text.provide_literal"),
        vec![
            NodeCapabilityParameterContract::required(
                duplicate_key.clone(),
                NodeCapabilityParameterConstraint::text_utf8_bytes(1, 64).unwrap(),
            ),
            NodeCapabilityParameterContract::required(
                duplicate_key,
                NodeCapabilityParameterConstraint::text_utf8_bytes(1, 64).unwrap(),
            ),
        ],
        Vec::new(),
        vec![primary_output("text", WorkflowDataType::Text)],
        NodeCapabilityExecutionKind::PureValue,
    );

    assert!(result.is_err());
}

#[test]
fn ordered_reference_contract_requires_roles_with_non_text_types() {
    let role = engine::node_capability::NodeCapabilityInputRoleKey::new("subject").unwrap();
    let accepted = WorkflowAcceptedDataTypeSet::try_new([WorkflowDataType::Image]).unwrap();
    let input = NodeCapabilityInputContract::new(
        NodeCapabilityInputKey::new("references").unwrap(),
        NodeCapabilityInputBindingContract::OrderedReferences {
            minimum_items: 1,
            maximum_items: Some(4),
            accepted_data_types_by_role: BTreeMap::from([(role, accepted)]),
        },
    );

    assert!(input.is_ok());
    assert!(WorkflowAcceptedDataTypeSet::try_new([WorkflowDataType::Text]).is_err());
}

fn capability_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}

fn parameter_key(value: &str) -> NodeCapabilityParameterKey {
    NodeCapabilityParameterKey::new(value).unwrap()
}

fn primary_output(key: &str, data_type: WorkflowDataType) -> NodeCapabilityOutputContract {
    NodeCapabilityOutputContract::new(NodeCapabilityOutputKey::new(key).unwrap(), data_type, true)
}
