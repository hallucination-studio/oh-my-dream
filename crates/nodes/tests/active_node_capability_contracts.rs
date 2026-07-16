use std::sync::Arc;

use engine::node_capability::*;
use nodes::*;

mod c5_support;
use c5_support::GenerationProfileAlwaysAvailableFakeImpl;

#[test]
fn c5_test_fixture_contains_only_the_frozen_seven_exact_contracts() {
    let implementations = active_capability_test_fixture();
    let registry = WorkflowNodeCapabilityRegistry::try_new(implementations.clone()).unwrap();
    let expected = expected_contracts();

    assert_eq!(registry.list_node_capability_contracts(), expected.iter().collect::<Vec<_>>());
    for contract in &expected {
        assert_eq!(
            registry
                .resolve_node_capability(contract.contract_ref())
                .unwrap()
                .node_capability_contract(),
            contract
        );
    }

    let duplicate = implementations[0].clone();
    assert!(matches!(
        WorkflowNodeCapabilityRegistry::try_new(
            implementations.into_iter().chain(std::iter::once(duplicate))
        ),
        Err(NodeCapabilityRegistryError::DuplicateContractRef { .. })
    ));
    assert!(matches!(
        registry.resolve_node_capability(&contract_ref("video.generate_from_text")),
        Err(NodeCapabilityRegistryError::ContractNotRegistered { .. })
    ));
}

fn active_capability_test_fixture() -> Vec<Arc<dyn WorkflowNodeCapabilityInterface>> {
    let catalog = Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap());
    vec![
        Arc::new(ProvideLiteralTextCapabilityImpl::try_new().unwrap()),
        Arc::new(
            ReadImageAssetCapabilityImpl::try_new(
                NodeCapabilityManagedMediaReaderFakeImpl::default(),
            )
            .unwrap(),
        ),
        Arc::new(
            ReadVideoAssetCapabilityImpl::try_new(
                NodeCapabilityManagedMediaReaderFakeImpl::default(),
            )
            .unwrap(),
        ),
        Arc::new(
            ReadAudioAssetCapabilityImpl::try_new(
                NodeCapabilityManagedMediaReaderFakeImpl::default(),
            )
            .unwrap(),
        ),
        Arc::new(
            TextToImageCapabilityImpl::try_new(
                catalog.clone(),
                GenerationProfileAlwaysAvailableFakeImpl,
                TextToImageProviderFakeImpl::try_new().unwrap(),
                NodeCapabilityProducedMediaWriterFakeImpl::default(),
            )
            .unwrap(),
        ),
        Arc::new(
            ImageToVideoCapabilityImpl::try_new(
                catalog.clone(),
                GenerationProfileAlwaysAvailableFakeImpl,
                NodeCapabilityManagedMediaReaderFakeImpl::default(),
                ImageToVideoProviderFakeImpl::try_new().unwrap(),
                NodeCapabilityProducedMediaWriterFakeImpl::default(),
            )
            .unwrap(),
        ),
        Arc::new(
            TextToSpeechCapabilityImpl::try_new(
                catalog,
                GenerationProfileAlwaysAvailableFakeImpl,
                TextToSpeechProviderFakeImpl::try_new().unwrap(),
                NodeCapabilityProducedMediaWriterFakeImpl::default(),
            )
            .unwrap(),
        ),
    ]
}

fn expected_contracts() -> Vec<NodeCapabilityContract> {
    let mut values = vec![
        literal_contract(),
        asset_contract("image.read_asset", "image", WorkflowDataType::Image),
        asset_contract("video.read_asset", "video", WorkflowDataType::Video),
        asset_contract("audio.read_asset", "audio", WorkflowDataType::Audio),
        text_to_image_contract(),
        image_to_video_contract(),
        text_to_speech_contract(),
    ];
    values.sort_by(|left, right| left.contract_ref().cmp(right.contract_ref()));
    values
}

fn literal_contract() -> NodeCapabilityContract {
    contract(
        "text.provide_literal",
        vec![NodeCapabilityParameterContract::required(
            parameter_key("text"),
            NodeCapabilityParameterConstraint::text_utf8_bytes(1, 65_536).unwrap(),
        )],
        vec![],
        output("text", WorkflowDataType::Text),
        NodeCapabilityExecutionKind::PureValue,
    )
}

fn asset_contract(id: &str, output_key: &str, kind: WorkflowDataType) -> NodeCapabilityContract {
    contract(
        id,
        vec![NodeCapabilityParameterContract::required(
            parameter_key("asset_id"),
            NodeCapabilityParameterConstraint::managed_asset_id(kind).unwrap(),
        )],
        vec![],
        output(output_key, kind),
        NodeCapabilityExecutionKind::ManagedAssetRead,
    )
}

fn text_to_image_contract() -> NodeCapabilityContract {
    contract(
        "image.generate_from_text",
        vec![
            profile_parameter(),
            NodeCapabilityParameterContract::optional_with_default(
                parameter_key("aspect_ratio"),
                NodeCapabilityParameterConstraint::choice_allowed_keys(
                    ["square", "landscape_4_3", "portrait_3_4", "landscape_16_9", "portrait_9_16"]
                        .map(choice_key),
                )
                .unwrap(),
                NodeCapabilityParameterValue::Choice(choice_key("square")),
            )
            .unwrap(),
        ],
        vec![required_input("prompt", WorkflowDataType::Text)],
        output("image", WorkflowDataType::Image),
        NodeCapabilityExecutionKind::ContentGeneration,
    )
}

fn image_to_video_contract() -> NodeCapabilityContract {
    contract(
        "video.generate_from_image",
        vec![
            profile_parameter(),
            NodeCapabilityParameterContract::optional_with_default(
                parameter_key("duration_seconds"),
                NodeCapabilityParameterConstraint::unsigned_integer_allowed_values([5, 10])
                    .unwrap(),
                NodeCapabilityParameterValue::UnsignedInteger(5),
            )
            .unwrap(),
        ],
        vec![
            required_input("image", WorkflowDataType::Image),
            NodeCapabilityInputContract::new(
                input_key("prompt"),
                NodeCapabilityInputBindingContract::OptionalSingleValue {
                    data_type: WorkflowDataType::Text,
                },
            )
            .unwrap(),
        ],
        output("video", WorkflowDataType::Video),
        NodeCapabilityExecutionKind::MediaTransformation,
    )
}

fn text_to_speech_contract() -> NodeCapabilityContract {
    contract(
        "audio.synthesize_speech_from_text",
        vec![profile_parameter()],
        vec![required_input("text", WorkflowDataType::Text)],
        output("audio", WorkflowDataType::Audio),
        NodeCapabilityExecutionKind::ContentGeneration,
    )
}

fn contract(
    id: &str,
    parameters: Vec<NodeCapabilityParameterContract>,
    inputs: Vec<NodeCapabilityInputContract>,
    output: NodeCapabilityOutputContract,
    kind: NodeCapabilityExecutionKind,
) -> NodeCapabilityContract {
    NodeCapabilityContract::try_new(contract_ref(id), parameters, inputs, vec![output], kind)
        .unwrap()
}

fn profile_parameter() -> NodeCapabilityParameterContract {
    NodeCapabilityParameterContract::required(
        parameter_key("generation_profile_ref"),
        NodeCapabilityParameterConstraint::GenerationProfileRef,
    )
}

fn required_input(key: &str, kind: WorkflowDataType) -> NodeCapabilityInputContract {
    NodeCapabilityInputContract::new(
        input_key(key),
        NodeCapabilityInputBindingContract::RequiredSingleValue { data_type: kind },
    )
    .unwrap()
}

fn output(key: &str, kind: WorkflowDataType) -> NodeCapabilityOutputContract {
    NodeCapabilityOutputContract::new(NodeCapabilityOutputKey::new(key).unwrap(), kind, true)
}

fn contract_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}

fn parameter_key(value: &str) -> NodeCapabilityParameterKey {
    NodeCapabilityParameterKey::new(value).unwrap()
}

fn input_key(value: &str) -> NodeCapabilityInputKey {
    NodeCapabilityInputKey::new(value).unwrap()
}

fn choice_key(value: &str) -> NodeCapabilityChoiceKey {
    NodeCapabilityChoiceKey::new(value).unwrap()
}
