use crate::error::NodesError;
use backends::{InferenceBackend, TaskHandle, TaskStatus};
use engine::NodeRunContext;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

const MAX_POLLS: usize = 60;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(crate) struct GeneratedOutput {
    pub(crate) reference: String,
    pub(crate) cost: Option<i64>,
}

pub(crate) fn wait_for_success(
    backend: &Arc<dyn InferenceBackend>,
    handle: &TaskHandle,
    context: &mut NodeRunContext<'_>,
) -> Result<GeneratedOutput, NodesError> {
    for poll_index in 0..MAX_POLLS {
        let status = pollster::block_on(backend.poll(handle))
            .map_err(|source| NodesError::Backend { operation: "poll backend task", source })?;

        match status {
            TaskStatus::Queued => log_pending(handle, poll_index, "queued"),
            TaskStatus::Running { progress } => {
                context.progress(progress.0);
                log_pending(handle, poll_index, "running");
            }
            TaskStatus::Succeeded { output, cost } => {
                info!(
                    backend = %handle.backend,
                    task_id = %handle.task_id,
                    output = %output,
                    "backend task succeeded"
                );
                return Ok(GeneratedOutput { reference: output, cost });
            }
            TaskStatus::Failed { reason } => {
                warn!(
                    backend = %handle.backend,
                    task_id = %handle.task_id,
                    reason = %reason,
                    "backend task failed"
                );
                return Err(NodesError::TaskFailed {
                    backend: handle.backend.clone(),
                    task_id: handle.task_id.clone(),
                    reason,
                });
            }
            TaskStatus::Cancelled => {
                warn!(
                    backend = %handle.backend,
                    task_id = %handle.task_id,
                    "backend task was cancelled"
                );
                return Err(NodesError::TaskCancelled {
                    backend: handle.backend.clone(),
                    task_id: handle.task_id.clone(),
                });
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    Err(NodesError::PollLimit {
        backend: handle.backend.clone(),
        task_id: handle.task_id.clone(),
        max_polls: MAX_POLLS,
    })
}

fn log_pending(handle: &TaskHandle, poll_index: usize, state: &str) {
    debug!(
        backend = %handle.backend,
        task_id = %handle.task_id,
        poll_index,
        state,
        "backend task still pending"
    );
}
