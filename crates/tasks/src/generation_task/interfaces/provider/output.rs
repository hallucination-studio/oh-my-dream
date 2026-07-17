//! Type-specific normalized Generation Provider results and progress.

use std::sync::Arc;

use crate::generation_task::domain::GenerationTaskText;

use super::GenerationProviderValueError;

const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_VIDEO_BYTES: usize = 512 * 1024 * 1024;
const MAX_VOICE_BYTES: usize = 64 * 1024 * 1024;

/// Validated inline Text returned by a Text provider route.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TextGenerationProviderResult(GenerationTaskText);

impl TextGenerationProviderResult {
    /// Wraps already-validated generated Text.
    #[must_use]
    pub const fn new(content: GenerationTaskText) -> Self {
        Self(content)
    }

    /// Returns exact generated Text.
    #[must_use]
    pub const fn content(&self) -> &GenerationTaskText {
        &self.0
    }

    /// Consumes the provider result and returns exact generated Text.
    #[must_use]
    pub fn into_content(self) -> GenerationTaskText {
        self.0
    }
}

macro_rules! media_provider_result {
    ($name:ident, $maximum:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, Hash, PartialEq, Eq)]
        pub struct $name(Arc<[u8]>);

        impl $name {
            /// Accepts one non-empty payload within the frozen media byte limit.
            pub fn try_new(bytes: Vec<u8>) -> Result<Self, GenerationProviderValueError> {
                if bytes.is_empty() || bytes.len() > $maximum {
                    return Err(GenerationProviderValueError::InvalidResult);
                }
                Ok(Self(bytes.into()))
            }

            /// Returns validated media bytes.
            #[must_use]
            pub fn bytes(&self) -> &[u8] {
                &self.0
            }

            /// Consumes the result and returns validated media bytes.
            #[must_use]
            pub fn into_bytes(self) -> Arc<[u8]> {
                self.0
            }
        }
    };
}

media_provider_result!(
    ImageGenerationProviderResult,
    MAX_IMAGE_BYTES,
    "Validated image bytes returned by an Image provider route."
);
media_provider_result!(
    VideoGenerationProviderResult,
    MAX_VIDEO_BYTES,
    "Validated video bytes returned by a Video provider route."
);
media_provider_result!(
    VoiceGenerationProviderResult,
    MAX_VOICE_BYTES,
    "Validated audio bytes returned by a Voice provider route."
);

/// Optional normalized remote progress in `0..=100`.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GenerationProviderProgress(Option<u8>);

impl GenerationProviderProgress {
    /// Validates optional normalized progress.
    pub const fn try_new(value: Option<u8>) -> Result<Self, GenerationProviderValueError> {
        if matches!(value, Some(percent) if percent > 100) {
            Err(GenerationProviderValueError::InvalidProgress)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns known normalized progress.
    #[must_use]
    pub const fn percent(self) -> Option<u8> {
        self.0
    }
}
