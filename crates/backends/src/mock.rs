//! Fully local deterministic inference backend for tests and early integration.

use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{debug, info};

use crate::error::{BackendError, Result};
use crate::request::{ImageToVideoRequest, TextToImageRequest};
use crate::task::{TaskHandle, TaskProgress, TaskStatus};
use crate::traits::InferenceBackend;

const BACKEND_NAME: &str = "mock";

/// A deterministic local backend with no network or provider credentials.
pub struct MockBackend {
    state: Mutex<MockState>,
    submitted_tasks: AtomicUsize,
    failure_reason: Option<String>,
}

impl MockBackend {
    /// Creates a mock backend whose tasks eventually succeed.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(MockState::default()),
            submitted_tasks: AtomicUsize::new(0),
            failure_reason: None,
        }
    }

    /// Creates a mock backend whose submitted tasks terminally fail.
    #[must_use]
    pub fn always_fails(reason: impl Into<String>) -> Self {
        Self {
            state: Mutex::new(MockState::default()),
            submitted_tasks: AtomicUsize::new(0),
            failure_reason: Some(reason.into()),
        }
    }

    /// Returns how many generation tasks this backend has accepted.
    #[must_use]
    pub fn submitted_task_count(&self) -> usize {
        self.submitted_tasks.load(Ordering::Relaxed)
    }

    fn submit(&self, kind: TaskKind) -> Result<TaskHandle> {
        let mut state = self.lock_state()?;
        state.next_id += 1;
        let task_id = format!("task-{}", state.next_id);
        state.tasks.insert(task_id.clone(), MockTask { kind, polls: 0, cancelled: false });
        self.submitted_tasks.fetch_add(1, Ordering::Relaxed);

        info!(backend = BACKEND_NAME, task_id = %task_id, kind = kind.as_path(), "mock task submitted");
        Ok(TaskHandle { backend: BACKEND_NAME.to_owned(), task_id })
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, MockState>> {
        self.state.lock().map_err(|_| BackendError::InvalidRequest {
            backend: BACKEND_NAME.to_owned(),
            reason: "mock backend state lock was poisoned".to_owned(),
        })
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceBackend for MockBackend {
    fn name(&self) -> &str {
        BACKEND_NAME
    }

    async fn text_to_image(&self, _request: TextToImageRequest) -> Result<TaskHandle> {
        self.submit(TaskKind::TextToImage)
    }

    async fn image_to_video(&self, _request: ImageToVideoRequest) -> Result<TaskHandle> {
        self.submit(TaskKind::ImageToVideo)
    }

    async fn poll(&self, handle: &TaskHandle) -> Result<TaskStatus> {
        let failure_reason = self.failure_reason.clone();
        let mut state = self.lock_state()?;
        let task = lookup_task(&mut state, handle)?;

        if task.cancelled {
            info!(backend = BACKEND_NAME, task_id = %handle.task_id, "mock task is cancelled");
            return Ok(TaskStatus::Cancelled);
        }

        if let Some(reason) = failure_reason {
            info!(backend = BACKEND_NAME, task_id = %handle.task_id, reason = %reason, "mock task failed");
            return Ok(TaskStatus::Failed { reason });
        }

        task.polls = task.polls.saturating_add(1);
        Ok(status_for_poll(handle, task))
    }

    async fn cancel(&self, handle: &TaskHandle) -> Result<()> {
        let mut state = self.lock_state()?;
        let task = lookup_task(&mut state, handle)?;
        task.cancelled = true;
        info!(backend = BACKEND_NAME, task_id = %handle.task_id, "mock task cancelled");
        Ok(())
    }
}

#[derive(Default)]
struct MockState {
    next_id: u64,
    tasks: BTreeMap<String, MockTask>,
}

struct MockTask {
    kind: TaskKind,
    polls: u8,
    cancelled: bool,
}

#[derive(Clone, Copy)]
enum TaskKind {
    TextToImage,
    ImageToVideo,
}

impl TaskKind {
    fn as_path(self) -> &'static str {
        match self {
            Self::TextToImage => "text-to-image",
            Self::ImageToVideo => "image-to-video",
        }
    }
}

fn lookup_task<'a>(state: &'a mut MockState, handle: &TaskHandle) -> Result<&'a mut MockTask> {
    if handle.backend != BACKEND_NAME {
        return Err(unknown_task(&handle.task_id));
    }

    state.tasks.get_mut(&handle.task_id).ok_or_else(|| unknown_task(&handle.task_id))
}

fn unknown_task(task_id: &str) -> BackendError {
    BackendError::UnknownTask { backend: BACKEND_NAME.to_owned(), task_id: task_id.to_owned() }
}

fn status_for_poll(handle: &TaskHandle, task: &MockTask) -> TaskStatus {
    match task.polls {
        1 => {
            debug!(backend = BACKEND_NAME, task_id = %handle.task_id, "mock task queued");
            TaskStatus::Queued
        }
        2 => {
            debug!(backend = BACKEND_NAME, task_id = %handle.task_id, "mock task running");
            TaskStatus::Running { progress: TaskProgress(0.5) }
        }
        _ => {
            let output =
                format!("mock://{}/{}/{}", BACKEND_NAME, task.kind.as_path(), handle.task_id);
            info!(backend = BACKEND_NAME, task_id = %handle.task_id, output = %output, "mock task succeeded");
            TaskStatus::Succeeded { output }
        }
    }
}
