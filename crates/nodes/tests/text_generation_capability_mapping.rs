use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use engine::node_capability::*;
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::*;
use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};
use uuid::Uuid;

mod c5_support;
use c5_support::GenerationProfileAlwaysAvailableFakeImpl;

struct ExactTextToImageProviderImpl {
    expected_profile: GenerationProfileRef,
    expected_context: WorkflowNodeExecutionContext,
}

#[async_trait]
impl TextToImageProviderInterface for ExactTextToImageProviderImpl {
    async fn generate_image_from_text(
        &self,
        request: TextToImageProviderRequest,
    ) -> Result<GeneratedImagePayload, NodeCapabilityProviderFailure> {
        assert_eq!(request.profile_ref(), &self.expected_profile);
        assert_context(request.context(), &self.expected_context);
        assert_eq!(request.prompt().parts(), [WorkflowTextPart::Literal("draw a moon".into())]);
        assert_eq!(request.aspect_ratio(), ImageAspectRatio::LandscapeSixteenByNine);
        Ok(GeneratedImagePayload::try_new(
            NodeCapabilityDeclaredMediaFacts::try_image(64, 32).unwrap(),
            source(vec![1; 16], request.context().deadline.monotonic_instant()),
        )
        .unwrap())
    }
}

struct ExactTextToSpeechProviderImpl {
    expected_profile: GenerationProfileRef,
    expected_context: WorkflowNodeExecutionContext,
}

#[async_trait]
impl TextToSpeechProviderInterface for ExactTextToSpeechProviderImpl {
    async fn synthesize_speech_from_text(
        &self,
        request: TextToSpeechProviderRequest,
    ) -> Result<SynthesizedSpeechPayload, NodeCapabilityProviderFailure> {
        assert_eq!(request.profile_ref(), &self.expected_profile);
        assert_context(request.context(), &self.expected_context);
        assert_eq!(request.text().parts(), [WorkflowTextPart::Literal("speak clearly".into())]);
        Ok(SynthesizedSpeechPayload::try_new(
            NodeCapabilityDeclaredMediaFacts::try_audio(1_000, 44_100, 2).unwrap(),
            source(vec![2; 12], request.context().deadline.monotonic_instant()),
        )
        .unwrap())
    }
}

struct ExactProducedMediaWriterImpl {
    expected_context: WorkflowNodeExecutionContext,
    expected_origin: WorkflowNodeExecutionOrigin,
    expected_profile: GenerationProfileRef,
    expected_output_key: NodeCapabilityOutputKey,
    expected_display_name: &'static str,
    expected_kind: NodeCapabilityMediaKind,
}

#[async_trait]
impl NodeCapabilityProducedMediaWriterInterface for ExactProducedMediaWriterImpl {
    async fn write_node_output_media(
        &self,
        request: NodeCapabilityProducedMediaWriteRequest,
    ) -> Result<NodeCapabilityProducedMediaReference, NodeCapabilityMediaBoundaryError> {
        assert_context(request.context(), &self.expected_context);
        assert_eq!(request.origin(), &self.expected_origin);
        assert_eq!(request.output_key().workflow_run_id(), self.expected_context.workflow_run_id);
        assert_eq!(
            request.output_key().node_execution_id(),
            self.expected_context.node_execution_id
        );
        assert_eq!(request.output_key().output_key(), &self.expected_output_key);
        assert_eq!(request.output_key().ordinal(), 0);
        assert_eq!(request.display_name().as_str(), self.expected_display_name);
        assert_eq!(request.payload().media_kind(), self.expected_kind);
        let NodeCapabilityProducedMediaProvenance::ProviderGenerated(provenance) =
            request.provenance()
        else {
            panic!("expected provider-generated provenance")
        };
        assert_eq!(provenance.generation_profile_ref(), &self.expected_profile);
        let digest = request.payload().digest();
        let asset_id =
            WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(240).into_bytes()).unwrap();
        let fingerprint = WorkflowManagedContentFingerprint::from_bytes(digest.as_bytes());
        Ok(match request.into_payload() {
            NodeCapabilityProducedMediaPayload::GeneratedImage(_) => {
                NodeCapabilityProducedMediaReference::Image(WorkflowManagedImageRef::new(
                    asset_id,
                    fingerprint,
                ))
            }
            NodeCapabilityProducedMediaPayload::SynthesizedSpeech(_) => {
                NodeCapabilityProducedMediaReference::Audio(WorkflowManagedAudioRef::new(
                    asset_id,
                    fingerprint,
                ))
            }
            NodeCapabilityProducedMediaPayload::GeneratedVideo(_) => {
                panic!("unexpected video payload")
            }
        })
    }
}

#[tokio::test]
async fn text_to_image_maps_every_semantic_provider_and_media_write_field() {
    let profile = profile_for("image.generate_from_text");
    let context = execution_context(1);
    let origin = execution_origin(1, "image.generate_from_text");
    let capability = TextToImageCapabilityImpl::try_new(
        catalog(),
        GenerationProfileAlwaysAvailableFakeImpl,
        ExactTextToImageProviderImpl {
            expected_profile: profile.clone(),
            expected_context: context.clone(),
        },
        ExactProducedMediaWriterImpl {
            expected_context: context.clone(),
            expected_origin: origin.clone(),
            expected_profile: profile.clone(),
            expected_output_key: output_key("image"),
            expected_display_name: "Generated Image",
            expected_kind: NodeCapabilityMediaKind::Image,
        },
    )
    .unwrap();
    let parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([
        (
            parameter_key("generation_profile_ref"),
            NodeCapabilityParameterValue::GenerationProfile(
                profile.to_node_capability_parameter_value().unwrap(),
            ),
        ),
        (
            parameter_key("aspect_ratio"),
            NodeCapabilityParameterValue::Choice(
                NodeCapabilityChoiceKey::new("landscape_16_9").unwrap(),
            ),
        ),
    ]))
    .unwrap();
    let outputs = capability
        .execute_node_capability(NodeCapabilityExecutionRequest {
            context,
            origin,
            normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
            inputs: text_inputs(&capability, "prompt", "draw a moon", 1),
        })
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    let expected = WorkflowManagedImageRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(240).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes(Sha256::digest(vec![1; 16]).into()),
    );
    assert_eq!(outputs.get(&output_key("image")), Some(&WorkflowRuntimeValue::Image(expected)));
}

#[tokio::test]
async fn text_to_speech_maps_every_semantic_provider_and_media_write_field() {
    let profile = profile_for("audio.synthesize_speech_from_text");
    let context = execution_context(2);
    let origin = execution_origin(2, "audio.synthesize_speech_from_text");
    let capability = TextToSpeechCapabilityImpl::try_new(
        catalog(),
        GenerationProfileAlwaysAvailableFakeImpl,
        ExactTextToSpeechProviderImpl {
            expected_profile: profile.clone(),
            expected_context: context.clone(),
        },
        ExactProducedMediaWriterImpl {
            expected_context: context.clone(),
            expected_origin: origin.clone(),
            expected_profile: profile.clone(),
            expected_output_key: output_key("audio"),
            expected_display_name: "Synthesized Speech",
            expected_kind: NodeCapabilityMediaKind::Audio,
        },
    )
    .unwrap();
    let parameters = profile_parameters(profile);
    let outputs = capability
        .execute_node_capability(NodeCapabilityExecutionRequest {
            context,
            origin,
            normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
            inputs: text_inputs(&capability, "text", "speak clearly", 2),
        })
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    let expected = WorkflowManagedAudioRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(240).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes(Sha256::digest(vec![2; 12]).into()),
    );
    assert_eq!(outputs.get(&output_key("audio")), Some(&WorkflowRuntimeValue::Audio(expected)));
}

fn assert_context(actual: &WorkflowNodeExecutionContext, expected: &WorkflowNodeExecutionContext) {
    assert_eq!(actual.project_id, expected.project_id);
    assert_eq!(actual.workflow_run_id, expected.workflow_run_id);
    assert_eq!(actual.node_execution_id, expected.node_execution_id);
    assert_eq!(actual.deadline.monotonic_instant(), expected.deadline.monotonic_instant());
    assert_eq!(actual.cancellation.is_cancelled(), expected.cancellation.is_cancelled());
}

fn text_inputs(
    capability: &impl WorkflowNodeCapabilityInterface,
    key: &str,
    text: &str,
    seed: u8,
) -> WorkflowNodeInputSet {
    WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([(
            NodeCapabilityInputKey::new(key).unwrap(),
            WorkflowNodeInputValue::Single(WorkflowRuntimeInputItem {
                input_item_id: WorkflowInputItemId::from_uuid(uuid(seed)).unwrap(),
                input_role_key: None,
                value: WorkflowRuntimeValue::Text(
                    WorkflowTextValue::try_new([WorkflowTextPart::Literal(text.into())]).unwrap(),
                ),
            }),
        )]),
    )
    .unwrap()
}

fn profile_parameters(profile: GenerationProfileRef) -> NodeCapabilityParameterSet {
    NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        parameter_key("generation_profile_ref"),
        NodeCapabilityParameterValue::GenerationProfile(
            profile.to_node_capability_parameter_value().unwrap(),
        ),
    )]))
    .unwrap()
}

fn source(bytes: Vec<u8>, deadline: Instant) -> NodeCapabilityMediaSourceLease {
    NodeCapabilityMediaSourceLease::try_new(
        bytes.len() as u64,
        NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(&bytes).into()),
        deadline,
        Box::pin(Cursor::new(bytes)),
    )
    .unwrap()
}

fn execution_context(seed: u8) -> WorkflowNodeExecutionContext {
    WorkflowNodeExecutionContext {
        project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 30)).unwrap(),
        node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap(),
        deadline: NodeCapabilityExecutionDeadline::at(Instant::now() + Duration::from_secs(5)),
        cancellation: NodeCapabilityExecutionCancellation::active(),
    }
}
fn execution_origin(seed: u8, id: &str) -> WorkflowNodeExecutionOrigin {
    WorkflowNodeExecutionOrigin::new(
        WorkflowId::from_uuid(uuid(seed + 90)).unwrap(),
        WorkflowRevision::new(u64::from(seed) + 1).unwrap(),
        WorkflowNodeId::from_uuid(uuid(seed + 120)).unwrap(),
        contract_ref(id),
    )
}
fn catalog() -> Arc<GenerationProfileCatalog> {
    Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap())
}
fn profile_for(id: &str) -> GenerationProfileRef {
    catalog().list_active_generation_profiles_for_capability(&contract_ref(id))[0]
        .profile_ref()
        .clone()
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
fn output_key(value: &str) -> NodeCapabilityOutputKey {
    NodeCapabilityOutputKey::new(value).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
