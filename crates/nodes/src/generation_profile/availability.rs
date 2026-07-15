use std::time::{Duration, Instant};

use async_trait::async_trait;
use engine::node_capability::NodeCapabilityContractRef;

use super::{GenerationProfileError, GenerationProfileRef};

/// Why a configured profile is currently unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationProfileUnavailableReason {
    /// No route is configured for the exact profile.
    NoConfiguredRoute,
    /// Required provider authentication is absent or invalid.
    AuthenticationRequired,
    /// Local or provider policy blocks the profile.
    PolicyBlocked,
    /// Provider quota cannot currently admit work.
    QuotaUnavailable,
    /// Provider rate limiting currently blocks work.
    RateLimited,
    /// The selected provider is currently unavailable.
    ProviderUnavailable,
    /// The route's exact native model is currently unavailable.
    NativeModelUnavailable,
}

/// Why current availability cannot be trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationProfileAvailabilityIndeterminateReason {
    /// The bounded availability probe timed out.
    ProbeTimedOut,
    /// Network state prevents a trustworthy observation.
    NetworkOffline,
    /// A response was received but cannot be trusted.
    UntrustedResponse,
}

/// Optional safe retry epoch for an unavailable observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationProfileRetryAfter(i64);

impl GenerationProfileRetryAfter {
    /// Creates a non-negative retry epoch; observation validation checks ordering.
    pub const fn try_new(epoch_ms: i64) -> Result<Self, GenerationProfileError> {
        if epoch_ms < 0 {
            Err(GenerationProfileError::InvalidAvailabilityObservation)
        } else {
            Ok(Self(epoch_ms))
        }
    }
    /// Returns the retry epoch milliseconds.
    #[must_use]
    pub const fn epoch_ms(self) -> i64 {
        self.0
    }
}

/// Current expiring operational state for one exact profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenerationProfileAvailabilityState {
    /// The exact profile can currently admit work.
    Available,
    /// The exact profile is known not to admit work.
    Unavailable {
        /// Structured reason for known unavailability.
        reason: GenerationProfileUnavailableReason,
        /// Optional safe epoch after which admission may be retried.
        retry_after: Option<GenerationProfileRetryAfter>,
    },
    /// Current availability cannot be determined reliably.
    Indeterminate {
        /// Structured reason the observation is indeterminate.
        reason: GenerationProfileAvailabilityIndeterminateReason,
    },
}

/// One validated current availability observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProfileAvailabilityObservation {
    profile_ref: GenerationProfileRef,
    state: GenerationProfileAvailabilityState,
    observed_at_epoch_ms: i64,
    expires_at_epoch_ms: i64,
}

impl GenerationProfileAvailabilityObservation {
    /// Validates observation lifetime and optional retry ordering.
    pub fn try_new(
        profile_ref: GenerationProfileRef,
        state: GenerationProfileAvailabilityState,
        observed_at_epoch_ms: i64,
        expires_at_epoch_ms: i64,
    ) -> Result<Self, GenerationProfileError> {
        let lifetime = expires_at_epoch_ms.checked_sub(observed_at_epoch_ms);
        let retry_valid = match &state {
            GenerationProfileAvailabilityState::Unavailable {
                retry_after: Some(value), ..
            } => value.epoch_ms() > observed_at_epoch_ms,
            _ => true,
        };
        if observed_at_epoch_ms < 0 || !matches!(lifetime, Some(1..=30_000)) || !retry_valid {
            return Err(GenerationProfileError::InvalidAvailabilityObservation);
        }
        Ok(Self { profile_ref, state, observed_at_epoch_ms, expires_at_epoch_ms })
    }
    /// Returns the observed exact profile.
    #[must_use]
    pub const fn profile_ref(&self) -> &GenerationProfileRef {
        &self.profile_ref
    }
    /// Returns the current closed availability state.
    #[must_use]
    pub const fn state(&self) -> &GenerationProfileAvailabilityState {
        &self.state
    }
    /// Returns observation epoch milliseconds.
    #[must_use]
    pub const fn observed_at_epoch_ms(&self) -> i64 {
        self.observed_at_epoch_ms
    }
    /// Returns expiry epoch milliseconds.
    #[must_use]
    pub const fn expires_at_epoch_ms(&self) -> i64 {
        self.expires_at_epoch_ms
    }
}

/// One ordered, exact-compatible bulk availability request.
pub struct GenerationProfileAvailabilityRequest {
    capability_ref: NodeCapabilityContractRef,
    profile_refs: Vec<GenerationProfileRef>,
    deadline: Instant,
}

impl GenerationProfileAvailabilityRequest {
    /// Validates size, strict ordering, and the five-second deadline bound.
    pub fn try_new(
        capability_ref: NodeCapabilityContractRef,
        profile_refs: Vec<GenerationProfileRef>,
        deadline: Instant,
    ) -> Result<Self, GenerationProfileError> {
        let now = Instant::now();
        let ordered = profile_refs.windows(2).all(|pair| pair[0] < pair[1]);
        if profile_refs.is_empty()
            || profile_refs.len() > 100
            || !ordered
            || deadline <= now
            || deadline.duration_since(now) > Duration::from_secs(5)
        {
            return Err(GenerationProfileError::AvailabilityRequestInvalid);
        }
        Ok(Self { capability_ref, profile_refs, deadline })
    }
    /// Returns the exact capability being observed.
    #[must_use]
    pub const fn capability_ref(&self) -> &NodeCapabilityContractRef {
        &self.capability_ref
    }
    /// Returns requested refs in strict ascending order.
    #[must_use]
    pub fn profile_refs(&self) -> &[GenerationProfileRef] {
        &self.profile_refs
    }
    /// Returns the process-monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }
}

/// Bulk provider-route availability boundary consumed by profile queries.
#[async_trait]
pub trait GenerationProfileAvailabilityReaderInterface: Send + Sync {
    /// Reads exactly one ordered observation per requested profile.
    async fn read_generation_profile_availability(
        &self,
        request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError>;
}
