use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::Instant;

use super::*;
use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityMediaFailure, WorkflowManagedAssetIdBoundaryValue, WorkflowManagedAudioRef,
    WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowManagedVideoRef,
};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

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
        let mut content_by_reference = self
            .content_by_project_and_reference
            .lock()
            .map_err(|_| NodeCapabilityMediaValueError::InvalidMediaFacts)?;
        if let Some((_, existing_reference)) =
            content_by_reference.keys().find(|(stored_project_id, stored_reference)| {
                *stored_project_id == project_id
                    && reference_asset_id(*stored_reference) == reference_asset_id(media_reference)
            })
            && *existing_reference != media_reference
        {
            return Err(if existing_reference.media_kind() == media_reference.media_kind() {
                NodeCapabilityMediaValueError::ContentFingerprintMismatch
            } else {
                NodeCapabilityMediaValueError::InvalidMediaFacts
            });
        }
        content_by_reference.insert(
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
        let (media_reference, content) = resolve_fake_media_selection(
            &content_by_reference,
            request.project_id(),
            request.selection(),
        )?;
        drop(content_by_reference);
        if Instant::now() >= request.deadline() {
            return Err(NodeCapabilityMediaBoundaryError::DeadlineExceeded);
        }
        let source = NodeCapabilityMediaSourceLease::try_new(
            content.bytes.len() as u64,
            digest_bytes(&content.bytes),
            request.deadline(),
            Box::pin(Cursor::new(content.bytes)),
        )
        .map_err(media_value_to_boundary_error)?;
        match media_reference {
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
            ProducedMediaFakeRecord,
        >,
    >,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProducedMediaFakeRecord {
    digest: NodeCapabilityMediaContentDigest,
    media_kind: NodeCapabilityMediaKind,
    origin: engine::node_capability::WorkflowNodeExecutionOrigin,
    provenance: super::NodeCapabilityProducedMediaProvenance,
    reference: NodeCapabilityProducedMediaReference,
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
        let origin = request.origin().clone();
        let provenance = request.provenance().clone();
        verify_produced_payload(request.into_payload(), expected_digest).await?;
        let mut outputs = self.output_by_key.lock().map_err(|_| {
            NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::StorageFailed)
        })?;
        if let Some(record) = outputs.get(&(project_id, key.clone())) {
            return if record.digest == expected_digest
                && record.media_kind == kind
                && record.origin == origin
                && record.provenance == provenance
            {
                Ok(record.reference)
            } else {
                Err(NodeCapabilityMediaBoundaryError::Media(
                    NodeCapabilityMediaFailure::OutputConflict,
                ))
            };
        }
        let reference = produced_reference(project_id, &key, kind, expected_digest)?;
        outputs.insert(
            (project_id, key),
            ProducedMediaFakeRecord {
                digest: expected_digest,
                media_kind: kind,
                origin,
                provenance,
                reference,
            },
        );
        Ok(reference)
    }
}

async fn verify_produced_payload(
    payload: NodeCapabilityProducedMediaPayload,
    expected_digest: NodeCapabilityMediaContentDigest,
) -> Result<(), NodeCapabilityMediaBoundaryError> {
    let source = match payload {
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
    Ok(())
}

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
fn resolve_fake_media_selection(
    content_by_reference: &HashMap<
        (projects::project::domain::ProjectId, NodeCapabilityManagedMediaReference),
        ManagedMediaFakeContent,
    >,
    project_id: projects::project::domain::ProjectId,
    selection: NodeCapabilityManagedMediaReadSelection,
) -> Result<
    (NodeCapabilityManagedMediaReference, ManagedMediaFakeContent),
    NodeCapabilityMediaBoundaryError,
> {
    if let NodeCapabilityManagedMediaReadSelection::ExactReference(reference) = selection
        && let Some(content) = content_by_reference.get(&(project_id, reference))
    {
        return Ok((reference, content.clone()));
    }
    let (asset_id, expected_kind, exact_reference) = match selection {
        NodeCapabilityManagedMediaReadSelection::AssetId(value) => {
            (value.asset_id(), value.expected_media_kind(), None)
        }
        NodeCapabilityManagedMediaReadSelection::ExactReference(reference) => {
            (reference_asset_id(reference), reference.media_kind(), Some(reference))
        }
    };
    let observed = content_by_reference.iter().find(|((stored_project_id, reference), _)| {
        *stored_project_id == project_id && reference_asset_id(*reference) == asset_id
    });
    let Some(((_, observed_reference), content)) = observed else {
        return Err(NodeCapabilityMediaBoundaryError::Media(
            NodeCapabilityMediaFailure::Unavailable,
        ));
    };
    if observed_reference.media_kind() != expected_kind {
        return Err(NodeCapabilityMediaBoundaryError::Media(
            NodeCapabilityMediaFailure::KindMismatch {
                expected: expected_kind.to_workflow_data_type(),
                observed: observed_reference.media_kind().to_workflow_data_type(),
            },
        ));
    }
    if exact_reference.is_some_and(|reference| reference != *observed_reference) {
        return Err(NodeCapabilityMediaBoundaryError::Media(
            NodeCapabilityMediaFailure::DigestMismatch,
        ));
    }
    Ok((*observed_reference, content.clone()))
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
