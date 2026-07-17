use std::collections::BTreeMap;

use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
    NodeCapabilityGenerationProfileRefParameterValue,
    NodeCapabilityParameterCanonicalDecodeErrorCategory, NodeCapabilityParameterConstraint,
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

#[test]
fn parameter_set_round_trips_every_canonical_value_shape() {
    let asset_id = WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid_v4_bytes(9)).unwrap();
    let values = BTreeMap::from([
        (
            NodeCapabilityParameterKey::new("asset_id").unwrap(),
            NodeCapabilityParameterValue::ManagedAsset(
                engine::node_capability::NodeCapabilityManagedAssetIdParameterValue::new(asset_id),
            ),
        ),
        (
            NodeCapabilityParameterKey::new("choice").unwrap(),
            NodeCapabilityParameterValue::Choice(
                engine::node_capability::NodeCapabilityChoiceKey::new("wide").unwrap(),
            ),
        ),
        (
            NodeCapabilityParameterKey::new("count").unwrap(),
            NodeCapabilityParameterValue::UnsignedInteger(10),
        ),
        (
            NodeCapabilityParameterKey::new("profile").unwrap(),
            NodeCapabilityParameterValue::GenerationProfile(
                NodeCapabilityGenerationProfileRefParameterValue::new("image.standard", 2).unwrap(),
            ),
        ),
        (
            NodeCapabilityParameterKey::new("text").unwrap(),
            NodeCapabilityParameterValue::Text("hello".to_owned()),
        ),
    ]);
    let original = NodeCapabilityParameterSet::try_from_map(values).unwrap();

    let restored =
        NodeCapabilityParameterSet::try_from_canonical_bytes(&original.canonical_bytes()).unwrap();

    assert_eq!(restored, original);
}

#[test]
fn parameter_set_decoder_rejects_trailing_and_unknown_tag_bytes() {
    let original = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new("count").unwrap(),
        NodeCapabilityParameterValue::UnsignedInteger(10),
    )]))
    .unwrap();
    let mut trailing = original.canonical_bytes();
    trailing.push(0);
    let mut unknown_tag = original.canonical_bytes();
    unknown_tag[4 + 4 + "count".len()] = 99;

    assert_eq!(
        NodeCapabilityParameterSet::try_from_canonical_bytes(&trailing).unwrap_err().category(),
        NodeCapabilityParameterCanonicalDecodeErrorCategory::TrailingBytes
    );
    assert_eq!(
        NodeCapabilityParameterSet::try_from_canonical_bytes(&unknown_tag).unwrap_err().category(),
        NodeCapabilityParameterCanonicalDecodeErrorCategory::UnknownValueTag
    );
}

#[test]
fn parameter_set_decoder_rejects_noncanonical_key_order_and_oversized_input() {
    let values = BTreeMap::from([
        (
            NodeCapabilityParameterKey::new("alpha").unwrap(),
            NodeCapabilityParameterValue::UnsignedInteger(1),
        ),
        (
            NodeCapabilityParameterKey::new("bravo").unwrap(),
            NodeCapabilityParameterValue::UnsignedInteger(2),
        ),
    ]);
    let original = NodeCapabilityParameterSet::try_from_map(values).unwrap();
    let mut reversed = original.canonical_bytes();
    let first_entry_length = 4 + "alpha".len() + 1 + 8;
    let entries = reversed.split_off(4);
    reversed.extend_from_slice(&entries[first_entry_length..]);
    reversed.extend_from_slice(&entries[..first_entry_length]);

    assert_eq!(
        NodeCapabilityParameterSet::try_from_canonical_bytes(&reversed).unwrap_err().category(),
        NodeCapabilityParameterCanonicalDecodeErrorCategory::NonCanonicalKeyOrder
    );
    assert_eq!(
        NodeCapabilityParameterSet::try_from_canonical_bytes(&vec![0; 1_048_577])
            .unwrap_err()
            .category(),
        NodeCapabilityParameterCanonicalDecodeErrorCategory::InputTooLarge
    );
}

fn uuid_v4_bytes(seed: u8) -> [u8; 16] {
    [seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed]
}
