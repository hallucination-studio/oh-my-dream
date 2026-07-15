//! Normalized user-visible Asset names.

use super::AssetDomainError;

/// Trimmed user-visible Asset name.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetDisplayName(String);

impl AssetDisplayName {
    /// Normalizes a 1..=255-scalar display name without control characters.
    pub fn try_new(value: impl AsRef<str>) -> Result<Self, AssetDomainError> {
        let value = value.as_ref().trim();
        if !valid_name(value) {
            return Err(AssetDomainError::InvalidDisplayName);
        }
        Ok(Self(value.to_owned()))
    }

    /// Returns normalized display text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Trimmed final file name without any path component.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetOriginalFileName(String);

impl AssetOriginalFileName {
    /// Normalizes and validates one final file name.
    pub fn try_new(value: impl AsRef<str>) -> Result<Self, AssetDomainError> {
        let value = value.as_ref().trim();
        if !valid_name(value) || value.contains(['/', '\\']) {
            return Err(AssetDomainError::InvalidOriginalFileName);
        }
        Ok(Self(value.to_owned()))
    }

    /// Returns the final file name only.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_name(value: &str) -> bool {
    let length = value.chars().count();
    (1..=255).contains(&length) && !value.chars().any(char::is_control)
}
