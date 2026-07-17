use tasks::generation_task::{application::*, interfaces::*};

use super::{effect, setup, task, time};

#[tokio::test]
async fn create_rolls_back_task_when_initial_effect_insert_fails() {
    let (connection, repository) = setup();
    connection
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_initial_outbox BEFORE INSERT ON generation_task_outbox
             BEGIN SELECT RAISE(ABORT, 'reject'); END;",
        )
        .unwrap();
    let task = task(80, 1, 100, "first prompt", "key-1");

    assert_eq!(
        repository
            .create_generation_task(
                &task,
                effect(&task, GenerationTaskEffectKind::SubmitTask, 100),
            )
            .await,
        Err(GenerationTaskRepositoryError::StorageFailure)
    );
    assert!(repository.load_generation_task(task.id()).await.unwrap().is_none());
}

#[tokio::test]
async fn save_rolls_back_task_and_claim_when_enqueue_fails() {
    let (connection, repository) = setup();
    let mut task = task(80, 1, 100, "first prompt", "key-1");
    repository
        .create_generation_task(&task, effect(&task, GenerationTaskEffectKind::SubmitTask, 100))
        .await
        .unwrap();
    let claimed = repository.claim_next_generation_task_effect(time(100)).await.unwrap().unwrap();
    connection
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_task_outbox BEFORE INSERT ON generation_task_outbox
             BEGIN SELECT RAISE(ABORT, 'reject'); END;",
        )
        .unwrap();
    task.begin_submission(time(101)).unwrap();

    assert_eq!(
        repository
            .save_generation_task(
                &task,
                1,
                GenerationTaskOutboxChanges {
                    consume: Some(claimed.claim()),
                    enqueue: vec![effect(&task, GenerationTaskEffectKind::PollTask, 102)],
                },
            )
            .await,
        Err(GenerationTaskRepositoryError::StorageFailure)
    );
    let restored = repository.load_generation_task(task.id()).await.unwrap().unwrap();
    assert_eq!(restored.revision().get(), 1);
    let state: String = connection
        .lock()
        .unwrap()
        .query_row("SELECT state FROM generation_task_outbox WHERE id = 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(state, "Claimed");
}

#[tokio::test]
async fn save_rejects_a_claim_owned_by_another_task_without_mutation() {
    let (connection, repository) = setup();
    let first = task(80, 1, 100, "first", "key-1");
    let second = task(81, 1, 100, "second", "key-2");
    for task in [&first, &second] {
        repository
            .create_generation_task(task, effect(task, GenerationTaskEffectKind::SubmitTask, 100))
            .await
            .unwrap();
    }
    let claimed = repository.claim_next_generation_task_effect(time(100)).await.unwrap().unwrap();
    assert_eq!(claimed.effect().task_id(), first.id());

    assert_eq!(
        repository
            .save_generation_task(
                &second,
                1,
                GenerationTaskOutboxChanges { consume: Some(claimed.claim()), enqueue: Vec::new() },
            )
            .await,
        Err(GenerationTaskRepositoryError::EffectClaimConflict)
    );
    let state: String = connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT state FROM generation_task_outbox WHERE id = ?1",
            [i64::try_from(claimed.claim().effect_id().get()).unwrap()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "Claimed");
    assert_eq!(repository.load_generation_task(second.id()).await.unwrap(), Some(second));
}
