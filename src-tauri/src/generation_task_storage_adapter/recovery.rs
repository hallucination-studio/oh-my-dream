use async_trait::async_trait;
use engine::workflow::{
    WorkflowApplicationError, WorkflowGenerationTaskOrigin,
    WorkflowGenerationTaskRecoveryObservation, WorkflowGenerationTaskRecoveryReaderInterface,
};
use rusqlite::params;
use tasks::generation_task::{GenerationTaskEffectKind, GenerationTaskState};

use super::{
    outbox::encode_effect_kind,
    repository::{SqliteGenerationTaskRepositoryAdapterImpl, storage},
    task_sql,
};

#[async_trait]
impl WorkflowGenerationTaskRecoveryReaderInterface for SqliteGenerationTaskRepositoryAdapterImpl {
    async fn read_workflow_generation_task_recovery(
        &self,
        origin: &WorkflowGenerationTaskOrigin,
    ) -> Result<WorkflowGenerationTaskRecoveryObservation, WorkflowApplicationError> {
        self.with_connection(|connection| {
            let Some(task) = task_sql::load_by_origin(
                connection,
                origin.project_id,
                origin.node_execution_id.as_uuid().as_bytes(),
            )?
            else {
                return Ok(WorkflowGenerationTaskRecoveryObservation::Absent);
            };
            if task.origin().workflow_id() != origin.workflow_id
                || task.origin().workflow_revision() != origin.workflow_revision
                || task.origin().workflow_run_id() != origin.workflow_run_id
                || task.origin().workflow_node_id() != origin.workflow_node_id
                || task.origin().capability_contract_ref() != &origin.capability_contract_ref
            {
                return Ok(WorkflowGenerationTaskRecoveryObservation::Corrupt);
            }
            let mut statement = connection
                .prepare(
                    "SELECT kind, state FROM generation_task_outbox
                     WHERE task_id = ?1 AND (state != 'Completed' OR kind = 'NotifyWorkflow')
                     ORDER BY id",
                )
                .map_err(storage)?;
            let rows = statement
                .query_map(params![task.id().as_uuid().as_bytes()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage)?;
            Ok(classify(&task, &rows))
        })
        .map_err(|_| WorkflowApplicationError::WorkflowGenerationTaskRecoveryReadFailure)
    }
}

fn classify(
    task: &tasks::generation_task::GenerationTaskAggregate,
    effects: &[(String, String)],
) -> WorkflowGenerationTaskRecoveryObservation {
    if task.state().is_terminal() {
        return classify_terminal(effects);
    }
    let active = effects.iter().filter(|(_, state)| state != "Completed").collect::<Vec<_>>();
    if active.len() != 1 || active[0].1 != "Ready" {
        return WorkflowGenerationTaskRecoveryObservation::Corrupt;
    }
    let kind = active[0].0.as_str();
    match task.state() {
        GenerationTaskState::Queued
            if kind == encode_effect_kind(GenerationTaskEffectKind::SubmitTask) =>
        {
            WorkflowGenerationTaskRecoveryObservation::QueuedPreHandoff
        }
        GenerationTaskState::Submitting
            if kind == encode_effect_kind(GenerationTaskEffectKind::SubmitTask) =>
        {
            WorkflowGenerationTaskRecoveryObservation::Active
        }
        GenerationTaskState::Running { .. }
            if kind == encode_effect_kind(GenerationTaskEffectKind::PollTask) =>
        {
            WorkflowGenerationTaskRecoveryObservation::Active
        }
        GenerationTaskState::CancelRequested { handle }
            if (handle.is_none()
                && kind == encode_effect_kind(GenerationTaskEffectKind::SubmitTask))
                || (handle.is_some()
                    && kind == encode_effect_kind(GenerationTaskEffectKind::CancelRemoteTask)) =>
        {
            WorkflowGenerationTaskRecoveryObservation::Active
        }
        _ => WorkflowGenerationTaskRecoveryObservation::Corrupt,
    }
}

fn classify_terminal(effects: &[(String, String)]) -> WorkflowGenerationTaskRecoveryObservation {
    let active = effects.iter().filter(|(_, state)| state != "Completed").collect::<Vec<_>>();
    if active.len() == 1
        && active[0].0 == encode_effect_kind(GenerationTaskEffectKind::NotifyWorkflow)
        && active[0].1 == "Ready"
    {
        return WorkflowGenerationTaskRecoveryObservation::TerminalNotificationPending;
    }
    if !active.is_empty() {
        return WorkflowGenerationTaskRecoveryObservation::Corrupt;
    }
    if effects.iter().any(|(kind, state)| {
        kind == encode_effect_kind(GenerationTaskEffectKind::NotifyWorkflow) && state == "Completed"
    }) {
        WorkflowGenerationTaskRecoveryObservation::NotificationCompleted
    } else {
        WorkflowGenerationTaskRecoveryObservation::Corrupt
    }
}
