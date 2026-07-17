use assets::asset::domain::{AssetContentDigest, AssetId, AssetMediaKind};
use tasks::generation_task::{application::*, domain::*, interfaces::*};

use super::{effect, project_id, setup, task, time, uuid};

#[tokio::test]
async fn round_trips_all_four_request_variants() {
    let (_, repository) = setup();
    let requests = [
        GenerationTaskRequest::Text(TextGenerationSpec::new(text("write text"))),
        GenerationTaskRequest::Image(ImageGenerationSpec::new(
            text("draw image"),
            ImageAspectRatio::Landscape16To9,
        )),
        GenerationTaskRequest::Voice(VoiceGenerationSpec::new(text("speak text"))),
        GenerationTaskRequest::Video(
            VideoGenerationSpec::try_new(
                AssetSnapshotRef::new(
                    AssetId::from_uuid(uuid(900)).unwrap(),
                    AssetMediaKind::Image,
                    AssetContentDigest::from_bytes([9; 32]),
                ),
                VideoDurationSeconds::Ten,
                Some(text("animate image")),
            )
            .unwrap(),
        ),
    ];
    for (index, request) in requests.into_iter().enumerate() {
        let task = task_for_request(200 + index as u128, request);
        repository
            .create_generation_task(&task, effect(&task, GenerationTaskEffectKind::SubmitTask, 100))
            .await
            .unwrap();
        assert_eq!(repository.load_generation_task(task.id()).await.unwrap(), Some(task));
    }
}

#[tokio::test]
async fn round_trips_every_lifecycle_state_and_result_shape() {
    let (_, repository) = setup();
    for (index, (state, result)) in state_cases().into_iter().enumerate() {
        let task = task_for_state(index, state, result);
        repository
            .create_generation_task(&task, effect(&task, GenerationTaskEffectKind::SubmitTask, 100))
            .await
            .unwrap();
        assert_eq!(repository.load_generation_task(task.id()).await.unwrap(), Some(task));
    }
}

fn state_cases() -> [(GenerationTaskState, Option<GenerationTaskResult>); 7] {
    [
        (GenerationTaskState::Queued, None),
        (GenerationTaskState::Submitting, None),
        (
            GenerationTaskState::Running {
                handle: GenerationProviderTaskHandle::try_new("remote-running").unwrap(),
                progress_percent: Some(42),
            },
            None,
        ),
        (GenerationTaskState::CancelRequested { handle: None }, None),
        (
            GenerationTaskState::Succeeded { completed_at: time(110) },
            Some(GenerationTaskResult::Asset(GenerationTaskAssetResult::new(
                AssetId::from_uuid(uuid(901)).unwrap(),
                AssetMediaKind::Image,
            ))),
        ),
        (
            GenerationTaskState::Failed {
                completed_at: time(110),
                failure: GenerationTaskFailure::try_new(
                    GenerationTaskFailureKind::ProviderRejected,
                    "PROVIDER_REJECTED",
                    "Provider rejected generation.",
                )
                .unwrap(),
            },
            None,
        ),
        (GenerationTaskState::Cancelled { completed_at: time(110) }, None),
    ]
}

fn task_for_state(
    index: usize,
    state: GenerationTaskState,
    result: Option<GenerationTaskResult>,
) -> GenerationTaskAggregate {
    let seeded = task(310 + index as u128, 1, 100, "state", &format!("state-{index}"));
    GenerationTaskAggregate::restore(
        seeded.id(),
        seeded.origin().clone(),
        seeded.idempotency_key().clone(),
        seeded.request_hash(),
        seeded.target().clone(),
        seeded.request().clone(),
        seeded.provider_deadline_at(),
        state,
        result,
        seeded.created_at(),
        if index == 0 { seeded.created_at() } else { time(110) },
        GenerationTaskRevision::try_new(if index == 0 { 1 } else { 2 }).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn filters_by_normalized_status_and_request_kind() {
    let (_, repository) = setup();
    let mut running = task(400, 1, 100, "image", "image-running");
    running.begin_submission(time(101)).unwrap();
    running
        .accept_remote_submission(
            GenerationProviderTaskHandle::try_new("remote-filter").unwrap(),
            time(102),
        )
        .unwrap();
    let text_task =
        task_for_request(401, GenerationTaskRequest::Text(TextGenerationSpec::new(text("text"))));
    for task in [&running, &text_task] {
        repository
            .create_generation_task(task, effect(task, GenerationTaskEffectKind::SubmitTask, 100))
            .await
            .unwrap();
    }
    let running_page = repository
        .list_generation_tasks(
            GenerationTaskListQuery::try_new(
                project_id(1),
                Some(GenerationTaskStatus::Running),
                None,
                None,
                10,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(running_page.items.iter().map(|item| item.id).collect::<Vec<_>>(), [running.id()]);
    let text_page = repository
        .list_generation_tasks(
            GenerationTaskListQuery::try_new(
                project_id(1),
                None,
                Some(GenerationTaskRequestKind::Text),
                None,
                10,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(text_page.items.iter().map(|item| item.id).collect::<Vec<_>>(), [text_task.id()]);
}

#[tokio::test]
async fn claims_globally_oldest_due_effect_and_restores_attempt_count() {
    let (_, repository) = setup();
    let later = task(500, 1, 100, "later", "later");
    let earlier = task(501, 1, 100, "earlier", "earlier");
    repository
        .create_generation_task(
            &later,
            GenerationTaskEffect::restore(
                later.id(),
                GenerationTaskEffectKind::PollTask,
                time(200),
                1,
            ),
        )
        .await
        .unwrap();
    repository
        .create_generation_task(
            &earlier,
            GenerationTaskEffect::restore(
                earlier.id(),
                GenerationTaskEffectKind::PollTask,
                time(100),
                3,
            ),
        )
        .await
        .unwrap();

    let claimed = repository.claim_next_generation_task_effect(time(300)).await.unwrap().unwrap();

    assert_eq!(claimed.effect().task_id(), earlier.id());
    assert_eq!(claimed.effect().delivery_attempts(), 3);
}

fn task_for_request(seed: u128, request: GenerationTaskRequest) -> GenerationTaskAggregate {
    let template = task(seed, 1, 100, "template", &format!("request-{seed}"));
    GenerationTaskAggregate::create(
        template.id(),
        template.origin().clone(),
        template.idempotency_key().clone(),
        template.target().clone(),
        request,
        template.created_at(),
        template.provider_deadline_at(),
    )
    .unwrap()
}

fn text(value: &str) -> GenerationTaskText {
    GenerationTaskText::try_new(value).unwrap()
}
