//! Closed SubmitTask effect execution.

use super::effect_support::{
    ambiguous_submission_failure, future_time, provider_failure, reschedule_before_deadline,
    route_is_cancellable, save_terminal,
};
use super::submit_dispatch::{SubmitDispatchError, SubmitEffectOutcome, execute_submit};
use super::{
    GenerationTaskApplicationError, GenerationTaskAssetKey, GenerationTaskAssetRecovery,
    GenerationTaskBoundaryError, GenerationTaskClaimedEffect, GenerationTaskEffectKind,
    GenerationTaskOriginState, GenerationTaskProviderResult, GenerationTaskStoreAssetCommand,
};
use crate::generation_task::domain::{
    GenerationTaskAggregate, GenerationTaskRequest, GenerationTaskResult, GenerationTaskState,
};
use crate::generation_task::interfaces::{
    GenerationProviderRegistryInterface, GenerationTaskAssetSinkInterface,
    GenerationTaskClockInterface, GenerationTaskOriginStateReaderInterface,
    GenerationTaskRepositoryInterface,
};

/// Executes exactly one claimed `SubmitTask` effect.
pub struct GenerationTaskSubmitEffectUseCase<R, P, O, A, C> {
    repository: R,
    provider_registry: P,
    origin_reader: O,
    asset_sink: A,
    clock: C,
}

impl<R, P, O, A, C> GenerationTaskSubmitEffectUseCase<R, P, O, A, C>
where
    R: GenerationTaskRepositoryInterface,
    P: GenerationProviderRegistryInterface,
    O: GenerationTaskOriginStateReaderInterface,
    A: GenerationTaskAssetSinkInterface,
    C: GenerationTaskClockInterface,
{
    /// Wires the exact dependencies consumed by Submit delivery.
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

    /// Executes one claimed Submit effect without ever retrying an uncertain call.
    pub async fn execute_generation_task_submit_effect(
        &self,
        claimed: GenerationTaskClaimedEffect,
    ) -> Result<(), GenerationTaskApplicationError> {
        require_submit(&claimed)?;
        let mut task = self.load_task(&claimed).await?;
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
        if task.state() == &GenerationTaskState::Submitting {
            return self.fail_ambiguous(task, &claimed, now).await;
        }
        let expected_revision = task.revision().get();
        task.begin_submission(now)?;
        self.repository.save_generation_task(&task, expected_revision, Default::default()).await?;
        let submit_revision = task.revision().get();
        let outcome = execute_submit(route, &task).await;
        let applied = self
            .apply_submit_outcome(
                task,
                claimed.clone(),
                now,
                route,
                submit_revision,
                outcome.clone(),
            )
            .await;
        if applied
            == Err(GenerationTaskApplicationError::Repository(
                super::GenerationTaskRepositoryError::OptimisticConflict,
            ))
        {
            return self.reconcile_cancelled_submit(claimed, now, route, outcome).await;
        }
        applied
    }

    async fn apply_submit_outcome(
        &self,
        mut task: GenerationTaskAggregate,
        claimed: GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &super::GenerationProviderResolvedRoute,
        submit_revision: u64,
        outcome: Result<SubmitEffectOutcome, SubmitDispatchError>,
    ) -> Result<(), GenerationTaskApplicationError> {
        match outcome {
            Ok(SubmitEffectOutcome::Accepted(handle)) => {
                self.save_accepted(&mut task, handle, &claimed, now, route, submit_revision).await
            }
            Ok(SubmitEffectOutcome::Completed(result)) => {
                self.save_completed(&mut task, result, claimed, now, route, submit_revision).await
            }
            Ok(SubmitEffectOutcome::Rejected(failure)) => {
                task.fail(provider_failure(failure)?, now)?;
                save_terminal(&self.repository, &task, submit_revision, &claimed, now).await
            }
            Err(SubmitDispatchError::ProviderCall) => {
                task.fail(ambiguous_submission_failure()?, now)?;
                save_terminal(&self.repository, &task, submit_revision, &claimed, now).await
            }
            Err(SubmitDispatchError::InvalidContext | SubmitDispatchError::InvalidRoute) => {
                Err(GenerationTaskApplicationError::InvalidEffect)
            }
        }
    }

    async fn save_accepted(
        &self,
        task: &mut GenerationTaskAggregate,
        handle: crate::generation_task::domain::GenerationProviderTaskHandle,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &super::GenerationProviderResolvedRoute,
        submit_revision: u64,
    ) -> Result<(), GenerationTaskApplicationError> {
        task.accept_remote_submission(handle, now)?;
        let poll_at = future_time(now, route.policy().poll_interval_milliseconds())?;
        if poll_at >= task.provider_deadline_at() {
            task.fail(super::effect_support::timeout_failure()?, now)?;
            return save_terminal(&self.repository, task, submit_revision, claimed, now).await;
        }
        self.repository
            .save_generation_task(
                task,
                submit_revision,
                super::GenerationTaskOutboxChanges {
                    consume: Some(claimed.claim()),
                    enqueue: vec![super::GenerationTaskEffect::new(
                        task.id(),
                        super::GenerationTaskEffectKind::PollTask,
                        poll_at,
                    )],
                },
            )
            .await
            .map_err(Into::into)
    }

    async fn save_completed(
        &self,
        task: &mut GenerationTaskAggregate,
        result: GenerationTaskProviderResult,
        claimed: GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &super::GenerationProviderResolvedRoute,
        submit_revision: u64,
    ) -> Result<(), GenerationTaskApplicationError> {
        match self.complete_result(task, result, now).await {
            Ok(()) => save_terminal(&self.repository, task, submit_revision, &claimed, now).await,
            Err(GenerationTaskApplicationError::Boundary(
                GenerationTaskBoundaryError::Transient,
            )) => self.reschedule(task, claimed, now, route).await,
            Err(error) => Err(error),
        }
    }

    async fn reconcile_cancelled_submit(
        &self,
        claimed: GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &super::GenerationProviderResolvedRoute,
        outcome: Result<SubmitEffectOutcome, SubmitDispatchError>,
    ) -> Result<(), GenerationTaskApplicationError> {
        let mut task = self.load_task(&claimed).await?;
        let expected_revision = task.revision().get();
        if !matches!(
            task.state(),
            GenerationTaskState::CancelRequested { .. } | GenerationTaskState::Cancelled { .. }
        ) {
            return Err(super::GenerationTaskRepositoryError::OptimisticConflict.into());
        }
        if let Ok(SubmitEffectOutcome::Accepted(handle)) = outcome {
            task.accept_remote_submission(handle, now)?;
            if route_is_cancellable(route) {
                return self.save_cancel_remote(&task, expected_revision, &claimed, now).await;
            }
        }
        if !task.state().is_terminal() {
            task.mark_cancelled(now)?;
        }
        save_terminal(&self.repository, &task, expected_revision, &claimed, now).await
    }

    async fn save_cancel_remote(
        &self,
        task: &GenerationTaskAggregate,
        expected_revision: u64,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskApplicationError> {
        self.repository
            .save_generation_task(
                task,
                expected_revision,
                super::GenerationTaskOutboxChanges {
                    consume: Some(claimed.claim()),
                    enqueue: vec![super::GenerationTaskEffect::new(
                        task.id(),
                        super::GenerationTaskEffectKind::CancelRemoteTask,
                        now,
                    )],
                },
            )
            .await
            .map_err(Into::into)
    }

    async fn handle_origin(
        &self,
        task: &mut GenerationTaskAggregate,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
        route: &super::GenerationProviderResolvedRoute,
    ) -> Result<bool, GenerationTaskApplicationError> {
        let state = self.origin_reader.read_generation_task_origin_state(task).await;
        match state {
            Ok(GenerationTaskOriginState::WaitingForExternalCompletion) => Ok(false),
            Ok(GenerationTaskOriginState::Cancelled | GenerationTaskOriginState::Terminal) => {
                self.cancel_without_submission(task.clone(), claimed, now).await?;
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
        route: &super::GenerationProviderResolvedRoute,
    ) -> Result<bool, GenerationTaskApplicationError> {
        match self.recover_media(task).await {
            Ok(None | Some(GenerationTaskAssetRecovery::SourceRequired)) => Ok(false),
            Ok(Some(GenerationTaskAssetRecovery::Available(asset))) => {
                self.complete_recovered(task.clone(), asset, claimed, now).await?;
                Ok(true)
            }
            Ok(Some(GenerationTaskAssetRecovery::Pending))
            | Err(GenerationTaskApplicationError::Boundary(
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
        route: &super::GenerationProviderResolvedRoute,
    ) -> Result<(), GenerationTaskApplicationError> {
        let proposed_at = future_time(now, route.policy().poll_interval_milliseconds())?;
        reschedule_before_deadline(&self.repository, task, claimed, proposed_at, now).await
    }

    async fn load_task(
        &self,
        claimed: &GenerationTaskClaimedEffect,
    ) -> Result<GenerationTaskAggregate, GenerationTaskApplicationError> {
        self.repository
            .load_generation_task(claimed.effect().task_id())
            .await?
            .ok_or(GenerationTaskApplicationError::TaskNotFound)
    }

    async fn recover_media(
        &self,
        task: &GenerationTaskAggregate,
    ) -> Result<Option<GenerationTaskAssetRecovery>, GenerationTaskApplicationError> {
        if matches!(task.request(), GenerationTaskRequest::Text(_)) {
            return Ok(None);
        }
        Ok(Some(
            self.asset_sink
                .recover_generation_task_asset(GenerationTaskAssetKey::from_task(task))
                .await?,
        ))
    }

    async fn complete_recovered(
        &self,
        mut task: GenerationTaskAggregate,
        asset: super::GenerationTaskAvailableAsset,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskApplicationError> {
        let expected_revision = task.revision().get();
        if task.state() == &GenerationTaskState::Queued {
            task.begin_submission(now)?;
        }
        task.complete(asset.result().clone(), now)?;
        save_terminal(&self.repository, &task, expected_revision, claimed, now).await
    }

    async fn fail_ambiguous(
        &self,
        mut task: GenerationTaskAggregate,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskApplicationError> {
        let expected_revision = task.revision().get();
        task.fail(ambiguous_submission_failure()?, now)?;
        save_terminal(&self.repository, &task, expected_revision, claimed, now).await
    }

    async fn cancel_without_submission(
        &self,
        mut task: GenerationTaskAggregate,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskApplicationError> {
        let expected_revision = task.revision().get();
        match task.state() {
            GenerationTaskState::Queued => task.request_cancellation(now)?,
            GenerationTaskState::Submitting => {
                task.request_cancellation(now)?;
                task.mark_cancelled(now)?;
            }
            GenerationTaskState::Running { .. } | GenerationTaskState::CancelRequested { .. } => {
                task.mark_cancelled(now)?;
            }
            _ => {}
        }
        save_terminal(&self.repository, &task, expected_revision, claimed, now).await
    }

    async fn complete_result(
        &self,
        task: &mut GenerationTaskAggregate,
        result: GenerationTaskProviderResult,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskApplicationError> {
        let result = match result {
            GenerationTaskProviderResult::Text(result) => {
                GenerationTaskResult::Text { content: result.into_content() }
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
        Ok(())
    }
}

fn require_submit(
    claimed: &GenerationTaskClaimedEffect,
) -> Result<(), GenerationTaskApplicationError> {
    if claimed.effect().kind() != GenerationTaskEffectKind::SubmitTask {
        return Err(GenerationTaskApplicationError::InvalidEffect);
    }
    Ok(())
}
