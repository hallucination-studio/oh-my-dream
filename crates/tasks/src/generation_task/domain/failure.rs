//! Structured safe Generation Task failures.

use super::GenerationTaskDomainError;

const MAX_FAILURE_CODE_BYTES: usize = 64;
const MAX_FAILURE_MESSAGE_BYTES: usize = 512;

/// Closed Generation Task failure category.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum GenerationTaskFailureKind {
    /// Semantic request rejection.
    InvalidRequest,
    /// Provider authentication failure.
    Authentication,
    /// Provider authorization failure.
    PermissionDenied,
    /// Content policy rejection.
    ContentPolicy,
    /// Provider rate limiting.
    RateLimited,
    /// Provider is unavailable.
    ProviderUnavailable,
    /// Provider deadline elapsed.
    Timeout,
    /// Provider declared terminal rejection.
    ProviderRejected,
    /// Provider returned an invalid response.
    InvalidProviderResponse,
    /// Submission may have been accepted but cannot be proven.
    AmbiguousSubmission,
    /// Exact input Asset is unavailable.
    InputAssetUnavailable,
    /// Generated output could not be finalized as an Asset.
    OutputAssetImport,
    /// Internal invariant or adapter failure.
    Internal,
}

/// Machine-readable failure with one safe bounded message.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenerationTaskFailure {
    kind: GenerationTaskFailureKind,
    code: String,
    message: String,
}

impl GenerationTaskFailure {
    /// Validates a structured failure safe for persistence and presentation.
    pub fn try_new(
        kind: GenerationTaskFailureKind,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, GenerationTaskDomainError> {
        let code = code.into();
        let message = message.into();
        if !valid_failure_code(&code)
            || message.is_empty()
            || message.len() > MAX_FAILURE_MESSAGE_BYTES
            || message.chars().any(char::is_control)
        {
            return Err(GenerationTaskDomainError::InvalidFailure);
        }
        Ok(Self { kind, code, message })
    }

    /// Returns the closed category.
    #[must_use]
    pub const fn kind(&self) -> GenerationTaskFailureKind {
        self.kind
    }

    /// Returns the machine-readable code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the safe message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

fn valid_failure_code(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'A'..=b'Z'))
        && value.len() <= MAX_FAILURE_CODE_BYTES
        && bytes.all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}
