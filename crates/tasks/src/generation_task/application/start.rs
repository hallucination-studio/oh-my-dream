//! Idempotent internal Generation Task admission.

use crate::generation_task::application::{
    GenerationTaskApplicationError, GenerationTaskEffect, GenerationTaskEffectKind,
    GenerationTaskStartCommand, GenerationTaskStartResult,
};
use crate::generation_task::domain::{GenerationTaskAggregate, GenerationTaskTimestamp};
use crate::generation_task::interfaces::{
    GenerationProviderRegistryInterface, GenerationTaskClockInterface,
    GenerationTaskRepositoryInterface,
};

/// Creates one durable Task and initial Submit effect for a Workflow node execution.
pub struct GenerationTaskStartUseCase<R, P, C> {
    repository: R,
    provider_registry: P,
    clock: C,
}

impl<R, P, C> GenerationTaskStartUseCase<R, P, C>
where
    R: GenerationTaskRepositoryInterface,
    P: GenerationProviderRegistryInterface,
    C: GenerationTaskClockInterface,
{
    /// Wires exact Task admission dependencies.
    #[must_use]
    pub const fn new(repository: R, provider_registry: P, clock: C) -> Self {
        Self { repository, provider_registry, clock }
    }

    /// Creates or idempotently replays one exact Task admission.
    pub async fn start_generation_task(
        &self,
        command: GenerationTaskStartCommand,
    ) -> Result<GenerationTaskStartResult, GenerationTaskApplicationError> {
        let route = self
            .provider_registry
            .resolve_generation_provider_route(command.target(), command.request().kind())?;
        let created_at = self.clock.observe_generation_task_time()?;
        let provider_deadline_at = deadline(created_at, route.policy())?;
        let task = GenerationTaskAggregate::create(
            command.task_id(),
            command.origin().clone(),
            command.idempotency_key().clone(),
            command.target().clone(),
            command.request().clone(),
            created_at,
            provider_deadline_at,
        )?;
        let result = self
            .repository
            .create_generation_task(
                &task,
                GenerationTaskEffect::new(
                    task.id(),
                    GenerationTaskEffectKind::SubmitTask,
                    created_at,
                ),
            )
            .await?;
        Ok(GenerationTaskStartResult::new(result.task().id()))
    }
}

fn deadline(
    created_at: GenerationTaskTimestamp,
    policy: super::GenerationProviderRoutePolicy,
) -> Result<GenerationTaskTimestamp, GenerationTaskApplicationError> {
    let milliseconds = i64::try_from(policy.task_deadline_milliseconds())
        .map_err(|_| crate::generation_task::domain::GenerationTaskDomainError::InvalidTimestamp)?;
    let value = created_at
        .as_utc_milliseconds()
        .checked_add(milliseconds)
        .ok_or(crate::generation_task::domain::GenerationTaskDomainError::InvalidTimestamp)?;
    Ok(GenerationTaskTimestamp::from_utc_milliseconds(value)?)
}
