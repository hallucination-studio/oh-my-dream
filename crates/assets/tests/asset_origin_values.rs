use assets::asset::domain::{
    AssetId, AssetImportId, AssetNodeOutputKey, AssetNodeOutputProduction, AssetOrigin,
    AssetOriginGenerationProfileRef, AssetOriginNodeCapabilityContractRef,
    AssetOriginNodeOutputKey, AssetOriginSourceAssetId, AssetOriginSourceAssetIds,
    AssetOriginWorkflowId, AssetOriginWorkflowNodeExecutionId, AssetOriginWorkflowNodeId,
    AssetOriginWorkflowRevision, AssetOriginWorkflowRunId, AssetOriginalFileName,
    AssetWorkflowNodeOrigin,
};
use uuid::Uuid;

#[test]
fn workflow_origin_integration_values_enforce_frozen_identity_grammars() {
    assert!(AssetOriginWorkflowId::from_uuid(Uuid::nil()).is_err());
    assert!(AssetOriginWorkflowRevision::new(0).is_err());
    assert!(AssetOriginNodeCapabilityContractRef::try_new("Image.Generate", 1, 0).is_err());
    assert!(AssetOriginNodeCapabilityContractRef::try_new("image.generate", 0, 0).is_err());
    assert!(AssetOriginNodeOutputKey::try_new("first-frame").is_err());
    assert!(AssetOriginGenerationProfileRef::try_new("image.high_quality", 0).is_err());
    assert!(AssetOriginGenerationProfileRef::try_new("image.high_quality", 1).is_ok());
}

#[test]
fn derived_source_ids_are_non_empty_and_preserve_duplicates_and_order() {
    assert!(AssetOriginSourceAssetIds::try_new(Vec::new()).is_err());
    let first = source_asset_id(1);
    let second = source_asset_id(2);
    let sources = AssetOriginSourceAssetIds::try_new(vec![first, second, first]).unwrap();
    assert_eq!(sources.as_slice(), [first, second, first]);
}

#[test]
fn workflow_output_origin_requires_output_key_to_match_its_producer() {
    let producer = producer(3, 4);
    let mismatched_key = AssetNodeOutputKey::new(
        AssetOriginWorkflowRunId::from_uuid(uuid(9)).unwrap(),
        producer.node_execution_id(),
        AssetOriginNodeOutputKey::try_new("image").unwrap(),
        0,
    );
    assert!(
        AssetOrigin::workflow_node_output(
            producer.clone(),
            AssetNodeOutputProduction::ProviderGenerated {
                generation_profile_ref: AssetOriginGenerationProfileRef::try_new(
                    "image.high_quality",
                    1,
                )
                .unwrap(),
            },
            mismatched_key,
        )
        .is_err()
    );

    let matching_key = AssetNodeOutputKey::new(
        producer.workflow_run_id(),
        producer.node_execution_id(),
        AssetOriginNodeOutputKey::try_new("image").unwrap(),
        0,
    );
    assert!(
        AssetOrigin::workflow_node_output(
            producer,
            AssetNodeOutputProduction::DeterministicDerived {
                source_asset_ids: AssetOriginSourceAssetIds::try_new(vec![source_asset_id(1)])
                    .unwrap(),
            },
            matching_key,
        )
        .is_ok()
    );
}

#[test]
fn imported_origin_retains_only_import_identity_and_final_file_name() {
    let origin = AssetOrigin::imported(
        AssetImportId::from_uuid(uuid(7)).unwrap(),
        AssetOriginalFileName::try_new("source.png").unwrap(),
    );
    assert!(matches!(origin, AssetOrigin::Imported(_)));
}

fn producer(run_seed: u8, execution_seed: u8) -> AssetWorkflowNodeOrigin {
    AssetWorkflowNodeOrigin::new(
        AssetOriginWorkflowId::from_uuid(uuid(1)).unwrap(),
        AssetOriginWorkflowRevision::new(1).unwrap(),
        AssetOriginWorkflowRunId::from_uuid(uuid(run_seed)).unwrap(),
        AssetOriginWorkflowNodeId::from_uuid(uuid(2)).unwrap(),
        AssetOriginWorkflowNodeExecutionId::from_uuid(uuid(execution_seed)).unwrap(),
        AssetOriginNodeCapabilityContractRef::try_new("image.generate", 1, 0).unwrap(),
    )
}

fn source_asset_id(seed: u8) -> AssetOriginSourceAssetId {
    AssetOriginSourceAssetId::from_asset_id(AssetId::from_uuid(uuid(seed)).unwrap())
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
