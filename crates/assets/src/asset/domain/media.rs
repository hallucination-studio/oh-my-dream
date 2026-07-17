//! Frozen media kinds, MIME values, and verified technical facts.

use super::AssetDomainError;

/// Closed managed-media kind.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetMediaKind {
    /// Still image.
    Image,
    /// Video with optional inspected audio.
    Video,
    /// Audio stream.
    Audio,
}

/// Closed accepted sniffed MIME value.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum AssetMediaMimeType {
    /// `image/png`.
    ImagePng,
    /// `image/jpeg`.
    ImageJpeg,
    /// `image/webp`.
    ImageWebp,
    /// `video/mp4`.
    VideoMp4,
    /// `video/webm`.
    VideoWebm,
    /// `audio/mpeg`.
    AudioMpeg,
    /// `audio/wav`.
    AudioWav,
    /// `audio/ogg`.
    AudioOgg,
}

impl AssetMediaMimeType {
    /// Returns the only media kind compatible with this MIME.
    #[must_use]
    pub const fn media_kind(self) -> AssetMediaKind {
        match self {
            Self::ImagePng | Self::ImageJpeg | Self::ImageWebp => AssetMediaKind::Image,
            Self::VideoMp4 | Self::VideoWebm => AssetMediaKind::Video,
            Self::AudioMpeg | Self::AudioWav | Self::AudioOgg => AssetMediaKind::Audio,
        }
    }

    /// Returns the canonical sniffed MIME text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ImagePng => "image/png",
            Self::ImageJpeg => "image/jpeg",
            Self::ImageWebp => "image/webp",
            Self::VideoMp4 => "video/mp4",
            Self::VideoWebm => "video/webm",
            Self::AudioMpeg => "audio/mpeg",
            Self::AudioWav => "audio/wav",
            Self::AudioOgg => "audio/ogg",
        }
    }
}

/// Validated image dimensions.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetImageMediaFacts {
    width: u32,
    height: u32,
}

/// Validated video dimensions, duration, and audio observation.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetVideoMediaFacts {
    width: u32,
    height: u32,
    duration_ms: u64,
    has_audio: bool,
}

/// Validated audio duration and stream format.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetAudioMediaFacts {
    duration_ms: u64,
    sample_rate_hz: u32,
    channels: u8,
}

impl AssetImageMediaFacts {
    /// Returns inspected width in pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }
    /// Returns inspected height in pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
}

impl AssetVideoMediaFacts {
    /// Returns inspected width in pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }
    /// Returns inspected height in pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
    /// Returns inspected duration in milliseconds.
    #[must_use]
    pub const fn duration_ms(self) -> u64 {
        self.duration_ms
    }
    /// Reports whether the inspected video contains audio.
    #[must_use]
    pub const fn has_audio(self) -> bool {
        self.has_audio
    }
}

impl AssetAudioMediaFacts {
    /// Returns inspected duration in milliseconds.
    #[must_use]
    pub const fn duration_ms(self) -> u64 {
        self.duration_ms
    }
    /// Returns inspected sample rate in Hertz.
    #[must_use]
    pub const fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }
    /// Returns inspected channel count.
    #[must_use]
    pub const fn channels(self) -> u8 {
        self.channels
    }
}

/// Closed immutable inspected technical facts.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum AssetMediaFacts {
    /// Image facts.
    Image(AssetImageMediaFacts),
    /// Video facts.
    Video(AssetVideoMediaFacts),
    /// Audio facts.
    Audio(AssetAudioMediaFacts),
}

impl AssetMediaFacts {
    /// Creates image facts within the frozen dimension bounds.
    pub fn try_image(width: u32, height: u32) -> Result<Self, AssetDomainError> {
        validate_dimensions(width, height)?;
        Ok(Self::Image(AssetImageMediaFacts { width, height }))
    }

    /// Creates video facts within the frozen dimension and duration bounds.
    pub fn try_video(
        width: u32,
        height: u32,
        duration_ms: u64,
        has_audio: bool,
    ) -> Result<Self, AssetDomainError> {
        validate_dimensions(width, height)?;
        validate_duration(duration_ms)?;
        Ok(Self::Video(AssetVideoMediaFacts { width, height, duration_ms, has_audio }))
    }

    /// Creates audio facts within the frozen duration, sample-rate, and channel bounds.
    pub fn try_audio(
        duration_ms: u64,
        sample_rate_hz: u32,
        channels: u8,
    ) -> Result<Self, AssetDomainError> {
        validate_duration(duration_ms)?;
        if !(8_000..=192_000).contains(&sample_rate_hz) || !(1..=8).contains(&channels) {
            return Err(AssetDomainError::InvalidMediaFacts);
        }
        Ok(Self::Audio(AssetAudioMediaFacts { duration_ms, sample_rate_hz, channels }))
    }

    /// Returns the exact media kind represented by these facts.
    #[must_use]
    pub const fn media_kind(self) -> AssetMediaKind {
        match self {
            Self::Image(_) => AssetMediaKind::Image,
            Self::Video(_) => AssetMediaKind::Video,
            Self::Audio(_) => AssetMediaKind::Audio,
        }
    }
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), AssetDomainError> {
    if !(1..=16_384).contains(&width) || !(1..=16_384).contains(&height) {
        return Err(AssetDomainError::InvalidMediaFacts);
    }
    Ok(())
}

fn validate_duration(duration_ms: u64) -> Result<(), AssetDomainError> {
    if !(1..=86_400_000).contains(&duration_ms) {
        return Err(AssetDomainError::InvalidMediaFacts);
    }
    Ok(())
}
