use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};

use crate::node_capability::{WorkflowNodeCapabilityRegistry, WorkflowRunId};
use crate::workflow_graph::{WorkflowAggregate, WorkflowId, WorkflowNodeId, WorkflowRevision};

use super::use_case::check_readiness_for_nodes;
use super::{
    WorkflowAggregateRepositoryInterface, WorkflowApplicationError, WorkflowClockInterface,
    WorkflowExecuteRunEffect, WorkflowExecutionPlan, WorkflowIdentityGeneratorInterface,
    WorkflowLoadKey, WorkflowPlannedInputBinding, WorkflowPlannedNode, WorkflowReadinessResult,
    WorkflowRunAdmissionCommit, WorkflowRunAdmissionReceipt, WorkflowRunAggregate,
    WorkflowRunCommandHash, WorkflowRunLoadKey, WorkflowRunRepositoryInterface,
    WorkflowRunRequestId, WorkflowRunScope,
};

/// Idempotent request to admit one immutable Workflow execution plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkflowStartRunCommand {
    run_request_id: WorkflowRunRequestId,
    workflow_id: WorkflowId,
    workflow_revision: WorkflowRevision,
    scope: WorkflowRunScope,
    command_hash: WorkflowRunCommandHash,
}

impl WorkflowStartRunCommand {
    /// Creates a command and its frozen canonical SHA-256 content hash.
    #[must_use]
    pub fn new(
        run_request_id: WorkflowRunRequestId,
        workflow_id: WorkflowId,
        workflow_revision: WorkflowRevision,
        scope: WorkflowRunScope,
    ) -> Self {
        let command_hash = hash_start_run(workflow_id, workflow_revision, scope);
        Self { run_request_id, workflow_id, workflow_revision, scope, command_hash }
    }
    /// Returns the request identity excluded from the content hash.
    #[must_use]
    pub const fn run_request_id(self) -> WorkflowRunRequestId {
        self.run_request_id
    }
    /// Returns the target Workflow.
    #[must_use]
    pub const fn workflow_id(self) -> WorkflowId {
        self.workflow_id
    }
    /// Returns the exact required revision.
    #[must_use]
    pub const fn workflow_revision(self) -> WorkflowRevision {
        self.workflow_revision
    }
    /// Returns the requested execution scope.
    #[must_use]
    pub const fn scope(self) -> WorkflowRunScope {
        self.scope
    }
    /// Returns the canonical content hash.
    #[must_use]
    pub const fn command_hash(self) -> WorkflowRunCommandHash {
        self.command_hash
    }
}

impl WorkflowRunAdmissionReceipt {
    /// Captures one exact request-to-Run admission mapping.
    #[must_use]
    pub fn new(command: WorkflowStartRunCommand, workflow_run_id: WorkflowRunId) -> Self {
        Self {
            request_id: command.run_request_id(),
            command_hash: command.command_hash(),
            workflow_run_id,
        }
    }
    /// Restores exact persisted request-to-Run admission evidence.
    #[must_use]
    pub const fn restore(
        request_id: WorkflowRunRequestId,
        command_hash: WorkflowRunCommandHash,
        workflow_run_id: WorkflowRunId,
    ) -> Self {
        Self { request_id, command_hash, workflow_run_id }
    }
    /// Returns the request identity.
    #[must_use]
    pub const fn request_id(&self) -> WorkflowRunRequestId {
        self.request_id
    }
    /// Returns the canonical admission hash.
    #[must_use]
    pub const fn command_hash(&self) -> WorkflowRunCommandHash {
        self.command_hash
    }
    /// Returns the admitted Run identity.
    #[must_use]
    pub const fn workflow_run_id(&self) -> WorkflowRunId {
        self.workflow_run_id
    }
    /// Returns the admitted Run only for an exact matching command.
    pub fn replay_run_id(
        &self,
        command: WorkflowStartRunCommand,
    ) -> Result<WorkflowRunId, WorkflowApplicationError> {
        if self.request_id == command.run_request_id()
            && self.command_hash == command.command_hash()
        {
            Ok(self.workflow_run_id)
        } else {
            Err(WorkflowApplicationError::WorkflowRunIdempotencyConflict)
        }
    }
}

/// Admits immutable Workflow Runs after a fresh scoped readiness evaluation.
pub struct WorkflowStartRunUseCase<A, R, C, I> {
    workflow_repository: Arc<A>,
    run_repository: Arc<R>,
    clock: Arc<C>,
    identity_generator: Arc<I>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl<A, R, C, I> WorkflowStartRunUseCase<A, R, C, I>
where
    A: WorkflowAggregateRepositoryInterface,
    R: WorkflowRunRepositoryInterface,
    C: WorkflowClockInterface,
    I: WorkflowIdentityGeneratorInterface,
{
    /// Wires the focused document, Run, time, identity, and capability boundaries.
    #[must_use]
    pub fn new(
        workflow_repository: Arc<A>,
        run_repository: Arc<R>,
        clock: Arc<C>,
        identity_generator: Arc<I>,
        capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    ) -> Self {
        Self { workflow_repository, run_repository, clock, identity_generator, capabilities }
    }

    /// Replays or atomically admits one ready frozen Run without starting provider work.
    pub async fn start_workflow_run(
        &self,
        command: WorkflowStartRunCommand,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        self.start_workflow_run_internal(None, command).await
    }

    /// Admits a Run only when its Workflow belongs to the trusted Project.
    pub async fn start_project_workflow_run(
        &self,
        project_id: ProjectId,
        command: WorkflowStartRunCommand,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        self.start_workflow_run_internal(Some(project_id), command).await
    }

    async fn start_workflow_run_internal(
        &self,
        project_id: Option<ProjectId>,
        command: WorkflowStartRunCommand,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        let workflow = self.load_workflow(project_id, command.workflow_id()).await?;
        if let Some(receipt) = self
            .run_repository
            .load_workflow_run_admission_receipt(command.run_request_id())
            .await?
        {
            let run_id = receipt.replay_run_id(command)?;
            return self
                .run_repository
                .load_workflow_run(WorkflowRunLoadKey::ProjectScoped {
                    project_id: workflow.project_id,
                    workflow_run_id: run_id,
                })
                .await?
                .ok_or(WorkflowApplicationError::WorkflowPersistenceFailure);
        }
        if workflow.revision != command.workflow_revision() {
            return Err(WorkflowApplicationError::WorkflowRunRevisionMismatch);
        }
        let selected = selected_node_ids(&workflow, command.scope())?;
        let readiness =
            check_readiness_for_nodes(&workflow, &self.capabilities, Some(&selected)).await?;
        if !matches!(readiness, WorkflowReadinessResult::Ready) {
            return Err(WorkflowApplicationError::WorkflowNotReady { readiness });
        }
        let plan = self.freeze_plan(&workflow, command.scope(), &selected)?;
        let run_id = self.identity_generator.generate_workflow_run_id();
        let now = self.clock.current_workflow_time()?;
        let run = WorkflowRunAggregate::try_new_queued(run_id, workflow.project_id, plan, now)
            .map_err(|_| WorkflowApplicationError::WorkflowPersistenceFailure)?;
        let receipt = WorkflowRunAdmissionReceipt::new(command, run_id);
        let effect = WorkflowExecuteRunEffect { workflow_run_id: run_id };
        self.run_repository
            .admit_workflow_run(WorkflowRunAdmissionCommit::try_new(run, receipt.clone(), effect)?)
            .await?;
        self.run_repository
            .load_workflow_run(WorkflowRunLoadKey::ProjectScoped {
                project_id: workflow.project_id,
                workflow_run_id: run_id,
            })
            .await?
            .ok_or(WorkflowApplicationError::WorkflowPersistenceFailure)
    }

    async fn load_workflow(
        &self,
        project_id: Option<ProjectId>,
        workflow_id: WorkflowId,
    ) -> Result<WorkflowAggregate, WorkflowApplicationError> {
        let key = WorkflowLoadKey::Workflow(workflow_id);
        let workflow = self
            .workflow_repository
            .load_workflow(key)
            .await?
            .filter(|workflow| project_id.is_none_or(|id| workflow.project_id == id))
            .ok_or(WorkflowApplicationError::WorkflowNotFound { key })?;
        Ok(workflow)
    }

    fn freeze_plan(
        &self,
        workflow: &WorkflowAggregate,
        scope: WorkflowRunScope,
        selected: &BTreeSet<WorkflowNodeId>,
    ) -> Result<WorkflowExecutionPlan, WorkflowApplicationError> {
        let order = deterministic_topological_order(workflow, selected)?;
        let mut nodes = Vec::with_capacity(order.len());
        for node_id in order {
            let node = &workflow.nodes()[&node_id];
            let capability = self
                .capabilities
                .resolve_node_capability(&node.capability_contract)
                .map_err(|_| WorkflowApplicationError::WorkflowNotReady {
                readiness: WorkflowReadinessResult::from_issues(vec![
                    super::WorkflowReadinessIssue::WorkflowCapabilityUnregistered {
                        node_id,
                        capability_ref: node.capability_contract.clone(),
                    },
                ]),
            })?;
            let normalized = capability
                .normalize_node_parameters(&node.parameter_set)
                .map_err(|_| WorkflowApplicationError::WorkflowPersistenceFailure)?;
            let input_bindings = workflow
                .input_bindings()
                .iter()
                .filter(|(target, _)| target.node_id == node_id)
                .map(|(target, binding)| WorkflowPlannedInputBinding {
                    input_key: target.input_key.clone(),
                    binding: binding.clone(),
                })
                .collect();
            nodes.push(WorkflowPlannedNode {
                node_id,
                node_execution_id: self.identity_generator.generate_workflow_node_execution_id(),
                capability_contract: node.capability_contract.clone(),
                normalized_parameters: normalized,
                input_bindings,
            });
        }
        WorkflowExecutionPlan::try_new(workflow.id, workflow.revision, scope, nodes)
            .map_err(|_| WorkflowApplicationError::WorkflowPersistenceFailure)
    }
}

fn selected_node_ids(
    workflow: &WorkflowAggregate,
    scope: WorkflowRunScope,
) -> Result<BTreeSet<WorkflowNodeId>, WorkflowApplicationError> {
    let selected = match scope {
        WorkflowRunScope::ThroughNode(selected) => selected,
        WorkflowRunScope::WholeWorkflow => return Ok(workflow.nodes().keys().copied().collect()),
    };
    if !workflow.nodes().contains_key(&selected) {
        return Err(crate::workflow_graph::WorkflowGraphError::NodeNotFound.into());
    }
    let incoming = workflow.input_bindings().iter().fold(
        BTreeMap::<WorkflowNodeId, BTreeSet<WorkflowNodeId>>::new(),
        |mut map, (target, binding)| {
            map.entry(target.node_id)
                .or_default()
                .extend(binding.items().map(|item| item.source_node_id));
            map
        },
    );
    let mut result = BTreeSet::from([selected]);
    let mut frontier = vec![selected];
    while let Some(node_id) = frontier.pop() {
        for source in incoming.get(&node_id).into_iter().flatten() {
            if result.insert(*source) {
                frontier.push(*source);
            }
        }
    }
    Ok(result)
}

fn deterministic_topological_order(
    workflow: &WorkflowAggregate,
    selected: &BTreeSet<WorkflowNodeId>,
) -> Result<Vec<WorkflowNodeId>, WorkflowApplicationError> {
    let dependencies = selected
        .iter()
        .map(|node_id| {
            let sources = workflow
                .input_bindings()
                .iter()
                .filter(|(target, _)| target.node_id == *node_id)
                .flat_map(|(_, binding)| binding.items().map(|item| item.source_node_id))
                .filter(|source| selected.contains(source))
                .collect::<BTreeSet<_>>();
            (*node_id, sources)
        })
        .collect::<BTreeMap<_, _>>();
    let mut remaining = selected.clone();
    let mut order = Vec::with_capacity(selected.len());
    while let Some(next) = remaining
        .iter()
        .copied()
        .find(|candidate| dependencies[candidate].iter().all(|source| !remaining.contains(source)))
    {
        remaining.remove(&next);
        order.push(next);
    }
    if order.len() == selected.len() {
        Ok(order)
    } else {
        Err(crate::workflow_graph::WorkflowGraphError::Cycle.into())
    }
}

fn hash_start_run(
    workflow_id: WorkflowId,
    revision: WorkflowRevision,
    scope: WorkflowRunScope,
) -> WorkflowRunCommandHash {
    let mut bytes = Vec::new();
    append_bytes(&mut bytes, b"oh-my-dream/workflow-start-run/v1");
    bytes.extend_from_slice(workflow_id.as_uuid().as_bytes());
    bytes.extend_from_slice(&revision.get().to_be_bytes());
    match scope {
        WorkflowRunScope::WholeWorkflow => bytes.push(0),
        WorkflowRunScope::ThroughNode(node_id) => {
            bytes.push(1);
            bytes.extend_from_slice(node_id.as_uuid().as_bytes());
        }
    }
    WorkflowRunCommandHash::from_bytes(Sha256::digest(bytes).into())
}

fn append_bytes(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u32).to_be_bytes());
    target.extend_from_slice(value);
}
