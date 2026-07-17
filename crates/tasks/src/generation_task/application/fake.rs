//! Deterministic in-memory fakes for Generation Task contract and use-case tests.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use engine::node_capability::WorkflowNodeExecutionId;
use projects::project::domain::ProjectId;

use super::{
    GenerationProviderRegistryError, GenerationProviderResolvedRoute, GenerationTaskBoundaryError,
    GenerationTaskClaimedEffect, GenerationTaskCreateResult, GenerationTaskCursorPage,
    GenerationTaskEffect, GenerationTaskEffectClaim, GenerationTaskEffectId,
    GenerationTaskEffectKind, GenerationTaskListQuery, GenerationTaskOutboxChanges,
    GenerationTaskRepositoryError, GenerationTaskSummaryView,
};
use crate::generation_task::domain::{
    GenerationTaskAggregate, GenerationTaskId, GenerationTaskRequestKind, GenerationTaskTarget,
    GenerationTaskTimestamp,
};
use crate::generation_task::interfaces::{
    GenerationProviderRegistryInterface, GenerationTaskClockInterface,
    GenerationTaskOutboxReaderInterface, GenerationTaskRepositoryInterface,
};

/// Deterministic in-memory repository implementing Task idempotency and outbox atomicity.
#[derive(Clone, Default)]
pub struct GenerationTaskRepositoryFakeImpl {
    inner: Arc<Mutex<RepositoryFakeState>>,
}

#[derive(Default)]
struct RepositoryFakeState {
    tasks: BTreeMap<GenerationTaskId, GenerationTaskAggregate>,
    idempotency: BTreeMap<(ProjectId, String), GenerationTaskId>,
    origins: BTreeMap<(ProjectId, WorkflowNodeExecutionId), GenerationTaskId>,
    effects: BTreeMap<GenerationTaskEffectId, StoredFakeEffect>,
    next_effect_id: u64,
}

#[derive(Clone)]
struct StoredFakeEffect {
    effect: GenerationTaskEffect,
    state: StoredFakeEffectState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StoredFakeEffectState {
    Ready,
    Claimed,
    Completed,
}

impl GenerationTaskRepositoryFakeImpl {
    /// Returns the number of durable aggregate rows.
    #[must_use]
    pub fn generation_task_count(&self) -> usize {
        self.inner.lock().map_or(0, |state| state.tasks.len())
    }

    /// Returns Ready effect kinds in row-ID order.
    #[must_use]
    pub fn ready_effect_kinds(&self) -> Vec<GenerationTaskEffectKind> {
        self.inner.lock().map_or_else(
            |_| Vec::new(),
            |state| {
                state
                    .effects
                    .values()
                    .filter(|stored| stored.state == StoredFakeEffectState::Ready)
                    .map(|stored| stored.effect.kind())
                    .collect()
            },
        )
    }

    /// Claims the first Ready effect of one exact kind.
    pub fn claim_ready_effect(
        &self,
        kind: GenerationTaskEffectKind,
    ) -> Result<Option<GenerationTaskClaimedEffect>, GenerationTaskRepositoryError> {
        let mut state = self.lock()?;
        let Some((id, stored)) = state.effects.iter_mut().find(|(_, stored)| {
            stored.state == StoredFakeEffectState::Ready && stored.effect.kind() == kind
        }) else {
            return Ok(None);
        };
        stored.state = StoredFakeEffectState::Claimed;
        Ok(Some(GenerationTaskClaimedEffect::new(
            GenerationTaskEffectClaim::new(*id),
            stored.effect.clone(),
        )))
    }

    /// Returns one trusted aggregate snapshot for assertions.
    pub fn generation_task(
        &self,
        id: GenerationTaskId,
    ) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError> {
        Ok(self.lock()?.tasks.get(&id).cloned())
    }

    fn lock(&self) -> Result<MutexGuard<'_, RepositoryFakeState>, GenerationTaskRepositoryError> {
        self.inner.lock().map_err(|_| GenerationTaskRepositoryError::StorageFailure)
    }
}

#[async_trait]
impl GenerationTaskOutboxReaderInterface for GenerationTaskRepositoryFakeImpl {
    async fn claim_next_generation_task_effect(
        &self,
        now: GenerationTaskTimestamp,
    ) -> Result<Option<GenerationTaskClaimedEffect>, GenerationTaskRepositoryError> {
        let mut state = self.lock()?;
        let claimed_tasks = state
            .effects
            .values()
            .filter(|stored| stored.state == StoredFakeEffectState::Claimed)
            .map(|stored| stored.effect.task_id())
            .collect::<std::collections::BTreeSet<_>>();
        let candidate = state
            .effects
            .iter()
            .filter(|(_, stored)| {
                stored.state == StoredFakeEffectState::Ready
                    && stored.effect.available_at() <= now
                    && !claimed_tasks.contains(&stored.effect.task_id())
            })
            .min_by_key(|(id, stored)| (stored.effect.available_at(), **id))
            .map(|(id, _)| *id);
        let Some(id) = candidate else {
            return Ok(None);
        };
        let stored =
            state.effects.get_mut(&id).ok_or(GenerationTaskRepositoryError::StorageFailure)?;
        stored.state = StoredFakeEffectState::Claimed;
        Ok(Some(GenerationTaskClaimedEffect::new(
            GenerationTaskEffectClaim::new(id),
            stored.effect.clone(),
        )))
    }

    async fn reset_claimed_generation_task_effects(
        &self,
    ) -> Result<u64, GenerationTaskRepositoryError> {
        let mut state = self.lock()?;
        let mut reset = 0_u64;
        for stored in state.effects.values_mut() {
            if stored.state == StoredFakeEffectState::Claimed {
                stored.state = StoredFakeEffectState::Ready;
                reset = reset.saturating_add(1);
            }
        }
        Ok(reset)
    }
}

#[async_trait]
impl GenerationTaskRepositoryInterface for GenerationTaskRepositoryFakeImpl {
    async fn create_generation_task(
        &self,
        task: &GenerationTaskAggregate,
        message: GenerationTaskEffect,
    ) -> Result<GenerationTaskCreateResult, GenerationTaskRepositoryError> {
        if message.task_id() != task.id() {
            return Err(GenerationTaskRepositoryError::Corruption);
        }
        let mut state = self.lock()?;
        let idempotency_key =
            (task.origin().project_id(), task.idempotency_key().as_str().to_owned());
        if let Some(existing_id) = state.idempotency.get(&idempotency_key) {
            return matching_existing(&state, *existing_id, task, true);
        }
        let origin_key = (task.origin().project_id(), task.origin().workflow_node_execution_id());
        if let Some(existing_id) = state.origins.get(&origin_key) {
            return matching_existing(&state, *existing_id, task, false);
        }
        state.idempotency.insert(idempotency_key, task.id());
        state.origins.insert(origin_key, task.id());
        state.tasks.insert(task.id(), task.clone());
        enqueue_effect(&mut state, message)?;
        Ok(GenerationTaskCreateResult::Created(task.clone()))
    }

    async fn load_generation_task(
        &self,
        id: GenerationTaskId,
    ) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError> {
        Ok(self.lock()?.tasks.get(&id).cloned())
    }

    async fn load_generation_task_for_project(
        &self,
        project_id: ProjectId,
        id: GenerationTaskId,
    ) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError> {
        Ok(self
            .lock()?
            .tasks
            .get(&id)
            .filter(|task| task.origin().project_id() == project_id)
            .cloned())
    }

    async fn save_generation_task(
        &self,
        task: &GenerationTaskAggregate,
        expected_revision: u64,
        outbox: GenerationTaskOutboxChanges,
    ) -> Result<(), GenerationTaskRepositoryError> {
        let mut state = self.lock()?;
        let current =
            state.tasks.get(&task.id()).ok_or(GenerationTaskRepositoryError::StorageFailure)?;
        if current.revision().get() != expected_revision
            || task.revision().get() < expected_revision
            || !current.has_same_immutable_facts(task)
            || outbox.enqueue.iter().any(|effect| effect.task_id() != task.id())
        {
            return Err(
                if current.revision().get() != expected_revision
                    || task.revision().get() < expected_revision
                {
                    GenerationTaskRepositoryError::OptimisticConflict
                } else {
                    GenerationTaskRepositoryError::Corruption
                },
            );
        }
        if let Some(claim) = outbox.consume {
            let stored = state
                .effects
                .get_mut(&claim.effect_id())
                .ok_or(GenerationTaskRepositoryError::EffectClaimConflict)?;
            if stored.state != StoredFakeEffectState::Claimed
                || stored.effect.task_id() != task.id()
            {
                return Err(GenerationTaskRepositoryError::EffectClaimConflict);
            }
            stored.state = StoredFakeEffectState::Completed;
        }
        state.tasks.insert(task.id(), task.clone());
        for effect in outbox.enqueue {
            enqueue_effect(&mut state, effect)?;
        }
        Ok(())
    }

    async fn list_generation_tasks(
        &self,
        query: GenerationTaskListQuery,
    ) -> Result<GenerationTaskCursorPage<GenerationTaskSummaryView>, GenerationTaskRepositoryError>
    {
        let state = self.lock()?;
        let mut tasks = state
            .tasks
            .values()
            .filter(|task| task.origin().project_id() == query.project_id())
            .filter(|task| {
                query.status().is_none_or(|status| {
                    super::GenerationTaskStatus::from_state(task.state()) == status
                })
            })
            .filter(|task| query.request_kind().is_none_or(|kind| task.request().kind() == kind))
            .filter(|task| {
                query.cursor().is_none_or(|cursor| {
                    (task.created_at(), task.id()) < (cursor.created_at, cursor.task_id)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by_key(|task| std::cmp::Reverse((task.created_at(), task.id())));
        let limit = usize::from(query.limit());
        let has_more = tasks.len() > limit;
        tasks.truncate(limit);
        let next_cursor = has_more.then(|| {
            let task = &tasks[tasks.len() - 1];
            super::GenerationTaskListCursor { created_at: task.created_at(), task_id: task.id() }
        });
        Ok(GenerationTaskCursorPage {
            items: tasks.iter().map(GenerationTaskSummaryView::from_task).collect(),
            next_cursor,
        })
    }
}

fn matching_existing(
    state: &RepositoryFakeState,
    existing_id: GenerationTaskId,
    task: &GenerationTaskAggregate,
    idempotency_match: bool,
) -> Result<GenerationTaskCreateResult, GenerationTaskRepositoryError> {
    let existing =
        state.tasks.get(&existing_id).ok_or(GenerationTaskRepositoryError::StorageFailure)?;
    if existing.request_hash() != task.request_hash() {
        return Err(if idempotency_match {
            GenerationTaskRepositoryError::IdempotencyConflict
        } else {
            GenerationTaskRepositoryError::OriginConflict
        });
    }
    Ok(GenerationTaskCreateResult::Existing(existing.clone()))
}

fn enqueue_effect(
    state: &mut RepositoryFakeState,
    effect: GenerationTaskEffect,
) -> Result<(), GenerationTaskRepositoryError> {
    state.next_effect_id =
        state.next_effect_id.checked_add(1).ok_or(GenerationTaskRepositoryError::StorageFailure)?;
    let id = GenerationTaskEffectId::try_new(state.next_effect_id)
        .ok_or(GenerationTaskRepositoryError::StorageFailure)?;
    state.effects.insert(id, StoredFakeEffect { effect, state: StoredFakeEffectState::Ready });
    Ok(())
}

/// Deterministic registry fake returning one exact type-matching route.
pub struct GenerationProviderRegistryFakeImpl {
    route: GenerationProviderResolvedRoute,
}

impl GenerationProviderRegistryFakeImpl {
    /// Stores one exact resolved route.
    #[must_use]
    pub const fn new(route: GenerationProviderResolvedRoute) -> Self {
        Self { route }
    }
}

impl GenerationProviderRegistryInterface for GenerationProviderRegistryFakeImpl {
    fn resolve_generation_provider_route(
        &self,
        _target: &GenerationTaskTarget,
        request_kind: GenerationTaskRequestKind,
    ) -> Result<&GenerationProviderResolvedRoute, GenerationProviderRegistryError> {
        if self.route.request_kind() != request_kind {
            return Err(GenerationProviderRegistryError::RequestKindMismatch);
        }
        Ok(&self.route)
    }
}

/// Fixed deterministic task clock fake.
#[derive(Clone, Copy)]
pub struct GenerationTaskClockFakeImpl {
    now: GenerationTaskTimestamp,
}

impl GenerationTaskClockFakeImpl {
    /// Creates a clock returning one fixed observation.
    #[must_use]
    pub const fn new(now: GenerationTaskTimestamp) -> Self {
        Self { now }
    }
}

impl GenerationTaskClockInterface for GenerationTaskClockFakeImpl {
    fn observe_generation_task_time(
        &self,
    ) -> Result<GenerationTaskTimestamp, GenerationTaskBoundaryError> {
        Ok(self.now)
    }
}
