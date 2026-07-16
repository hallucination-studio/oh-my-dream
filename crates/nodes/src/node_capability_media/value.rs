use std::pin::Pin;
use std::time::Instant;

use crate::GenerationProfileRef;
use engine::node_capability::{
    NodeCapabilityOutputKey, WorkflowManagedAudioRef, WorkflowManagedImageRef,
    WorkflowManagedVideoRef, WorkflowNodeExecutionId, WorkflowRunId,
};
use tokio::io::AsyncRead;

/// Invalid typed media boundary value or expired source handoff.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum NodeCapabilityMediaValueError {
    /// MIME does not belong to the declared media kind.
    #[error("node capability media MIME does not match its kind")]
    InvalidMimeForKind,
    /// Declared technical media facts are invalid.
    #[error("node capability media facts are invalid")]
    InvalidMediaFacts,
    /// Byte length is zero or exceeds the kind limit.
    #[error("node capability media byte length is invalid")]
    InvalidByteLength,
    /// Managed reference fingerprint differs from the source digest.
    #[error("node capability media fingerprint does not match content")]
    ContentFingerprintMismatch,
    /// Produced output coordinates disagree with execution context.
    #[error("node capability produced output coordinates are invalid")]
    InvalidOutputCoordinates,
    /// Produced-media provenance is empty or oversized.
    #[error("node capability produced media provenance is invalid")]
    InvalidProvenance,
    /// Produced-media display name is invalid.
    #[error("node capability produced media display name is invalid")]
    InvalidDisplayName,
    /// Source handoff reached its process-monotonic deadline.
    #[error("node capability media source deadline exceeded")]
    DeadlineExceeded,
}

/// Exact media MIME values accepted by the MVP.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum NodeCapabilityMediaMimeType {
    /// PNG image.
    ImagePng,
    /// JPEG image.
    ImageJpeg,
    /// WebP image.
    ImageWebp,
    /// MP4 video.
    VideoMp4,
    /// WebM video.
    VideoWebm,
    /// MPEG audio.
    AudioMpeg,
    /// WAV audio.
    AudioWav,
    /// Ogg audio.
    AudioOgg,
}

/// Declared media kind derived from MIME and facts.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum NodeCapabilityMediaKind {
    /// Image media.
    Image,
    /// Video media.
    Video,
    /// Audio media.
    Audio,
}

impl NodeCapabilityMediaKind {
    pub(crate) const fn to_workflow_data_type(self) -> engine::node_capability::WorkflowDataType {
        match self {
            Self::Image => engine::node_capability::WorkflowDataType::Image,
            Self::Video => engine::node_capability::WorkflowDataType::Video,
            Self::Audio => engine::node_capability::WorkflowDataType::Audio,
        }
    }
}

impl NodeCapabilityMediaMimeType {
    /// Returns the MIME's exact media kind.
    #[must_use]
    pub const fn media_kind(self) -> NodeCapabilityMediaKind {
        match self {
            Self::ImagePng | Self::ImageJpeg | Self::ImageWebp => NodeCapabilityMediaKind::Image,
            Self::VideoMp4 | Self::VideoWebm => NodeCapabilityMediaKind::Video,
            Self::AudioMpeg | Self::AudioWav | Self::AudioOgg => NodeCapabilityMediaKind::Audio,
        }
    }
}

/// Validated technical facts declared for media bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityDeclaredMediaFacts {
    /// Image dimensions.
    Image(NodeCapabilityImageMediaFacts),
    /// Video dimensions, duration, and audio presence.
    Video(NodeCapabilityVideoMediaFacts),
    /// Audio duration, sample rate, and channels.
    Audio(NodeCapabilityAudioMediaFacts),
}

/// Validated image dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeCapabilityImageMediaFacts {
    width: u32,
    height: u32,
}
/// Validated video dimensions, duration, and audio presence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeCapabilityVideoMediaFacts {
    width: u32,
    height: u32,
    duration_ms: u64,
    has_audio: bool,
}
/// Validated audio duration, sample rate, and channel count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeCapabilityAudioMediaFacts {
    duration_ms: u64,
    sample_rate_hz: u32,
    channels: u8,
}

impl NodeCapabilityImageMediaFacts {
    /// Returns the exact pixel width.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the exact pixel height.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
}

impl NodeCapabilityDeclaredMediaFacts {
    /// Creates validated Image facts.
    pub const fn try_image(width: u32, height: u32) -> Result<Self, NodeCapabilityMediaValueError> {
        if valid_dimension(width) && valid_dimension(height) {
            Ok(Self::Image(NodeCapabilityImageMediaFacts { width, height }))
        } else {
            Err(NodeCapabilityMediaValueError::InvalidMediaFacts)
        }
    }
    /// Creates validated Video facts.
    pub const fn try_video(
        width: u32,
        height: u32,
        duration_ms: u64,
        has_audio: bool,
    ) -> Result<Self, NodeCapabilityMediaValueError> {
        if valid_dimension(width) && valid_dimension(height) && valid_duration(duration_ms) {
            Ok(Self::Video(NodeCapabilityVideoMediaFacts { width, height, duration_ms, has_audio }))
        } else {
            Err(NodeCapabilityMediaValueError::InvalidMediaFacts)
        }
    }
    /// Creates validated Audio facts.
    pub const fn try_audio(
        duration_ms: u64,
        sample_rate_hz: u32,
        channels: u8,
    ) -> Result<Self, NodeCapabilityMediaValueError> {
        if valid_duration(duration_ms)
            && sample_rate_hz >= 8_000
            && sample_rate_hz <= 192_000
            && channels >= 1
            && channels <= 8
        {
            Ok(Self::Audio(NodeCapabilityAudioMediaFacts { duration_ms, sample_rate_hz, channels }))
        } else {
            Err(NodeCapabilityMediaValueError::InvalidMediaFacts)
        }
    }
    /// Returns the facts' exact media kind.
    #[must_use]
    pub const fn media_kind(self) -> NodeCapabilityMediaKind {
        match self {
            Self::Image(_) => NodeCapabilityMediaKind::Image,
            Self::Video(_) => NodeCapabilityMediaKind::Video,
            Self::Audio(_) => NodeCapabilityMediaKind::Audio,
        }
    }
}

const fn valid_dimension(value: u32) -> bool {
    value >= 1 && value <= 16_384
}
const fn valid_duration(value: u64) -> bool {
    value >= 1 && value <= 86_400_000
}

/// Exact SHA-256 digest bytes for one media source.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct NodeCapabilityMediaContentDigest([u8; 32]);

impl NodeCapabilityMediaContentDigest {
    /// Restores exact digest bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Returns exact digest bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One-shot exact-length, exact-digest media byte source.
pub struct NodeCapabilityMediaSourceLease {
    byte_length: u64,
    digest: NodeCapabilityMediaContentDigest,
    deadline: Instant,
    stream: Pin<Box<dyn AsyncRead + Send>>,
}

impl NodeCapabilityMediaSourceLease {
    /// Creates a non-empty one-shot source.
    pub fn try_new(
        byte_length: u64,
        digest: NodeCapabilityMediaContentDigest,
        deadline: Instant,
        stream: Pin<Box<dyn AsyncRead + Send>>,
    ) -> Result<Self, NodeCapabilityMediaValueError> {
        if byte_length == 0 {
            return Err(NodeCapabilityMediaValueError::InvalidByteLength);
        }
        Ok(Self { byte_length, digest, deadline, stream })
    }
    /// Returns the exact expected byte length.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    /// Returns the exact expected digest.
    #[must_use]
    pub const fn digest(&self) -> NodeCapabilityMediaContentDigest {
        self.digest
    }
    /// Returns the exact process-monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }
    /// Consumes the lease and hands off its stream before the deadline.
    pub fn try_take_stream(
        self,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, NodeCapabilityMediaValueError> {
        if Instant::now() >= self.deadline {
            Err(NodeCapabilityMediaValueError::DeadlineExceeded)
        } else {
            Ok(self.stream)
        }
    }
}

/// Typed available managed-media reference selected by a capability.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeCapabilityManagedMediaReference {
    /// Managed Image reference.
    Image(WorkflowManagedImageRef),
    /// Managed Video reference.
    Video(WorkflowManagedVideoRef),
    /// Managed Audio reference.
    Audio(WorkflowManagedAudioRef),
}

impl NodeCapabilityManagedMediaReference {
    /// Returns the reference's exact media kind.
    #[must_use]
    pub const fn media_kind(self) -> NodeCapabilityMediaKind {
        match self {
            Self::Image(_) => NodeCapabilityMediaKind::Image,
            Self::Video(_) => NodeCapabilityMediaKind::Video,
            Self::Audio(_) => NodeCapabilityMediaKind::Audio,
        }
    }
}

/// Durable idempotency identity for one produced output slot.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeCapabilityProducedMediaOutputKey {
    workflow_run_id: WorkflowRunId,
    node_execution_id: WorkflowNodeExecutionId,
    output_key: NodeCapabilityOutputKey,
    ordinal: u32,
}

impl NodeCapabilityProducedMediaOutputKey {
    /// Creates one exact output slot identity.
    #[must_use]
    pub const fn new(
        workflow_run_id: WorkflowRunId,
        node_execution_id: WorkflowNodeExecutionId,
        output_key: NodeCapabilityOutputKey,
        ordinal: u32,
    ) -> Self {
        Self { workflow_run_id, node_execution_id, output_key, ordinal }
    }
    /// Returns the Workflow Run identity.
    #[must_use]
    pub const fn workflow_run_id(&self) -> WorkflowRunId {
        self.workflow_run_id
    }
    /// Returns the node execution identity.
    #[must_use]
    pub const fn node_execution_id(&self) -> WorkflowNodeExecutionId {
        self.node_execution_id
    }
    /// Returns the declared output key.
    #[must_use]
    pub const fn output_key(&self) -> &NodeCapabilityOutputKey {
        &self.output_key
    }
    /// Returns the zero-based output ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

/// Validated user-visible name for one produced media output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityProducedMediaDisplayName(String);

impl NodeCapabilityProducedMediaDisplayName {
    /// Validates trimmed non-control display text.
    pub fn try_new(value: impl Into<String>) -> Result<Self, NodeCapabilityMediaValueError> {
        let value = value.into();
        let count = value.chars().count();
        if value.trim() != value
            || !(1..=80).contains(&count)
            || value.chars().any(char::is_control)
        {
            return Err(NodeCapabilityMediaValueError::InvalidDisplayName);
        }
        Ok(Self(value))
    }
    /// Returns display text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Exact semantic provenance of one provider-produced media output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCapabilityProducedMediaProvenance {
    /// Provider output without source managed media.
    ProviderGenerated(NodeCapabilityProviderGeneratedMediaProvenance),
    /// Provider output conditioned by ordered source managed media.
    ProviderDerived(NodeCapabilityProviderDerivedMediaProvenance),
}

/// Provider-generated provenance without source managed media.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityProviderGeneratedMediaProvenance {
    profile_ref: GenerationProfileRef,
}

/// Provider-derived provenance with validated ordered sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityProviderDerivedMediaProvenance {
    source_media_refs: Vec<NodeCapabilityManagedMediaReference>,
    profile_ref: GenerationProfileRef,
}

impl NodeCapabilityProviderGeneratedMediaProvenance {
    /// Returns the exact profile that generated the media.
    #[must_use]
    pub const fn generation_profile_ref(&self) -> &GenerationProfileRef {
        &self.profile_ref
    }
}

impl NodeCapabilityProviderDerivedMediaProvenance {
    /// Returns ordered exact source media references.
    #[must_use]
    pub fn source_media_references(&self) -> &[NodeCapabilityManagedMediaReference] {
        &self.source_media_refs
    }
    /// Returns the exact profile that derived the media.
    #[must_use]
    pub const fn generation_profile_ref(&self) -> &GenerationProfileRef {
        &self.profile_ref
    }
}

impl NodeCapabilityProducedMediaProvenance {
    /// Creates provider-generated provenance.
    #[must_use]
    pub const fn provider_generated(profile_ref: GenerationProfileRef) -> Self {
        Self::ProviderGenerated(NodeCapabilityProviderGeneratedMediaProvenance { profile_ref })
    }
    /// Creates provider-derived provenance with `1..=64` ordered sources.
    pub fn try_provider_derived(
        source_media_refs: Vec<NodeCapabilityManagedMediaReference>,
        profile_ref: GenerationProfileRef,
    ) -> Result<Self, NodeCapabilityMediaValueError> {
        if source_media_refs.is_empty() || source_media_refs.len() > 64 {
            return Err(NodeCapabilityMediaValueError::InvalidProvenance);
        }
        Ok(Self::ProviderDerived(NodeCapabilityProviderDerivedMediaProvenance {
            source_media_refs,
            profile_ref,
        }))
    }
}
