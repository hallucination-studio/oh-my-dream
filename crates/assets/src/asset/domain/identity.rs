//! Asset-owned UUIDv4 identities and creation time.

use std::fmt;

use uuid::{Uuid, Variant, Version};

use super::AssetDomainError;

macro_rules! asset_uuid {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            /// Restores the identity only from an RFC 9562 UUIDv4.
            pub fn from_uuid(value: Uuid) -> Result<Self, AssetDomainError> {
                if value.get_version() != Some(Version::Random)
                    || value.get_variant() != Variant::RFC4122
                {
                    return Err(AssetDomainError::InvalidIdentity);
                }
                Ok(Self(value))
            }

            /// Returns the UUID without selecting a boundary encoding.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}", self.0.hyphenated())
            }
        }
    };
}

asset_uuid!(AssetId, "Identity of one logical media Asset.");
asset_uuid!(AssetImportId, "Idempotency identity of one trusted import.");
asset_uuid!(AssetContentFinalizationId, "Identity of one exact managed-content finalization.");
asset_uuid!(AssetPreviewLeaseId, "Identity of one short-lived preview permission.");

/// Non-negative Asset creation time in UTC milliseconds.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetCreatedAt(i64);

impl AssetCreatedAt {
    /// Restores a non-negative UTC-millisecond timestamp.
    pub const fn from_utc_milliseconds(value: i64) -> Result<Self, AssetDomainError> {
        if value < 0 { Err(AssetDomainError::InvalidDescriptor) } else { Ok(Self(value)) }
    }

    /// Returns UTC milliseconds.
    #[must_use]
    pub const fn as_utc_milliseconds(self) -> i64 {
        self.0
    }
}
