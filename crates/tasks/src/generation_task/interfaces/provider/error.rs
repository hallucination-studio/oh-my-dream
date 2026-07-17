//! Structured Generation Provider contract and call failures.

use crate::generation_task::domain::GenerationTaskTimestamp;

const MAX_PROVIDER_FAILURE_CODE_BYTES: usize = 64;
const MAX_PROVIDER_FAILURE_MESSAGE_BYTES: usize = 512;

/// Construction or composition failure for a Generation Provider contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationProviderContractError {
    /// Provider or route display text violates its bounded contract.
    #[error("Generation Provider display name is invalid")]
    InvalidDisplayName,
    /// A route has no compatible Generation Profile.
    #[error("Generation Provider route has no compatible profile")]
    EmptyCompatibleProfiles,
    /// A focused provider capability has no route.
    #[error("Generation Provider capability has no route")]
    EmptyRoutes,
    /// One route identity appears more than once in a provider composition.
    #[error("Generation Provider route identity is duplicated")]
    DuplicateRouteId,
    /// A provider contributes no complete focused capability.
    #[error("Generation Provider capability composition is empty")]
    EmptyCapabilities,
    /// A declared route cannot be resolved by its focused capability.
    #[error("Generation Provider route resolution disagrees with its contract")]
    RouteResolutionMismatch,
}

/// Construction failure for a normalized Generation Provider boundary value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationProviderValueError {
    /// A provider result violates its type-specific bound.
    #[error("Generation Provider result is invalid")]
    InvalidResult,
    /// Normalized provider progress is outside `0..=100`.
    #[error("Generation Provider progress is invalid")]
    InvalidProgress,
    /// A structured provider failure is not safely bounded.
    #[error("Generation Provider failure is invalid")]
    InvalidFailure,
    /// Provider call context timestamps are inconsistent.
    #[error("Generation Provider call context is invalid")]
    InvalidCallContext,
}

/// Exact route lookup failure exposed by a focused provider capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationProviderRouteResolutionError {
    /// The route is not part of the focused shipped contract.
    #[error("Generation Provider route is not found")]
    RouteNotFound,
}

/// Closed terminal failure category declared by a provider.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum GenerationProviderFailureKind {
    /// Provider rejected semantic request fields.
    InvalidSemanticRequest,
    /// Provider authentication failed.
    AuthenticationFailed,
    /// Provider denied permission.
    PermissionDenied,
    /// Provider content policy rejected the request.
    ContentPolicyRejected,
    /// Provider rate limit was reached.
    RateLimited,
    /// Provider is unavailable.
    ProviderUnavailable,
    /// Provider deadline elapsed.
    DeadlineExceeded,
    /// Provider declared another terminal rejection.
    ProviderRejected,
    /// Provider returned invalid response data.
    InvalidResponse,
    /// Remote media download was rejected.
    DownloadRejected,
    /// Submission acceptance is uncertain.
    AmbiguousSubmission,
}

/// Structured terminal failure reported by a Generation Provider.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenerationProviderFailure {
    kind: GenerationProviderFailureKind,
    code: String,
    message: String,
}

impl GenerationProviderFailure {
    /// Validates one machine-readable code and safe message.
    pub fn try_new(
        kind: GenerationProviderFailureKind,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, GenerationProviderValueError> {
        let code = code.into();
        let message = message.into();
        if !valid_code(&code) || !valid_message(&message) {
            return Err(GenerationProviderValueError::InvalidFailure);
        }
        Ok(Self { kind, code, message })
    }

    /// Returns the closed provider failure category.
    #[must_use]
    pub const fn kind(&self) -> GenerationProviderFailureKind {
        self.kind
    }

    /// Returns the machine-readable provider-independent code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the safe bounded message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Whether the same accepted-handle observation may be attempted again.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum GenerationProviderCallErrorKind {
    /// A safe read, poll, or cancellation call may be repeated.
    Transient,
    /// The current call cannot produce a trustworthy result and is terminal.
    Permanent,
}

/// Technical call failure distinct from provider-declared terminal failure.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Generation Provider call failed")]
pub struct GenerationProviderCallError {
    kind: GenerationProviderCallErrorKind,
    code: String,
    message: String,
    retry_at: Option<GenerationTaskTimestamp>,
}

impl GenerationProviderCallError {
    /// Validates one safe technical call failure.
    pub fn try_new(
        kind: GenerationProviderCallErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        retry_at: Option<GenerationTaskTimestamp>,
        observed_at: GenerationTaskTimestamp,
    ) -> Result<Self, GenerationProviderValueError> {
        let code = code.into();
        let message = message.into();
        if !valid_code(&code)
            || !valid_message(&message)
            || (kind == GenerationProviderCallErrorKind::Permanent && retry_at.is_some())
            || retry_at.is_some_and(|retry_at| retry_at <= observed_at)
        {
            return Err(GenerationProviderValueError::InvalidFailure);
        }
        Ok(Self { kind, code, message, retry_at })
    }

    /// Returns whether bounded delivery may retry a safe operation.
    #[must_use]
    pub const fn kind(&self) -> GenerationProviderCallErrorKind {
        self.kind
    }

    /// Returns the machine-readable provider-independent code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the safe bounded message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the optional provider retry time for a transient call.
    #[must_use]
    pub const fn retry_at(&self) -> Option<GenerationTaskTimestamp> {
        self.retry_at
    }
}

fn valid_code(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'A'..=b'Z'))
        && value.len() <= MAX_PROVIDER_FAILURE_CODE_BYTES
        && bytes.all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_message(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROVIDER_FAILURE_MESSAGE_BYTES
        && !value.chars().any(char::is_control)
}
