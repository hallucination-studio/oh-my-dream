//! Shared pure decisions and atomic persistence helpers for closed Task effects.

use super::{
    GenerationTaskApplicationError, GenerationTaskClaimedEffect, GenerationTaskEffect,
    GenerationTaskEffectKind, GenerationTaskOutboxChanges,
};
use crate::generation_task::domain::{
    GenerationTaskAggregate, GenerationTaskFailure, GenerationTaskFailureKind,
    GenerationTaskTimestamp,
};
use crate::generation_task::interfaces::{
    GenerationProviderCallError, GenerationProviderFailure, GenerationProviderFailureKind,
    GenerationTaskRepositoryInterface,
};

pub(super) fn future_time(
    now: GenerationTaskTimestamp,
    milliseconds: u64,
) -> Result<GenerationTaskTimestamp, GenerationTaskApplicationError> {
    let delta = i64::try_from(milliseconds)
        .map_err(|_| crate::generation_task::domain::GenerationTaskDomainError::InvalidTimestamp)?;
    let value = now
        .as_utc_milliseconds()
        .checked_add(delta)
        .ok_or(crate::generation_task::domain::GenerationTaskDomainError::InvalidTimestamp)?;
    Ok(GenerationTaskTimestamp::from_utc_milliseconds(value)?)
}

pub(super) async fn reschedule_effect<R: GenerationTaskRepositoryInterface>(
    repository: &R,
    task: &GenerationTaskAggregate,
    claimed: GenerationTaskClaimedEffect,
    available_at: GenerationTaskTimestamp,
) -> Result<(), GenerationTaskApplicationError> {
    let expected_revision = task.revision().get();
    let next = claimed.effect().clone().rescheduled(available_at);
    repository
        .save_generation_task(
            task,
            expected_revision,
            GenerationTaskOutboxChanges { consume: Some(claimed.claim()), enqueue: vec![next] },
        )
        .await?;
    Ok(())
}

pub(super) async fn reschedule_before_deadline<R: GenerationTaskRepositoryInterface>(
    repository: &R,
    task: &mut GenerationTaskAggregate,
    claimed: GenerationTaskClaimedEffect,
    proposed_at: GenerationTaskTimestamp,
    now: GenerationTaskTimestamp,
) -> Result<(), GenerationTaskApplicationError> {
    if let Some(available_at) = bounded_retry_time(task, proposed_at) {
        return reschedule_effect(repository, task, claimed, available_at).await;
    }
    let expected_revision = task.revision().get();
    if matches!(
        task.state(),
        crate::generation_task::domain::GenerationTaskState::CancelRequested { .. }
    ) {
        task.mark_cancelled(now)?;
    } else {
        task.fail(timeout_failure()?, now)?;
    }
    save_terminal(repository, task, expected_revision, &claimed, now).await
}

pub(super) async fn save_terminal<R: GenerationTaskRepositoryInterface>(
    repository: &R,
    task: &GenerationTaskAggregate,
    expected_revision: u64,
    claimed: &GenerationTaskClaimedEffect,
    now: GenerationTaskTimestamp,
) -> Result<(), GenerationTaskApplicationError> {
    repository
        .save_generation_task(
            task,
            expected_revision,
            GenerationTaskOutboxChanges {
                consume: Some(claimed.claim()),
                enqueue: vec![GenerationTaskEffect::new(
                    task.id(),
                    GenerationTaskEffectKind::NotifyWorkflow,
                    now,
                )],
            },
        )
        .await?;
    Ok(())
}

pub(super) fn provider_failure(
    failure: GenerationProviderFailure,
) -> Result<GenerationTaskFailure, GenerationTaskApplicationError> {
    let kind = match failure.kind() {
        GenerationProviderFailureKind::InvalidSemanticRequest => {
            GenerationTaskFailureKind::InvalidRequest
        }
        GenerationProviderFailureKind::AuthenticationFailed => {
            GenerationTaskFailureKind::Authentication
        }
        GenerationProviderFailureKind::PermissionDenied => {
            GenerationTaskFailureKind::PermissionDenied
        }
        GenerationProviderFailureKind::ContentPolicyRejected => {
            GenerationTaskFailureKind::ContentPolicy
        }
        GenerationProviderFailureKind::RateLimited => GenerationTaskFailureKind::RateLimited,
        GenerationProviderFailureKind::ProviderUnavailable => {
            GenerationTaskFailureKind::ProviderUnavailable
        }
        GenerationProviderFailureKind::DeadlineExceeded => GenerationTaskFailureKind::Timeout,
        GenerationProviderFailureKind::ProviderRejected => {
            GenerationTaskFailureKind::ProviderRejected
        }
        GenerationProviderFailureKind::InvalidResponse
        | GenerationProviderFailureKind::DownloadRejected => {
            GenerationTaskFailureKind::InvalidProviderResponse
        }
        GenerationProviderFailureKind::AmbiguousSubmission => {
            GenerationTaskFailureKind::AmbiguousSubmission
        }
    };
    Ok(GenerationTaskFailure::try_new(kind, failure.code(), failure.message())?)
}

pub(super) fn ambiguous_submission_failure()
-> Result<GenerationTaskFailure, GenerationTaskApplicationError> {
    Ok(GenerationTaskFailure::try_new(
        GenerationTaskFailureKind::AmbiguousSubmission,
        "AMBIGUOUS_SUBMISSION",
        "Provider submission outcome is uncertain.",
    )?)
}

pub(super) fn timeout_failure() -> Result<GenerationTaskFailure, GenerationTaskApplicationError> {
    Ok(GenerationTaskFailure::try_new(
        GenerationTaskFailureKind::Timeout,
        "PROVIDER_DEADLINE_EXCEEDED",
        "Generation Provider deadline was exceeded.",
    )?)
}

pub(super) fn provider_call_failure(
    error: &GenerationProviderCallError,
) -> Result<GenerationTaskFailure, GenerationTaskApplicationError> {
    Ok(GenerationTaskFailure::try_new(
        GenerationTaskFailureKind::ProviderUnavailable,
        error.code(),
        error.message(),
    )?)
}

pub(super) fn bounded_retry_time(
    task: &GenerationTaskAggregate,
    proposed: GenerationTaskTimestamp,
) -> Option<GenerationTaskTimestamp> {
    (proposed < task.provider_deadline_at()).then_some(proposed)
}

pub(super) const fn route_is_cancellable(route: &super::GenerationProviderResolvedRoute) -> bool {
    matches!(
        route,
        super::GenerationProviderResolvedRoute::Text {
            execution: crate::generation_task::interfaces::TextGenerationProviderExecution::CancellableRemote { .. }, ..
        } | super::GenerationProviderResolvedRoute::Image {
            execution: crate::generation_task::interfaces::ImageGenerationProviderExecution::CancellableRemote { .. }, ..
        } | super::GenerationProviderResolvedRoute::Video {
            execution: crate::generation_task::interfaces::VideoGenerationProviderExecution::CancellableRemote { .. }, ..
        } | super::GenerationProviderResolvedRoute::Voice {
            execution: crate::generation_task::interfaces::VoiceGenerationProviderExecution::CancellableRemote { .. }, ..
        }
    )
}
