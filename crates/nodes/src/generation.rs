//! Consumer-owned contracts for generated media capabilities.

use thiserror::Error;

/// A text-to-image generation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextToImageRequest {
    /// Opaque model identifier selected by the workflow.
    pub model: String,
    /// Positive generation prompt.
    pub prompt: String,
    /// Optional negative prompt.
    pub negative_prompt: Option<String>,
    /// Optional number of generation steps.
    pub steps: Option<u32>,
    /// Optional reproducibility seed.
    pub seed: Option<u64>,
}

/// An image generation request using ordered image references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceImageGenerationRequest {
    /// Opaque model identifier selected by the workflow.
    pub model: String,
    /// Ordered local image paths resolved from the asset store.
    pub images: Vec<String>,
    /// Positive generation prompt.
    pub prompt: String,
    /// Optional negative prompt.
    pub negative_prompt: Option<String>,
    /// Optional number of generation steps.
    pub steps: Option<u32>,
    /// Optional reproducibility seed.
    pub seed: Option<u64>,
}

/// An image-to-video generation request.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageToVideoRequest {
    /// Opaque model identifier selected by the workflow.
    pub model: String,
    /// Local source-image path resolved from the asset store.
    pub image: String,
    /// Optional requested clip duration in seconds.
    pub duration_seconds: Option<f32>,
    /// Optional requested frame rate.
    pub fps: Option<u32>,
}

/// A video generation request using ordered image references.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceVideoGenerationRequest {
    /// Opaque model identifier selected by the workflow.
    pub model: String,
    /// Ordered local image paths resolved from the asset store.
    pub images: Vec<String>,
    /// Positive generation prompt.
    pub prompt: String,
    /// Optional requested clip duration in seconds.
    pub duration_seconds: Option<f32>,
    /// Optional requested display aspect ratio.
    pub aspect_ratio: Option<String>,
    /// Optional requested output resolution.
    pub resolution: Option<String>,
    /// Optional requested frame rate.
    pub fps: Option<u32>,
}

/// A text-to-audio generation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextToAudioRequest {
    /// Opaque model identifier selected by the workflow.
    pub model: String,
    /// Positive generation prompt.
    pub prompt: String,
    /// Optional reproducibility seed.
    pub seed: Option<u64>,
}

/// The business modality carried by inline media.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    /// Still-image media.
    Image,
    /// Moving-image media.
    Video,
    /// Audio media.
    Audio,
}

impl MediaKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
        }
    }
}

/// The encoded format carried by inline media.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaFormat {
    /// Portable Network Graphics image bytes.
    Png,
    /// Waveform Audio File Format bytes.
    Wav,
    /// Opaque video bytes whose codec is not advertised as playable.
    OpaqueVideo,
}

impl MediaFormat {
    pub(crate) fn file_suffix(self) -> &'static str {
        match self {
            Self::Png => ".png",
            Self::Wav => ".wav",
            Self::OpaqueVideo => ".video-data",
        }
    }
}

/// Encoded media returned directly by a generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineMedia {
    kind: MediaKind,
    format: MediaFormat,
    bytes: Vec<u8>,
}

impl InlineMedia {
    /// Creates inline PNG image media.
    #[must_use]
    pub fn png(bytes: Vec<u8>) -> Self {
        Self { kind: MediaKind::Image, format: MediaFormat::Png, bytes }
    }

    /// Creates inline WAV audio media.
    #[must_use]
    pub fn wav(bytes: Vec<u8>) -> Self {
        Self { kind: MediaKind::Audio, format: MediaFormat::Wav, bytes }
    }

    /// Creates inline opaque video media.
    #[must_use]
    pub fn opaque_video(bytes: Vec<u8>) -> Self {
        Self { kind: MediaKind::Video, format: MediaFormat::OpaqueVideo, bytes }
    }

    /// Returns the media modality.
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    /// Returns the encoded media format.
    #[must_use]
    pub fn format(&self) -> MediaFormat {
        self.format
    }

    /// Returns the encoded bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A typed artifact returned by a generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedArtifact {
    /// A remote URL that requires a separate resolver before local persistence.
    RemoteUrl(String),
    /// Encoded media available in the current process.
    InlineMedia(InlineMedia),
}

/// Generated media and its optional estimated cost in micro-USD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedOutput {
    /// The generated artifact.
    pub artifact: GeneratedArtifact,
    /// Optional estimated cost in micro-USD.
    pub cost: Option<i64>,
}

/// Failures exposed by a generation capability implementation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GenerationError {
    /// Submitting or checking a generation operation failed.
    #[error("{operation} failed: {reason}")]
    OperationFailed {
        /// Stable operation description.
        operation: &'static str,
        /// Adapter-normalized failure reason.
        reason: String,
    },

    /// A submitted generation task reached a failed terminal state.
    #[error("generation task failed: {reason}")]
    TaskFailed {
        /// Provider-normalized failure reason.
        reason: String,
    },

    /// A submitted generation task was cancelled.
    #[error("generation task was cancelled")]
    TaskCancelled,

    /// A submitted generation task exceeded the bounded polling policy.
    #[error("generation task did not complete after {max_polls} polls")]
    PollLimit {
        /// Maximum number of status checks attempted.
        max_polls: usize,
    },

    /// The adapter received an output that did not match its provider contract.
    #[error("generation service returned invalid output")]
    InvalidOutput,
}

/// Run-scoped controls exposed to generation capability implementations.
pub trait GenerationContext {
    /// Reports normalized progress for the current workflow node.
    fn progress(&mut self, progress: f32);

    /// Returns whether the owning workflow run requested cancellation.
    fn is_cancelled(&self) -> bool;

    /// Rejects work after the owning workflow run requested cancellation.
    fn ensure_active(&self) -> Result<(), GenerationError> {
        if self.is_cancelled() { Err(GenerationError::TaskCancelled) } else { Ok(()) }
    }
}

impl GenerationContext for engine::NodeRunContext<'_> {
    fn progress(&mut self, progress: f32) {
        engine::NodeRunContext::progress(self, progress);
    }

    fn is_cancelled(&self) -> bool {
        engine::NodeRunContext::is_cancelled(self)
    }
}

/// Generates image media from text.
pub trait TextToImageGenerator: Send + Sync {
    /// Generates an image under the current run controls.
    fn generate(
        &self,
        request: TextToImageRequest,
        context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError>;
}

/// Generates image media from ordered local image references and text.
pub trait ReferenceImageGenerator: Send + Sync {
    /// Generates an image under the current run controls.
    fn generate(
        &self,
        request: ReferenceImageGenerationRequest,
        context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError>;
}

/// Generates video media from a local image.
pub trait ImageToVideoGenerator: Send + Sync {
    /// Generates a video under the current run controls.
    fn generate(
        &self,
        request: ImageToVideoRequest,
        context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError>;
}

/// Generates video media from ordered local image references and text.
pub trait ReferenceVideoGenerator: Send + Sync {
    /// Generates a video under the current run controls.
    fn generate(
        &self,
        request: ReferenceVideoGenerationRequest,
        context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError>;
}

/// Generates audio media from text.
pub trait TextToAudioGenerator: Send + Sync {
    /// Generates audio under the current run controls.
    fn generate(
        &self,
        request: TextToAudioRequest,
        context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError>;
}
