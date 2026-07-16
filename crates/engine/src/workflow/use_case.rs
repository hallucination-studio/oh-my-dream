use std::sync::Arc;
use std::time::{Duration, Instant};

use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};

use crate::node_capability::{
    NodeCapabilityParameterErrorCategory, NodeCapabilityReadinessCategory,
    NodeCapabilityReadinessDeadline, NodeCapabilityReadinessRequest, NodeCapabilityReadinessTarget,
    WorkflowNodeCapabilityRegistry,
};
use crate::workflow_graph::{
    WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowApplyMutationCommand,
    WorkflowCreatedAt, WorkflowId, WorkflowMutationReceipt, WorkflowSchemaVersion,
    WorkflowUpdatedAt,
};

use super::{
    WorkflowAggregateRepositoryInterface, WorkflowApplicationError, WorkflowClockInterface,
    WorkflowCreateCommandHash, WorkflowCreateReceipt, WorkflowCreateRequestId,
    WorkflowCreationCommit, WorkflowIdentityGeneratorInterface, WorkflowLoadKey,
    WorkflowMutationCommit, WorkflowReadinessIssue, WorkflowReadinessPolicy,
    WorkflowReadinessResult, WorkflowStructuralReadinessNode,
};

/// Idempotent request to create one Project's current empty Workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkflowCreateCommand {
    request_id: WorkflowCreateRequestId,
    project_id: ProjectId,
    command_hash: WorkflowCreateCommandHash,
}

impl WorkflowCreateCommand {
    /// Creates the frozen command and canonical content hash.
    #[must_use]
    pub fn new(request_id: WorkflowCreateRequestId, project_id: ProjectId) -> Self {
        let mut bytes = Vec::new();
        append_bytes(&mut bytes, b"oh-my-dream/workflow-create/v1");
        bytes.extend_from_slice(project_id.as_uuid().as_bytes());
        Self {
            request_id,
            project_id,
            command_hash: WorkflowCreateCommandHash::from_bytes(Sha256::digest(bytes).into()),
        }
    }
    /// Returns the request identity excluded from the hash.
    #[must_use]
    pub const fn request_id(self) -> WorkflowCreateRequestId {
        self.request_id
    }
    /// Returns the owning Project.
    #[must_use]
    pub const fn project_id(self) -> ProjectId {
        self.project_id
    }
    /// Returns the canonical content hash.
    #[must_use]
    pub const fn command_hash(self) -> WorkflowCreateCommandHash {
        self.command_hash
    }
}

/// Result of a committed mutation together with current readiness.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowMutationResult {
    /// Exact committed Workflow snapshot.
    pub workflow: WorkflowAggregate,
    /// Readiness derived from that snapshot.
    pub readiness: WorkflowReadinessResult,
}

/// Creates one empty current Workflow per Project with exact idempotent replay.
pub struct WorkflowCreateUseCase<R, C, I> {
    repository: Arc<R>,
    clock: Arc<C>,
    identity_generator: Arc<I>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl<R, C, I> WorkflowCreateUseCase<R, C, I>
where
    R: WorkflowAggregateRepositoryInterface,
    C: WorkflowClockInterface,
    I: WorkflowIdentityGeneratorInterface,
{
    /// Wires consumer-owned interfaces and the immutable exact capability registry.
    #[must_use]
    pub fn new(
        repository: Arc<R>,
        clock: Arc<C>,
        identity_generator: Arc<I>,
        capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    ) -> Self {
        Self { repository, clock, identity_generator, capabilities }
    }

    /// Creates or exactly replays one Project's initial empty Workflow.
    pub async fn create_workflow(
        &self,
        command: WorkflowCreateCommand,
    ) -> Result<WorkflowAggregate, WorkflowApplicationError> {
        if let Some(receipt) =
            self.repository.load_workflow_creation_receipt(command.request_id()).await?
        {
            return if receipt.command_hash() == command.command_hash() {
                Ok(receipt.created_workflow().clone())
            } else {
                Err(WorkflowApplicationError::WorkflowCreationIdempotencyConflict)
            };
        }
        if self
            .repository
            .load_workflow(WorkflowLoadKey::Project(command.project_id()))
            .await?
            .is_some()
        {
            return Err(WorkflowApplicationError::WorkflowAlreadyExistsForProject);
        }
        let observed = self.clock.current_workflow_time()?.as_utc_milliseconds();
        let workflow = WorkflowAggregate::try_restore(
            WorkflowAggregateRestoreData {
                schema_version: WorkflowSchemaVersion::CURRENT,
                id: self.identity_generator.generate_workflow_id(),
                project_id: command.project_id(),
                revision: crate::workflow_graph::WorkflowRevision::new(1)?,
                created_at: WorkflowCreatedAt::from_utc_milliseconds(observed)?,
                updated_at: WorkflowUpdatedAt::from_utc_milliseconds(observed)?,
                nodes: Vec::new(),
                input_bindings: Vec::new(),
            },
            &self.capabilities,
        )?;
        let receipt = WorkflowCreateReceipt::new(
            command.request_id(),
            command.command_hash(),
            workflow.clone(),
        )?;
        self.repository
            .commit_workflow_creation(WorkflowCreationCommit::try_new(workflow, receipt)?)
            .await
    }
}

/// Loads the one current Workflow for a Project.
pub struct WorkflowGetCurrentUseCase<R> {
    repository: Arc<R>,
}

/// One authoritative current Workflow snapshot and readiness derived from it.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowCurrentResult {
    /// The single loaded current Workflow.
    pub workflow: WorkflowAggregate,
    /// Readiness evaluated from that exact aggregate snapshot.
    pub readiness: WorkflowReadinessResult,
}

impl<R: WorkflowAggregateRepositoryInterface> WorkflowGetCurrentUseCase<R> {
    /// Wires the Workflow repository.
    #[must_use]
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
    /// Loads the Project's current Workflow or returns a typed not-found error.
    pub async fn get_current_workflow(
        &self,
        project_id: ProjectId,
    ) -> Result<WorkflowAggregate, WorkflowApplicationError> {
        let key = WorkflowLoadKey::Project(project_id);
        self.repository
            .load_workflow(key)
            .await?
            .ok_or(WorkflowApplicationError::WorkflowNotFound { key })
    }

    /// Loads once and evaluates readiness from the exact returned snapshot.
    pub async fn get_current_workflow_with_readiness(
        &self,
        project_id: ProjectId,
        capabilities: &WorkflowNodeCapabilityRegistry,
    ) -> Result<WorkflowCurrentResult, WorkflowApplicationError> {
        let workflow = self.get_current_workflow(project_id).await?;
        let readiness = check_readiness(&workflow, capabilities).await?;
        Ok(WorkflowCurrentResult { workflow, readiness })
    }
}

/// Computes structural and capability-owned external readiness.
pub struct WorkflowCheckReadinessUseCase<R> {
    repository: Arc<R>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl<R: WorkflowAggregateRepositoryInterface> WorkflowCheckReadinessUseCase<R> {
    /// Wires the Workflow repository and immutable exact capability registry.
    #[must_use]
    pub fn new(repository: Arc<R>, capabilities: Arc<WorkflowNodeCapabilityRegistry>) -> Self {
        Self { repository, capabilities }
    }
    /// Loads one Workflow and evaluates all nodes against one shared five-second deadline.
    pub async fn check_workflow_readiness(
        &self,
        workflow_id: WorkflowId,
    ) -> Result<WorkflowReadinessResult, WorkflowApplicationError> {
        let key = WorkflowLoadKey::Workflow(workflow_id);
        let workflow = self
            .repository
            .load_workflow(key)
            .await?
            .ok_or(WorkflowApplicationError::WorkflowNotFound { key })?;
        check_readiness(&workflow, &self.capabilities).await
    }
}

/// Applies all ten-action mutations through one idempotent revision compare-and-swap.
pub struct WorkflowApplyMutationUseCase<R, C> {
    repository: Arc<R>,
    clock: Arc<C>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl<R, C> WorkflowApplyMutationUseCase<R, C>
where
    R: WorkflowAggregateRepositoryInterface,
    C: WorkflowClockInterface,
{
    /// Wires mutation boundaries and the immutable exact capability registry.
    #[must_use]
    pub fn new(
        repository: Arc<R>,
        clock: Arc<C>,
        capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    ) -> Self {
        Self { repository, clock, capabilities }
    }
    /// Applies, commits, and returns the exact snapshot plus current readiness.
    pub async fn apply_workflow_mutation(
        &self,
        command: WorkflowApplyMutationCommand,
    ) -> Result<WorkflowMutationResult, WorkflowApplicationError> {
        if let Some(receipt) =
            self.repository.load_workflow_mutation_receipt(command.request_id()).await?
        {
            let workflow = receipt
                .replay_matching_command(&command)
                .map_err(|error| match error {
                    crate::workflow_graph::WorkflowGraphError::MutationIdempotencyConflict => {
                        WorkflowApplicationError::WorkflowMutationIdempotencyConflict
                    }
                    other => WorkflowApplicationError::WorkflowGraph(other),
                })?
                .clone();
            let readiness = check_readiness(&workflow, &self.capabilities).await?;
            return Ok(WorkflowMutationResult { workflow, readiness });
        }
        let key = WorkflowLoadKey::Workflow(command.workflow_id());
        let current = self
            .repository
            .load_workflow(key)
            .await?
            .ok_or(WorkflowApplicationError::WorkflowNotFound { key })?;
        if current.revision != command.base_revision() {
            return Err(WorkflowApplicationError::WorkflowRevisionConflict);
        }
        let observed = WorkflowUpdatedAt::from_utc_milliseconds(
            self.clock.current_workflow_time()?.as_utc_milliseconds(),
        )?;
        let candidate = current.apply_mutation_command(&command, observed, &self.capabilities)?;
        let receipt = WorkflowMutationReceipt::new(&command, candidate.clone());
        let committed = self
            .repository
            .commit_workflow_mutation(WorkflowMutationCommit::try_new(
                candidate,
                current.revision,
                receipt,
            )?)
            .await?;
        let workflow = committed.committed_workflow().clone();
        let readiness = check_readiness(&workflow, &self.capabilities).await?;
        Ok(WorkflowMutationResult { workflow, readiness })
    }
}

pub(super) async fn check_readiness(
    workflow: &WorkflowAggregate,
    capabilities: &WorkflowNodeCapabilityRegistry,
) -> Result<WorkflowReadinessResult, WorkflowApplicationError> {
    check_readiness_for_nodes(workflow, capabilities, None).await
}

pub(super) async fn check_readiness_for_nodes(
    workflow: &WorkflowAggregate,
    capabilities: &WorkflowNodeCapabilityRegistry,
    selected_node_ids: Option<&std::collections::BTreeSet<crate::workflow_graph::WorkflowNodeId>>,
) -> Result<WorkflowReadinessResult, WorkflowApplicationError> {
    let deadline = NodeCapabilityReadinessDeadline::at(Instant::now() + Duration::from_secs(5));
    let mut issues = Vec::new();
    for node in workflow.nodes().values() {
        if selected_node_ids.is_some_and(|selected| !selected.contains(&node.id)) {
            continue;
        }
        let Ok(capability) = capabilities.resolve_node_capability(&node.capability_contract) else {
            issues.push(WorkflowReadinessIssue::WorkflowCapabilityUnregistered {
                node_id: node.id,
                capability_ref: node.capability_contract.clone(),
            });
            continue;
        };
        let bindings = workflow
            .input_bindings()
            .iter()
            .filter(|(target, _)| target.node_id == node.id)
            .map(|(target, binding)| (target.clone(), binding.clone()))
            .collect::<Vec<_>>();
        if let WorkflowReadinessResult::Blocked { issues: structural } =
            WorkflowReadinessPolicy::check(&[WorkflowStructuralReadinessNode {
                node_id: node.id,
                contract: capability.node_capability_contract(),
                parameters: &node.parameter_set,
                input_bindings: &bindings,
            }])
        {
            issues.extend(structural);
        }
        let normalized = match capability.normalize_node_parameters(&node.parameter_set) {
            Ok(normalized) => normalized,
            Err(error)
                if error.category()
                    == NodeCapabilityParameterErrorCategory::RequiredParameterMissing =>
            {
                continue;
            }
            Err(_) => return Err(WorkflowApplicationError::WorkflowPersistenceFailure),
        };
        for issue in capability
            .check_node_external_readiness(NodeCapabilityReadinessRequest {
                project_id: workflow.project_id,
                normalized_parameters: normalized,
                deadline,
            })
            .await
        {
            issues.push(project_external_issue(node.id, &node.capability_contract, issue));
        }
    }
    Ok(WorkflowReadinessResult::from_issues(issues))
}

fn project_external_issue(
    node_id: crate::workflow_graph::WorkflowNodeId,
    capability_ref: &crate::node_capability::NodeCapabilityContractRef,
    issue: crate::node_capability::NodeCapabilityReadinessIssue,
) -> WorkflowReadinessIssue {
    match (issue.category(), issue.target()) {
        (
            NodeCapabilityReadinessCategory::GenerationProfileIncompatible,
            NodeCapabilityReadinessTarget::GenerationProfile { generation_profile_ref, .. },
        ) => WorkflowReadinessIssue::WorkflowGenerationProfileIncompatible {
            node_id,
            profile_ref: generation_profile_ref.clone(),
            capability_ref: capability_ref.clone(),
        },
        (
            NodeCapabilityReadinessCategory::GenerationProfileUnavailable,
            NodeCapabilityReadinessTarget::GenerationProfile { generation_profile_ref, .. },
        ) => WorkflowReadinessIssue::WorkflowGenerationProfileUnavailable {
            node_id,
            profile_ref: generation_profile_ref.clone(),
        },
        (
            NodeCapabilityReadinessCategory::GenerationProfileAvailabilityIndeterminate,
            NodeCapabilityReadinessTarget::GenerationProfile { generation_profile_ref, .. },
        ) => WorkflowReadinessIssue::WorkflowGenerationProfileAvailabilityIndeterminate {
            node_id,
            profile_ref: generation_profile_ref.clone(),
        },
        _ => WorkflowReadinessIssue::WorkflowCapabilityExternalReadinessIssue { node_id, issue },
    }
}

fn append_bytes(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u32).to_be_bytes());
    target.extend_from_slice(value);
}
