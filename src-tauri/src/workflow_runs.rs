use engine::{
    CancellationSignalInterface, EngineError, Executor, NodeProgressEvent, NodeRegistry,
    ResultCache, RunOutputs, Workflow,
};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use thiserror::Error;
use tracing::error;

const MAX_RUN_ID_BYTES: usize = 64;

/// Validated identity supplied by a scoped workflow-run caller.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RunId(String);

impl RunId {
    /// Parses a bounded run id containing only ASCII letters, digits, `-`, or `_`.
    pub fn parse(value: &str) -> Result<Self, RunIdError> {
        if value.is_empty() {
            return Err(RunIdError::Empty);
        }
        if value.len() > MAX_RUN_ID_BYTES {
            return Err(RunIdError::TooLong { max_bytes: MAX_RUN_ID_BYTES });
        }
        if !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) {
            return Err(RunIdError::InvalidCharacter);
        }
        Ok(Self(value.to_owned()))
    }

    /// Returns the validated wire value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validation failures for client-provided run identities.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RunIdError {
    /// The id had no bytes.
    #[error("run_id must not be empty")]
    Empty,
    /// The id exceeded the bounded wire contract.
    #[error("run_id must be at most {max_bytes} bytes")]
    TooLong { max_bytes: usize },
    /// The id contained a character outside the stable wire alphabet.
    #[error("run_id may contain only ASCII letters, digits, `-`, and `_`")]
    InvalidCharacter,
}

/// Ordered, run-scoped events emitted while a workflow executes.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowRunEvent {
    /// Confirms that the run was atomically registered and can now be cancelled.
    Started { run_id: RunId, project_id: String },
    /// Reports one engine-owned node progress transition.
    Progress { run_id: RunId, node: NodeProgressEvent },
}

/// Error returned by a workflow run event sink.
pub type WorkflowRunEventError = Box<dyn Error + Send + Sync>;

/// Consumer-owned boundary for ordered workflow run events.
pub trait WorkflowRunEventSink: Send {
    /// Sends one event or reports that the observer is no longer available.
    fn send(&mut self, event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError>;
}

/// Authoritative terminal result of one registered workflow run.
#[derive(Debug)]
pub enum WorkflowRunOutcome {
    /// Every node committed successfully.
    Succeeded(RunOutputs),
    /// The engine observed cancellation, or cancellation beat a successful terminal commit.
    /// Non-cancellation execution failures remain [`Self::Failed`] so provider cancel failures
    /// and other actionable errors are never reported as confirmed cancellation.
    Cancelled,
    /// Execution failed for a reason other than cancellation.
    Failed(EngineError),
}

/// Result of an idempotent cancellation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationRequest {
    /// An active run now carries the cancellation request.
    Requested,
    /// No active run currently owns that id.
    NotActive,
}

/// Coordination failures before an authoritative terminal outcome exists.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum WorkflowRunsError {
    /// Another active run already owns the id.
    #[error("workflow run `{run_id}` is already active")]
    DuplicateRunId { run_id: String },
    /// Another active run already owns the project execution slot.
    #[error("project `{project_id}` already has an active workflow run")]
    ProjectBusy { project_id: String },
    /// The active-run registry could not be accessed safely.
    #[error("workflow active-run registry lock was poisoned")]
    ActiveRunsLock,
    /// The project cache registry could not be accessed safely.
    #[error("workflow cache registry lock was poisoned")]
    CacheRegistryLock,
    /// One project's cache could not be accessed safely.
    #[error("workflow cache lock was poisoned for project `{project_id}`")]
    ProjectCacheLock { project_id: String },
    /// A registration disappeared before its owner committed a terminal result.
    #[error("workflow run `{run_id}` lost its active registration")]
    RegistrationLost { run_id: String },
    /// The monotonic registration generation cannot advance further.
    #[error("workflow run generation exhausted")]
    GenerationExhausted,
}

/// App-lifetime coordinator for active runs and project-scoped result caches.
pub struct WorkflowRuns {
    registry: Arc<NodeRegistry>,
    active: Mutex<ActiveRuns>,
    caches: Mutex<HashMap<String, Arc<Mutex<ResultCache>>>>,
}

impl WorkflowRuns {
    /// Creates a coordinator around the application's authoritative node registry.
    #[must_use]
    pub fn new(registry: Arc<NodeRegistry>) -> Self {
        Self {
            registry,
            active: Mutex::new(ActiveRuns::default()),
            caches: Mutex::new(HashMap::new()),
        }
    }

    /// Runs one client-identified workflow under project and cancellation coordination.
    pub fn run(
        self: &Arc<Self>,
        run_id: RunId,
        workflow: Workflow,
        sink: &mut dyn WorkflowRunEventSink,
    ) -> Result<WorkflowRunOutcome, WorkflowRunsError> {
        let project_id = workflow.project_id.clone();
        let registration = self.register(run_id.clone(), &project_id)?;
        if let Err(source) = sink.send(WorkflowRunEvent::Started {
            run_id: run_id.clone(),
            project_id: project_id.clone(),
        }) {
            error!(error = %source, run_id = %run_id.as_str(), "workflow run event sink rejected Started");
            registration.cancellation.request();
            return registration.finish_cancelled();
        }
        if registration.cancellation.is_cancelled() {
            return registration.finish_cancelled();
        }

        let cache = self.cache_for(&project_id)?;
        let mut cache = cache
            .lock()
            .map_err(|_| WorkflowRunsError::ProjectCacheLock { project_id: project_id.clone() })?;
        let cancellation = Arc::clone(&registration.cancellation);
        let mut sink_available = true;
        let result = Executor::new(&self.registry).execute_interruptible(
            &workflow,
            &mut cache,
            cancellation.as_ref(),
            &mut |node| {
                if sink_available {
                    let event = WorkflowRunEvent::Progress {
                        run_id: run_id.clone(),
                        node: node.clone(),
                    };
                    if let Err(source) = sink.send(event) {
                        error!(error = %source, run_id = %run_id.as_str(), "workflow run progress sink failed");
                        sink_available = false;
                        cancellation.request();
                    }
                }
            },
        );
        drop(cache);

        match result {
            Ok(outputs) => registration.finish_success(outputs),
            Err(EngineError::Cancelled) => registration.finish_cancelled(),
            Err(source) => registration.finish_failed(source),
        }
    }

    /// Requests cancellation without claiming that execution has terminated.
    pub fn cancel(&self, run_id: &RunId) -> Result<CancellationRequest, WorkflowRunsError> {
        let active = self.active.lock().map_err(|_| WorkflowRunsError::ActiveRunsLock)?;
        let Some(run) = active.by_run_id.get(run_id) else {
            return Ok(CancellationRequest::NotActive);
        };
        run.cancellation.request();
        Ok(CancellationRequest::Requested)
    }

    /// Returns the active Run for one Project without exposing another Project's registry state.
    pub fn active_run_id(&self, project_id: &str) -> Result<Option<RunId>, WorkflowRunsError> {
        let active = self.active.lock().map_err(|_| WorkflowRunsError::ActiveRunsLock)?;
        Ok(active.by_project_id.get(project_id).map(|key| key.run_id.clone()))
    }

    fn register(
        self: &Arc<Self>,
        run_id: RunId,
        project_id: &str,
    ) -> Result<RunRegistration, WorkflowRunsError> {
        let mut active = self.active.lock().map_err(|_| WorkflowRunsError::ActiveRunsLock)?;
        if active.by_run_id.contains_key(&run_id) {
            return Err(WorkflowRunsError::DuplicateRunId { run_id: run_id.0 });
        }
        if active.by_project_id.contains_key(project_id) {
            return Err(WorkflowRunsError::ProjectBusy { project_id: project_id.to_owned() });
        }
        let generation = active.next_generation;
        active.next_generation =
            generation.checked_add(1).ok_or(WorkflowRunsError::GenerationExhausted)?;
        let cancellation = Arc::new(RunCancellationImpl::default());
        let key =
            ActiveKey { run_id: run_id.clone(), project_id: project_id.to_owned(), generation };
        active.by_run_id.insert(
            run_id,
            ActiveRun {
                project_id: project_id.to_owned(),
                generation,
                cancellation: Arc::clone(&cancellation),
            },
        );
        active.by_project_id.insert(project_id.to_owned(), key.clone());
        drop(active);
        Ok(RunRegistration { owner: Arc::clone(self), key, cancellation, active: true })
    }

    fn cache_for(&self, project_id: &str) -> Result<Arc<Mutex<ResultCache>>, WorkflowRunsError> {
        let mut caches = self.caches.lock().map_err(|_| WorkflowRunsError::CacheRegistryLock)?;
        Ok(Arc::clone(
            caches
                .entry(project_id.to_owned())
                .or_insert_with(|| Arc::new(Mutex::new(ResultCache::new()))),
        ))
    }

    fn finish_success(&self, key: &ActiveKey) -> Result<bool, WorkflowRunsError> {
        let mut active = self.active.lock().map_err(|_| WorkflowRunsError::ActiveRunsLock)?;
        let cancellation_won = active
            .by_run_id
            .get(&key.run_id)
            .filter(|run| run.owns(key))
            .map(|run| run.cancellation.is_cancelled())
            .ok_or_else(|| WorkflowRunsError::RegistrationLost {
                run_id: key.run_id.as_str().to_owned(),
            })?;
        cleanup_active_run(&mut active, key);
        Ok(cancellation_won)
    }

    fn cleanup(&self, key: &ActiveKey) -> Result<(), WorkflowRunsError> {
        let mut active = self.active.lock().map_err(|_| WorkflowRunsError::ActiveRunsLock)?;
        cleanup_active_run(&mut active, key);
        Ok(())
    }
}

#[derive(Default)]
struct ActiveRuns {
    next_generation: u64,
    by_run_id: HashMap<RunId, ActiveRun>,
    by_project_id: HashMap<String, ActiveKey>,
}

struct ActiveRun {
    project_id: String,
    generation: u64,
    cancellation: Arc<RunCancellationImpl>,
}

impl ActiveRun {
    fn owns(&self, key: &ActiveKey) -> bool {
        self.project_id == key.project_id && self.generation == key.generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveKey {
    run_id: RunId,
    project_id: String,
    generation: u64,
}

struct RunRegistration {
    owner: Arc<WorkflowRuns>,
    key: ActiveKey,
    cancellation: Arc<RunCancellationImpl>,
    active: bool,
}

impl RunRegistration {
    fn finish_success(
        mut self,
        outputs: RunOutputs,
    ) -> Result<WorkflowRunOutcome, WorkflowRunsError> {
        let cancellation_won = self.owner.finish_success(&self.key)?;
        self.active = false;
        Ok(if cancellation_won {
            WorkflowRunOutcome::Cancelled
        } else {
            WorkflowRunOutcome::Succeeded(outputs)
        })
    }

    fn finish_cancelled(mut self) -> Result<WorkflowRunOutcome, WorkflowRunsError> {
        self.owner.cleanup(&self.key)?;
        self.active = false;
        Ok(WorkflowRunOutcome::Cancelled)
    }

    fn finish_failed(
        mut self,
        source: EngineError,
    ) -> Result<WorkflowRunOutcome, WorkflowRunsError> {
        self.owner.cleanup(&self.key)?;
        self.active = false;
        Ok(WorkflowRunOutcome::Failed(source))
    }
}

impl fmt::Debug for RunRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("RunRegistration").field("key", &self.key).finish()
    }
}

impl Drop for RunRegistration {
    fn drop(&mut self) {
        if self.active
            && let Err(source) = self.owner.cleanup(&self.key)
        {
            error!(error = %source, run_id = %self.key.run_id.as_str(), "failed to clean workflow run registration");
        }
    }
}

#[derive(Default)]
struct RunCancellationImpl {
    requested: AtomicBool,
}

impl RunCancellationImpl {
    fn request(&self) {
        self.requested.store(true, Ordering::SeqCst);
    }
}

impl CancellationSignalInterface for RunCancellationImpl {
    fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }
}

fn cleanup_active_run(active: &mut ActiveRuns, key: &ActiveKey) {
    let owns_registration = active.by_run_id.get(&key.run_id).is_some_and(|run| run.owns(key));
    if !owns_registration {
        return;
    }
    active.by_run_id.remove(&key.run_id);
    if active.by_project_id.get(&key.project_id) == Some(key) {
        active.by_project_id.remove(&key.project_id);
    }
}

#[cfg(test)]
mod tests;
