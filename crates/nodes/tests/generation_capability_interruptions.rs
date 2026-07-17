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

enum ProviderInterruption {
    None,
    Cancel(NodeCapabilityExecutionCancellation),
    DelayPastDeadline,
}

struct InterruptingTextToImageProviderImpl(ProviderInterruption);

#[async_trait]
impl TextToImageProviderInterface for InterruptingTextToImageProviderImpl {
    async fn generate_image_from_text(
        &self,
        request: TextToImageProviderRequest,
    ) -> Result<GeneratedImagePayload, NodeCapabilityProviderFailure> {
        match &self.0 {
            ProviderInterruption::None => {}
            ProviderInterruption::Cancel(cancellation) => cancellation.cancel(),
            ProviderInterruption::DelayPastDeadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        Ok(image_payload(request.context().deadline.monotonic_instant()))
    }
}

enum WriterInterruption {
    Cancel(NodeCapabilityExecutionCancellation),
    DelayPastDeadline,
    MustNotBeCalled,
}

struct InterruptingProducedMediaWriterImpl(WriterInterruption);

#[async_trait]
impl NodeCapabilityProducedMediaWriterInterface for InterruptingProducedMediaWriterImpl {
    async fn write_node_output_media(
        &self,
        request: NodeCapabilityProducedMediaWriteRequest,
    ) -> Result<NodeCapabilityProducedMediaReference, NodeCapabilityMediaBoundaryError> {
        match &self.0 {
            WriterInterruption::Cancel(cancellation) => cancellation.cancel(),
            WriterInterruption::DelayPastDeadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            WriterInterruption::MustNotBeCalled => panic!("writer must not be called"),
        }
        let reference = WorkflowManagedImageRef::new(
            WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(200).into_bytes()).unwrap(),
            WorkflowManagedContentFingerprint::from_bytes(request.payload().digest().as_bytes()),
        );
        Ok(NodeCapabilityProducedMediaReference::Image(reference))
    }
}

#[tokio::test]
async fn shared_cancellation_is_observed_after_provider_and_writer_awaits() {
    let provider_cancellation = NodeCapabilityExecutionCancellation::active();
    let provider_capability = capability(
        ProviderInterruption::Cancel(provider_cancellation.clone()),
        WriterInterruption::MustNotBeCalled,
    );
    let provider_error = provider_capability
        .execute_node_capability(request(
            &provider_capability,
            1,
            future_instant(),
            provider_cancellation,
        ))
        .await
        .unwrap_err();
    assert_interruption(
        &provider_error,
        NodeCapabilityExecutionStage::CallProvider,
        NodeCapabilityExecutionFailure::Cancelled,
    );

    let writer_cancellation = NodeCapabilityExecutionCancellation::active();
    let writer_capability = capability(
        ProviderInterruption::None,
        WriterInterruption::Cancel(writer_cancellation.clone()),
    );
    let writer_error = writer_capability
        .execute_node_capability(request(
            &writer_capability,
            2,
            future_instant(),
            writer_cancellation,
        ))
        .await
        .unwrap_err();
    assert_interruption(
        &writer_error,
        NodeCapabilityExecutionStage::WriteManagedMedia,
        NodeCapabilityExecutionFailure::Cancelled,
    );
}

#[tokio::test]
async fn deadline_is_observed_after_provider_and_writer_awaits() {
    let provider_capability =
        capability(ProviderInterruption::DelayPastDeadline, WriterInterruption::MustNotBeCalled);
    let provider_error = provider_capability
        .execute_node_capability(request(
            &provider_capability,
            3,
            near_deadline(),
            NodeCapabilityExecutionCancellation::active(),
        ))
        .await
        .unwrap_err();
    assert_interruption(
        &provider_error,
        NodeCapabilityExecutionStage::CallProvider,
        NodeCapabilityExecutionFailure::DeadlineExceeded,
    );

    let writer_capability =
        capability(ProviderInterruption::None, WriterInterruption::DelayPastDeadline);
    let writer_error = writer_capability
        .execute_node_capability(request(
            &writer_capability,
            4,
            near_deadline(),
            NodeCapabilityExecutionCancellation::active(),
        ))
        .await
        .unwrap_err();
    assert_interruption(
        &writer_error,
        NodeCapabilityExecutionStage::WriteManagedMedia,
        NodeCapabilityExecutionFailure::DeadlineExceeded,
    );
}

#[tokio::test]
async fn cancellation_precedes_deadline_when_both_are_observed() {
    let capability = capability(ProviderInterruption::None, WriterInterruption::MustNotBeCalled);
    let cancellation = NodeCapabilityExecutionCancellation::active();
    cancellation.cancel();
    let error = capability
        .execute_node_capability(request(&capability, 5, Instant::now(), cancellation))
        .await
        .unwrap_err();
    assert_interruption(
        &error,
        NodeCapabilityExecutionStage::CallProvider,
        NodeCapabilityExecutionFailure::Cancelled,
    );
}

fn capability(
    provider: ProviderInterruption,
    writer: WriterInterruption,
) -> TextToImageCapabilityImpl<
    GenerationProfileAlwaysAvailableFakeImpl,
    InterruptingTextToImageProviderImpl,
    InterruptingProducedMediaWriterImpl,
> {
    TextToImageCapabilityImpl::try_new(
        catalog(),
        GenerationProfileAlwaysAvailableFakeImpl,
        InterruptingTextToImageProviderImpl(provider),
        InterruptingProducedMediaWriterImpl(writer),
    )
    .unwrap()
}

fn request(
    capability: &impl WorkflowNodeCapabilityInterface,
    seed: u8,
    deadline: Instant,
    cancellation: NodeCapabilityExecutionCancellation,
) -> NodeCapabilityExecutionRequest {
    let parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
        NodeCapabilityParameterValue::GenerationProfile(
            profile().to_node_capability_parameter_value().unwrap(),
        ),
    )]))
    .unwrap();
    let inputs = WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([(
            NodeCapabilityInputKey::new("prompt").unwrap(),
            WorkflowNodeInputValue::Single(WorkflowRuntimeInputItem {
                input_item_id: WorkflowInputItemId::from_uuid(uuid(seed)).unwrap(),
                input_role_key: None,
                value: WorkflowRuntimeValue::Text(
                    WorkflowTextValue::try_new([WorkflowTextPart::Literal("draw".into())]).unwrap(),
                ),
            }),
        )]),
    )
    .unwrap();
    NodeCapabilityExecutionRequest {
        context: WorkflowNodeExecutionContext {
            project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
            workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 30)).unwrap(),
            node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap(),
            deadline: NodeCapabilityExecutionDeadline::at(deadline),
            cancellation,
        },
        origin: WorkflowNodeExecutionOrigin::new(
            WorkflowId::from_uuid(uuid(seed + 90)).unwrap(),
            WorkflowRevision::new(u64::from(seed) + 1).unwrap(),
            WorkflowNodeId::from_uuid(uuid(seed + 120)).unwrap(),
            contract_ref(),
        ),
        normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
        inputs,
    }
}

fn assert_interruption(
    error: &NodeCapabilityExecutionError,
    stage: NodeCapabilityExecutionStage,
    failure: NodeCapabilityExecutionFailure,
) {
    assert_eq!(error.stage(), stage);
    assert_eq!(error.failure(), &failure);
    let target = if stage == NodeCapabilityExecutionStage::CallProvider {
        NodeCapabilityExecutionTarget::Capability
    } else {
        NodeCapabilityExecutionTarget::Output(NodeCapabilityOutputKey::new("image").unwrap())
    };
    assert_eq!(error.target(), &target);
}

fn image_payload(deadline: Instant) -> GeneratedImagePayload {
    let bytes = vec![1; 16];
    GeneratedImagePayload::try_new(
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
        NodeCapabilityMediaSourceLease::try_new(
            bytes.len() as u64,
            NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(&bytes).into()),
            deadline,
            Box::pin(Cursor::new(bytes)),
        )
        .unwrap(),
    )
    .unwrap()
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
        NodeCapabilityContractId::new("image.generate_from_text").unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
fn future_instant() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn near_deadline() -> Instant {
    Instant::now() + Duration::from_millis(10)
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
