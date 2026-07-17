use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tasks::generation_task::*;

use crate::application_effect_fakes::*;
use crate::support::{handle, new_task, time};

#[tokio::test]
async fn uncertain_submit_fails_ambiguous_without_repeating() {
    let repository = seeded_repository().await;
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::SubmitTask).unwrap().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let executor = Arc::new(AmbiguousImageExecutorFakeImpl { calls: calls.clone() });
    let registry =
        GenerationProviderRegistryFakeImpl::new(GenerationProviderResolvedRoute::Image {
            execution: ImageGenerationProviderExecution::Immediate(executor),
            policy: policy(),
        });
    let asset_sink = source_required_asset_sink();
    let use_case = GenerationTaskSubmitEffectUseCase::new(
        repository.clone(),
        registry,
        GenerationTaskOriginStateReaderFakeImpl::new(
            GenerationTaskOriginState::WaitingForExternalCompletion,
        ),
        asset_sink,
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_submit_effect(claimed).await.unwrap();

    let task = repository.generation_task(new_task().id()).unwrap().unwrap();
    assert!(matches!(
        task.state(),
        GenerationTaskState::Failed {
            failure,
            ..
        } if failure.kind() == GenerationTaskFailureKind::AmbiguousSubmission
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::NotifyWorkflow]);
}

#[tokio::test]
async fn running_origin_reschedules_submit_without_provider_call() {
    let repository = seeded_repository().await;
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::SubmitTask).unwrap().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let executor = Arc::new(AmbiguousImageExecutorFakeImpl { calls: calls.clone() });
    let registry =
        GenerationProviderRegistryFakeImpl::new(GenerationProviderResolvedRoute::Image {
            execution: ImageGenerationProviderExecution::Immediate(executor),
            policy: policy(),
        });
    let use_case = GenerationTaskSubmitEffectUseCase::new(
        repository.clone(),
        registry,
        GenerationTaskOriginStateReaderFakeImpl::new(GenerationTaskOriginState::Running),
        source_required_asset_sink(),
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_submit_effect(claimed).await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::SubmitTask]);
    let task = repository.generation_task(new_task().id()).unwrap().unwrap();
    assert_eq!(task.state(), &GenerationTaskState::Queued);
}

#[tokio::test]
async fn cancelled_origin_converges_non_cancellable_running_task_locally() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let mut task = new_task();
    task.begin_submission(time(101)).unwrap();
    task.accept_remote_submission(handle(), time(102)).unwrap();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(task.id(), GenerationTaskEffectKind::PollTask, time(102)),
        )
        .await
        .unwrap();
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::PollTask).unwrap().unwrap();
    let route = GenerationProviderResolvedRoute::Image {
        execution: ImageGenerationProviderExecution::Remote {
            submitter: Arc::new(UnusedImageRemoteFakeImpl),
            poller: Arc::new(UnusedImageRemoteFakeImpl),
        },
        policy: policy(),
    };
    let use_case = GenerationTaskPollEffectUseCase::new(
        repository.clone(),
        GenerationProviderRegistryFakeImpl::new(route),
        GenerationTaskOriginStateReaderFakeImpl::new(GenerationTaskOriginState::Cancelled),
        source_required_asset_sink(),
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_poll_effect(claimed).await.unwrap();

    let saved = repository.generation_task(task.id()).unwrap().unwrap();
    assert!(matches!(saved.state(), GenerationTaskState::Cancelled { .. }));
    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::NotifyWorkflow]);
}

#[tokio::test]
async fn applied_workflow_notification_consumes_terminal_effect() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let mut task = new_task();
    task.request_cancellation(time(101)).unwrap();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(
                task.id(),
                GenerationTaskEffectKind::NotifyWorkflow,
                time(101),
            ),
        )
        .await
        .unwrap();
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::NotifyWorkflow).unwrap().unwrap();
    let use_case = GenerationTaskNotifyWorkflowEffectUseCase::new(
        repository.clone(),
        GenerationTaskWorkflowCompletionFakeImpl::new(
            GenerationTaskWorkflowCompletionOutcome::Applied,
        ),
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_notify_workflow_effect(claimed).await.unwrap();

    assert!(repository.ready_effect_kinds().is_empty());
}

#[tokio::test]
async fn transient_poll_error_reschedules_the_same_safe_observation() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let mut task = new_task();
    task.begin_submission(time(101)).unwrap();
    task.accept_remote_submission(handle(), time(102)).unwrap();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(task.id(), GenerationTaskEffectKind::PollTask, time(102)),
        )
        .await
        .unwrap();
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::PollTask).unwrap().unwrap();
    let route = GenerationProviderResolvedRoute::Image {
        execution: ImageGenerationProviderExecution::Remote {
            submitter: Arc::new(UnusedImageRemoteFakeImpl),
            poller: Arc::new(TransientImagePollerFakeImpl),
        },
        policy: policy(),
    };
    let use_case = GenerationTaskPollEffectUseCase::new(
        repository.clone(),
        GenerationProviderRegistryFakeImpl::new(route),
        GenerationTaskOriginStateReaderFakeImpl::new(
            GenerationTaskOriginState::WaitingForExternalCompletion,
        ),
        source_required_asset_sink(),
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_poll_effect(claimed).await.unwrap();

    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::PollTask]);
    assert!(matches!(
        repository.generation_task(task.id()).unwrap().unwrap().state(),
        GenerationTaskState::Running { .. }
    ));
}

#[tokio::test]
async fn crash_recovery_lookup_precedes_remote_handle_polling() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let mut task = new_task();
    task.begin_submission(time(101)).unwrap();
    task.accept_remote_submission(handle(), time(102)).unwrap();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(task.id(), GenerationTaskEffectKind::PollTask, time(102)),
        )
        .await
        .unwrap();
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::PollTask).unwrap().unwrap();
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let route = GenerationProviderResolvedRoute::Image {
        execution: ImageGenerationProviderExecution::Remote {
            submitter: Arc::new(UnusedImageRemoteFakeImpl),
            poller: Arc::new(RecordingImagePollerFakeImpl { events: events.clone() }),
        },
        policy: policy(),
    };
    let use_case = GenerationTaskPollEffectUseCase::new(
        repository,
        GenerationProviderRegistryFakeImpl::new(route),
        GenerationTaskOriginStateReaderFakeImpl::new(
            GenerationTaskOriginState::WaitingForExternalCompletion,
        ),
        RecordingAssetSinkFakeImpl {
            events: events.clone(),
            recovery: GenerationTaskAssetRecovery::SourceRequired,
        },
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_poll_effect(claimed).await.unwrap();

    assert_eq!(*events.lock().unwrap(), vec!["recover", "poll"]);
}

#[tokio::test]
async fn transient_origin_read_reschedules_submit_without_provider_call() {
    let repository = seeded_repository().await;
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::SubmitTask).unwrap().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry =
        GenerationProviderRegistryFakeImpl::new(GenerationProviderResolvedRoute::Image {
            execution: ImageGenerationProviderExecution::Immediate(Arc::new(
                AmbiguousImageExecutorFakeImpl { calls: calls.clone() },
            )),
            policy: policy(),
        });
    let use_case = GenerationTaskSubmitEffectUseCase::new(
        repository.clone(),
        registry,
        TransientOriginReaderFakeImpl,
        source_required_asset_sink(),
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_submit_effect(claimed).await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::SubmitTask]);
}

#[tokio::test]
async fn transient_poll_past_deadline_fails_timeout() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let mut task = new_task();
    task.begin_submission(time(101)).unwrap();
    task.accept_remote_submission(handle(), time(102)).unwrap();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(task.id(), GenerationTaskEffectKind::PollTask, time(102)),
        )
        .await
        .unwrap();
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::PollTask).unwrap().unwrap();
    let route = GenerationProviderResolvedRoute::Image {
        execution: ImageGenerationProviderExecution::Remote {
            submitter: Arc::new(UnusedImageRemoteFakeImpl),
            poller: Arc::new(TransientImagePollerAtFakeImpl { observed_at: time(30_000) }),
        },
        policy: policy(),
    };
    let use_case = GenerationTaskPollEffectUseCase::new(
        repository.clone(),
        GenerationProviderRegistryFakeImpl::new(route),
        GenerationTaskOriginStateReaderFakeImpl::new(
            GenerationTaskOriginState::WaitingForExternalCompletion,
        ),
        source_required_asset_sink(),
        GenerationTaskClockFakeImpl::new(time(30_000)),
    );

    use_case.execute_generation_task_poll_effect(claimed).await.unwrap();

    let saved = repository.generation_task(task.id()).unwrap().unwrap();
    assert!(
        matches!(saved.state(), GenerationTaskState::Failed { failure, .. } if failure.kind() == GenerationTaskFailureKind::Timeout)
    );
    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::NotifyWorkflow]);
}

#[tokio::test]
async fn cancellable_remote_task_converges_after_provider_accepts_cancel() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let mut task = new_task();
    task.begin_submission(time(101)).unwrap();
    task.accept_remote_submission(handle(), time(102)).unwrap();
    task.request_cancellation(time(103)).unwrap();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(
                task.id(),
                GenerationTaskEffectKind::CancelRemoteTask,
                time(103),
            ),
        )
        .await
        .unwrap();
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::CancelRemoteTask).unwrap().unwrap();
    let cancel_calls = Arc::new(AtomicUsize::new(0));
    let route = GenerationProviderResolvedRoute::Image {
        execution: ImageGenerationProviderExecution::CancellableRemote {
            submitter: Arc::new(UnusedImageRemoteFakeImpl),
            poller: Arc::new(UnusedImageRemoteFakeImpl),
            canceller: Arc::new(AcceptingCancellerFakeImpl { calls: cancel_calls.clone() }),
        },
        policy: policy(),
    };
    let use_case = GenerationTaskCancelRemoteEffectUseCase::new(
        repository.clone(),
        GenerationProviderRegistryFakeImpl::new(route),
        GenerationTaskOriginStateReaderFakeImpl::new(GenerationTaskOriginState::Cancelled),
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_cancel_remote_effect(claimed).await.unwrap();

    assert_eq!(cancel_calls.load(Ordering::SeqCst), 1);
    let saved = repository.generation_task(task.id()).unwrap().unwrap();
    assert!(matches!(saved.state(), GenerationTaskState::Cancelled { .. }));
    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::NotifyWorkflow]);
}

#[tokio::test]
async fn transient_workflow_notification_remains_durable() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let mut task = new_task();
    task.request_cancellation(time(101)).unwrap();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(
                task.id(),
                GenerationTaskEffectKind::NotifyWorkflow,
                time(101),
            ),
        )
        .await
        .unwrap();
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::NotifyWorkflow).unwrap().unwrap();
    let use_case = GenerationTaskNotifyWorkflowEffectUseCase::new(
        repository.clone(),
        TransientWorkflowCompletionFakeImpl,
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_notify_workflow_effect(claimed).await.unwrap();

    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::NotifyWorkflow]);
}

#[tokio::test]
async fn cancellation_committed_during_submit_rejects_late_result_attachment() {
    let repository = seeded_repository().await;
    let task_id = new_task().id();
    let claimed =
        repository.claim_ready_effect(GenerationTaskEffectKind::SubmitTask).unwrap().unwrap();
    let executor = CancellingImageExecutorFakeImpl { repository: repository.clone(), task_id };
    let route = GenerationProviderResolvedRoute::Image {
        execution: ImageGenerationProviderExecution::Immediate(Arc::new(executor)),
        policy: policy(),
    };
    let use_case = GenerationTaskSubmitEffectUseCase::new(
        repository.clone(),
        GenerationProviderRegistryFakeImpl::new(route),
        GenerationTaskOriginStateReaderFakeImpl::new(
            GenerationTaskOriginState::WaitingForExternalCompletion,
        ),
        source_required_asset_sink(),
        GenerationTaskClockFakeImpl::new(time(110)),
    );

    use_case.execute_generation_task_submit_effect(claimed).await.unwrap();

    let saved = repository.generation_task(task_id).unwrap().unwrap();
    assert!(matches!(saved.state(), GenerationTaskState::Cancelled { .. }));
    assert!(saved.result().is_none());
    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::NotifyWorkflow]);
}
