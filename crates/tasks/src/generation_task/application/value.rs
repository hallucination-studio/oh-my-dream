//! Generation Task application commands, effects, and routing values.

use crate::generation_task::domain::{
    GenerationTaskAggregate, GenerationTaskId, GenerationTaskIdempotencyKey, GenerationTaskOrigin,
    GenerationTaskRequest, GenerationTaskRequestKind, GenerationTaskTarget,
    GenerationTaskTimestamp,
};
use crate::generation_task::interfaces::{
    ImageGenerationProviderExecution, TextGenerationProviderExecution,
    VideoGenerationProviderExecution, VoiceGenerationProviderExecution,
};

/// Bounded deadline and poll timing for one exact shipped route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationProviderRoutePolicy {
    task_deadline_milliseconds: u64,
    poll_interval_milliseconds: u64,
}

impl GenerationProviderRoutePolicy {
    /// Validates a positive deadline and the frozen 500..=5,000 ms poll interval.
    pub const fn try_new(
        task_deadline_milliseconds: u64,
        poll_interval_milliseconds: u64,
    ) -> Result<Self, super::GenerationProviderRegistryError> {
        if task_deadline_milliseconds == 0
            || task_deadline_milliseconds <= poll_interval_milliseconds
            || poll_interval_milliseconds < 500
            || poll_interval_milliseconds > 5_000
        {
            return Err(super::GenerationProviderRegistryError::RouteNotFound);
        }
        Ok(Self { task_deadline_milliseconds, poll_interval_milliseconds })
    }

    /// Returns the immutable task deadline budget.
    #[must_use]
    pub const fn task_deadline_milliseconds(self) -> u64 {
        self.task_deadline_milliseconds
    }

    /// Returns the immutable poll interval.
    #[must_use]
    pub const fn poll_interval_milliseconds(self) -> u64 {
        self.poll_interval_milliseconds
    }
}

/// Exact type-specific route execution resolved from one immutable target.
pub enum GenerationProviderResolvedRoute {
    /// Complete Text execution.
    Text {
        /// Immediate or remote Text execution.
        execution: TextGenerationProviderExecution,
        /// Frozen route timing.
        policy: GenerationProviderRoutePolicy,
    },
    /// Complete Image execution.
    Image {
        /// Immediate or remote Image execution.
        execution: ImageGenerationProviderExecution,
        /// Frozen route timing.
        policy: GenerationProviderRoutePolicy,
    },
    /// Complete Video execution.
    Video {
        /// Immediate or remote Video execution.
        execution: VideoGenerationProviderExecution,
        /// Frozen route timing.
        policy: GenerationProviderRoutePolicy,
    },
    /// Complete Voice execution.
    Voice {
        /// Immediate or remote Voice execution.
        execution: VoiceGenerationProviderExecution,
        /// Frozen route timing.
        policy: GenerationProviderRoutePolicy,
    },
}

impl GenerationProviderResolvedRoute {
    /// Returns the exact request kind supported by this route.
    #[must_use]
    pub const fn request_kind(&self) -> GenerationTaskRequestKind {
        match self {
            Self::Text { .. } => GenerationTaskRequestKind::Text,
            Self::Image { .. } => GenerationTaskRequestKind::Image,
            Self::Video { .. } => GenerationTaskRequestKind::Video,
            Self::Voice { .. } => GenerationTaskRequestKind::Voice,
        }
    }

    /// Returns the frozen route timing.
    #[must_use]
    pub const fn policy(&self) -> GenerationProviderRoutePolicy {
        match self {
            Self::Text { policy, .. }
            | Self::Image { policy, .. }
            | Self::Video { policy, .. }
            | Self::Voice { policy, .. } => *policy,
        }
    }
}

/// Internal command admitting one exact Workflow-owned generation intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationTaskStartCommand {
    task_id: GenerationTaskId,
    origin: GenerationTaskOrigin,
    idempotency_key: GenerationTaskIdempotencyKey,
    target: GenerationTaskTarget,
    request: GenerationTaskRequest,
}

impl GenerationTaskStartCommand {
    /// Combines already-validated immutable admission facts.
    #[must_use]
    pub const fn new(
        task_id: GenerationTaskId,
        origin: GenerationTaskOrigin,
        idempotency_key: GenerationTaskIdempotencyKey,
        target: GenerationTaskTarget,
        request: GenerationTaskRequest,
    ) -> Self {
        Self { task_id, origin, idempotency_key, target, request }
    }

    pub(super) const fn task_id(&self) -> GenerationTaskId {
        self.task_id
    }
    pub(super) const fn origin(&self) -> &GenerationTaskOrigin {
        &self.origin
    }
    pub(super) const fn idempotency_key(&self) -> &GenerationTaskIdempotencyKey {
        &self.idempotency_key
    }
    pub(super) const fn target(&self) -> &GenerationTaskTarget {
        &self.target
    }
    pub(super) const fn request(&self) -> &GenerationTaskRequest {
        &self.request
    }
}

/// Durable result proving one exact Generation Task exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationTaskStartResult {
    task_id: GenerationTaskId,
}

impl GenerationTaskStartResult {
    pub(super) const fn new(task_id: GenerationTaskId) -> Self {
        Self { task_id }
    }

    /// Returns the durable local task identity.
    #[must_use]
    pub const fn task_id(self) -> GenerationTaskId {
        self.task_id
    }
}

/// Closed delayed Generation Task effect kind.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum GenerationTaskEffectKind {
    /// Submit or execute one not-yet-accepted task.
    SubmitTask,
    /// Poll one accepted remote handle.
    PollTask,
    /// Attempt cancellation through a complete remote canceller.
    CancelRemoteTask,
    /// Deliver one terminal Task outcome to Workflow.
    NotifyWorkflow,
}

/// One closed delayed effect with no arbitrary payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationTaskEffect {
    task_id: GenerationTaskId,
    kind: GenerationTaskEffectKind,
    available_at: GenerationTaskTimestamp,
    delivery_attempts: u32,
}

impl GenerationTaskEffect {
    /// Creates initial delayed work for one Task.
    #[must_use]
    pub const fn new(
        task_id: GenerationTaskId,
        kind: GenerationTaskEffectKind,
        available_at: GenerationTaskTimestamp,
    ) -> Self {
        Self { task_id, kind, available_at, delivery_attempts: 0 }
    }

    /// Returns the owning Task.
    #[must_use]
    pub const fn task_id(&self) -> GenerationTaskId {
        self.task_id
    }

    /// Returns the closed work kind.
    #[must_use]
    pub const fn kind(&self) -> GenerationTaskEffectKind {
        self.kind
    }

    /// Returns the earliest execution time.
    #[must_use]
    pub const fn available_at(&self) -> GenerationTaskTimestamp {
        self.available_at
    }

    /// Returns the diagnostic delivery-attempt count.
    #[must_use]
    pub const fn delivery_attempts(&self) -> u32 {
        self.delivery_attempts
    }

    pub(super) fn rescheduled(mut self, available_at: GenerationTaskTimestamp) -> Self {
        self.available_at = available_at;
        self.delivery_attempts = self.delivery_attempts.saturating_add(1);
        self
    }
}

/// Non-zero identity of one claimed outbox row.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GenerationTaskEffectId(u64);

impl GenerationTaskEffectId {
    /// Restores a non-zero effect row identity.
    #[must_use]
    pub const fn try_new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// Returns the stored row identity.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Proof that one exact outbox row remains claimed by the worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationTaskEffectClaim {
    effect_id: GenerationTaskEffectId,
}

impl GenerationTaskEffectClaim {
    /// Wraps one claimed effect identity.
    #[must_use]
    pub const fn new(effect_id: GenerationTaskEffectId) -> Self {
        Self { effect_id }
    }

    /// Returns the exact claimed effect identity.
    #[must_use]
    pub const fn effect_id(self) -> GenerationTaskEffectId {
        self.effect_id
    }
}

/// Claimed effect plus its closed task-owned payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationTaskClaimedEffect {
    claim: GenerationTaskEffectClaim,
    effect: GenerationTaskEffect,
}

impl GenerationTaskClaimedEffect {
    /// Combines one claim with its decoded closed effect.
    #[must_use]
    pub const fn new(claim: GenerationTaskEffectClaim, effect: GenerationTaskEffect) -> Self {
        Self { claim, effect }
    }

    /// Returns claim proof.
    #[must_use]
    pub const fn claim(&self) -> GenerationTaskEffectClaim {
        self.claim
    }

    /// Returns the closed effect.
    #[must_use]
    pub const fn effect(&self) -> &GenerationTaskEffect {
        &self.effect
    }
}

/// Atomic outbox changes accompanying one aggregate save.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenerationTaskOutboxChanges {
    /// Claimed row consumed by this transition, when any.
    pub consume: Option<GenerationTaskEffectClaim>,
    /// New closed effects committed with the transition.
    pub enqueue: Vec<GenerationTaskEffect>,
}

/// Idempotent repository creation outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenerationTaskCreateResult {
    /// The supplied aggregate and effect were inserted.
    Created(GenerationTaskAggregate),
    /// Matching immutable facts already identify this aggregate.
    Existing(GenerationTaskAggregate),
}

impl GenerationTaskCreateResult {
    /// Returns the created or replayed aggregate.
    #[must_use]
    pub const fn task(&self) -> &GenerationTaskAggregate {
        match self {
            Self::Created(task) | Self::Existing(task) => task,
        }
    }
}
