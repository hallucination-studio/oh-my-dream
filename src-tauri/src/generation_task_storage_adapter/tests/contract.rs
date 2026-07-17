use tasks::generation_task::{application::*, interfaces::*};

use super::{effect, project_id, setup, task, time};

#[tokio::test]
async fn fake_satisfies_generation_task_repository_contract() {
    run_contract(GenerationTaskRepositoryFakeImpl::default()).await;
}

#[tokio::test]
async fn sqlite_satisfies_generation_task_repository_contract() {
    let (_, repository) = setup();
    run_contract(repository).await;
}

async fn run_contract<R>(repository: R)
where
    R: GenerationTaskRepositoryInterface + GenerationTaskOutboxReaderInterface,
{
    let existing = task(700, 1, 100, "contract", "contract-key");
    let initial = effect(&existing, GenerationTaskEffectKind::SubmitTask, 100);

    assert!(matches!(
        repository.create_generation_task(&existing, initial.clone()).await.unwrap(),
        GenerationTaskCreateResult::Created(_)
    ));
    assert!(matches!(
        repository.create_generation_task(&existing, initial).await.unwrap(),
        GenerationTaskCreateResult::Existing(_)
    ));
    assert_eq!(
        repository
            .load_generation_task_for_project(existing.origin().project_id(), existing.id())
            .await
            .unwrap(),
        Some(existing.clone())
    );
    assert!(
        repository
            .load_generation_task_for_project(project_id(2), existing.id())
            .await
            .unwrap()
            .is_none()
    );
    let altered = task(700, 1, 100, "contract", "altered-key");
    assert_eq!(
        repository.save_generation_task(&altered, 1, GenerationTaskOutboxChanges::default()).await,
        Err(GenerationTaskRepositoryError::Corruption)
    );
    let claimed = repository.claim_next_generation_task_effect(time(100)).await.unwrap().unwrap();
    assert_eq!(claimed.effect().task_id(), existing.id());
    assert_eq!(repository.reset_claimed_generation_task_effects().await.unwrap(), 1);
    assert_eq!(
        repository.claim_next_generation_task_effect(time(100)).await.unwrap().unwrap().claim(),
        claimed.claim()
    );

    let other = task(701, 1, 101, "other", "other-key");
    assert_eq!(
        repository
            .create_generation_task(
                &other,
                effect(&existing, GenerationTaskEffectKind::SubmitTask, 101),
            )
            .await,
        Err(GenerationTaskRepositoryError::Corruption)
    );
}
