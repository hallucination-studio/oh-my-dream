//! Authoritative Project domain values.

use super::ProjectDomainError;
use uuid::{Uuid, Variant, Version};

/// Immutable identity of one Project.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectId(Uuid);

impl ProjectId {
    /// Restores an identity only when the UUID is version four.
    #[must_use]
    pub fn from_uuid(value: Uuid) -> Option<Self> {
        (value.get_version() == Some(Version::Random) && value.get_variant() == Variant::RFC4122)
            .then_some(Self(value))
    }

    /// Returns the UUID value without choosing a boundary encoding.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// Normalized user-visible Project name.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ProjectName(String);

impl ProjectName {
    /// Normalizes and validates a Project name.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ProjectDomainError> {
        let normalized = value.as_ref().trim();
        if normalized.is_empty() {
            return Err(ProjectDomainError::ProjectNameEmpty);
        }
        if normalized.chars().any(char::is_control) {
            return Err(ProjectDomainError::ProjectNameContainsControl);
        }
        if normalized.chars().count() > 120 {
            return Err(ProjectDomainError::ProjectNameTooLong);
        }
        Ok(Self(normalized.to_owned()))
    }

    /// Returns the normalized name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Non-zero optimistic-concurrency revision of a Project.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectRevision(u64);

impl ProjectRevision {
    /// Returns the initial revision assigned at Project creation.
    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Restores a non-zero revision.
    #[must_use]
    pub const fn from_non_zero(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// Returns the stored revision.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(super) fn next(self) -> Result<Self, ProjectDomainError> {
        self.0.checked_add(1).map(Self).ok_or(ProjectDomainError::ProjectRevisionOverflow)
    }
}

macro_rules! project_timestamp {
    ($name:ident) => {
        #[doc = "Non-negative UTC milliseconds since the Unix epoch."]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(i64);

        impl $name {
            #[doc = "Creates a validated Project timestamp."]
            pub const fn new(value: i64) -> Result<Self, ProjectDomainError> {
                if value < 0 {
                    Err(ProjectDomainError::ProjectTimestampOutOfRange)
                } else {
                    Ok(Self(value))
                }
            }

            #[doc = "Returns UTC milliseconds since the Unix epoch."]
            #[must_use]
            pub const fn get(self) -> i64 {
                self.0
            }
        }
    };
}

project_timestamp!(ProjectCreatedAt);
project_timestamp!(ProjectUpdatedAt);

impl From<ProjectCreatedAt> for ProjectUpdatedAt {
    fn from(value: ProjectCreatedAt) -> Self {
        Self(value.get())
    }
}

impl ProjectCreatedAt {
    /// Creates a creation timestamp from the validated Project clock observation.
    #[must_use]
    pub const fn from_observed_project_time(value: ProjectUpdatedAt) -> Self {
        Self(value.get())
    }
}
