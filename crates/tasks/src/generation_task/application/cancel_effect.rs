//! Closed CancelRemoteTask effect execution.

use std::sync::Arc;

use super::effect_support::{future_time, reschedule_before_deadline, save_terminal};
use super::{
    GenerationProviderResolvedRoute, GenerationTaskApplicationError, GenerationTaskBoundaryError,
    GenerationTaskClaimedEffect, GenerationTaskEffectKind, GenerationTaskOriginState,
};
use crate::generation_task::domain::GenerationTaskAggregate;
use crate::generation_task::interfaces::{
    GenerationCancellerInterface, GenerationProviderCallContext, GenerationProviderCallErrorKind,
    GenerationProviderRegistryInterface, GenerationTaskClockInterface,
    GenerationTaskOriginStateReaderInterface, GenerationTaskRepositoryInterface,
};

/// Executes exactly one claimed remote cancellation effect.
pub struct GenerationTaskCancelRemoteEffectUseCase<R, P, O, C> {
    repository: R,
    provider_registry: P,
    origin_reader: O,
    clock: C,
}

impl<R, P, O, C> GenerationTaskCancelRemoteEffectUseCase<R, P, O, C>
where
    R: GenerationTaskRepositoryInterface,
    P: GenerationProviderRegistryInterface,
    O: GenerationTaskOriginStateReaderInterface,
    C: GenerationTaskClockInterface,
{
    /// Wires remote cancellation dependencies.
    #[must_use]
    pub const fn new(repository: R, provider_registry: P, origin_reader: O, clock: C) -> Self {
        Self { repository, provider_registry, origin_reader, clock }
    }

    /// Requests safe remote cancellation and converges locally.
    pub async fn execute_generation_task_cancel_remote_effect(
        &self,
        claimed: GenerationTaskClaimedEffect,
    ) -> Result<(), GenerationTaskApplicationError> {
        if claimed.effect().kind() != GenerationTaskEffectKind::CancelRemoteTask {
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
        let origin_state = self.origin_reader.read_generation_task_origin_state(&task).await;
        match origin_state {
            Ok(GenerationTaskOriginState::Running)
            | Err(GenerationTaskBoundaryError::Transient) => {
                let proposed_at = future_time(now, route.policy().poll_interval_milliseconds())?;
                return reschedule_before_deadline(
                    &self.repository,
                    &mut task,
                    claimed,
                    proposed_at,
                    now,
                )
                .await;
            }
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        let handle = task
            .state()
            .remote_handle()
            .ok_or(GenerationTaskApplicationError::InvalidEffect)?
            .clone();
        let canceller =
            route_canceller(route).ok_or(GenerationTaskApplicationError::InvalidEffect)?;
        let context = GenerationProviderCallContext::try_new(
            task.id(),
            task.target().clone(),
            task.created_at(),
            task.provider_deadline_at(),
        )
        .map_err(|_| GenerationTaskApplicationError::InvalidEffect)?;
        match canceller.cancel_generation(&context, &handle).await {
            Ok(_) => self.finish(&mut task, &claimed, now).await,
            Err(error) if error.kind() == GenerationProviderCallErrorKind::Transient => {
                let proposed_at = error
                    .retry_at()
                    .unwrap_or(future_time(now, route.policy().poll_interval_milliseconds())?);
                reschedule_before_deadline(&self.repository, &mut task, claimed, proposed_at, now)
                    .await
            }
            Err(_) => self.finish(&mut task, &claimed, now).await,
        }
    }

    async fn finish(
        &self,
        task: &mut GenerationTaskAggregate,
        claimed: &GenerationTaskClaimedEffect,
        now: crate::generation_task::domain::GenerationTaskTimestamp,
    ) -> Result<(), GenerationTaskApplicationError> {
        let expected = task.revision().get();
        task.mark_cancelled(now)?;
        save_terminal(&self.repository, task, expected, claimed, now).await
    }
}

fn route_canceller(
    route: &GenerationProviderResolvedRoute,
) -> Option<&Arc<dyn GenerationCancellerInterface>> {
    match route {
        GenerationProviderResolvedRoute::Text { execution: crate::generation_task::interfaces::TextGenerationProviderExecution::CancellableRemote { canceller, .. }, .. }
        | GenerationProviderResolvedRoute::Image { execution: crate::generation_task::interfaces::ImageGenerationProviderExecution::CancellableRemote { canceller, .. }, .. }
        | GenerationProviderResolvedRoute::Video { execution: crate::generation_task::interfaces::VideoGenerationProviderExecution::CancellableRemote { canceller, .. }, .. }
        | GenerationProviderResolvedRoute::Voice { execution: crate::generation_task::interfaces::VoiceGenerationProviderExecution::CancellableRemote { canceller, .. }, .. } => Some(canceller),
        _ => None,
    }
}
