use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityMediaFailure, WorkflowManagedAssetIdBoundaryValue, WorkflowManagedAudioRef,
    WorkflowManagedImageRef, WorkflowManagedVideoRef, WorkflowNodeExecutionContext,
    WorkflowNodeExecutionOrigin,
};
use projects::project::domain::ProjectId;

use super::{
    GeneratedImagePayload, GeneratedVideoPayload, NodeCapabilityDeclaredMediaFacts,
    NodeCapabilityManagedMediaReference, NodeCapabilityMediaKind, NodeCapabilityMediaMimeType,
    NodeCapabilityMediaSourceLease, NodeCapabilityMediaValueError,
    NodeCapabilityProducedMediaDisplayName, NodeCapabilityProducedMediaOutputKey,
    NodeCapabilityProducedMediaProvenance, SynthesizedSpeechPayload, byte_length_within_kind_limit,
};

/// Media-boundary failure preserving cancellation and deadline categories.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum NodeCapabilityMediaBoundaryError {
    /// Exact managed-media failure.
    #[error("node capability managed media operation failed")]
    Media(NodeCapabilityMediaFailure),
    /// Execution cancellation was observed.
    #[error("node capability managed media operation cancelled")]
    Cancelled,
    /// Call-scoped deadline was observed.
    #[error("node capability managed media deadline exceeded")]
    DeadlineExceeded,
}

/// Asset-ID selection with one required media kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeCapabilityAssetIdMediaReadSelection {
    asset_id: WorkflowManagedAssetIdBoundaryValue,
    expected_media_kind: NodeCapabilityMediaKind,
}

impl NodeCapabilityAssetIdMediaReadSelection {
    /// Selects one Asset ID with its required media kind.
    #[must_use]
    pub const fn new(
        asset_id: WorkflowManagedAssetIdBoundaryValue,
        expected_media_kind: NodeCapabilityMediaKind,
    ) -> Self {
        Self { asset_id, expected_media_kind }
    }
    /// Returns the selected Asset ID.
    #[must_use]
    pub const fn asset_id(self) -> WorkflowManagedAssetIdBoundaryValue {
        self.asset_id
    }
    /// Returns the required media kind.
    #[must_use]
    pub const fn expected_media_kind(self) -> NodeCapabilityMediaKind {
        self.expected_media_kind
    }
}

/// Exact managed-media selection accepted by the reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityManagedMediaReadSelection {
    /// Resolve an Asset ID inside one Project.
    AssetId(NodeCapabilityAssetIdMediaReadSelection),
    /// Read one exact Available managed-media reference.
    ExactReference(NodeCapabilityManagedMediaReference),
}

/// Exact Project-scoped managed-media read request.
pub struct NodeCapabilityManagedMediaReadRequest {
    project_id: ProjectId,
    selection: NodeCapabilityManagedMediaReadSelection,
    deadline: std::time::Instant,
}

impl NodeCapabilityManagedMediaReadRequest {
    /// Creates one typed deadline-bounded read request.
    #[must_use]
    pub const fn new(
        project_id: ProjectId,
        selection: NodeCapabilityManagedMediaReadSelection,
        deadline: std::time::Instant,
    ) -> Self {
        Self { project_id, selection, deadline }
    }
    /// Returns the owning Project.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }
    /// Returns the Asset-ID or exact-reference read selection.
    #[must_use]
    pub const fn selection(&self) -> NodeCapabilityManagedMediaReadSelection {
        self.selection
    }
    /// Returns the exact monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> std::time::Instant {
        self.deadline
    }
}

macro_rules! readable_media_input {
    ($name:ident, $reference:ty, $kind:expr) => {
        #[doc = concat!("Readable exact ", stringify!($name), " managed-media input.")]
        pub struct $name {
            media_reference: $reference,
            mime_type: NodeCapabilityMediaMimeType,
            facts: NodeCapabilityDeclaredMediaFacts,
            source: NodeCapabilityMediaSourceLease,
        }
        impl $name {
            /// Validates reference fingerprint, kind, MIME, facts, and length.
            pub fn try_new(
                media_reference: $reference,
                mime_type: NodeCapabilityMediaMimeType,
                facts: NodeCapabilityDeclaredMediaFacts,
                source: NodeCapabilityMediaSourceLease,
            ) -> Result<Self, NodeCapabilityMediaValueError> {
                if media_reference.content_fingerprint().as_bytes() != source.digest().as_bytes() {
                    return Err(NodeCapabilityMediaValueError::ContentFingerprintMismatch);
                }
                if mime_type.media_kind() != $kind {
                    return Err(NodeCapabilityMediaValueError::InvalidMimeForKind);
                }
                if facts.media_kind() != $kind {
                    return Err(NodeCapabilityMediaValueError::InvalidMediaFacts);
                }
                if !byte_length_within_kind_limit(source.byte_length(), $kind) {
                    return Err(NodeCapabilityMediaValueError::InvalidByteLength);
                }
                Ok(Self { media_reference, mime_type, facts, source })
            }
            /// Returns the exact managed reference.
            #[must_use]
            pub const fn media_reference(&self) -> $reference {
                self.media_reference
            }
            /// Returns verified MIME.
            #[must_use]
            pub const fn mime_type(&self) -> NodeCapabilityMediaMimeType {
                self.mime_type
            }
            /// Returns declared media facts.
            #[must_use]
            pub const fn facts(&self) -> NodeCapabilityDeclaredMediaFacts {
                self.facts
            }
            /// Returns the one-shot source lease.
            #[must_use]
            pub const fn source(&self) -> &NodeCapabilityMediaSourceLease {
                &self.source
            }
            /// Consumes the input and returns its source lease.
            #[must_use]
            pub fn into_source(self) -> NodeCapabilityMediaSourceLease {
                self.source
            }
        }
    };
}

readable_media_input!(
    NodeCapabilityReadableImageInput,
    WorkflowManagedImageRef,
    NodeCapabilityMediaKind::Image
);
readable_media_input!(
    NodeCapabilityReadableVideoInput,
    WorkflowManagedVideoRef,
    NodeCapabilityMediaKind::Video
);
readable_media_input!(
    NodeCapabilityReadableAudioInput,
    WorkflowManagedAudioRef,
    NodeCapabilityMediaKind::Audio
);

/// Closed typed readable managed-media result.
pub enum NodeCapabilityReadableMediaInput {
    /// Readable Image.
    Image(NodeCapabilityReadableImageInput),
    /// Readable Video.
    Video(NodeCapabilityReadableVideoInput),
    /// Readable Audio.
    Audio(NodeCapabilityReadableAudioInput),
}

impl NodeCapabilityReadableMediaInput {
    /// Returns the exact readable managed-media kind.
    #[must_use]
    pub const fn media_kind(&self) -> NodeCapabilityMediaKind {
        match self {
            Self::Image(_) => NodeCapabilityMediaKind::Image,
            Self::Video(_) => NodeCapabilityMediaKind::Video,
            Self::Audio(_) => NodeCapabilityMediaKind::Audio,
        }
    }
}

/// Closed typed produced media payload.
pub enum NodeCapabilityProducedMediaPayload {
    /// Generated Image payload.
    GeneratedImage(GeneratedImagePayload),
    /// Generated Video payload.
    GeneratedVideo(GeneratedVideoPayload),
    /// Synthesized speech Audio payload.
    SynthesizedSpeech(SynthesizedSpeechPayload),
}

impl NodeCapabilityProducedMediaPayload {
    /// Returns the payload's exact media kind.
    #[must_use]
    pub const fn media_kind(&self) -> NodeCapabilityMediaKind {
        match self {
            Self::GeneratedImage(_) => NodeCapabilityMediaKind::Image,
            Self::GeneratedVideo(_) => NodeCapabilityMediaKind::Video,
            Self::SynthesizedSpeech(_) => NodeCapabilityMediaKind::Audio,
        }
    }
    /// Returns the payload's exact content digest.
    #[must_use]
    pub const fn digest(&self) -> super::NodeCapabilityMediaContentDigest {
        match self {
            Self::GeneratedImage(value) => value.source().digest(),
            Self::GeneratedVideo(value) => value.source().digest(),
            Self::SynthesizedSpeech(value) => value.source().digest(),
        }
    }
}

/// Exact produced-media publication request.
pub struct NodeCapabilityProducedMediaWriteRequest {
    context: WorkflowNodeExecutionContext,
    origin: WorkflowNodeExecutionOrigin,
    output_key: NodeCapabilityProducedMediaOutputKey,
    display_name: NodeCapabilityProducedMediaDisplayName,
    provenance: NodeCapabilityProducedMediaProvenance,
    payload: NodeCapabilityProducedMediaPayload,
}

impl NodeCapabilityProducedMediaWriteRequest {
    /// Creates a write only when output and execution coordinates agree.
    pub fn try_new(
        context: WorkflowNodeExecutionContext,
        origin: WorkflowNodeExecutionOrigin,
        output_key: NodeCapabilityProducedMediaOutputKey,
        display_name: NodeCapabilityProducedMediaDisplayName,
        provenance: NodeCapabilityProducedMediaProvenance,
        payload: NodeCapabilityProducedMediaPayload,
    ) -> Result<Self, NodeCapabilityMediaValueError> {
        if !output_key_matches_context(&output_key, &context) {
            return Err(NodeCapabilityMediaValueError::InvalidOutputCoordinates);
        }
        Ok(Self { context, origin, output_key, display_name, provenance, payload })
    }
    /// Returns execution identity, deadline, and cancellation.
    #[must_use]
    pub const fn context(&self) -> &WorkflowNodeExecutionContext {
        &self.context
    }
    /// Returns the unchanged frozen Workflow producer coordinates.
    #[must_use]
    pub const fn origin(&self) -> &WorkflowNodeExecutionOrigin {
        &self.origin
    }
    /// Returns the exact output slot identity.
    #[must_use]
    pub const fn output_key(&self) -> &NodeCapabilityProducedMediaOutputKey {
        &self.output_key
    }
    /// Returns the user-visible produced-media name.
    #[must_use]
    pub const fn display_name(&self) -> &NodeCapabilityProducedMediaDisplayName {
        &self.display_name
    }
    /// Returns exact semantic provenance.
    #[must_use]
    pub const fn provenance(&self) -> &NodeCapabilityProducedMediaProvenance {
        &self.provenance
    }
    /// Returns the typed payload.
    #[must_use]
    pub const fn payload(&self) -> &NodeCapabilityProducedMediaPayload {
        &self.payload
    }
    /// Consumes the request and returns its payload.
    #[must_use]
    pub fn into_payload(self) -> NodeCapabilityProducedMediaPayload {
        self.payload
    }
}

/// Closed typed Available reference returned after media publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityProducedMediaReference {
    /// Available managed Image.
    Image(WorkflowManagedImageRef),
    /// Available managed Video.
    Video(WorkflowManagedVideoRef),
    /// Available managed Audio.
    Audio(WorkflowManagedAudioRef),
}

impl NodeCapabilityProducedMediaReference {
    /// Returns the exact produced managed-media kind.
    #[must_use]
    pub const fn media_kind(&self) -> NodeCapabilityMediaKind {
        match self {
            Self::Image(_) => NodeCapabilityMediaKind::Image,
            Self::Video(_) => NodeCapabilityMediaKind::Video,
            Self::Audio(_) => NodeCapabilityMediaKind::Audio,
        }
    }
}

/// Reads exact Project-visible managed media for node capabilities.
#[async_trait]
pub trait NodeCapabilityManagedMediaReaderInterface: Send + Sync {
    /// Resolves and reads one typed managed-media selection.
    async fn read_managed_media(
        &self,
        request: NodeCapabilityManagedMediaReadRequest,
    ) -> Result<NodeCapabilityReadableMediaInput, NodeCapabilityMediaBoundaryError>;
}

/// Publishes one exact provider-produced media output as Available managed media.
#[async_trait]
pub trait NodeCapabilityProducedMediaWriterInterface: Send + Sync {
    /// Writes one exact output slot and returns only its matching Available reference.
    async fn write_node_output_media(
        &self,
        request: NodeCapabilityProducedMediaWriteRequest,
    ) -> Result<NodeCapabilityProducedMediaReference, NodeCapabilityMediaBoundaryError>;
}

fn output_key_matches_context(
    output_key: &NodeCapabilityProducedMediaOutputKey,
    context: &WorkflowNodeExecutionContext,
) -> bool {
    output_key.workflow_run_id() == context.workflow_run_id
        && output_key.node_execution_id() == context.node_execution_id
}
