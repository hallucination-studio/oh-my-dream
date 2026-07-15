use engine::node_capability::{
    NodeCapabilityExecutionKind, WorkflowDataType, WorkflowNodeCapabilityInterface,
};
use nodes::{
    NodeCapabilityManagedMediaReaderFakeImpl, ProvideLiteralTextCapabilityImpl,
    ReadAudioAssetCapabilityImpl, ReadImageAssetCapabilityImpl, ReadVideoAssetCapabilityImpl,
};

#[test]
fn c3_capabilities_publish_the_four_frozen_contracts() {
    let literal = ProvideLiteralTextCapabilityImpl::try_new().unwrap();
    assert_contract(
        &literal,
        "text.provide_literal",
        WorkflowDataType::Text,
        NodeCapabilityExecutionKind::PureValue,
    );
    let image =
        ReadImageAssetCapabilityImpl::try_new(NodeCapabilityManagedMediaReaderFakeImpl::default())
            .unwrap();
    assert_contract(
        &image,
        "image.read_asset",
        WorkflowDataType::Image,
        NodeCapabilityExecutionKind::ManagedAssetRead,
    );
    let video =
        ReadVideoAssetCapabilityImpl::try_new(NodeCapabilityManagedMediaReaderFakeImpl::default())
            .unwrap();
    assert_contract(
        &video,
        "video.read_asset",
        WorkflowDataType::Video,
        NodeCapabilityExecutionKind::ManagedAssetRead,
    );
    let audio =
        ReadAudioAssetCapabilityImpl::try_new(NodeCapabilityManagedMediaReaderFakeImpl::default())
            .unwrap();
    assert_contract(
        &audio,
        "audio.read_asset",
        WorkflowDataType::Audio,
        NodeCapabilityExecutionKind::ManagedAssetRead,
    );
}

fn assert_contract(
    capability: &impl WorkflowNodeCapabilityInterface,
    expected_id: &str,
    expected_output: WorkflowDataType,
    expected_kind: NodeCapabilityExecutionKind,
) {
    let contract = capability.node_capability_contract();
    assert_eq!(contract.contract_ref().id().as_str(), expected_id);
    assert_eq!(contract.contract_ref().version().major(), 1);
    assert_eq!(contract.contract_ref().version().minor(), 0);
    assert_eq!(contract.inputs(), []);
    assert_eq!(contract.outputs().len(), 1);
    assert_eq!(contract.outputs()[0].data_type(), expected_output);
    assert_eq!(contract.execution_kind(), expected_kind);
}
