//! Asset-owned mechanical representations of external business identities.

use uuid::{Uuid, Variant, Version};

use super::{AssetDomainError, AssetId};

macro_rules! asset_origin_uuid {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            /// Restores only an RFC 9562 UUIDv4 integration identity.
            pub fn from_uuid(value: Uuid) -> Result<Self, AssetDomainError> {
                if value.get_version() != Some(Version::Random)
                    || value.get_variant() != Variant::RFC4122
                {
                    return Err(AssetDomainError::InvalidOrigin);
                }
                Ok(Self(value))
            }

            /// Returns exact UUID bytes without choosing a wire encoding.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }
    };
}

asset_origin_uuid!(AssetOriginWorkflowId, "Mechanical Workflow identity in Asset provenance.");
asset_origin_uuid!(
    AssetOriginWorkflowRunId,
    "Mechanical Workflow Run identity in Asset provenance."
);
asset_origin_uuid!(
    AssetOriginWorkflowNodeId,
    "Mechanical Workflow node identity in Asset provenance."
);
asset_origin_uuid!(
    AssetOriginWorkflowNodeExecutionId,
    "Mechanical node-execution identity in Asset provenance."
);

/// Mechanical non-zero Workflow revision in Asset provenance.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetOriginWorkflowRevision(u64);

impl AssetOriginWorkflowRevision {
    /// Restores a non-zero Workflow revision without owning revision semantics.
    pub const fn new(value: u64) -> Result<Self, AssetDomainError> {
        if value == 0 { Err(AssetDomainError::InvalidOrigin) } else { Ok(Self(value)) }
    }

    /// Returns the mechanical revision number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Mechanical exact node-capability contract reference in Asset provenance.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetOriginNodeCapabilityContractRef {
    id: String,
    major: u16,
    minor: u16,
}

impl AssetOriginNodeCapabilityContractRef {
    /// Validates only the frozen canonical capability-ref shape.
    pub fn try_new(
        id: impl Into<String>,
        major: u16,
        minor: u16,
    ) -> Result<Self, AssetDomainError> {
        let id = id.into();
        if major == 0 || !valid_dot_id(&id) {
            return Err(AssetDomainError::InvalidOrigin);
        }
        Ok(Self { id, major, minor })
    }

    /// Returns canonical capability ID text.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Returns capability major version.
    #[must_use]
    pub const fn major(&self) -> u16 {
        self.major
    }
    /// Returns capability minor version.
    #[must_use]
    pub const fn minor(&self) -> u16 {
        self.minor
    }
}

/// Mechanical declared output key in Asset provenance.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetOriginNodeOutputKey(String);

impl AssetOriginNodeOutputKey {
    /// Validates the frozen capability-key grammar.
    pub fn try_new(value: impl Into<String>) -> Result<Self, AssetDomainError> {
        let value = value.into();
        if !valid_key(&value, 64) {
            return Err(AssetDomainError::InvalidOrigin);
        }
        Ok(Self(value))
    }

    /// Returns canonical output key text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Mechanical provider-independent Generation Profile reference.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetOriginGenerationProfileRef {
    id: String,
    version: u32,
}

impl AssetOriginGenerationProfileRef {
    /// Validates only canonical profile identity shape and non-zero version.
    pub fn try_new(id: impl Into<String>, version: u32) -> Result<Self, AssetDomainError> {
        let id = id.into();
        if version == 0 || !valid_dot_id(&id) {
            return Err(AssetDomainError::InvalidOrigin);
        }
        Ok(Self { id, version })
    }

    /// Returns canonical profile ID text.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Returns the non-zero profile version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }
}

/// Asset-owned source identity retained in derived provenance.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetOriginSourceAssetId(AssetId);

impl AssetOriginSourceAssetId {
    /// Wraps one already validated Asset identity.
    #[must_use]
    pub const fn from_asset_id(value: AssetId) -> Self {
        Self(value)
    }
    /// Returns the authoritative Asset identity.
    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.0
    }
}

/// Non-empty ordered source Asset identities without inference or deduplication.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetOriginSourceAssetIds(Vec<AssetOriginSourceAssetId>);

impl AssetOriginSourceAssetIds {
    /// Preserves one supplied non-empty source sequence verbatim.
    pub fn try_new(values: Vec<AssetOriginSourceAssetId>) -> Result<Self, AssetDomainError> {
        if values.is_empty() {
            return Err(AssetDomainError::InvalidOrigin);
        }
        Ok(Self(values))
    }

    /// Returns source identities in their supplied provenance order.
    #[must_use]
    pub fn as_slice(&self) -> &[AssetOriginSourceAssetId] {
        &self.0
    }
}

fn valid_dot_id(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && value.split('.').count() >= 2
        && value.split('.').all(|segment| valid_key(segment, 128))
}

fn valid_key(value: &str, maximum_bytes: usize) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && value.len() <= maximum_bytes
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
