//! Generation Task aggregate and canonical admission hash.

use assets::asset::domain::AssetMediaKind;

use super::{
    GenerationProviderTaskHandle, GenerationTaskDomainError, GenerationTaskFailure,
    GenerationTaskId, GenerationTaskIdempotencyKey, GenerationTaskOrigin, GenerationTaskRequest,
    GenerationTaskRequestHash, GenerationTaskRequestKind, GenerationTaskResult,
    GenerationTaskRevision, GenerationTaskState, GenerationTaskTarget, GenerationTaskTimestamp,
    canonical::canonical_request_hash,
};

/// Authoritative aggregate for one durable provider-backed generation lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationTaskAggregate {
    id: GenerationTaskId,
    origin: GenerationTaskOrigin,
    idempotency_key: GenerationTaskIdempotencyKey,
    request_hash: GenerationTaskRequestHash,
    target: GenerationTaskTarget,
    request: GenerationTaskRequest,
    provider_deadline_at: GenerationTaskTimestamp,
    state: GenerationTaskState,
    result: Option<GenerationTaskResult>,
    created_at: GenerationTaskTimestamp,
    updated_at: GenerationTaskTimestamp,
    revision: GenerationTaskRevision,
}

impl GenerationTaskAggregate {
    /// Creates one queued task and computes its canonical request hash.
    pub fn create(
        id: GenerationTaskId,
        origin: GenerationTaskOrigin,
        idempotency_key: GenerationTaskIdempotencyKey,
        target: GenerationTaskTarget,
        request: GenerationTaskRequest,
        created_at: GenerationTaskTimestamp,
        provider_deadline_at: GenerationTaskTimestamp,
    ) -> Result<Self, GenerationTaskDomainError> {
        if provider_deadline_at <= created_at {
            return Err(GenerationTaskDomainError::InvalidTimestamp);
        }
        let request_hash = canonical_request_hash(&origin, &request, &target);
        Ok(Self {
            id,
            origin,
            idempotency_key,
            request_hash,
            target,
            request,
            provider_deadline_at,
            state: GenerationTaskState::Queued,
            result: None,
            created_at,
            updated_at: created_at,
            revision: GenerationTaskRevision::initial(),
        })
    }

    /// Restores an aggregate only when every persisted invariant agrees.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: GenerationTaskId,
        origin: GenerationTaskOrigin,
        idempotency_key: GenerationTaskIdempotencyKey,
        request_hash: GenerationTaskRequestHash,
        target: GenerationTaskTarget,
        request: GenerationTaskRequest,
        provider_deadline_at: GenerationTaskTimestamp,
        state: GenerationTaskState,
        result: Option<GenerationTaskResult>,
        created_at: GenerationTaskTimestamp,
        updated_at: GenerationTaskTimestamp,
        revision: GenerationTaskRevision,
    ) -> Result<Self, GenerationTaskDomainError> {
        if request_hash != canonical_request_hash(&origin, &request, &target) {
            return Err(GenerationTaskDomainError::InvalidRequestHash);
        }
        validate_restored_state(
            &request,
            &state,
            result.as_ref(),
            created_at,
            updated_at,
            provider_deadline_at,
        )?;
        Ok(Self {
            id,
            origin,
            idempotency_key,
            request_hash,
            target,
            request,
            provider_deadline_at,
            state,
            result,
            created_at,
            updated_at,
            revision,
        })
    }

    /// Moves queued work into the submission uncertainty window.
    pub fn begin_submission(
        &mut self,
        occurred_at: GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskDomainError> {
        if self.state != GenerationTaskState::Queued {
            return Err(GenerationTaskDomainError::IllegalTransition);
        }
        self.commit_transition(GenerationTaskState::Submitting, occurred_at)
    }

    /// Commits an accepted handle, preserving any concurrent cancellation intent.
    pub fn accept_remote_submission(
        &mut self,
        handle: GenerationProviderTaskHandle,
        occurred_at: GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskDomainError> {
        let next_state = match &self.state {
            GenerationTaskState::Submitting => {
                GenerationTaskState::Running { handle, progress_percent: None }
            }
            GenerationTaskState::CancelRequested { handle: None } => {
                GenerationTaskState::CancelRequested { handle: Some(handle) }
            }
            GenerationTaskState::CancelRequested { handle: Some(existing) }
                if existing == &handle =>
            {
                return Ok(());
            }
            _ => return Err(GenerationTaskDomainError::IllegalTransition),
        };
        self.commit_transition(next_state, occurred_at)
    }

    /// Records optional normalized progress without allowing regression.
    pub fn record_progress(
        &mut self,
        progress_percent: Option<u8>,
        occurred_at: GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskDomainError> {
        if progress_percent.is_some_and(|value| value > 100) {
            return Err(GenerationTaskDomainError::ProgressOutOfRange);
        }
        let GenerationTaskState::Running { handle, progress_percent: current } = &self.state else {
            return Err(GenerationTaskDomainError::IllegalTransition);
        };
        if matches!((current, progress_percent), (Some(_), None))
            || matches!((current, progress_percent), (Some(previous), Some(next)) if next < *previous)
        {
            return Err(GenerationTaskDomainError::ProgressRegressed);
        }
        let next_state = GenerationTaskState::Running { handle: handle.clone(), progress_percent };
        self.commit_transition(next_state, occurred_at)
    }

    /// Commits cancellation intent or immediate queued cancellation.
    pub fn request_cancellation(
        &mut self,
        occurred_at: GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskDomainError> {
        let next_state = match &self.state {
            GenerationTaskState::Queued => {
                GenerationTaskState::Cancelled { completed_at: occurred_at }
            }
            GenerationTaskState::Submitting => {
                GenerationTaskState::CancelRequested { handle: None }
            }
            GenerationTaskState::Running { handle, .. } => {
                GenerationTaskState::CancelRequested { handle: Some(handle.clone()) }
            }
            GenerationTaskState::CancelRequested { .. } => return Ok(()),
            _ => return Err(GenerationTaskDomainError::IllegalTransition),
        };
        self.commit_transition(next_state, occurred_at)
    }

    /// Commits local or provider-observed cancellation.
    pub fn mark_cancelled(
        &mut self,
        completed_at: GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskDomainError> {
        if !matches!(
            self.state,
            GenerationTaskState::Running { .. } | GenerationTaskState::CancelRequested { .. }
        ) {
            return Err(GenerationTaskDomainError::IllegalTransition);
        }
        self.commit_transition(GenerationTaskState::Cancelled { completed_at }, completed_at)
    }

    /// Commits one result only when it matches the immutable request kind.
    pub fn complete(
        &mut self,
        result: GenerationTaskResult,
        completed_at: GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskDomainError> {
        if !matches!(
            self.state,
            GenerationTaskState::Submitting | GenerationTaskState::Running { .. }
        ) {
            return Err(GenerationTaskDomainError::IllegalTransition);
        }
        if !result_matches_request(&self.request, &result) {
            return Err(GenerationTaskDomainError::ResultKindMismatch);
        }
        self.commit_transition(GenerationTaskState::Succeeded { completed_at }, completed_at)?;
        self.result = Some(result);
        Ok(())
    }

    /// Commits one structured failure from an active non-cancelling state.
    pub fn fail(
        &mut self,
        failure: GenerationTaskFailure,
        completed_at: GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskDomainError> {
        if !matches!(
            self.state,
            GenerationTaskState::Queued
                | GenerationTaskState::Submitting
                | GenerationTaskState::Running { .. }
        ) {
            return Err(GenerationTaskDomainError::IllegalTransition);
        }
        self.commit_transition(GenerationTaskState::Failed { completed_at, failure }, completed_at)
    }

    fn commit_transition(
        &mut self,
        state: GenerationTaskState,
        occurred_at: GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskDomainError> {
        if occurred_at < self.updated_at {
            return Err(GenerationTaskDomainError::InvalidTimestamp);
        }
        let revision = self.revision.next()?;
        self.state = state;
        self.updated_at = occurred_at;
        self.revision = revision;
        Ok(())
    }

    /// Returns the local task identity.
    #[must_use]
    pub const fn id(&self) -> GenerationTaskId {
        self.id
    }

    /// Returns the immutable Workflow origin.
    #[must_use]
    pub const fn origin(&self) -> &GenerationTaskOrigin {
        &self.origin
    }

    /// Returns the caller idempotency key.
    #[must_use]
    pub const fn idempotency_key(&self) -> &GenerationTaskIdempotencyKey {
        &self.idempotency_key
    }

    /// Returns the canonical immutable request hash.
    #[must_use]
    pub const fn request_hash(&self) -> GenerationTaskRequestHash {
        self.request_hash
    }

    /// Returns the immutable admitted target.
    #[must_use]
    pub const fn target(&self) -> &GenerationTaskTarget {
        &self.target
    }

    /// Returns the immutable semantic request.
    #[must_use]
    pub const fn request(&self) -> &GenerationTaskRequest {
        &self.request
    }

    /// Returns the immutable provider deadline.
    #[must_use]
    pub const fn provider_deadline_at(&self) -> GenerationTaskTimestamp {
        self.provider_deadline_at
    }

    /// Returns the authoritative lifecycle state.
    #[must_use]
    pub const fn state(&self) -> &GenerationTaskState {
        &self.state
    }

    /// Returns known running progress.
    #[must_use]
    pub const fn progress_percent(&self) -> Option<u8> {
        self.state.progress_percent()
    }

    /// Returns the single result only after success.
    #[must_use]
    pub const fn result(&self) -> Option<&GenerationTaskResult> {
        self.result.as_ref()
    }

    /// Returns creation time.
    #[must_use]
    pub const fn created_at(&self) -> GenerationTaskTimestamp {
        self.created_at
    }

    /// Returns the latest committed transition time.
    #[must_use]
    pub const fn updated_at(&self) -> GenerationTaskTimestamp {
        self.updated_at
    }

    /// Returns the optimistic-lock revision.
    #[must_use]
    pub const fn revision(&self) -> GenerationTaskRevision {
        self.revision
    }

    /// Compares every immutable persisted fact while excluding lifecycle state and revision.
    #[must_use]
    pub fn has_same_immutable_facts(&self, other: &Self) -> bool {
        self.id == other.id
            && self.origin == other.origin
            && self.idempotency_key == other.idempotency_key
            && self.request_hash == other.request_hash
            && self.target == other.target
            && self.request == other.request
            && self.provider_deadline_at == other.provider_deadline_at
            && self.created_at == other.created_at
    }
}

fn validate_restored_state(
    request: &GenerationTaskRequest,
    state: &GenerationTaskState,
    result: Option<&GenerationTaskResult>,
    created_at: GenerationTaskTimestamp,
    updated_at: GenerationTaskTimestamp,
    provider_deadline_at: GenerationTaskTimestamp,
) -> Result<(), GenerationTaskDomainError> {
    let valid_result = match (state, result) {
        (GenerationTaskState::Succeeded { .. }, Some(result)) => {
            result_matches_request(request, result)
        }
        (GenerationTaskState::Succeeded { .. }, None) => false,
        (_, None) => true,
        (_, Some(_)) => false,
    };
    let valid_progress = state.progress_percent().is_none_or(|value| value <= 100);
    let valid_time = updated_at >= created_at
        && provider_deadline_at > created_at
        && state.completed_at().is_none_or(|value| value >= created_at && value <= updated_at);
    if !valid_result || !valid_progress || !valid_time {
        return Err(GenerationTaskDomainError::InvalidRestoredState);
    }
    Ok(())
}

fn result_matches_request(request: &GenerationTaskRequest, result: &GenerationTaskResult) -> bool {
    match (request.kind(), result) {
        (GenerationTaskRequestKind::Text, GenerationTaskResult::Text { .. }) => true,
        (GenerationTaskRequestKind::Image, GenerationTaskResult::Asset(result)) => {
            result.media_kind() == AssetMediaKind::Image
        }
        (GenerationTaskRequestKind::Voice, GenerationTaskResult::Asset(result)) => {
            result.media_kind() == AssetMediaKind::Audio
        }
        (GenerationTaskRequestKind::Video, GenerationTaskResult::Asset(result)) => {
            result.media_kind() == AssetMediaKind::Video
        }
        _ => false,
    }
}
