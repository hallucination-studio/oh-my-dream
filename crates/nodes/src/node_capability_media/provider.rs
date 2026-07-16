use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityProviderFailure, WorkflowNodeExecutionContext, WorkflowTextValue,
};

use crate::GenerationProfileRef;

use super::{
    NodeCapabilityDeclaredMediaFacts, NodeCapabilityMediaKind, NodeCapabilityMediaSourceLease,
    NodeCapabilityMediaValueError, NodeCapabilityReadableImageInput, byte_length_within_kind_limit,
};

/// Provider-independent text-to-image aspect ratio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageAspectRatio {
    /// Square 1:1.
    Square,
    /// Landscape 4:3.
    LandscapeFourByThree,
    /// Portrait 3:4.
    PortraitThreeByFour,
    /// Landscape 16:9.
    LandscapeSixteenByNine,
    /// Portrait 9:16.
    PortraitNineBySixteen,
}

/// Provider-independent image-to-video duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageToVideoDurationSeconds {
    /// Five seconds.
    Five,
    /// Ten seconds.
    Ten,
}

macro_rules! generated_payload {
    ($name:ident, $kind:expr, $mime_type:expr, $facts_pattern:pat) => {
        #[doc = concat!("Validated exact ", stringify!($name), " provider payload.")]
        pub struct $name {
            facts: NodeCapabilityDeclaredMediaFacts,
            source: NodeCapabilityMediaSourceLease,
        }
        impl $name {
            /// Creates a payload only with exact kind facts and bounded bytes.
            pub fn try_new(
                facts: NodeCapabilityDeclaredMediaFacts,
                source: NodeCapabilityMediaSourceLease,
            ) -> Result<Self, NodeCapabilityMediaValueError> {
                if !matches!(facts, $facts_pattern) {
                    return Err(NodeCapabilityMediaValueError::InvalidMediaFacts);
                }
                if !byte_length_within_kind_limit(source.byte_length(), $kind) {
                    return Err(NodeCapabilityMediaValueError::InvalidByteLength);
                }
                Ok(Self { facts, source })
            }
            /// Returns declared media facts.
            #[must_use]
            pub const fn facts(&self) -> NodeCapabilityDeclaredMediaFacts {
                self.facts
            }
            /// Returns the only MIME emitted by this provider payload.
            #[must_use]
            pub const fn mime_type(&self) -> super::NodeCapabilityMediaMimeType {
                $mime_type
            }
            /// Returns the one-shot exact-content source.
            #[must_use]
            pub const fn source(&self) -> &NodeCapabilityMediaSourceLease {
                &self.source
            }
            /// Consumes the payload and returns its source.
            #[must_use]
            pub fn into_source(self) -> NodeCapabilityMediaSourceLease {
                self.source
            }
        }
    };
}

generated_payload!(
    GeneratedImagePayload,
    NodeCapabilityMediaKind::Image,
    super::NodeCapabilityMediaMimeType::ImagePng,
    NodeCapabilityDeclaredMediaFacts::Image(_)
);
generated_payload!(
    GeneratedVideoPayload,
    NodeCapabilityMediaKind::Video,
    super::NodeCapabilityMediaMimeType::VideoMp4,
    NodeCapabilityDeclaredMediaFacts::Video(_)
);
generated_payload!(
    SynthesizedSpeechPayload,
    NodeCapabilityMediaKind::Audio,
    super::NodeCapabilityMediaMimeType::AudioMpeg,
    NodeCapabilityDeclaredMediaFacts::Audio(_)
);

/// Exact semantic text-to-image provider request.
pub struct TextToImageProviderRequest {
    profile_ref: GenerationProfileRef,
    context: WorkflowNodeExecutionContext,
    prompt: WorkflowTextValue,
    aspect_ratio: ImageAspectRatio,
}

impl TextToImageProviderRequest {
    /// Creates one complete provider-independent request.
    #[must_use]
    pub const fn new(
        profile_ref: GenerationProfileRef,
        context: WorkflowNodeExecutionContext,
        prompt: WorkflowTextValue,
        aspect_ratio: ImageAspectRatio,
    ) -> Self {
        Self { profile_ref, context, prompt, aspect_ratio }
    }
    /// Returns the stable Generation Profile selection.
    #[must_use]
    pub const fn profile_ref(&self) -> &GenerationProfileRef {
        &self.profile_ref
    }
    /// Returns execution identity, deadline, and cancellation.
    #[must_use]
    pub const fn context(&self) -> &WorkflowNodeExecutionContext {
        &self.context
    }
    /// Returns normalized semantic prompt text.
    #[must_use]
    pub const fn prompt(&self) -> &WorkflowTextValue {
        &self.prompt
    }
    /// Returns the provider-independent aspect ratio.
    #[must_use]
    pub const fn aspect_ratio(&self) -> ImageAspectRatio {
        self.aspect_ratio
    }
}

/// Exact semantic image-to-video provider request.
pub struct ImageToVideoProviderRequest {
    profile_ref: GenerationProfileRef,
    context: WorkflowNodeExecutionContext,
    image: NodeCapabilityReadableImageInput,
    prompt: Option<WorkflowTextValue>,
    duration_seconds: ImageToVideoDurationSeconds,
}

impl ImageToVideoProviderRequest {
    /// Creates one complete provider-independent request.
    #[must_use]
    pub const fn new(
        profile_ref: GenerationProfileRef,
        context: WorkflowNodeExecutionContext,
        image: NodeCapabilityReadableImageInput,
        prompt: Option<WorkflowTextValue>,
        duration_seconds: ImageToVideoDurationSeconds,
    ) -> Self {
        Self { profile_ref, context, image, prompt, duration_seconds }
    }
    /// Returns the stable Generation Profile selection.
    #[must_use]
    pub const fn profile_ref(&self) -> &GenerationProfileRef {
        &self.profile_ref
    }
    /// Returns execution identity, deadline, and cancellation.
    #[must_use]
    pub const fn context(&self) -> &WorkflowNodeExecutionContext {
        &self.context
    }
    /// Returns the exact readable source Image.
    #[must_use]
    pub const fn image(&self) -> &NodeCapabilityReadableImageInput {
        &self.image
    }
    /// Returns optional normalized semantic prompt text.
    #[must_use]
    pub const fn prompt(&self) -> Option<&WorkflowTextValue> {
        self.prompt.as_ref()
    }
    /// Returns the provider-independent duration.
    #[must_use]
    pub const fn duration_seconds(&self) -> ImageToVideoDurationSeconds {
        self.duration_seconds
    }
    /// Consumes the request and returns the exact readable source Image.
    #[must_use]
    pub fn into_readable_image(self) -> NodeCapabilityReadableImageInput {
        self.image
    }
}

/// Exact semantic text-to-speech provider request.
pub struct TextToSpeechProviderRequest {
    profile_ref: GenerationProfileRef,
    context: WorkflowNodeExecutionContext,
    text: WorkflowTextValue,
}

impl TextToSpeechProviderRequest {
    /// Creates one complete provider-independent request.
    #[must_use]
    pub const fn new(
        profile_ref: GenerationProfileRef,
        context: WorkflowNodeExecutionContext,
        text: WorkflowTextValue,
    ) -> Self {
        Self { profile_ref, context, text }
    }
    /// Returns the stable Generation Profile selection.
    #[must_use]
    pub const fn profile_ref(&self) -> &GenerationProfileRef {
        &self.profile_ref
    }
    /// Returns execution identity, deadline, and cancellation.
    #[must_use]
    pub const fn context(&self) -> &WorkflowNodeExecutionContext {
        &self.context
    }
    /// Returns normalized semantic speech text.
    #[must_use]
    pub const fn text(&self) -> &WorkflowTextValue {
        &self.text
    }
}

/// Semantic provider boundary for text-to-image generation.
#[async_trait]
pub trait TextToImageProviderInterface: Send + Sync {
    /// Generates one validated PNG Image payload.
    async fn generate_image_from_text(
        &self,
        request: TextToImageProviderRequest,
    ) -> Result<GeneratedImagePayload, NodeCapabilityProviderFailure>;
}

/// Semantic provider boundary for image-to-video generation.
#[async_trait]
pub trait ImageToVideoProviderInterface: Send + Sync {
    /// Generates one validated MP4 Video payload.
    async fn generate_video_from_image(
        &self,
        request: ImageToVideoProviderRequest,
    ) -> Result<GeneratedVideoPayload, NodeCapabilityProviderFailure>;
}

/// Semantic provider boundary for text-to-speech synthesis.
#[async_trait]
pub trait TextToSpeechProviderInterface: Send + Sync {
    /// Synthesizes one validated MPEG Audio payload.
    async fn synthesize_speech_from_text(
        &self,
        request: TextToSpeechProviderRequest,
    ) -> Result<SynthesizedSpeechPayload, NodeCapabilityProviderFailure>;
}
