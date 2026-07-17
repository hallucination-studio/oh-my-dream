use tasks::generation_task::*;

use crate::support::{new_task, time};

#[tokio::test]
async fn claims_one_due_effect_per_task_and_resets_prior_process_claims() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let task = new_task();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(task.id(), GenerationTaskEffectKind::SubmitTask, time(200)),
        )
        .await
        .unwrap();
    repository
        .save_generation_task(
            &task,
            task.revision().get(),
            GenerationTaskOutboxChanges {
                consume: None,
                enqueue: vec![GenerationTaskEffect::new(
                    task.id(),
                    GenerationTaskEffectKind::PollTask,
                    time(100),
                )],
            },
        )
        .await
        .unwrap();

    let claimed = repository.claim_next_generation_task_effect(time(150)).await.unwrap().unwrap();
    assert_eq!(claimed.effect().kind(), GenerationTaskEffectKind::PollTask);
    assert!(repository.claim_next_generation_task_effect(time(300)).await.unwrap().is_none());

    assert_eq!(repository.reset_claimed_generation_task_effects().await.unwrap(), 1);
    let reclaimed = repository.claim_next_generation_task_effect(time(150)).await.unwrap().unwrap();
    assert_eq!(reclaimed.claim(), claimed.claim());
}
