use std::fmt;

use engine::node_capability::NodeCapabilityGenerationProfileRefParameterValue;

/// Stable Generation Profile failures without provider details.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum GenerationProfileError {
    /// Profile identity or version is invalid.
    #[error("Generation Profile ref is invalid")]
    InvalidProfileRef,
    /// Display name is invalid.
    #[error("Generation Profile display name is invalid")]
    InvalidDisplayName,
    /// Definition fields are inconsistent.
    #[error("Generation Profile definition is invalid")]
    InvalidDefinition,
    /// Capability is not registered.
    #[error("node capability is not registered")]
    CapabilityNotFound,
    /// Profile is not present in the catalog.
    #[error("Generation Profile is not found")]
    ProfileNotFound,
    /// Profile is not compatible with the exact capability.
    #[error("Generation Profile is incompatible")]
    ProfileIncompatible,
    /// Availability observation violates the request contract.
    #[error("Generation Profile availability observation is invalid")]
    InvalidAvailabilityObservation,
    /// Bulk availability request is invalid.
    #[error("Generation Profile availability request is invalid")]
    AvailabilityRequestInvalid,
    /// Availability boundary failed technically.
    #[error("Generation Profile availability read failed")]
    AvailabilityReadFailed,
    /// Availability deadline elapsed.
    #[error("Generation Profile availability deadline exceeded")]
    DeadlineExceeded,
}

/// Stable provider-independent profile family identity.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GenerationProfileId(String);

impl GenerationProfileId {
    /// Validates the lowercase multi-segment identity grammar.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GenerationProfileError> {
        let value = value.into();
        let valid = (3..=128).contains(&value.len())
            && value.split('.').count() >= 2
            && value.split('.').all(valid_segment);
        if !valid {
            return Err(GenerationProfileError::InvalidProfileRef);
        }
        Ok(Self(value))
    }
    /// Returns canonical profile identity text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Non-zero immutable Generation Profile version.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GenerationProfileVersion(u32);

impl GenerationProfileVersion {
    /// Creates a non-zero version.
    pub const fn try_new(value: u32) -> Result<Self, GenerationProfileError> {
        if value == 0 { Err(GenerationProfileError::InvalidProfileRef) } else { Ok(Self(value)) }
    }
    /// Returns the version number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Exact stable provider-independent Generation Profile selection.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GenerationProfileRef {
    id: GenerationProfileId,
    version: GenerationProfileVersion,
}

impl GenerationProfileRef {
    /// Combines an already validated ID and version.
    #[must_use]
    pub const fn new(id: GenerationProfileId, version: GenerationProfileVersion) -> Self {
        Self { id, version }
    }
    /// Returns the profile family identity.
    #[must_use]
    pub const fn id(&self) -> &GenerationProfileId {
        &self.id
    }
    /// Returns the exact profile version.
    #[must_use]
    pub const fn version(&self) -> GenerationProfileVersion {
        self.version
    }
    /// Converts from the engine-owned mechanical parameter representation.
    pub fn try_from_node_capability_parameter_value(
        value: &NodeCapabilityGenerationProfileRefParameterValue,
    ) -> Result<Self, GenerationProfileError> {
        Ok(Self::new(
            GenerationProfileId::try_new(value.profile_id())?,
            GenerationProfileVersion::try_new(value.version())?,
        ))
    }
    /// Converts to the engine-owned mechanical parameter representation.
    pub fn to_node_capability_parameter_value(
        &self,
    ) -> Result<NodeCapabilityGenerationProfileRefParameterValue, GenerationProfileError> {
        NodeCapabilityGenerationProfileRefParameterValue::new(self.id.as_str(), self.version.get())
            .map_err(|_| GenerationProfileError::InvalidProfileRef)
    }
}

impl fmt::Display for GenerationProfileRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.id.as_str(), self.version.get())
    }
}

/// Validated user-facing Generation Profile name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProfileDisplayName(String);

impl GenerationProfileDisplayName {
    /// Validates trimming, scalar count, and control-character exclusion.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GenerationProfileError> {
        let value = value.into();
        let scalar_count = value.chars().count();
        if value.trim() != value
            || !(1..=80).contains(&scalar_count)
            || value.chars().any(char::is_control)
        {
            return Err(GenerationProfileError::InvalidDisplayName);
        }
        Ok(Self(value))
    }
    /// Returns the display text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
