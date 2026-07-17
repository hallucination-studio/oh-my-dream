use projects::project::domain::ProjectId;
use tasks::generation_task::*;

use crate::support::{new_task, time, uuid};

#[tokio::test]
async fn get_and_list_never_cross_project_scope() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let task = new_task();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(task.id(), GenerationTaskEffectKind::SubmitTask, time(100)),
        )
        .await
        .unwrap();
    let project_id = task.origin().project_id();
    let other_project_id = ProjectId::from_uuid(uuid(500)).unwrap();
    let get = GenerationTaskGetUseCase::new(repository.clone());
    let list = GenerationTaskListUseCase::new(repository);

    assert_eq!(get.get_generation_task(project_id, task.id()).await.unwrap(), task);
    assert_eq!(
        get.get_generation_task(other_project_id, task.id()).await,
        Err(GenerationTaskApplicationError::TaskNotFound)
    );
    let own_page = list
        .list_generation_tasks(
            GenerationTaskListQuery::try_new(project_id, None, None, None, 10).unwrap(),
        )
        .await
        .unwrap();
    let other_page = list
        .list_generation_tasks(
            GenerationTaskListQuery::try_new(other_project_id, None, None, None, 10).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(own_page.items.len(), 1);
    assert!(other_page.items.is_empty());
}
