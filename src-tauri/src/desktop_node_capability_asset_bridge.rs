//! Desktop translation between Node Capability media boundaries and Asset use cases.

mod error;
#[cfg(test)]
mod tests;

use std::{sync::Arc, time::Instant};

use assets::asset::{
    application::{
        AssetNodeOutputSourceLease, AssetRecordNodeOutputCommand, AssetRecordNodeOutputUseCase,
        AssetResolveContentQuery, AssetResolveContentUseCase, AssetResolvedContent,
    },
    domain::{
        AssetDisplayName, AssetId, AssetManagedContentState, AssetMediaFacts, AssetMediaKind,
        AssetMediaMimeType, AssetNodeOutputKey, AssetNodeOutputProduction,
        AssetOriginGenerationProfileRef, AssetOriginNodeCapabilityContractRef,
        AssetOriginNodeOutputKey, AssetOriginSourceAssetId, AssetOriginSourceAssetIds,
        AssetOriginWorkflowId, AssetOriginWorkflowNodeExecutionId, AssetOriginWorkflowNodeId,
        AssetOriginWorkflowRevision, AssetOriginWorkflowRunId, AssetWorkflowNodeOrigin,
    },
};
use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityMediaFailure, WorkflowManagedAssetIdBoundaryValue, WorkflowManagedAudioRef,
    WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowManagedVideoRef,
};
use nodes::{
    NodeCapabilityDeclaredMediaFacts, NodeCapabilityManagedMediaReadRequest,
    NodeCapabilityManagedMediaReadSelection, NodeCapabilityManagedMediaReaderInterface,
    NodeCapabilityMediaBoundaryError, NodeCapabilityMediaContentDigest,
    NodeCapabilityMediaMimeType, NodeCapabilityMediaSourceLease,
    NodeCapabilityProducedMediaPayload, NodeCapabilityProducedMediaProvenance,
    NodeCapabilityProducedMediaReference, NodeCapabilityProducedMediaWriteRequest,
    NodeCapabilityProducedMediaWriterInterface, NodeCapabilityReadableAudioInput,
    NodeCapabilityReadableImageInput, NodeCapabilityReadableMediaInput,
    NodeCapabilityReadableVideoInput,
};

use self::error::{invalid_media, map_asset_error, media_kind};

/// Asset-backed exact managed-media bridge consumed by Node Capabilities.
pub struct DesktopNodeCapabilityAssetBridgeAdapterImpl {
    resolve_content: Arc<AssetResolveContentUseCase>,
    record_node_output: Arc<AssetRecordNodeOutputUseCase>,
}

impl DesktopNodeCapabilityAssetBridgeAdapterImpl {
    /// Wires the Asset application boundary without exposing its adapters.
    #[must_use]
    pub const fn new(
        resolve_content: Arc<AssetResolveContentUseCase>,
        record_node_output: Arc<AssetRecordNodeOutputUseCase>,
    ) -> Self {
        Self { resolve_content, record_node_output }
    }
}

#[async_trait]
impl NodeCapabilityManagedMediaReaderInterface for DesktopNodeCapabilityAssetBridgeAdapterImpl {
    async fn read_managed_media(
        &self,
        request: NodeCapabilityManagedMediaReadRequest,
    ) -> Result<NodeCapabilityReadableMediaInput, NodeCapabilityMediaBoundaryError> {
        if Instant::now() >= request.deadline() {
            return Err(NodeCapabilityMediaBoundaryError::DeadlineExceeded);
        }
        let selection = ReadSelection::try_from(request.selection())?;
        let resolved = self
            .resolve_content
            .resolve_asset_content(AssetResolveContentQuery::new(
                request.project_id(),
                selection.asset_id,
                selection.expected_kind,
                request.deadline(),
            ))
            .await
            .map_err(map_asset_error)?;
        selection.verify_fingerprint(&resolved)?;
        readable_input(selection, resolved)
    }
}

#[async_trait]
impl NodeCapabilityProducedMediaWriterInterface for DesktopNodeCapabilityAssetBridgeAdapterImpl {
    async fn write_node_output_media(
        &self,
        request: NodeCapabilityProducedMediaWriteRequest,
    ) -> Result<NodeCapabilityProducedMediaReference, NodeCapabilityMediaBoundaryError> {
        let cancellation = request.context().cancellation.clone();
        if cancellation.is_cancelled() {
            return Err(NodeCapabilityMediaBoundaryError::Cancelled);
        }
        if request.context().deadline.is_reached_at(Instant::now()) {
            return Err(NodeCapabilityMediaBoundaryError::DeadlineExceeded);
        }
        let expected_digest = request.payload().digest().as_bytes();
        let expected_kind = media_kind(request.payload().media_kind());
        let command = record_command(request)?;
        let asset = self
            .record_node_output
            .record_asset_node_output(command)
            .await
            .map_err(map_asset_error)?;
        if cancellation.is_cancelled() {
            return Err(NodeCapabilityMediaBoundaryError::Cancelled);
        }
        if asset.media_kind() != expected_kind {
            return Err(invalid_media());
        }
        let AssetManagedContentState::Available { descriptor } = asset.content_state() else {
            return Err(NodeCapabilityMediaBoundaryError::Media(
                NodeCapabilityMediaFailure::FinalizationFailed,
            ));
        };
        if descriptor.digest().as_bytes() != expected_digest {
            return Err(NodeCapabilityMediaBoundaryError::Media(
                NodeCapabilityMediaFailure::DigestMismatch,
            ));
        }
        produced_reference(asset.id(), expected_kind, expected_digest)
    }
}

struct ReadSelection {
    asset_id: AssetId,
    expected_kind: AssetMediaKind,
    expected_fingerprint: Option<[u8; 32]>,
}

impl TryFrom<NodeCapabilityManagedMediaReadSelection> for ReadSelection {
    type Error = NodeCapabilityMediaBoundaryError;

    fn try_from(value: NodeCapabilityManagedMediaReadSelection) -> Result<Self, Self::Error> {
        match value {
            NodeCapabilityManagedMediaReadSelection::AssetId(value) => Ok(Self {
                asset_id: asset_id(value.asset_id())?,
                expected_kind: media_kind(value.expected_media_kind()),
                expected_fingerprint: None,
            }),
            NodeCapabilityManagedMediaReadSelection::ExactReference(value) => {
                let (asset_id_value, fingerprint) = match value {
                    nodes::NodeCapabilityManagedMediaReference::Image(value) => {
                        (value.asset_id(), value.content_fingerprint())
                    }
                    nodes::NodeCapabilityManagedMediaReference::Video(value) => {
                        (value.asset_id(), value.content_fingerprint())
                    }
                    nodes::NodeCapabilityManagedMediaReference::Audio(value) => {
                        (value.asset_id(), value.content_fingerprint())
                    }
                };
                Ok(Self {
                    asset_id: asset_id(asset_id_value)?,
                    expected_kind: media_kind(value.media_kind()),
                    expected_fingerprint: Some(fingerprint.as_bytes()),
                })
            }
        }
    }
}

impl ReadSelection {
    fn verify_fingerprint(
        &self,
        resolved: &AssetResolvedContent,
    ) -> Result<(), NodeCapabilityMediaBoundaryError> {
        if self
            .expected_fingerprint
            .is_some_and(|value| value != resolved.descriptor().digest().as_bytes())
        {
            Err(NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::DigestMismatch))
        } else {
            Ok(())
        }
    }
}

fn readable_input(
    selection: ReadSelection,
    resolved: AssetResolvedContent,
) -> Result<NodeCapabilityReadableMediaInput, NodeCapabilityMediaBoundaryError> {
    let descriptor = resolved.descriptor().clone();
    let facts = declared_facts(resolved.media_facts())?;
    let mime = mime_type(descriptor.mime_type());
    let fingerprint = WorkflowManagedContentFingerprint::from_bytes(descriptor.digest().as_bytes());
    let boundary_id =
        WorkflowManagedAssetIdBoundaryValue::from_bytes(selection.asset_id.as_uuid().into_bytes())
            .map_err(|_| invalid_media())?;
    let lease = resolved.into_content_lease();
    let source = NodeCapabilityMediaSourceLease::try_new(
        lease.byte_length(),
        NodeCapabilityMediaContentDigest::from_bytes(lease.content_id().digest().as_bytes()),
        lease.deadline(),
        lease.try_take_stream().map_err(map_asset_error)?,
    )
    .map_err(|_| invalid_media())?;
    match selection.expected_kind {
        AssetMediaKind::Image => NodeCapabilityReadableImageInput::try_new(
            WorkflowManagedImageRef::new(boundary_id, fingerprint),
            mime,
            facts,
            source,
        )
        .map(NodeCapabilityReadableMediaInput::Image)
        .map_err(|_| invalid_media()),
        AssetMediaKind::Video => NodeCapabilityReadableVideoInput::try_new(
            WorkflowManagedVideoRef::new(boundary_id, fingerprint),
            mime,
            facts,
            source,
        )
        .map(NodeCapabilityReadableMediaInput::Video)
        .map_err(|_| invalid_media()),
        AssetMediaKind::Audio => NodeCapabilityReadableAudioInput::try_new(
            WorkflowManagedAudioRef::new(boundary_id, fingerprint),
            mime,
            facts,
            source,
        )
        .map(NodeCapabilityReadableMediaInput::Audio)
        .map_err(|_| invalid_media()),
    }
}

const fn mime_type(value: AssetMediaMimeType) -> NodeCapabilityMediaMimeType {
    match value {
        AssetMediaMimeType::ImagePng => NodeCapabilityMediaMimeType::ImagePng,
        AssetMediaMimeType::ImageJpeg => NodeCapabilityMediaMimeType::ImageJpeg,
        AssetMediaMimeType::ImageWebp => NodeCapabilityMediaMimeType::ImageWebp,
        AssetMediaMimeType::VideoMp4 => NodeCapabilityMediaMimeType::VideoMp4,
        AssetMediaMimeType::VideoWebm => NodeCapabilityMediaMimeType::VideoWebm,
        AssetMediaMimeType::AudioMpeg => NodeCapabilityMediaMimeType::AudioMpeg,
        AssetMediaMimeType::AudioWav => NodeCapabilityMediaMimeType::AudioWav,
        AssetMediaMimeType::AudioOgg => NodeCapabilityMediaMimeType::AudioOgg,
    }
}

fn declared_facts(
    value: AssetMediaFacts,
) -> Result<NodeCapabilityDeclaredMediaFacts, NodeCapabilityMediaBoundaryError> {
    let result = match value {
        AssetMediaFacts::Image(value) => {
            NodeCapabilityDeclaredMediaFacts::try_image(value.width(), value.height())
        }
        AssetMediaFacts::Video(value) => NodeCapabilityDeclaredMediaFacts::try_video(
            value.width(),
            value.height(),
            value.duration_ms(),
            value.has_audio(),
        ),
        AssetMediaFacts::Audio(value) => NodeCapabilityDeclaredMediaFacts::try_audio(
            value.duration_ms(),
            value.sample_rate_hz(),
            value.channels(),
        ),
    };
    result.map_err(|_| invalid_media())
}

fn asset_id(
    value: WorkflowManagedAssetIdBoundaryValue,
) -> Result<AssetId, NodeCapabilityMediaBoundaryError> {
    AssetId::from_uuid(uuid::Uuid::from_bytes(value.as_bytes())).map_err(|_| invalid_media())
}

fn record_command(
    request: NodeCapabilityProducedMediaWriteRequest,
) -> Result<AssetRecordNodeOutputCommand, NodeCapabilityMediaBoundaryError> {
    let context = request.context().clone();
    let origin = request.origin().clone();
    let output = request.output_key().clone();
    let display_name =
        AssetDisplayName::try_new(request.display_name().as_str()).map_err(|_| invalid_media())?;
    let production = production(request.provenance().clone())?;
    let expected_kind = media_kind(request.payload().media_kind());
    let source = match request.into_payload() {
        NodeCapabilityProducedMediaPayload::GeneratedImage(value) => value.into_source(),
        NodeCapabilityProducedMediaPayload::GeneratedVideo(value) => value.into_source(),
        NodeCapabilityProducedMediaPayload::SynthesizedSpeech(value) => value.into_source(),
    };
    let deadline = source.deadline();
    let stream = source.try_take_stream().map_err(|_| {
        if Instant::now() >= deadline {
            NodeCapabilityMediaBoundaryError::DeadlineExceeded
        } else {
            invalid_media()
        }
    })?;
    AssetRecordNodeOutputCommand::try_new(
        context.project_id,
        expected_kind,
        display_name,
        AssetWorkflowNodeOrigin::new(
            AssetOriginWorkflowId::from_uuid(origin.workflow_id().as_uuid())
                .map_err(|_| invalid_media())?,
            AssetOriginWorkflowRevision::new(origin.workflow_revision().get())
                .map_err(|_| invalid_media())?,
            AssetOriginWorkflowRunId::from_uuid(context.workflow_run_id.as_uuid())
                .map_err(|_| invalid_media())?,
            AssetOriginWorkflowNodeId::from_uuid(origin.workflow_node_id().as_uuid())
                .map_err(|_| invalid_media())?,
            AssetOriginWorkflowNodeExecutionId::from_uuid(context.node_execution_id.as_uuid())
                .map_err(|_| invalid_media())?,
            capability_ref(origin.capability_contract_ref())?,
        ),
        production,
        AssetNodeOutputKey::new(
            AssetOriginWorkflowRunId::from_uuid(output.workflow_run_id().as_uuid())
                .map_err(|_| invalid_media())?,
            AssetOriginWorkflowNodeExecutionId::from_uuid(output.node_execution_id().as_uuid())
                .map_err(|_| invalid_media())?,
            AssetOriginNodeOutputKey::try_new(output.output_key().as_str())
                .map_err(|_| invalid_media())?,
            output.ordinal(),
        ),
        AssetNodeOutputSourceLease::new(deadline, stream),
    )
    .map_err(map_asset_error)
}

fn capability_ref(
    value: &engine::node_capability::NodeCapabilityContractRef,
) -> Result<AssetOriginNodeCapabilityContractRef, NodeCapabilityMediaBoundaryError> {
    AssetOriginNodeCapabilityContractRef::try_new(
        value.id().as_str(),
        value.version().major(),
        value.version().minor(),
    )
    .map_err(|_| invalid_media())
}

fn production(
    value: NodeCapabilityProducedMediaProvenance,
) -> Result<AssetNodeOutputProduction, NodeCapabilityMediaBoundaryError> {
    match value {
        NodeCapabilityProducedMediaProvenance::ProviderGenerated(value) => {
            Ok(AssetNodeOutputProduction::ProviderGenerated {
                generation_profile_ref: profile_ref(value.generation_profile_ref())?,
            })
        }
        NodeCapabilityProducedMediaProvenance::ProviderDerived(value) => {
            Ok(AssetNodeOutputProduction::ProviderDerived {
                source_asset_ids: source_asset_ids(value.source_media_references())?,
                generation_profile_ref: profile_ref(value.generation_profile_ref())?,
            })
        }
    }
}

fn profile_ref(
    value: &nodes::GenerationProfileRef,
) -> Result<AssetOriginGenerationProfileRef, NodeCapabilityMediaBoundaryError> {
    AssetOriginGenerationProfileRef::try_new(value.id().as_str(), value.version().get())
        .map_err(|_| invalid_media())
}

fn source_asset_ids(
    values: &[nodes::NodeCapabilityManagedMediaReference],
) -> Result<AssetOriginSourceAssetIds, NodeCapabilityMediaBoundaryError> {
    let values = values
        .iter()
        .copied()
        .map(|value| {
            let id = match value {
                nodes::NodeCapabilityManagedMediaReference::Image(value) => value.asset_id(),
                nodes::NodeCapabilityManagedMediaReference::Video(value) => value.asset_id(),
                nodes::NodeCapabilityManagedMediaReference::Audio(value) => value.asset_id(),
            };
            asset_id(id).map(AssetOriginSourceAssetId::from_asset_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    AssetOriginSourceAssetIds::try_new(values).map_err(|_| invalid_media())
}

fn produced_reference(
    asset_id: AssetId,
    kind: AssetMediaKind,
    digest: [u8; 32],
) -> Result<NodeCapabilityProducedMediaReference, NodeCapabilityMediaBoundaryError> {
    let id = WorkflowManagedAssetIdBoundaryValue::from_bytes(asset_id.as_uuid().into_bytes())
        .map_err(|_| invalid_media())?;
    let fingerprint = WorkflowManagedContentFingerprint::from_bytes(digest);
    Ok(match kind {
        AssetMediaKind::Image => NodeCapabilityProducedMediaReference::Image(
            WorkflowManagedImageRef::new(id, fingerprint),
        ),
        AssetMediaKind::Video => NodeCapabilityProducedMediaReference::Video(
            WorkflowManagedVideoRef::new(id, fingerprint),
        ),
        AssetMediaKind::Audio => NodeCapabilityProducedMediaReference::Audio(
            WorkflowManagedAudioRef::new(id, fingerprint),
        ),
    })
}
