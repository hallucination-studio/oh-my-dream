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
use tokio::io::AsyncReadExt;
use uuid::Uuid;

mod c5_support;
use c5_support::GenerationProfileAlwaysAvailableFakeImpl;

struct ExactImageToVideoProviderImpl {
    expected_profile: GenerationProfileRef,
    expected_context: WorkflowNodeExecutionContext,
    expected_image: WorkflowManagedImageRef,
    expected_digest: NodeCapabilityMediaContentDigest,
}

#[async_trait]
impl ImageToVideoProviderInterface for ExactImageToVideoProviderImpl {
    async fn generate_video_from_image(
        &self,
        request: ImageToVideoProviderRequest,
    ) -> Result<GeneratedVideoPayload, NodeCapabilityProviderFailure> {
        assert_eq!(request.profile_ref(), &self.expected_profile);
        assert_context(request.context(), &self.expected_context);
        assert_eq!(request.image().media_reference(), self.expected_image);
        assert_eq!(request.image().mime_type(), NodeCapabilityMediaMimeType::ImagePng);
        assert_eq!(
            request.image().facts(),
            NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap()
        );
        assert_eq!(request.image().source().byte_length(), 16);
        assert_eq!(request.image().source().digest(), self.expected_digest);
        assert_eq!(
            request.prompt().unwrap().parts(),
            [WorkflowTextPart::Literal("slow camera".into())]
        );
        assert_eq!(request.duration_seconds(), ImageToVideoDurationSeconds::Ten);
        let deadline = request.context().deadline.monotonic_instant();
        let mut stream = request.into_readable_image().into_source().try_take_stream().unwrap();
        let mut source_bytes = Vec::new();
        stream.read_to_end(&mut source_bytes).await.unwrap();
        assert_eq!(source_bytes, vec![4; 16]);
        Ok(GeneratedVideoPayload::try_new(
            NodeCapabilityDeclaredMediaFacts::try_video(64, 32, 10_000, false).unwrap(),
            source(vec![3; 24], deadline),
        )
        .unwrap())
    }
}

struct ExactVideoWriterImpl {
    expected_context: WorkflowNodeExecutionContext,
    expected_origin: WorkflowNodeExecutionOrigin,
    expected_profile: GenerationProfileRef,
    expected_image: WorkflowManagedImageRef,
}

#[async_trait]
impl NodeCapabilityProducedMediaWriterInterface for ExactVideoWriterImpl {
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
        assert_eq!(request.output_key().output_key(), &output_key("video"));
        assert_eq!(request.output_key().ordinal(), 0);
        assert_eq!(request.display_name().as_str(), "Generated Video");
        assert!(matches!(request.payload(), NodeCapabilityProducedMediaPayload::GeneratedVideo(_)));
        let NodeCapabilityProducedMediaProvenance::ProviderDerived(provenance) =
            request.provenance()
        else {
            panic!("expected provider-derived provenance")
        };
        assert_eq!(provenance.generation_profile_ref(), &self.expected_profile);
        assert_eq!(
            provenance.source_media_references(),
            [NodeCapabilityManagedMediaReference::Image(self.expected_image)]
        );
        let digest = request.payload().digest();
        let reference = WorkflowManagedVideoRef::new(
            WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(240).into_bytes()).unwrap(),
            WorkflowManagedContentFingerprint::from_bytes(digest.as_bytes()),
        );
        Ok(NodeCapabilityProducedMediaReference::Video(reference))
    }
}

#[tokio::test]
async fn image_to_video_maps_source_prompt_duration_context_origin_and_derived_provenance() {
    let image_bytes = vec![4; 16];
    let image_digest = digest(&image_bytes);
    let image = WorkflowManagedImageRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(4).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes(image_digest.as_bytes()),
    );
    let context = execution_context(4);
    let origin = execution_origin(4);
    let profile = profile();
    let capability =
        configured_capability(&context, &origin, &profile, image, image_digest, image_bytes);
    let parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([
        (
            parameter_key("generation_profile_ref"),
            NodeCapabilityParameterValue::GenerationProfile(
                profile.to_node_capability_parameter_value().unwrap(),
            ),
        ),
        (parameter_key("duration_seconds"), NodeCapabilityParameterValue::UnsignedInteger(10)),
    ]))
    .unwrap();
    let inputs = WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([
            (
                input_key("image"),
                WorkflowNodeInputValue::Single(input_item(WorkflowRuntimeValue::Image(image), 4)),
            ),
            (
                input_key("prompt"),
                WorkflowNodeInputValue::Single(input_item(text("slow camera"), 5)),
            ),
        ]),
    )
    .unwrap();
    let outputs = capability
        .execute_node_capability(NodeCapabilityExecutionRequest {
            context,
            origin,
            normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
            inputs,
        })
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    let expected = WorkflowManagedVideoRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(240).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes(digest(&[3; 24]).as_bytes()),
    );
    assert_eq!(outputs.get(&output_key("video")), Some(&WorkflowRuntimeValue::Video(expected)));
}

fn configured_capability(
    context: &WorkflowNodeExecutionContext,
    origin: &WorkflowNodeExecutionOrigin,
    profile: &GenerationProfileRef,
    image: WorkflowManagedImageRef,
    image_digest: NodeCapabilityMediaContentDigest,
    image_bytes: Vec<u8>,
) -> ImageToVideoCapabilityImpl<
    NodeCapabilityManagedMediaReaderFakeImpl,
    GenerationProfileAlwaysAvailableFakeImpl,
    ExactImageToVideoProviderImpl,
    ExactVideoWriterImpl,
> {
    let reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    reader
        .register_managed_media(
            context.project_id,
            NodeCapabilityManagedMediaReference::Image(image),
            NodeCapabilityMediaMimeType::ImagePng,
            NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
            image_bytes,
        )
        .unwrap();
    ImageToVideoCapabilityImpl::try_new(
        catalog(),
        GenerationProfileAlwaysAvailableFakeImpl,
        reader,
        ExactImageToVideoProviderImpl {
            expected_profile: profile.clone(),
            expected_context: context.clone(),
            expected_image: image,
            expected_digest: image_digest,
        },
        ExactVideoWriterImpl {
            expected_context: context.clone(),
            expected_origin: origin.clone(),
            expected_profile: profile.clone(),
            expected_image: image,
        },
    )
    .unwrap()
}

fn assert_context(actual: &WorkflowNodeExecutionContext, expected: &WorkflowNodeExecutionContext) {
    assert_eq!(actual.project_id, expected.project_id);
    assert_eq!(actual.workflow_run_id, expected.workflow_run_id);
    assert_eq!(actual.node_execution_id, expected.node_execution_id);
    assert_eq!(actual.deadline.monotonic_instant(), expected.deadline.monotonic_instant());
    assert_eq!(actual.cancellation.is_cancelled(), expected.cancellation.is_cancelled());
}

fn input_item(value: WorkflowRuntimeValue, seed: u8) -> WorkflowRuntimeInputItem {
    WorkflowRuntimeInputItem {
        input_item_id: WorkflowInputItemId::from_uuid(uuid(seed)).unwrap(),
        input_role_key: None,
        value,
    }
}
fn text(value: &str) -> WorkflowRuntimeValue {
    WorkflowRuntimeValue::Text(
        WorkflowTextValue::try_new([WorkflowTextPart::Literal(value.into())]).unwrap(),
    )
}
fn source(bytes: Vec<u8>, deadline: Instant) -> NodeCapabilityMediaSourceLease {
    NodeCapabilityMediaSourceLease::try_new(
        bytes.len() as u64,
        digest(&bytes),
        deadline,
        Box::pin(Cursor::new(bytes)),
    )
    .unwrap()
}
fn digest(bytes: &[u8]) -> NodeCapabilityMediaContentDigest {
    NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(bytes).into())
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
fn execution_origin(seed: u8) -> WorkflowNodeExecutionOrigin {
    WorkflowNodeExecutionOrigin::new(
        WorkflowId::from_uuid(uuid(seed + 90)).unwrap(),
        WorkflowRevision::new(u64::from(seed) + 1).unwrap(),
        WorkflowNodeId::from_uuid(uuid(seed + 120)).unwrap(),
        contract_ref(),
    )
}
fn catalog() -> Arc<GenerationProfileCatalog> {
    Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap())
}
fn profile() -> GenerationProfileRef {
    catalog().list_active_generation_profiles_for_capability(&contract_ref())[0]
        .profile_ref()
        .clone()
}
fn contract_ref() -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new("video.generate_from_image").unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
fn parameter_key(value: &str) -> NodeCapabilityParameterKey {
    NodeCapabilityParameterKey::new(value).unwrap()
}
fn input_key(value: &str) -> NodeCapabilityInputKey {
    NodeCapabilityInputKey::new(value).unwrap()
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
