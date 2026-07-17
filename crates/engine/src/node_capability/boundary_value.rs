//! Mechanical boundary representations for values owned by other modules.

use uuid::{Uuid, Variant, Version};

/// Invalid engine boundary representation of a Generation Profile ref parameter.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("node capability Generation Profile ref parameter value is invalid")]
pub struct NodeCapabilityGenerationProfileRefParameterValueError;

/// Invalid engine boundary representation of a managed Asset ID.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("Workflow managed Asset ID boundary value is invalid")]
pub struct WorkflowManagedAssetIdBoundaryValueError;

/// Engine-owned canonical bytes for a provider-independent Generation Profile ref.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeCapabilityGenerationProfileRefParameterValue {
    profile_id: String,
    version: u32,
}

impl NodeCapabilityGenerationProfileRefParameterValue {
    /// Restores a canonical profile ref boundary representation.
    pub fn new(
        profile_id: impl Into<String>,
        version: u32,
    ) -> Result<Self, NodeCapabilityGenerationProfileRefParameterValueError> {
        let profile_id = profile_id.into();
        let valid_id = (3..=128).contains(&profile_id.len())
            && profile_id.split('.').count() >= 2
            && profile_id.split('.').all(valid_profile_segment);
        if !valid_id || version == 0 {
            return Err(NodeCapabilityGenerationProfileRefParameterValueError);
        }
        Ok(Self { profile_id, version })
    }

    /// Returns the canonical profile ID bytes.
    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// Returns the non-zero profile version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }
}

fn valid_profile_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Engine-owned RFC 9562 UUIDv4 boundary representation for an Asset ID.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowManagedAssetIdBoundaryValue([u8; 16]);

impl WorkflowManagedAssetIdBoundaryValue {
    /// Restores exact RFC 9562 UUIDv4 bytes.
    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, WorkflowManagedAssetIdBoundaryValueError> {
        let uuid = Uuid::from_bytes(bytes);
        if uuid.get_version() != Some(Version::Random) || uuid.get_variant() != Variant::RFC4122 {
            return Err(WorkflowManagedAssetIdBoundaryValueError);
        }
        Ok(Self(bytes))
    }

    /// Returns the canonical UUID bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}

/// Asset identity boundary value used by a node parameter.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeCapabilityManagedAssetIdParameterValue(WorkflowManagedAssetIdBoundaryValue);

impl NodeCapabilityManagedAssetIdParameterValue {
    /// Wraps the shared engine Asset-ID boundary shape.
    #[must_use]
    pub const fn new(value: WorkflowManagedAssetIdBoundaryValue) -> Self {
        Self(value)
    }

    /// Returns the shared engine Asset-ID boundary shape.
    #[must_use]
    pub const fn asset_id(self) -> WorkflowManagedAssetIdBoundaryValue {
        self.0
    }
}
