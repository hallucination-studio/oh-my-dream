//! Consumer-owned persistence boundary for the Desktop effect worker.

use async_trait::async_trait;

use super::{
    DesktopApplicationInstanceId, DesktopPostCommitEffect, DesktopPostCommitEffectAbandonReason,
    DesktopPostCommitEffectId, DesktopPostCommitEffectState, DesktopPostCommitTimestamp,
};

/// Outbox storage or compare-and-swap failure.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum DesktopPostCommitEffectOutboxError {
    /// Storage could not complete the requested operation.
    #[error("Desktop post-commit effect storage failed")]
    StorageFailure,
    /// The durable effect no longer has the expected state.
    #[error("Desktop post-commit effect state conflict")]
    StateConflict,
}

/// Validated page size for startup recovery.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct DesktopPostCommitRecoveryLimit(u8);

impl DesktopPostCommitRecoveryLimit {
    /// Restores a limit in the frozen inclusive range 1..=100.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        if value == 0 || value > 100 { None } else { Some(Self(value)) }
    }

    /// Returns the validated limit.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Opaque stable cursor over `(created_at, effect_id)`.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct DesktopPostCommitRecoveryCursor {
    created_at: DesktopPostCommitTimestamp,
    effect_id: DesktopPostCommitEffectId,
}

impl DesktopPostCommitRecoveryCursor {
    /// Constructs a cursor from the exact last returned ordering tuple.
    #[must_use]
    pub const fn new(
        created_at: DesktopPostCommitTimestamp,
        effect_id: DesktopPostCommitEffectId,
    ) -> Self {
        Self { created_at, effect_id }
    }

    /// Returns the ordering timestamp.
    #[must_use]
    pub const fn created_at(self) -> DesktopPostCommitTimestamp {
        self.created_at
    }

    /// Returns the ordering identity.
    #[must_use]
    pub const fn effect_id(self) -> DesktopPostCommitEffectId {
        self.effect_id
    }
}

/// Complete durable effect returned to the worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesktopPostCommitEffectRecord {
    effect_id: DesktopPostCommitEffectId,
    effect: DesktopPostCommitEffect,
    state: DesktopPostCommitEffectState,
    attempt_count: u32,
    created_at: DesktopPostCommitTimestamp,
}

impl DesktopPostCommitEffectRecord {
    /// Restores one complete validated outbox record.
    #[must_use]
    pub const fn new(
        effect_id: DesktopPostCommitEffectId,
        effect: DesktopPostCommitEffect,
        state: DesktopPostCommitEffectState,
        attempt_count: u32,
        created_at: DesktopPostCommitTimestamp,
    ) -> Self {
        Self { effect_id, effect, state, attempt_count, created_at }
    }

    /// Returns the durable effect identity.
    #[must_use]
    pub const fn effect_id(self) -> DesktopPostCommitEffectId {
        self.effect_id
    }

    /// Returns the closed business effect.
    #[must_use]
    pub const fn effect(self) -> DesktopPostCommitEffect {
        self.effect
    }

    /// Returns the durable delivery state.
    #[must_use]
    pub const fn state(self) -> DesktopPostCommitEffectState {
        self.state
    }

    /// Returns the non-zero number of claims made so far.
    #[must_use]
    pub const fn attempt_count(self) -> u32 {
        self.attempt_count
    }

    /// Returns the stable creation time.
    #[must_use]
    pub const fn created_at(self) -> DesktopPostCommitTimestamp {
        self.created_at
    }
}

/// One bounded ordered recovery page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopPostCommitRecoveryPage {
    records: Vec<DesktopPostCommitEffectRecord>,
    next_cursor: Option<DesktopPostCommitRecoveryCursor>,
}

impl DesktopPostCommitRecoveryPage {
    /// Creates a page that does not exceed its requested bound.
    pub fn try_new(
        records: Vec<DesktopPostCommitEffectRecord>,
        next_cursor: Option<DesktopPostCommitRecoveryCursor>,
        limit: DesktopPostCommitRecoveryLimit,
    ) -> Result<Self, DesktopPostCommitEffectOutboxError> {
        if records.len() > usize::from(limit.get()) {
            Err(DesktopPostCommitEffectOutboxError::StorageFailure)
        } else {
            Ok(Self { records, next_cursor })
        }
    }

    /// Returns records in stable creation-time/effect-ID order.
    #[must_use]
    pub fn records(&self) -> &[DesktopPostCommitEffectRecord] {
        &self.records
    }

    /// Returns the cursor for the next page, when more results exist.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<DesktopPostCommitRecoveryCursor> {
        self.next_cursor
    }
}

/// Persistence boundary consumed only by `DesktopPostCommitEffectWorker` and startup recovery.
#[async_trait]
pub trait DesktopPostCommitEffectOutboxInterface: Send + Sync {
    /// Atomically claims the oldest Ready effect and increments its attempt count.
    async fn claim_next_post_commit_effect(
        &self,
        instance_id: DesktopApplicationInstanceId,
        claimed_at: DesktopPostCommitTimestamp,
    ) -> Result<Option<DesktopPostCommitEffectRecord>, DesktopPostCommitEffectOutboxError>;

    /// Atomically completes an effect claimed by the supplied current instance.
    async fn complete_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
        completed_at: DesktopPostCommitTimestamp,
    ) -> Result<(), DesktopPostCommitEffectOutboxError>;

    /// Atomically releases a transiently failed current claim to Ready.
    async fn release_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError>;

    /// Atomically abandons an effect claimed by the supplied current instance.
    async fn abandon_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
        abandoned_at: DesktopPostCommitTimestamp,
        reason: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError>;

    /// Lists prior-instance claims plus Ready Workflow effects in stable bounded order.
    async fn list_recoverable_post_commit_effects(
        &self,
        current_instance_id: DesktopApplicationInstanceId,
        cursor: Option<DesktopPostCommitRecoveryCursor>,
        limit: DesktopPostCommitRecoveryLimit,
    ) -> Result<DesktopPostCommitRecoveryPage, DesktopPostCommitEffectOutboxError>;

    /// CASes one observed prior Asset/Assistant claim back to Ready.
    async fn recover_replayable_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        prior_instance_id: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError>;

    /// CASes one observed Ready or prior-claimed Workflow effect to Abandoned.
    async fn recover_abandoned_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        expected_state: DesktopPostCommitEffectState,
        abandoned_at: DesktopPostCommitTimestamp,
        reason: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError>;
}
