use tasks::generation_task::domain::*;

use crate::support::{failure, handle, new_task, restore_state, result_for, time};

#[test]
fn restores_every_valid_state_shape() {
    let request = new_task().request().clone();
    for (state, result) in [
        (GenerationTaskState::Queued, None),
        (GenerationTaskState::Submitting, None),
        (GenerationTaskState::Running { handle: handle(), progress_percent: Some(100) }, None),
        (GenerationTaskState::CancelRequested { handle: None }, None),
        (GenerationTaskState::CancelRequested { handle: Some(handle()) }, None),
        (GenerationTaskState::Succeeded { completed_at: time(120) }, Some(result_for(&request))),
        (GenerationTaskState::Failed { completed_at: time(120), failure: failure() }, None),
        (GenerationTaskState::Cancelled { completed_at: time(120) }, None),
    ] {
        assert!(restore_state(state, result).is_ok());
    }
}

#[test]
fn rejects_corrupt_result_and_progress_shapes() {
    let result = result_for(new_task().request());
    assert_eq!(
        restore_state(GenerationTaskState::Succeeded { completed_at: time(120) }, None),
        Err(GenerationTaskDomainError::InvalidRestoredState)
    );
    assert_eq!(
        restore_state(GenerationTaskState::Queued, Some(result.clone())),
        Err(GenerationTaskDomainError::InvalidRestoredState)
    );
    assert_eq!(
        restore_state(
            GenerationTaskState::Succeeded { completed_at: time(120) },
            Some(GenerationTaskResult::Text {
                content: GenerationTaskText::try_new("wrong kind").unwrap(),
            }),
        ),
        Err(GenerationTaskDomainError::InvalidRestoredState)
    );
    assert_eq!(
        restore_state(
            GenerationTaskState::Running { handle: handle(), progress_percent: Some(101) },
            None,
        ),
        Err(GenerationTaskDomainError::InvalidRestoredState)
    );
}

#[test]
fn rejects_corrupt_hash_and_time_ordering() {
    let task = new_task();
    assert_eq!(
        GenerationTaskAggregate::restore(
            task.id(),
            task.origin().clone(),
            task.idempotency_key().clone(),
            GenerationTaskRequestHash::from_bytes([0; 32]),
            task.target().clone(),
            task.request().clone(),
            task.provider_deadline_at(),
            GenerationTaskState::Queued,
            None,
            task.created_at(),
            task.updated_at(),
            task.revision(),
        ),
        Err(GenerationTaskDomainError::InvalidRequestHash)
    );
    assert_eq!(
        restore_with_times(&task, time(121), time(120), task.provider_deadline_at()),
        Err(GenerationTaskDomainError::InvalidRestoredState)
    );
    assert_eq!(
        restore_with_times(&task, time(100), time(120), time(100)),
        Err(GenerationTaskDomainError::InvalidRestoredState)
    );
    assert_eq!(
        restore_terminal_after_update(&task),
        Err(GenerationTaskDomainError::InvalidRestoredState)
    );
}

#[test]
fn transition_time_and_revision_are_monotonic() {
    let mut task = new_task();
    task.begin_submission(time(110)).unwrap();
    assert_eq!(task.revision().get(), 2);
    assert_eq!(task.updated_at(), time(110));
    assert_eq!(
        task.accept_remote_submission(handle(), time(109)),
        Err(GenerationTaskDomainError::InvalidTimestamp)
    );
    assert_eq!(task.state(), &GenerationTaskState::Submitting);
    assert_eq!(task.revision().get(), 2);
}

fn restore_with_times(
    task: &GenerationTaskAggregate,
    created_at: GenerationTaskTimestamp,
    updated_at: GenerationTaskTimestamp,
    deadline: GenerationTaskTimestamp,
) -> Result<GenerationTaskAggregate, GenerationTaskDomainError> {
    GenerationTaskAggregate::restore(
        task.id(),
        task.origin().clone(),
        task.idempotency_key().clone(),
        task.request_hash(),
        task.target().clone(),
        task.request().clone(),
        deadline,
        GenerationTaskState::Queued,
        None,
        created_at,
        updated_at,
        task.revision(),
    )
}

fn restore_terminal_after_update(
    task: &GenerationTaskAggregate,
) -> Result<GenerationTaskAggregate, GenerationTaskDomainError> {
    GenerationTaskAggregate::restore(
        task.id(),
        task.origin().clone(),
        task.idempotency_key().clone(),
        task.request_hash(),
        task.target().clone(),
        task.request().clone(),
        task.provider_deadline_at(),
        GenerationTaskState::Cancelled { completed_at: time(121) },
        None,
        time(100),
        time(120),
        task.revision(),
    )
}
