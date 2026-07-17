use std::sync::{Arc, Mutex};

use assets::asset::domain::AssetMediaKind;
use engine::node_capability::{WorkflowNodeExecutionId, WorkflowRunId};
use engine::workflow_graph::{WorkflowId, WorkflowNodeId};
use nodes::{GenerationProfileId, GenerationProfileRef, GenerationProfileVersion};
use projects::project::domain::ProjectId;
use rusqlite::Connection;
use tasks::generation_task::{application::*, domain::*, interfaces::*};
use uuid::Uuid;

use super::*;

#[path = "tests/codec.rs"]
mod codec;
#[path = "tests/contract.rs"]
mod contract;
#[path = "tests/transactions.rs"]
mod transactions;

fn setup() -> (Arc<Mutex<Connection>>, SqliteGenerationTaskRepositoryAdapterImpl) {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let repository =
        SqliteGenerationTaskRepositoryAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    (connection, repository)
}

#[test]
fn creates_only_the_two_generation_task_tables() {
    let (connection, _) = setup();
    let names = connection
        .lock()
        .unwrap()
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name LIKE 'generation_task%'
             ORDER BY name",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(names, ["generation_task_outbox", "generation_tasks"]);
}

#[tokio::test]
async fn create_is_atomic_idempotent_and_project_scoped() {
    let (_, repository) = setup();
    let task = task(80, 1, 100, "first prompt", "key-1");
    let message = effect(&task, GenerationTaskEffectKind::SubmitTask, 100);

    let created = repository.create_generation_task(&task, message.clone()).await.unwrap();
    let replay = repository.create_generation_task(&task, message).await.unwrap();

    assert!(matches!(created, GenerationTaskCreateResult::Created(_)));
    assert!(matches!(replay, GenerationTaskCreateResult::Existing(_)));
    assert_eq!(repository.load_generation_task(task.id()).await.unwrap(), Some(task.clone()));
    assert_eq!(
        repository
            .load_generation_task_for_project(task.origin().project_id(), task.id())
            .await
            .unwrap(),
        Some(task.clone())
    );
    assert!(
        repository
            .load_generation_task_for_project(project_id(99), task.id())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn create_rejects_idempotency_and_origin_conflicts() {
    let (_, repository) = setup();
    let first = task(80, 1, 100, "first prompt", "key-1");
    repository
        .create_generation_task(&first, effect(&first, GenerationTaskEffectKind::SubmitTask, 100))
        .await
        .unwrap();
    let same_key = task(81, 1, 101, "different prompt", "key-1");
    assert_eq!(
        repository
            .create_generation_task(
                &same_key,
                effect(&same_key, GenerationTaskEffectKind::SubmitTask, 101),
            )
            .await,
        Err(GenerationTaskRepositoryError::IdempotencyConflict)
    );
    let same_origin = task_with_origin(82, first.origin().clone(), 102, "other", "key-2");
    assert_eq!(
        repository
            .create_generation_task(
                &same_origin,
                effect(&same_origin, GenerationTaskEffectKind::SubmitTask, 102),
            )
            .await,
        Err(GenerationTaskRepositoryError::OriginConflict)
    );
}

#[tokio::test]
async fn save_atomically_consumes_enqueues_and_restores_terminal_result() {
    let (connection, repository) = setup();
    let mut task = task(80, 1, 100, "first prompt", "key-1");
    repository
        .create_generation_task(&task, effect(&task, GenerationTaskEffectKind::SubmitTask, 100))
        .await
        .unwrap();
    let claimed = repository.claim_next_generation_task_effect(time(100)).await.unwrap().unwrap();
    task.begin_submission(time(101)).unwrap();
    task.accept_remote_submission(
        GenerationProviderTaskHandle::try_new("remote-1").unwrap(),
        time(102),
    )
    .unwrap();
    task.complete(
        GenerationTaskResult::Asset(GenerationTaskAssetResult::new(
            assets::asset::domain::AssetId::from_uuid(uuid(500)).unwrap(),
            AssetMediaKind::Image,
        )),
        time(103),
    )
    .unwrap();

    repository
        .save_generation_task(
            &task,
            1,
            GenerationTaskOutboxChanges {
                consume: Some(claimed.claim()),
                enqueue: vec![effect(&task, GenerationTaskEffectKind::NotifyWorkflow, 103)],
            },
        )
        .await
        .unwrap();

    assert_eq!(repository.load_generation_task(task.id()).await.unwrap(), Some(task));
    let states: Vec<String> = connection
        .lock()
        .unwrap()
        .prepare("SELECT state FROM generation_task_outbox ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(states, ["Completed", "Ready"]);
}

#[tokio::test]
async fn rejects_optimistic_conflict_and_corrupt_rows() {
    let (connection, repository) = setup();
    let task = task(80, 1, 100, "first prompt", "key-1");
    repository
        .create_generation_task(&task, effect(&task, GenerationTaskEffectKind::SubmitTask, 100))
        .await
        .unwrap();
    assert_eq!(
        repository.save_generation_task(&task, 2, GenerationTaskOutboxChanges::default()).await,
        Err(GenerationTaskRepositoryError::OptimisticConflict)
    );
    connection
        .lock()
        .unwrap()
        .execute("UPDATE generation_tasks SET request_json = request_json || ' '", [])
        .unwrap();
    assert_eq!(
        repository.load_generation_task(task.id()).await,
        Err(GenerationTaskRepositoryError::Corruption)
    );
}

#[tokio::test]
async fn list_is_project_scoped_filtered_and_stably_paginated() {
    let (_, repository) = setup();
    for task in [
        task(80, 1, 100, "one", "key-1"),
        task(81, 1, 200, "two", "key-2"),
        task(82, 1, 300, "three", "key-3"),
        task(83, 99, 400, "other project", "key-4"),
    ] {
        repository
            .create_generation_task(
                &task,
                effect(
                    &task,
                    GenerationTaskEffectKind::SubmitTask,
                    task.created_at().as_utc_milliseconds(),
                ),
            )
            .await
            .unwrap();
    }
    let first = repository
        .list_generation_tasks(
            GenerationTaskListQuery::try_new(project_id(1), None, None, None, 2).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        first.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        [task_id(82), task_id(81)]
    );
    let second = repository
        .list_generation_tasks(
            GenerationTaskListQuery::try_new(project_id(1), None, None, first.next_cursor, 2)
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.items.iter().map(|item| item.id).collect::<Vec<_>>(), [task_id(80)]);
    assert!(second.next_cursor.is_none());
}

#[tokio::test]
async fn claim_is_due_ordered_one_per_task_and_startup_resettable() {
    let (_, repository) = setup();
    let task = task(80, 1, 100, "one", "key-1");
    repository
        .create_generation_task(&task, effect(&task, GenerationTaskEffectKind::SubmitTask, 200))
        .await
        .unwrap();
    repository
        .save_generation_task(
            &task,
            1,
            GenerationTaskOutboxChanges {
                consume: None,
                enqueue: vec![effect(&task, GenerationTaskEffectKind::PollTask, 100)],
            },
        )
        .await
        .unwrap();
    let claimed = repository.claim_next_generation_task_effect(time(150)).await.unwrap().unwrap();
    assert_eq!(claimed.effect().kind(), GenerationTaskEffectKind::PollTask);
    assert!(repository.claim_next_generation_task_effect(time(300)).await.unwrap().is_none());
    assert_eq!(repository.reset_claimed_generation_task_effects().await.unwrap(), 1);
    assert_eq!(
        repository.claim_next_generation_task_effect(time(150)).await.unwrap().unwrap().claim(),
        claimed.claim()
    );
}

fn task(
    task_seed: u128,
    project_seed: u128,
    created_at: i64,
    prompt: &str,
    key: &str,
) -> GenerationTaskAggregate {
    task_with_origin(task_seed, origin(project_seed, task_seed + 1_000), created_at, prompt, key)
}

fn task_with_origin(
    task_seed: u128,
    origin: GenerationTaskOrigin,
    created_at: i64,
    prompt: &str,
    key: &str,
) -> GenerationTaskAggregate {
    GenerationTaskAggregate::create(
        task_id(task_seed),
        origin,
        GenerationTaskIdempotencyKey::try_new(key).unwrap(),
        GenerationTaskTarget::new(
            GenerationProfileRef::new(
                GenerationProfileId::try_new("image.high_quality_general").unwrap(),
                GenerationProfileVersion::try_new(1).unwrap(),
            ),
            GenerationProviderId::try_new("mock").unwrap(),
            GenerationProviderRouteId::try_new("mock.image.high-quality-general.v1").unwrap(),
        ),
        GenerationTaskRequest::Image(ImageGenerationSpec::new(
            GenerationTaskText::try_new(prompt).unwrap(),
            ImageAspectRatio::Square,
        )),
        time(created_at),
        time(created_at + 30_000),
    )
    .unwrap()
}

fn origin(project_seed: u128, workflow_seed: u128) -> GenerationTaskOrigin {
    GenerationTaskOrigin::new(
        project_id(project_seed),
        WorkflowId::from_uuid(uuid(workflow_seed + 1)).unwrap(),
        WorkflowRunId::from_uuid(uuid(workflow_seed + 2)).unwrap(),
        WorkflowNodeId::from_uuid(uuid(workflow_seed + 3)).unwrap(),
        WorkflowNodeExecutionId::from_uuid(uuid(workflow_seed + 4)).unwrap(),
    )
}

fn effect(
    task: &GenerationTaskAggregate,
    kind: GenerationTaskEffectKind,
    available_at: i64,
) -> GenerationTaskEffect {
    GenerationTaskEffect::new(task.id(), kind, time(available_at))
}

fn task_id(seed: u128) -> GenerationTaskId {
    GenerationTaskId::from_uuid(uuid(seed)).unwrap()
}

fn project_id(seed: u128) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}

fn time(value: i64) -> GenerationTaskTimestamp {
    GenerationTaskTimestamp::from_utc_milliseconds(value).unwrap()
}

fn uuid(seed: u128) -> Uuid {
    let mut bytes = seed.to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
