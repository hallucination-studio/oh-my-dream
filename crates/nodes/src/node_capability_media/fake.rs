use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityMediaFailure, NodeCapabilityProviderFailure,
    NodeCapabilityProviderFailureCategory, NodeCapabilityProviderFailureConstructionError,
    WorkflowManagedAssetIdBoundaryValue, WorkflowManagedAudioRef,
    WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowManagedVideoRef,
};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

use super::*;

#[derive(Clone)]
struct ManagedMediaFakeContent {
    mime_type: NodeCapabilityMediaMimeType,
    facts: NodeCapabilityDeclaredMediaFacts,
    bytes: Vec<u8>,
}

/// Deterministic in-memory managed-media reader fake.
#[derive(Default)]
pub struct NodeCapabilityManagedMediaReaderFakeImpl {
    content_by_project_and_reference: Mutex<
        HashMap<
            (projects::project::domain::ProjectId, NodeCapabilityManagedMediaReference),
            ManagedMediaFakeContent,
        >,
    >,
}

impl NodeCapabilityManagedMediaReaderFakeImpl {
    /// Registers exact bytes only when their digest matches the managed reference fingerprint.
    pub fn register_managed_media(
        &self,
        project_id: projects::project::domain::ProjectId,
        media_reference: NodeCapabilityManagedMediaReference,
        mime_type: NodeCapabilityMediaMimeType,
        facts: NodeCapabilityDeclaredMediaFacts,
        bytes: Vec<u8>,
    ) -> Result<(), NodeCapabilityMediaValueError> {
        let digest = digest_bytes(&bytes);
        if reference_fingerprint(media_reference) != digest.as_bytes() {
            return Err(NodeCapabilityMediaValueError::ContentFingerprintMismatch);
        }
        if mime_type.media_kind() != media_reference.media_kind() {
            return Err(NodeCapabilityMediaValueError::InvalidMimeForKind);
        }
        if facts.media_kind() != media_reference.media_kind() {
            return Err(NodeCapabilityMediaValueError::InvalidMediaFacts);
        }
        if !byte_length_within_kind_limit(bytes.len() as u64, media_reference.media_kind()) {
            return Err(NodeCapabilityMediaValueError::InvalidByteLength);
        }
        self.content_by_project_and_reference
            .lock()
            .map_err(|_| NodeCapabilityMediaValueError::InvalidMediaFacts)?
            .insert(
                (project_id, media_reference),
                ManagedMediaFakeContent { mime_type, facts, bytes },
            );
        Ok(())
    }
}

#[async_trait]
impl NodeCapabilityManagedMediaReaderInterface for NodeCapabilityManagedMediaReaderFakeImpl {
    async fn read_managed_media(
        &self,
        request: NodeCapabilityManagedMediaReadRequest,
    ) -> Result<NodeCapabilityReadableMediaInput, NodeCapabilityMediaBoundaryError> {
        if Instant::now() >= request.deadline() {
            return Err(NodeCapabilityMediaBoundaryError::DeadlineExceeded);
        }
        let content_by_reference = self.content_by_project_and_reference.lock().map_err(|_| {
            NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::StorageFailed)
        })?;
        let content =
            content_by_reference.get(&(request.project_id(), request.media_reference())).cloned();
        if content.is_none()
            && content_by_reference.keys().any(|(project_id, reference)| {
                *project_id == request.project_id()
                    && reference_asset_id(*reference)
                        == reference_asset_id(request.media_reference())
            })
        {
            return Err(NodeCapabilityMediaBoundaryError::Media(
                NodeCapabilityMediaFailure::KindMismatch,
            ));
        }
        let content = content.ok_or(NodeCapabilityMediaBoundaryError::Media(
            NodeCapabilityMediaFailure::Unavailable,
        ))?;
        drop(content_by_reference);
        let source = NodeCapabilityMediaSourceLease::try_new(
            content.bytes.len() as u64,
            digest_bytes(&content.bytes),
            request.deadline(),
            Box::pin(Cursor::new(content.bytes)),
        )
        .map_err(media_value_to_boundary_error)?;
        match request.media_reference() {
            NodeCapabilityManagedMediaReference::Image(reference) => {
                NodeCapabilityReadableImageInput::try_new(
                    reference,
                    content.mime_type,
                    content.facts,
                    source,
                )
                .map(NodeCapabilityReadableMediaInput::Image)
            }
            NodeCapabilityManagedMediaReference::Video(reference) => {
                NodeCapabilityReadableVideoInput::try_new(
                    reference,
                    content.mime_type,
                    content.facts,
                    source,
                )
                .map(NodeCapabilityReadableMediaInput::Video)
            }
            NodeCapabilityManagedMediaReference::Audio(reference) => {
                NodeCapabilityReadableAudioInput::try_new(
                    reference,
                    content.mime_type,
                    content.facts,
                    source,
                )
                .map(NodeCapabilityReadableMediaInput::Audio)
            }
        }
        .map_err(media_value_to_boundary_error)
    }
}

/// Deterministic idempotent in-memory produced-media writer fake.
#[derive(Default)]
pub struct NodeCapabilityProducedMediaWriterFakeImpl {
    output_by_key: Mutex<
        HashMap<
            (projects::project::domain::ProjectId, NodeCapabilityProducedMediaOutputKey),
            (NodeCapabilityMediaContentDigest, NodeCapabilityProducedMediaReference),
        >,
    >,
}

#[async_trait]
impl NodeCapabilityProducedMediaWriterInterface for NodeCapabilityProducedMediaWriterFakeImpl {
    async fn write_node_output_media(
        &self,
        request: NodeCapabilityProducedMediaWriteRequest,
    ) -> Result<NodeCapabilityProducedMediaReference, NodeCapabilityMediaBoundaryError> {
        if request.context().cancellation.is_cancelled() {
            return Err(NodeCapabilityMediaBoundaryError::Cancelled);
        }
        if request.context().deadline.is_reached_at(Instant::now()) {
            return Err(NodeCapabilityMediaBoundaryError::DeadlineExceeded);
        }
        let project_id = request.context().project_id;
        let key = request.output_key().clone();
        let kind = request.payload().media_kind();
        let expected_digest = request.payload().digest();
        let source = match request.into_payload() {
            NodeCapabilityProducedMediaPayload::GeneratedImage(value) => value.into_source(),
            NodeCapabilityProducedMediaPayload::GeneratedVideo(value) => value.into_source(),
            NodeCapabilityProducedMediaPayload::SynthesizedSpeech(value) => value.into_source(),
        };
        let expected_length = source.byte_length();
        let mut stream = source.try_take_stream().map_err(media_value_to_boundary_error)?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.map_err(|_| {
            NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::StorageFailed)
        })?;
        if bytes.len() as u64 != expected_length || digest_bytes(&bytes) != expected_digest {
            return Err(NodeCapabilityMediaBoundaryError::Media(
                NodeCapabilityMediaFailure::DigestMismatch,
            ));
        }
        let mut outputs = self.output_by_key.lock().map_err(|_| {
            NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::StorageFailed)
        })?;
        if let Some((digest, reference)) = outputs.get(&(project_id, key.clone())) {
            return if digest == &expected_digest {
                Ok(*reference)
            } else {
                Err(NodeCapabilityMediaBoundaryError::Media(
                    NodeCapabilityMediaFailure::OutputConflict,
                ))
            };
        }
        let reference = produced_reference(project_id, &key, kind, expected_digest)?;
        outputs.insert((project_id, key), (expected_digest, reference));
        Ok(reference)
    }
}

macro_rules! provider_fake {
    ($name:ident, $interface:ident, $method:ident, $request:ty, $payload:ty, $facts:expr, $bytes:expr) => {
        #[doc = concat!("Deterministic ", stringify!($interface), " fake implementation.")]
        pub struct $name {
            deadline_failure: NodeCapabilityProviderFailure,
            invalid_response_failure: NodeCapabilityProviderFailure,
        }
        impl $name {
            /// Creates a fake with fixed valid provider failures.
            pub fn try_new() -> Result<Self, NodeCapabilityProviderFailureConstructionError> {
                Ok(Self {
                    deadline_failure: provider_failure(
                        NodeCapabilityProviderFailureCategory::DeadlineExceeded,
                    )?,
                    invalid_response_failure: provider_failure(
                        NodeCapabilityProviderFailureCategory::InvalidResponse,
                    )?,
                })
            }
        }
        #[async_trait]
        impl $interface for $name {
            async fn $method(
                &self,
                request: $request,
            ) -> Result<$payload, NodeCapabilityProviderFailure> {
                if request.context().deadline.is_reached_at(Instant::now()) {
                    return Err(self.deadline_failure.clone());
                }
                let bytes: Vec<u8> = $bytes;
                let source = NodeCapabilityMediaSourceLease::try_new(
                    bytes.len() as u64,
                    digest_bytes(&bytes),
                    request.context().deadline.monotonic_instant(),
                    Box::pin(Cursor::new(bytes)),
                )
                .map_err(|_| self.invalid_response_failure.clone())?;
                let facts = $facts.map_err(|_| self.invalid_response_failure.clone())?;
                <$payload>::try_new(facts, source)
                    .map_err(|_| self.invalid_response_failure.clone())
            }
        }
    };
}

provider_fake!(
    TextToImageProviderFakeImpl,
    TextToImageProviderInterface,
    generate_image_from_text,
    TextToImageProviderRequest,
    GeneratedImagePayload,
    NodeCapabilityDeclaredMediaFacts::try_image(32, 32),
    vec![1; 16]
);
provider_fake!(
    ImageToVideoProviderFakeImpl,
    ImageToVideoProviderInterface,
    generate_video_from_image,
    ImageToVideoProviderRequest,
    GeneratedVideoPayload,
    NodeCapabilityDeclaredMediaFacts::try_video(32, 32, 5_000, false),
    vec![2; 24]
);
provider_fake!(
    TextToSpeechProviderFakeImpl,
    TextToSpeechProviderInterface,
    synthesize_speech_from_text,
    TextToSpeechProviderRequest,
    SynthesizedSpeechPayload,
    NodeCapabilityDeclaredMediaFacts::try_audio(1_000, 44_100, 1),
    vec![3; 12]
);

fn digest_bytes(bytes: &[u8]) -> NodeCapabilityMediaContentDigest {
    NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(bytes).into())
}

fn reference_fingerprint(reference: NodeCapabilityManagedMediaReference) -> [u8; 32] {
    match reference {
        NodeCapabilityManagedMediaReference::Image(value) => value.content_fingerprint().as_bytes(),
        NodeCapabilityManagedMediaReference::Video(value) => value.content_fingerprint().as_bytes(),
        NodeCapabilityManagedMediaReference::Audio(value) => value.content_fingerprint().as_bytes(),
    }
}

fn reference_asset_id(
    reference: NodeCapabilityManagedMediaReference,
) -> WorkflowManagedAssetIdBoundaryValue {
    match reference {
        NodeCapabilityManagedMediaReference::Image(value) => value.asset_id(),
        NodeCapabilityManagedMediaReference::Video(value) => value.asset_id(),
        NodeCapabilityManagedMediaReference::Audio(value) => value.asset_id(),
    }
}

fn media_value_to_boundary_error(
    error: NodeCapabilityMediaValueError,
) -> NodeCapabilityMediaBoundaryError {
    match error {
        NodeCapabilityMediaValueError::DeadlineExceeded => {
            NodeCapabilityMediaBoundaryError::DeadlineExceeded
        }
        NodeCapabilityMediaValueError::ContentFingerprintMismatch => {
            NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::DigestMismatch)
        }
        NodeCapabilityMediaValueError::InvalidByteLength => {
            NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::SizeLimitExceeded)
        }
        _ => NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::InvalidMedia),
    }
}

fn produced_reference(
    project_id: projects::project::domain::ProjectId,
    output_key: &NodeCapabilityProducedMediaOutputKey,
    kind: NodeCapabilityMediaKind,
    digest: NodeCapabilityMediaContentDigest,
) -> Result<NodeCapabilityProducedMediaReference, NodeCapabilityMediaBoundaryError> {
    let mut hasher = Sha256::new();
    hasher.update(project_id.as_uuid().as_bytes());
    hasher.update(output_key.workflow_run_id().as_uuid().as_bytes());
    hasher.update(output_key.node_execution_id().as_uuid().as_bytes());
    hasher.update(output_key.output_key().as_str().as_bytes());
    hasher.update(output_key.ordinal().to_be_bytes());
    let identity_digest = hasher.finalize();
    let mut identity: [u8; 16] = identity_digest[..16].try_into().map_err(|_| {
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::StorageFailed)
    })?;
    identity[6] = (identity[6] & 0x0f) | 0x40;
    identity[8] = (identity[8] & 0x3f) | 0x80;
    let asset_id = WorkflowManagedAssetIdBoundaryValue::from_bytes(identity).map_err(|_| {
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::StorageFailed)
    })?;
    let fingerprint = WorkflowManagedContentFingerprint::from_bytes(digest.as_bytes());
    Ok(match kind {
        NodeCapabilityMediaKind::Image => NodeCapabilityProducedMediaReference::Image(
            WorkflowManagedImageRef::new(asset_id, fingerprint),
        ),
        NodeCapabilityMediaKind::Video => NodeCapabilityProducedMediaReference::Video(
            WorkflowManagedVideoRef::new(asset_id, fingerprint),
        ),
        NodeCapabilityMediaKind::Audio => NodeCapabilityProducedMediaReference::Audio(
            WorkflowManagedAudioRef::new(asset_id, fingerprint),
        ),
    })
}

fn provider_failure(
    category: NodeCapabilityProviderFailureCategory,
) -> Result<NodeCapabilityProviderFailure, NodeCapabilityProviderFailureConstructionError> {
    NodeCapabilityProviderFailure::try_new(category, false, Instant::now(), None)
}
