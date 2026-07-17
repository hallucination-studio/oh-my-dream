//! Project-scoped Generation Task get and list use cases.

use projects::project::domain::ProjectId;

use super::{
    GenerationTaskApplicationError, GenerationTaskCursorPage, GenerationTaskListQuery,
    GenerationTaskSummaryView,
};
use crate::generation_task::domain::{GenerationTaskAggregate, GenerationTaskId};
use crate::generation_task::interfaces::GenerationTaskRepositoryInterface;

/// Loads one Task only inside an explicit Project scope.
pub struct GenerationTaskGetUseCase<R> {
    repository: R,
}

impl<R> GenerationTaskGetUseCase<R>
where
    R: GenerationTaskRepositoryInterface,
{
    /// Wires the Task repository.
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Returns one Project-local Task or the same not-found result across Projects.
    pub async fn get_generation_task(
        &self,
        project_id: ProjectId,
        task_id: GenerationTaskId,
    ) -> Result<GenerationTaskAggregate, GenerationTaskApplicationError> {
        self.repository
            .load_generation_task_for_project(project_id, task_id)
            .await?
            .ok_or(GenerationTaskApplicationError::TaskNotFound)
    }
}

/// Lists stable bounded Task pages inside one explicit Project scope.
pub struct GenerationTaskListUseCase<R> {
    repository: R,
}

impl<R> GenerationTaskListUseCase<R>
where
    R: GenerationTaskRepositoryInterface,
{
    /// Wires the Task repository.
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Returns one stable Project-scoped Task page.
    pub async fn list_generation_tasks(
        &self,
        query: GenerationTaskListQuery,
    ) -> Result<GenerationTaskCursorPage<GenerationTaskSummaryView>, GenerationTaskApplicationError>
    {
        Ok(self.repository.list_generation_tasks(query).await?)
    }
}
