use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::node_capability::{
    NodeCapabilityExecutionCancellation, NodeCapabilityExecutionError,
    NodeCapabilityExecutionFailure, NodeCapabilityExecutionRequest, NodeCapabilityExecutionStage,
    NodeCapabilityExecutionTarget, NodeCapabilityReadinessDeadline, NodeCapabilityReadinessRequest,
    NodeCapabilityReadinessTarget, WorkflowNodeCapabilityRegistry, WorkflowNodeExecutionContext,
    WorkflowNodeExecutionId, WorkflowNodeExecutionOrigin, WorkflowNodeInputSet,
    WorkflowNodeInputValue, WorkflowRunId, WorkflowRuntimeInputItem,
};
use crate::workflow_graph::WorkflowNodeId;

use super::{
    WorkflowApplicationError, WorkflowClockInterface, WorkflowNodeExecutionFailure,
    WorkflowNodeExecutionState, WorkflowRunAggregate, WorkflowRunEventPublisherInterface,
    WorkflowRunLoadKey, WorkflowRunRepositoryInterface, WorkflowRunState,
};

mod input;

use input::{
    build_execution_request, failed_ancestors, readiness_execution_error, ready_node_execution_ids,
};

/// Process-scoped active execution cancellation signals shared by execute and cancel use cases.
#[derive(Default)]
pub struct WorkflowExecutionCancellationRegistry {
    state: Mutex<WorkflowExecutionCancellationState>,
}

#[derive(Default)]
struct WorkflowExecutionCancellationState {
    signals: BTreeMap<
        WorkflowRunId,
        BTreeMap<WorkflowNodeExecutionId, NodeCapabilityExecutionCancellation>,
    >,
    cancelled_runs: BTreeSet<WorkflowRunId>,
}

impl WorkflowExecutionCancellationRegistry {
    fn register(
        &self,
        run_id: WorkflowRunId,
        execution_id: WorkflowNodeExecutionId,
        signal: NodeCapabilityExecutionCancellation,
    ) -> Result<(), WorkflowApplicationError> {
        let mut state = self.state.lock().map_err(|_| persistence())?;
        if state.cancelled_runs.contains(&run_id) {
            signal.cancel();
        }
        state.signals.entry(run_id).or_default().insert(execution_id, signal);
        Ok(())
    }

    fn finish(
        &self,
        run_id: WorkflowRunId,
        execution_id: WorkflowNodeExecutionId,
    ) -> Result<(), WorkflowApplicationError> {
        let mut state = self.state.lock().map_err(|_| persistence())?;
        if let Some(run_signals) = state.signals.get_mut(&run_id) {
            run_signals.remove(&execution_id);
            if run_signals.is_empty() {
                state.signals.remove(&run_id);
            }
        }
        Ok(())
    }

    fn cancel_run(&self, run_id: WorkflowRunId) -> Result<(), WorkflowApplicationError> {
        let mut state = self.state.lock().map_err(|_| persistence())?;
        state.cancelled_runs.insert(run_id);
        if let Some(run_signals) = state.signals.get(&run_id) {
            for signal in run_signals.values() {
                signal.cancel();
            }
        }
        Ok(())
    }

    fn clear_run(&self, run_id: WorkflowRunId) -> Result<(), WorkflowApplicationError> {
        let mut state = self.state.lock().map_err(|_| persistence())?;
        state.signals.remove(&run_id);
        state.cancelled_runs.remove(&run_id);
        Ok(())
    }
}

/// Coordinates ready nodes from one frozen plan with bounded parallel capability calls.
pub struct WorkflowExecuteRunUseCase<R, C, P> {
    repository: Arc<R>,
    clock: Arc<C>,
    publisher: Arc<P>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    cancellations: Arc<WorkflowExecutionCancellationRegistry>,
    maximum_concurrency: usize,
}

impl<R, C, P> WorkflowExecuteRunUseCase<R, C, P>
where
    R: WorkflowRunRepositoryInterface + 'static,
    C: WorkflowClockInterface,
    P: WorkflowRunEventPublisherInterface,
{
    /// Wires coordination boundaries and validates a non-zero concurrency bound.
    pub fn try_new(
        repository: Arc<R>,
        clock: Arc<C>,
        publisher: Arc<P>,
        capabilities: Arc<WorkflowNodeCapabilityRegistry>,
        cancellations: Arc<WorkflowExecutionCancellationRegistry>,
        maximum_concurrency: usize,
    ) -> Result<Self, WorkflowApplicationError> {
        if maximum_concurrency == 0 || maximum_concurrency > 64 {
            return Err(WorkflowApplicationError::WorkflowCapabilityExecutionFailure);
        }
        Ok(Self { repository, clock, publisher, capabilities, cancellations, maximum_concurrency })
    }

    /// Executes ready batches until the Run is terminal, with no retry, fallback, or substitution.
    pub async fn execute_workflow_run(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        let mut run = self.load_run(run_id).await?;
        if run.state() == WorkflowRunState::Queued {
            let previous = run.events().len();
            run.start(self.clock.current_workflow_time()?)?;
            run = self.commit_and_publish(run, previous).await?;
        }
        loop {
            if is_terminal(run.state()) {
                self.cancellations.clear_run(run_id)?;
                return Ok(run);
            }
            run = self.block_failed_descendants(run).await?;
            let ready = ready_node_execution_ids(&run);
            if ready.is_empty() {
                let previous = run.events().len();
                run.finish(self.clock.current_workflow_time()?)?;
                return self.commit_and_publish(run, previous).await;
            }
            let batch = ready.into_iter().take(self.maximum_concurrency).collect::<Vec<_>>();
            let previous = run.events().len();
            for execution_id in &batch {
                run.start_node(*execution_id, self.clock.current_workflow_time()?)?;
            }
            self.commit_and_publish(run, previous).await?;
            self.execute_batch(run_id, batch).await?;
            run = self.load_run(run_id).await?;
        }
    }

    /// Marks one non-terminal Run interrupted during startup recovery.
    pub async fn interrupt_workflow_run_after_restart(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        let mut run = self.load_run(run_id).await?;
        let previous = run.events().len();
        run.interrupt_by_restart(self.clock.current_workflow_time()?)?;
        self.commit_and_publish(run, previous).await
    }

    async fn block_failed_descendants(
        &self,
        mut run: WorkflowRunAggregate,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        let mut changed = false;
        let previous = run.events().len();
        for execution in run.node_executions().to_vec() {
            if execution.state() != WorkflowNodeExecutionState::Pending {
                continue;
            }
            let failed = failed_ancestors(&run, execution.node_id());
            if !failed.is_empty() {
                run.block_node(
                    execution.execution_id(),
                    failed,
                    self.clock.current_workflow_time()?,
                )?;
                changed = true;
            }
        }
        if changed { self.commit_and_publish(run, previous).await } else { Ok(run) }
    }

    async fn execute_batch(
        &self,
        run_id: WorkflowRunId,
        execution_ids: Vec<WorkflowNodeExecutionId>,
    ) -> Result<(), WorkflowApplicationError> {
        let run = self.load_run(run_id).await?;
        let mut tasks = tokio::task::JoinSet::new();
        for execution_id in execution_ids {
            let signal = NodeCapabilityExecutionCancellation::active();
            self.cancellations.register(run_id, execution_id, signal.clone())?;
            let (capability, request) =
                build_execution_request(&run, execution_id, signal, &self.capabilities)?;
            tasks.spawn(async move {
                let readiness = capability
                    .check_node_external_readiness(NodeCapabilityReadinessRequest {
                        project_id: request.context.project_id,
                        normalized_parameters: request.normalized_parameters.clone(),
                        deadline: NodeCapabilityReadinessDeadline::at(
                            Instant::now() + Duration::from_secs(5),
                        ),
                    })
                    .await;
                if let Some(issue) = readiness.into_iter().next() {
                    let error = readiness_execution_error(
                        request.origin.capability_contract_ref().clone(),
                        request.context.node_execution_id,
                        issue,
                    );
                    (execution_id, Err(error))
                } else {
                    (execution_id, capability.execute_node_capability(request).await)
                }
            });
        }
        while let Some(joined) = tasks.join_next().await {
            let (execution_id, result) =
                joined.map_err(|_| WorkflowApplicationError::WorkflowCapabilityExecutionFailure)?;
            self.cancellations.finish(run_id, execution_id)?;
            self.commit_execution_result(run_id, execution_id, result).await?;
        }
        Ok(())
    }

    async fn commit_execution_result(
        &self,
        run_id: WorkflowRunId,
        execution_id: WorkflowNodeExecutionId,
        result: Result<crate::node_capability::WorkflowNodeOutputSet, NodeCapabilityExecutionError>,
    ) -> Result<(), WorkflowApplicationError> {
        let mut run = self.load_run(run_id).await?;
        if run.state() == WorkflowRunState::Cancelled {
            return Ok(());
        }
        let previous = run.events().len();
        match result {
            Ok(outputs) => {
                run.succeed_node(execution_id, outputs, self.clock.current_workflow_time()?)
            }
            Err(capability_error) => run.fail_node(
                execution_id,
                WorkflowNodeExecutionFailure { capability_error },
                self.clock.current_workflow_time()?,
            ),
        }?;
        self.commit_and_publish(run, previous).await.map(|_| ())
    }

    async fn load_run(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        self.repository
            .load_workflow_run(WorkflowRunLoadKey::Run(run_id))
            .await?
            .ok_or(WorkflowApplicationError::WorkflowRunNotFound)
    }

    async fn commit_and_publish(
        &self,
        run: WorkflowRunAggregate,
        previous_event_count: usize,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        let committed =
            self.repository.commit_workflow_run_transition(run, previous_event_count).await?;
        for event in &committed.events()[previous_event_count..] {
            self.publisher.publish_committed_workflow_run_event(event.clone()).await?;
        }
        Ok(committed)
    }
}

/// Durably cancels a Run before signalling active provider calls.
pub struct WorkflowCancelRunUseCase<R, C, P> {
    repository: Arc<R>,
    clock: Arc<C>,
    publisher: Arc<P>,
    cancellations: Arc<WorkflowExecutionCancellationRegistry>,
}

impl<R, C, P> WorkflowCancelRunUseCase<R, C, P>
where
    R: WorkflowRunRepositoryInterface,
    C: WorkflowClockInterface,
    P: WorkflowRunEventPublisherInterface,
{
    /// Wires persistence, time, delivery, and process cancellation state.
    #[must_use]
    pub fn new(
        repository: Arc<R>,
        clock: Arc<C>,
        publisher: Arc<P>,
        cancellations: Arc<WorkflowExecutionCancellationRegistry>,
    ) -> Self {
        Self { repository, clock, publisher, cancellations }
    }

    /// Commits cancellation and events before signalling active executions.
    pub async fn cancel_workflow_run(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        self.cancel_workflow_run_internal(WorkflowRunLoadKey::Run(run_id), run_id).await
    }

    /// Cancels a Run only through its trusted owning Project.
    pub async fn cancel_project_workflow_run(
        &self,
        project_id: projects::project::domain::ProjectId,
        run_id: WorkflowRunId,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        self.cancel_workflow_run_internal(
            WorkflowRunLoadKey::ProjectScoped { project_id, workflow_run_id: run_id },
            run_id,
        )
        .await
    }

    async fn cancel_workflow_run_internal(
        &self,
        key: WorkflowRunLoadKey,
        run_id: WorkflowRunId,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        let mut run = self
            .repository
            .load_workflow_run(key)
            .await?
            .ok_or(WorkflowApplicationError::WorkflowRunNotFound)?;
        if run.state() == WorkflowRunState::Cancelled {
            self.cancellations.cancel_run(run_id)?;
            return Ok(run);
        }
        let previous = run.events().len();
        run.cancel(self.clock.current_workflow_time()?)?;
        let committed = self.repository.commit_workflow_run_transition(run, previous).await?;
        for event in &committed.events()[previous..] {
            self.publisher.publish_committed_workflow_run_event(event.clone()).await?;
        }
        self.cancellations.cancel_run(run_id)?;
        Ok(committed)
    }
}

fn is_terminal(state: WorkflowRunState) -> bool {
    matches!(
        state,
        WorkflowRunState::Succeeded | WorkflowRunState::Failed | WorkflowRunState::Cancelled
    )
}

fn persistence() -> WorkflowApplicationError {
    WorkflowApplicationError::WorkflowPersistenceFailure
}
