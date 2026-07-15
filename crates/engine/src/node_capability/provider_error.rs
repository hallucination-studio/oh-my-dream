//! Provider failures shared by exact generation capability interfaces.

use std::time::Instant;

use thiserror::Error;

/// Closed provider failure category shared by exact generation interfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityProviderFailureCategory {
    /// Semantic request was invalid.
    InvalidSemanticRequest,
    /// Provider authentication failed.
    AuthenticationFailed,
    /// Provider denied the operation.
    PermissionDenied,
    /// Content policy rejected the request.
    ContentPolicyRejected,
    /// Provider rate limit was reached.
    RateLimited,
    /// Provider was temporarily unavailable.
    ProviderUnavailable,
    /// Operation deadline was exceeded.
    DeadlineExceeded,
    /// Provider rejected an otherwise valid operation.
    ProviderRejected,
    /// Provider response was invalid.
    InvalidResponse,
    /// Provider content download was rejected.
    DownloadRejected,
    /// Submission outcome could not be proven.
    AmbiguousSubmission,
}

/// Validated provider failure without provider-private text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityProviderFailure {
    category: NodeCapabilityProviderFailureCategory,
    retryable: bool,
    safe_retry_at: Option<Instant>,
}

/// Invalid provider retry metadata for its closed category.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("node capability provider retry metadata is invalid")]
pub struct NodeCapabilityProviderFailureConstructionError;

impl NodeCapabilityProviderFailure {
    /// Creates a provider failure and rejects inconsistent retry metadata.
    pub fn try_new(
        category: NodeCapabilityProviderFailureCategory,
        submission_was_accepted: bool,
        observed_at: Instant,
        safe_retry_at: Option<Instant>,
    ) -> Result<Self, NodeCapabilityProviderFailureConstructionError> {
        let retryable = matches!(
            category,
            NodeCapabilityProviderFailureCategory::RateLimited
                | NodeCapabilityProviderFailureCategory::ProviderUnavailable
        ) || (category == NodeCapabilityProviderFailureCategory::DeadlineExceeded
            && !submission_was_accepted);
        if (!retryable && safe_retry_at.is_some())
            || safe_retry_at.is_some_and(|retry_at| retry_at <= observed_at)
        {
            return Err(NodeCapabilityProviderFailureConstructionError);
        }
        Ok(Self { category, retryable, safe_retry_at })
    }

    /// Returns the closed provider failure category.
    #[must_use]
    pub const fn category(&self) -> NodeCapabilityProviderFailureCategory {
        self.category
    }

    /// Reports whether a new Run may safely retry the semantic operation.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        self.retryable
    }

    /// Returns the optional monotonic instant before which retry is unsafe.
    #[must_use]
    pub const fn safe_retry_at(&self) -> Option<Instant> {
        self.safe_retry_at
    }
}
