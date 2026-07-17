//! Closed provider-neutral Generation Task requests.

use assets::asset::domain::{AssetContentDigest, AssetId, AssetMediaKind};

use super::GenerationTaskDomainError;

const MAX_GENERATION_TASK_TEXT_BYTES: usize = 65_536;

/// Non-empty bounded UTF-8 text carried by a Generation Task.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenerationTaskText(String);

impl GenerationTaskText {
    /// Validates the frozen non-empty 65,536-byte bound without rewriting content.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GenerationTaskDomainError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_GENERATION_TASK_TEXT_BYTES {
            return Err(GenerationTaskDomainError::InvalidText);
        }
        Ok(Self(value))
    }

    /// Returns the exact admitted text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Closed normalized image aspect ratio.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ImageAspectRatio {
    /// 1:1.
    Square,
    /// 4:3.
    Landscape4To3,
    /// 3:4.
    Portrait3To4,
    /// 16:9.
    Landscape16To9,
    /// 9:16.
    Portrait9To16,
}

/// Closed image-to-video duration.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum VideoDurationSeconds {
    /// Five seconds.
    Five,
    /// Ten seconds.
    Ten,
}

impl VideoDurationSeconds {
    /// Returns the exact duration in seconds.
    #[must_use]
    pub const fn get(self) -> u8 {
        match self {
            Self::Five => 5,
            Self::Ten => 10,
        }
    }
}

/// Exact immutable Asset identity, kind, and content digest used as task input.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetSnapshotRef {
    asset_id: AssetId,
    media_kind: AssetMediaKind,
    content_hash: AssetContentDigest,
}

impl AssetSnapshotRef {
    /// Combines one already-validated Asset snapshot.
    #[must_use]
    pub const fn new(
        asset_id: AssetId,
        media_kind: AssetMediaKind,
        content_hash: AssetContentDigest,
    ) -> Self {
        Self { asset_id, media_kind, content_hash }
    }

    /// Returns the logical Asset identity.
    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }

    /// Returns the exact media kind.
    #[must_use]
    pub const fn media_kind(self) -> AssetMediaKind {
        self.media_kind
    }

    /// Returns the exact immutable content digest.
    #[must_use]
    pub const fn content_hash(self) -> AssetContentDigest {
        self.content_hash
    }
}

/// Text-generation semantic request.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TextGenerationSpec {
    prompt: GenerationTaskText,
}

impl TextGenerationSpec {
    /// Creates a Text request from validated prompt text.
    #[must_use]
    pub const fn new(prompt: GenerationTaskText) -> Self {
        Self { prompt }
    }

    /// Returns the exact prompt.
    #[must_use]
    pub const fn prompt(&self) -> &GenerationTaskText {
        &self.prompt
    }
}

/// Text-to-image semantic request.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ImageGenerationSpec {
    prompt: GenerationTaskText,
    aspect_ratio: ImageAspectRatio,
}

impl ImageGenerationSpec {
    /// Creates an Image request from its closed values.
    #[must_use]
    pub const fn new(prompt: GenerationTaskText, aspect_ratio: ImageAspectRatio) -> Self {
        Self { prompt, aspect_ratio }
    }

    /// Returns the exact prompt.
    #[must_use]
    pub const fn prompt(&self) -> &GenerationTaskText {
        &self.prompt
    }

    /// Returns the normalized aspect ratio.
    #[must_use]
    pub const fn aspect_ratio(&self) -> ImageAspectRatio {
        self.aspect_ratio
    }
}

/// Text-to-speech semantic request.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct VoiceGenerationSpec {
    text: GenerationTaskText,
}

impl VoiceGenerationSpec {
    /// Creates a Voice request from validated text.
    #[must_use]
    pub const fn new(text: GenerationTaskText) -> Self {
        Self { text }
    }

    /// Returns the exact speech text.
    #[must_use]
    pub const fn text(&self) -> &GenerationTaskText {
        &self.text
    }
}

/// Image-to-video semantic request.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct VideoGenerationSpec {
    input_image: AssetSnapshotRef,
    duration_seconds: VideoDurationSeconds,
    prompt: Option<GenerationTaskText>,
}

impl VideoGenerationSpec {
    /// Creates a Video request only from an exact Image Asset snapshot.
    pub fn try_new(
        input_image: AssetSnapshotRef,
        duration_seconds: VideoDurationSeconds,
        prompt: Option<GenerationTaskText>,
    ) -> Result<Self, GenerationTaskDomainError> {
        if input_image.media_kind() != AssetMediaKind::Image {
            return Err(GenerationTaskDomainError::InvalidRequest);
        }
        Ok(Self { input_image, duration_seconds, prompt })
    }

    /// Returns the exact input Image snapshot.
    #[must_use]
    pub const fn input_image(&self) -> AssetSnapshotRef {
        self.input_image
    }

    /// Returns the closed duration.
    #[must_use]
    pub const fn duration_seconds(&self) -> VideoDurationSeconds {
        self.duration_seconds
    }

    /// Returns the optional non-empty prompt.
    #[must_use]
    pub const fn prompt(&self) -> Option<&GenerationTaskText> {
        self.prompt.as_ref()
    }
}

/// Closed Generation Task request kind.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum GenerationTaskRequestKind {
    /// Text generation.
    Text,
    /// Image generation.
    Image,
    /// Voice generation.
    Voice,
    /// Video generation.
    Video,
}

/// Immutable provider-neutral generation request snapshot.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum GenerationTaskRequest {
    /// Text generation.
    Text(TextGenerationSpec),
    /// Image generation.
    Image(ImageGenerationSpec),
    /// Voice generation.
    Voice(VoiceGenerationSpec),
    /// Video generation.
    Video(VideoGenerationSpec),
}

impl GenerationTaskRequest {
    /// Returns the variant-owned generation kind.
    #[must_use]
    pub const fn kind(&self) -> GenerationTaskRequestKind {
        match self {
            Self::Text(_) => GenerationTaskRequestKind::Text,
            Self::Image(_) => GenerationTaskRequestKind::Image,
            Self::Voice(_) => GenerationTaskRequestKind::Voice,
            Self::Video(_) => GenerationTaskRequestKind::Video,
        }
    }
}
