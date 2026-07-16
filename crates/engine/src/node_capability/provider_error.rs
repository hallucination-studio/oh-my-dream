//! Provider failures shared by exact generation capability interfaces.

use std::time::{Duration, Instant};

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
    /// Creates a failure without an optional retry instant.
    #[must_use]
    pub fn without_retry_at(
        category: NodeCapabilityProviderFailureCategory,
        submission_was_accepted: bool,
    ) -> Self {
        let retryable = matches!(
            category,
            NodeCapabilityProviderFailureCategory::RateLimited
                | NodeCapabilityProviderFailureCategory::ProviderUnavailable
        ) || (category == NodeCapabilityProviderFailureCategory::DeadlineExceeded
            && !submission_was_accepted);
        Self { category, retryable, safe_retry_at: None }
    }

    /// Creates a provider failure and rejects inconsistent retry metadata.
    pub fn try_new(
        category: NodeCapabilityProviderFailureCategory,
        submission_was_accepted: bool,
        observed_at: Instant,
        safe_retry_at: Option<Instant>,
    ) -> Result<Self, NodeCapabilityProviderFailureConstructionError> {
        let failure = Self::without_retry_at(category, submission_was_accepted);
        let retryable = failure.retryable;
        if (!retryable && safe_retry_at.is_some())
            || safe_retry_at.is_some_and(|retry_at| retry_at <= observed_at)
        {
            return Err(NodeCapabilityProviderFailureConstructionError);
        }
        Ok(Self { safe_retry_at, ..failure })
    }

    /// Restores durable provider failure semantics without process-local retry timing.
    pub fn try_restore(
        category: NodeCapabilityProviderFailureCategory,
        retryable: bool,
    ) -> Result<Self, NodeCapabilityProviderFailureConstructionError> {
        Self::try_restore_with_retry_after(category, retryable, None)
    }

    /// Restores durable retry semantics as a conservative delay from this process observation.
    pub fn try_restore_with_retry_after(
        category: NodeCapabilityProviderFailureCategory,
        retryable: bool,
        safe_retry_after: Option<Duration>,
    ) -> Result<Self, NodeCapabilityProviderFailureConstructionError> {
        let valid = match category {
            NodeCapabilityProviderFailureCategory::RateLimited
            | NodeCapabilityProviderFailureCategory::ProviderUnavailable => retryable,
            NodeCapabilityProviderFailureCategory::DeadlineExceeded => true,
            _ => !retryable,
        };
        if !valid || (!retryable && safe_retry_after.is_some()) {
            return Err(NodeCapabilityProviderFailureConstructionError);
        }
        let safe_retry_at = match safe_retry_after {
            Some(delay) => Some(
                Instant::now()
                    .checked_add(delay)
                    .ok_or(NodeCapabilityProviderFailureConstructionError)?,
            ),
            None => None,
        };
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
