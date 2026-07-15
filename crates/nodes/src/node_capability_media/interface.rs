use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityMediaFailure, WorkflowManagedAudioRef, WorkflowManagedImageRef,
    WorkflowManagedVideoRef, WorkflowNodeExecutionContext,
};
use projects::project::domain::ProjectId;

use super::{
    GeneratedImagePayload, GeneratedVideoPayload, NodeCapabilityDeclaredMediaFacts,
    NodeCapabilityManagedMediaReference, NodeCapabilityMediaKind, NodeCapabilityMediaMimeType,
    NodeCapabilityMediaSourceLease, NodeCapabilityMediaValueError,
    NodeCapabilityProducedMediaDisplayName, NodeCapabilityProducedMediaOutputKey,
    NodeCapabilityProducedMediaProvenance, SynthesizedSpeechPayload, byte_length_within_kind_limit,
    output_key_matches_context,
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

/// Exact Project-scoped managed-media read request.
pub struct NodeCapabilityManagedMediaReadRequest {
    project_id: ProjectId,
    media_reference: NodeCapabilityManagedMediaReference,
    deadline: std::time::Instant,
}

impl NodeCapabilityManagedMediaReadRequest {
    /// Creates one typed deadline-bounded read request.
    #[must_use]
    pub const fn new(
        project_id: ProjectId,
        media_reference: NodeCapabilityManagedMediaReference,
        deadline: std::time::Instant,
    ) -> Self {
        Self { project_id, media_reference, deadline }
    }
    /// Returns the owning Project.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }
    /// Returns the typed managed-media reference.
    #[must_use]
    pub const fn media_reference(&self) -> NodeCapabilityManagedMediaReference {
        self.media_reference
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
    output_key: NodeCapabilityProducedMediaOutputKey,
    display_name: NodeCapabilityProducedMediaDisplayName,
    provenance: NodeCapabilityProducedMediaProvenance,
    payload: NodeCapabilityProducedMediaPayload,
}

impl NodeCapabilityProducedMediaWriteRequest {
    /// Creates a write only when output and execution coordinates agree.
    pub fn try_new(
        context: WorkflowNodeExecutionContext,
        output_key: NodeCapabilityProducedMediaOutputKey,
        display_name: NodeCapabilityProducedMediaDisplayName,
        provenance: NodeCapabilityProducedMediaProvenance,
        payload: NodeCapabilityProducedMediaPayload,
    ) -> Result<Self, NodeCapabilityMediaValueError> {
        if !output_key_matches_context(&output_key, &context) {
            return Err(NodeCapabilityMediaValueError::InvalidOutputCoordinates);
        }
        Ok(Self { context, output_key, display_name, provenance, payload })
    }
    /// Returns execution identity, deadline, and cancellation.
    #[must_use]
    pub const fn context(&self) -> &WorkflowNodeExecutionContext {
        &self.context
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

/// Reads exact Project-visible managed media for node capabilities.
#[async_trait]
pub trait NodeCapabilityManagedMediaReaderInterface: Send + Sync {
    /// Reads one typed exact-content reference.
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
