//! Media facts returned by the Asset inspector boundary.

use crate::asset::domain::{AssetMediaFacts, AssetMediaMimeType};

use super::AssetApplicationError;

/// MIME and technical facts verified by the media inspector.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetInspectedMedia {
    mime_type: AssetMediaMimeType,
    media_facts: AssetMediaFacts,
}

impl AssetInspectedMedia {
    /// Combines MIME and facts only when their media kinds agree.
    pub fn try_new(
        mime_type: AssetMediaMimeType,
        media_facts: AssetMediaFacts,
    ) -> Result<Self, AssetApplicationError> {
        if mime_type.media_kind() != media_facts.media_kind() {
            return Err(AssetApplicationError::InvalidMedia);
        }
        Ok(Self { mime_type, media_facts })
    }

    /// Returns the sniffed supported MIME.
    #[must_use]
    pub const fn mime_type(self) -> AssetMediaMimeType {
        self.mime_type
    }

    /// Returns the inspected immutable technical facts.
    #[must_use]
    pub const fn media_facts(self) -> AssetMediaFacts {
        self.media_facts
    }
}
