use std::collections::BTreeMap;

use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
    NodeCapabilityGenerationProfileRefParameterValue, NodeCapabilityParameterConstraint,
    NodeCapabilityParameterErrorCategory, NodeCapabilityParameterKey, NodeCapabilityParameterSet,
    NodeCapabilityParameterValue, WorkflowDataType, WorkflowInputItemId,
    WorkflowManagedAssetIdBoundaryValue,
};
use uuid::Uuid;

#[test]
fn contract_identity_rejects_ambiguous_names_and_zero_major_versions() {
    assert!(NodeCapabilityContractId::new("image.generate_from_text").is_ok());
    assert!(NodeCapabilityContractId::new("ImageToVideo").is_err());
    assert!(NodeCapabilityContractId::new("image").is_err());
    assert!(NodeCapabilityContractVersion::new(0, 1).is_err());
    let contract_ref = NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new("image.generate_from_text").unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    );
    assert_eq!(contract_ref.to_string(), "image.generate_from_text@1.0");
}

#[test]
fn workflow_identity_accepts_only_rfc_uuid_version_four() {
    assert!(WorkflowInputItemId::from_uuid(Uuid::from_bytes(uuid_v4_bytes(1))).is_some());
    assert!(WorkflowInputItemId::from_uuid(Uuid::nil()).is_none());
}

#[test]
fn cross_module_boundary_values_validate_only_canonical_identity_shapes() {
    assert!(NodeCapabilityGenerationProfileRefParameterValue::new("image.standard", 1).is_ok());
    assert!(NodeCapabilityGenerationProfileRefParameterValue::new("Image", 1).is_err());
    assert!(WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid_v4_bytes(2)).is_ok());
    assert!(WorkflowManagedAssetIdBoundaryValue::from_bytes([0; 16]).is_err());
}

#[test]
fn parameter_constraints_reject_wrong_kinds_and_out_of_bounds_values() {
    let constraint =
        NodeCapabilityParameterConstraint::unsigned_integer_allowed_values([5, 10]).unwrap();

    assert!(
        constraint
            .validate_parameter_value(&NodeCapabilityParameterValue::UnsignedInteger(5))
            .is_ok()
    );
    assert_eq!(
        constraint.validate_parameter_value(&NodeCapabilityParameterValue::UnsignedInteger(6)),
        Err(NodeCapabilityParameterErrorCategory::ParameterValueOutOfBounds)
    );
    assert_eq!(
        constraint.validate_parameter_value(&NodeCapabilityParameterValue::Text("5".to_owned())),
        Err(NodeCapabilityParameterErrorCategory::ParameterValueKindMismatch)
    );
    assert!(NodeCapabilityParameterConstraint::managed_asset_id(WorkflowDataType::Text).is_err());
}

#[test]
fn raw_parameter_set_rejects_more_than_sixty_four_values() {
    let values = (0..65)
        .map(|index| {
            (
                NodeCapabilityParameterKey::new(format!("parameter_{index}")).unwrap(),
                NodeCapabilityParameterValue::UnsignedInteger(index),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let error = NodeCapabilityParameterSet::try_from_map(values).unwrap_err();

    assert_eq!(error.category(), NodeCapabilityParameterErrorCategory::ParameterSetTooLarge);
}

fn uuid_v4_bytes(seed: u8) -> [u8; 16] {
    [seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed]
}
