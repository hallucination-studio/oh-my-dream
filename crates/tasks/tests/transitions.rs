use tasks::generation_task::domain::*;

use crate::support::{failure, handle, new_task, restore_state, result_for, time};

#[test]
fn exhausts_every_legal_transition() {
    let mut submitted = new_task();
    submitted.begin_submission(time(101)).unwrap();
    assert_eq!(submitted.state(), &GenerationTaskState::Submitting);

    let mut immediate = new_task();
    immediate.begin_submission(time(101)).unwrap();
    immediate.complete(result_for(immediate.request()), time(102)).unwrap();
    assert!(matches!(immediate.state(), GenerationTaskState::Succeeded { .. }));

    let mut remote = new_task();
    remote.begin_submission(time(101)).unwrap();
    remote.accept_remote_submission(handle(), time(102)).unwrap();
    remote.record_progress(Some(25), time(103)).unwrap();
    remote.complete(result_for(remote.request()), time(104)).unwrap();
    assert!(matches!(remote.state(), GenerationTaskState::Succeeded { .. }));

    let mut queued_cancel = new_task();
    queued_cancel.request_cancellation(time(101)).unwrap();
    assert!(matches!(queued_cancel.state(), GenerationTaskState::Cancelled { .. }));

    let mut submission_cancel = new_task();
    submission_cancel.begin_submission(time(101)).unwrap();
    submission_cancel.request_cancellation(time(102)).unwrap();
    let cancellation_revision = submission_cancel.revision();
    submission_cancel.request_cancellation(time(102)).unwrap();
    assert_eq!(submission_cancel.revision(), cancellation_revision);
    submission_cancel.accept_remote_submission(handle(), time(103)).unwrap();
    assert_eq!(submission_cancel.state().remote_handle(), Some(&handle()));
    let handle_revision = submission_cancel.revision();
    submission_cancel.accept_remote_submission(handle(), time(103)).unwrap();
    assert_eq!(submission_cancel.revision(), handle_revision);
    submission_cancel.mark_cancelled(time(104)).unwrap();

    let mut running_cancel = new_task();
    running_cancel.begin_submission(time(101)).unwrap();
    running_cancel.accept_remote_submission(handle(), time(102)).unwrap();
    running_cancel.request_cancellation(time(103)).unwrap();
    running_cancel.mark_cancelled(time(104)).unwrap();

    let mut provider_cancel = new_task();
    provider_cancel.begin_submission(time(101)).unwrap();
    provider_cancel.accept_remote_submission(handle(), time(102)).unwrap();
    provider_cancel.mark_cancelled(time(103)).unwrap();

    for mut task in [queued_task(), submitted_task(), running_task()] {
        task.fail(failure(), time(130)).unwrap();
        assert!(matches!(task.state(), GenerationTaskState::Failed { .. }));
    }
}

#[test]
fn rejects_every_unspecified_from_state_for_submission() {
    for mut task in [submitted_task(), running_task(), cancel_requested_task()] {
        assert_eq!(
            task.begin_submission(time(130)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
    }
    for mut task in [queued_task(), running_task()] {
        assert_eq!(
            task.accept_remote_submission(handle(), time(130)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
    }
}

#[test]
fn rejects_every_unspecified_from_state_for_terminal_outcomes() {
    let result = result_for(new_task().request());
    for mut task in [queued_task(), cancel_requested_task()] {
        assert_eq!(
            task.complete(result.clone(), time(130)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
    }
    for mut task in [queued_task(), submitted_task()] {
        assert_eq!(
            task.mark_cancelled(time(130)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
    }
    let mut cancelling = cancel_requested_task();
    assert_eq!(
        cancelling.fail(failure(), time(130)),
        Err(GenerationTaskDomainError::IllegalTransition)
    );
}

#[test]
fn terminal_states_are_immutable() {
    for mut task in [succeeded_task(), failed_task(), cancelled_task()] {
        assert_eq!(
            task.begin_submission(time(140)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
        assert_eq!(
            task.accept_remote_submission(handle(), time(140)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
        assert_eq!(
            task.record_progress(Some(50), time(140)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
        assert_eq!(
            task.request_cancellation(time(140)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
        assert_eq!(
            task.mark_cancelled(time(140)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
        assert_eq!(
            task.complete(result_for(task.request()), time(140)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
        assert_eq!(
            task.fail(failure(), time(140)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
    }
}

#[test]
fn progress_is_bounded_monotonic_and_running_only() {
    let mut running = running_task();
    running.record_progress(Some(25), time(130)).unwrap();
    running.record_progress(Some(25), time(131)).unwrap();
    running.record_progress(Some(100), time(132)).unwrap();
    assert_eq!(running.progress_percent(), Some(100));
    assert_eq!(
        running.record_progress(Some(99), time(133)),
        Err(GenerationTaskDomainError::ProgressRegressed)
    );
    assert_eq!(
        running.record_progress(None, time(133)),
        Err(GenerationTaskDomainError::ProgressRegressed)
    );
    assert_eq!(
        running.record_progress(Some(101), time(133)),
        Err(GenerationTaskDomainError::ProgressOutOfRange)
    );
    let mut queued = queued_task();
    assert_eq!(
        queued.record_progress(Some(1), time(130)),
        Err(GenerationTaskDomainError::IllegalTransition)
    );
    for mut task in [submitted_task(), cancel_requested_task()] {
        assert_eq!(
            task.record_progress(Some(1), time(130)),
            Err(GenerationTaskDomainError::IllegalTransition)
        );
    }
}

#[test]
fn cancellation_wins_against_late_submit_and_complete() {
    let mut task = submitted_task();
    task.request_cancellation(time(121)).unwrap();
    task.accept_remote_submission(handle(), time(122)).unwrap();
    assert!(matches!(task.state(), GenerationTaskState::CancelRequested { handle: Some(_) }));
    assert_eq!(
        task.complete(result_for(task.request()), time(123)),
        Err(GenerationTaskDomainError::IllegalTransition)
    );
}

fn queued_task() -> GenerationTaskAggregate {
    restore_state(GenerationTaskState::Queued, None).unwrap()
}

fn submitted_task() -> GenerationTaskAggregate {
    restore_state(GenerationTaskState::Submitting, None).unwrap()
}

fn running_task() -> GenerationTaskAggregate {
    restore_state(GenerationTaskState::Running { handle: handle(), progress_percent: None }, None)
        .unwrap()
}

fn cancel_requested_task() -> GenerationTaskAggregate {
    restore_state(GenerationTaskState::CancelRequested { handle: None }, None).unwrap()
}

fn succeeded_task() -> GenerationTaskAggregate {
    let request = new_task().request().clone();
    restore_state(
        GenerationTaskState::Succeeded { completed_at: time(120) },
        Some(result_for(&request)),
    )
    .unwrap()
}

fn failed_task() -> GenerationTaskAggregate {
    restore_state(GenerationTaskState::Failed { completed_at: time(120), failure: failure() }, None)
        .unwrap()
}

fn cancelled_task() -> GenerationTaskAggregate {
    restore_state(GenerationTaskState::Cancelled { completed_at: time(120) }, None).unwrap()
}
