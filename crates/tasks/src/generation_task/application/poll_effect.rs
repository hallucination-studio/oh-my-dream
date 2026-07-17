//! Closed PollTask effect execution.

use super::effect_support::{
    future_time, provider_call_failure, provider_failure, reschedule_before_deadline,
    route_is_cancellable, save_terminal,
};
use super::poll_dispatch::{PollDispatchError, PollEffectOutcome, execute_poll};
use super::{
    GenerationProviderResolvedRoute, GenerationTaskApplicationError, GenerationTaskAssetKey,
    GenerationTaskAssetRecovery, GenerationTaskBoundaryError, GenerationTaskClaimedEffect,
    GenerationTaskEffect, GenerationTaskEffectKind, GenerationTaskOriginState,
    GenerationTaskOutboxChanges, GenerationTaskProviderResult, GenerationTaskStoreAssetCommand,
};
use crate::generation_task::domain::{GenerationTaskAggregate, GenerationTaskResult};
use crate::generation_task::interfaces::{
    GenerationProviderCallErrorKind, GenerationProviderRegistryInterface,
    GenerationTaskAssetSinkInterface, GenerationTaskClockInterface,
    GenerationTaskOriginStateReaderInterface, GenerationTaskRepositoryInterface,
};

/// Executes exactly one claimed `PollTask` effect.
pub struct GenerationTaskPollEffectUseCase<R, P, O, A, C> {
    repository: R,
    provider_registry: P,
    origin_reader: O,
    asset_sink: A,
    clock: C,
}

impl<R, P, O, A, C> GenerationTaskPollEffectUseCase<R, P, O, A, C>
where
    R: GenerationTaskRepositoryInterface,
    P: GenerationProviderRegistryInterface,
    O: GenerationTaskOriginStateReaderInterface,
    A: GenerationTaskAssetSinkInterface,
    C: GenerationTaskClockInterface,
{
    /// Wires Poll delivery dependencies.
    #[must_use]
    pub const fn new(
        repository: R,
        provider_registry: P,
        origin_reader: O,
        asset_sink: A,
        clock: C,
    ) -> Self {
        Self { repository, provider_registry, origin_reader, asset_sink, clock }
    }

    /// Executes a safe accepted-handle observation or reschedules it.
    pub async fn execute_generation_task_poll_effect(
        &self,
        claimed: GenerationTaskClaimedEffect,
    ) -> Result<(), GenerationTaskApplicationError> {
        if claimed.effect().kind() != GenerationTaskEffectKind::PollTask {
            return Err(GenerationTaskApplicationError::InvalidEffect);
        }
        let mut task = self
            .repository
            .load_generation_task(claimed.effect().task_id())
            .await?
            .ok_or(GenerationTaskApplicationError::TaskNotFound)?;
        let route = self
            .provider_registry
            .resolve_generation_provider_route(task.target(), task.request().kind())?;
        let now = self.clock.observe_generation_task_time()?;
        if self.handle_origin(&mut task, &claimed, now, route).await? {
            return Ok(());
        }
        if self.handle_recovery(&mut task, &claimed, now, route).await? {
            return Ok(());
        }
        let handle = task
            .state()
            .remote_handle()
            .ok_or(GenerationTaskApplicationError::InvalidEffect)?
            .clone();
        let outcome = execute_poll(route, &task, &handle).await;
        self.apply_poll_outcome(task, claimed, now, route, outcome).await
    }

    async fn apply_poll_outcome(
        &self,
        mut task: GenerationTaskAggregate,
        claimed: GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &GenerationProviderResolvedRoute,
        outcome: Result<PollEffectOutcome, PollDispatchError>,
    ) -> Result<(), GenerationTaskApplicationError> {
        match outcome {
            Ok(PollEffectOutcome::Pending(progress)) => {
                self.save_pending(&mut task, progress.percent(), &claimed, now, route).await
            }
            Ok(PollEffectOutcome::Completed(result)) => {
                match self.complete(&mut task, result, &claimed, now).await {
                    Err(GenerationTaskApplicationError::Boundary(
                        GenerationTaskBoundaryError::Transient,
                    )) => {
                        let proposed_at =
                            future_time(now, route.policy().poll_interval_milliseconds())?;
                        reschedule_before_deadline(
                            &self.repository,
                            &mut task,
                            claimed,
                            proposed_at,
                            now,
                        )
                        .await
                    }
                    outcome => outcome,
                }
            }
            Ok(PollEffectOutcome::Failed(failure)) => {
                let expected = task.revision().get();
                task.fail(provider_failure(failure)?, now)?;
                save_terminal(&self.repository, &task, expected, &claimed, now).await
            }
            Ok(PollEffectOutcome::Cancelled) => {
                let expected = task.revision().get();
                task.mark_cancelled(now)?;
                save_terminal(&self.repository, &task, expected, &claimed, now).await
            }
            Err(PollDispatchError::ProviderCall(error))
                if error.kind() == GenerationProviderCallErrorKind::Transient =>
            {
                let proposed_at = error
                    .retry_at()
                    .unwrap_or(future_time(now, route.policy().poll_interval_milliseconds())?);
                reschedule_before_deadline(&self.repository, &mut task, claimed, proposed_at, now)
                    .await
            }
            Err(PollDispatchError::ProviderCall(error)) => {
                let expected = task.revision().get();
                task.fail(provider_call_failure(&error)?, now)?;
                save_terminal(&self.repository, &task, expected, &claimed, now).await
            }
            Err(PollDispatchError::InvalidRoute) => {
                Err(GenerationTaskApplicationError::InvalidEffect)
            }
        }
    }

    async fn handle_origin(
        &self,
        task: &mut GenerationTaskAggregate,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &GenerationProviderResolvedRoute,
    ) -> Result<bool, GenerationTaskApplicationError> {
        let state = self.origin_reader.read_generation_task_origin_state(task).await;
        match state {
            Ok(GenerationTaskOriginState::WaitingForExternalCompletion) => Ok(false),
            Ok(GenerationTaskOriginState::Cancelled | GenerationTaskOriginState::Terminal) => {
                self.cancel_from_origin(task.clone(), claimed.clone(), now, route).await?;
                Ok(true)
            }
            Ok(GenerationTaskOriginState::Running)
            | Err(GenerationTaskBoundaryError::Transient) => {
                self.reschedule(task, claimed.clone(), now, route).await?;
                Ok(true)
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn handle_recovery(
        &self,
        task: &mut GenerationTaskAggregate,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &GenerationProviderResolvedRoute,
    ) -> Result<bool, GenerationTaskApplicationError> {
        match self.recover(task, claimed, now, route).await {
            Ok(Some(())) => Ok(true),
            Ok(None) => Ok(false),
            Err(GenerationTaskApplicationError::Boundary(
                GenerationTaskBoundaryError::Transient,
            )) => {
                self.reschedule(task, claimed.clone(), now, route).await?;
                Ok(true)
            }
            Err(error) => Err(error),
        }
    }

    async fn reschedule(
        &self,
        task: &mut GenerationTaskAggregate,
        claimed: GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &GenerationProviderResolvedRoute,
    ) -> Result<(), GenerationTaskApplicationError> {
        let proposed_at = future_time(now, route.policy().poll_interval_milliseconds())?;
        reschedule_before_deadline(&self.repository, task, claimed, proposed_at, now).await
    }

    async fn save_pending(
        &self,
        task: &mut GenerationTaskAggregate,
        progress_percent: Option<u8>,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &GenerationProviderResolvedRoute,
    ) -> Result<(), GenerationTaskApplicationError> {
        let expected = task.revision().get();
        task.record_progress(progress_percent, now)?;
        let proposed_at = future_time(now, route.policy().poll_interval_milliseconds())?;
        if proposed_at >= task.provider_deadline_at() {
            task.fail(super::effect_support::timeout_failure()?, now)?;
            return save_terminal(&self.repository, task, expected, claimed, now).await;
        }
        self.repository
            .save_generation_task(
                task,
                expected,
                GenerationTaskOutboxChanges {
                    consume: Some(claimed.claim()),
                    enqueue: vec![GenerationTaskEffect::new(
                        task.id(),
                        GenerationTaskEffectKind::PollTask,
                        proposed_at,
                    )],
                },
            )
            .await
            .map_err(Into::into)
    }

    async fn recover(
        &self,
        task: &mut GenerationTaskAggregate,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &GenerationProviderResolvedRoute,
    ) -> Result<Option<()>, GenerationTaskApplicationError> {
        if matches!(task.request(), crate::generation_task::domain::GenerationTaskRequest::Text(_))
        {
            return Ok(None);
        }
        match self
            .asset_sink
            .recover_generation_task_asset(GenerationTaskAssetKey::from_task(task))
            .await?
        {
            GenerationTaskAssetRecovery::Available(asset) => {
                let expected = task.revision().get();
                task.complete(asset.result().clone(), now)?;
                save_terminal(&self.repository, task, expected, claimed, now).await?;
                Ok(Some(()))
            }
            GenerationTaskAssetRecovery::Pending => {
                let proposed_at = future_time(now, route.policy().poll_interval_milliseconds())?;
                reschedule_before_deadline(
                    &self.repository,
                    task,
                    claimed.clone(),
                    proposed_at,
                    now,
                )
                .await?;
                Ok(Some(()))
            }
            GenerationTaskAssetRecovery::SourceRequired => Ok(None),
        }
    }

    async fn complete(
        &self,
        task: &mut GenerationTaskAggregate,
        result: GenerationTaskProviderResult,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskApplicationError> {
        let expected = task.revision().get();
        let result = match result {
            GenerationTaskProviderResult::Text(value) => {
                GenerationTaskResult::Text { content: value.into_content() }
            }
            media => self
                .asset_sink
                .store_generation_task_asset(GenerationTaskStoreAssetCommand::from_task(
                    task, media, now,
                ))
                .await?
                .result()
                .clone(),
        };
        task.complete(result, now)?;
        save_terminal(&self.repository, task, expected, claimed, now).await
    }

    async fn cancel_from_origin(
        &self,
        mut task: GenerationTaskAggregate,
        claimed: GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &GenerationProviderResolvedRoute,
    ) -> Result<(), GenerationTaskApplicationError> {
        let expected = task.revision().get();
        task.request_cancellation(now)?;
        if route_is_cancellable(route) {
            self.repository
                .save_generation_task(
                    &task,
                    expected,
                    GenerationTaskOutboxChanges {
                        consume: Some(claimed.claim()),
                        enqueue: vec![GenerationTaskEffect::new(
                            task.id(),
                            GenerationTaskEffectKind::CancelRemoteTask,
                            now,
                        )],
                    },
                )
                .await?;
            Ok(())
        } else {
            task.mark_cancelled(now)?;
            save_terminal(&self.repository, &task, expected, &claimed, now).await
        }
    }
}
