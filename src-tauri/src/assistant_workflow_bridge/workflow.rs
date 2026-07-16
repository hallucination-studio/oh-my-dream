use std::sync::Arc;

use assistant::{
    domain::{
        AssistantWorkflowChangeCandidate, AssistantWorkflowFingerprint, AssistantWorkflowMutation,
        AssistantWorkflowMutationDigest, AssistantWorkflowReadinessIssueBoundaryValue,
        AssistantWorkflowStableAliasEntry, AssistantWorkflowStableAliasSet,
    },
    interfaces::{
        AssistantApplicationError, AssistantFailedWorkflowRunId,
        AssistantWorkflowApplyReceiptBoundaryValue, AssistantWorkflowApplyRequest,
        AssistantWorkflowEvaluationRequest, AssistantWorkflowEvaluationResult,
        AssistantWorkflowMutationApplierInterface, AssistantWorkflowMutationEvaluatorInterface,
        AssistantWorkflowRunBoundaryValue, AssistantWorkflowRunReaderInterface,
        AssistantWorkflowRunRequest, AssistantWorkflowRunStarterInterface,
    },
};
use async_trait::async_trait;
use engine::{
    workflow::{
        WorkflowAggregateRepositoryInterface, WorkflowApplicationError, WorkflowClockInterface,
        WorkflowEvaluateMutationCommand, WorkflowEvaluateMutationUseCase,
        WorkflowGetCurrentUseCase, WorkflowGetRunUseCase, WorkflowIdentityGeneratorInterface,
        WorkflowListRunEventsUseCase, WorkflowReadinessResult, WorkflowRunRepositoryInterface,
        WorkflowRunScope, WorkflowStartRunCommand, WorkflowStartRunUseCase,
    },
    workflow_graph::{
        WorkflowApplyMutationCommand, WorkflowMutationAction, WorkflowMutationRequestId,
        WorkflowNodeId, WorkflowRevision,
    },
};
use sha2::{Digest, Sha256};

use super::{
    proposal::translate_proposals,
    receipt::{apply_receipt_bytes, decode_apply_receipt, run_boundary_bytes},
    run_projection::run_with_events_boundary,
};

/// Canonical Assistant evaluation bridge over Workflow application use cases.
pub struct DesktopAssistantWorkflowBridgeAdapterImpl<A, R, C, I> {
    evaluate_mutation: Arc<WorkflowEvaluateMutationUseCase<A, C>>,
    apply_mutation: Arc<engine::workflow::WorkflowApplyMutationUseCase<A, C>>,
    get_current: Arc<WorkflowGetCurrentUseCase<A>>,
    start_run: Arc<WorkflowStartRunUseCase<A, R, C, I>>,
    get_run: Arc<WorkflowGetRunUseCase<R>>,
    list_run_events: Arc<WorkflowListRunEventsUseCase<R>>,
}

impl<A, R, C, I> DesktopAssistantWorkflowBridgeAdapterImpl<A, R, C, I> {
    /// Wires only canonical Workflow application use cases.
    #[must_use]
    pub const fn new(
        evaluate_mutation: Arc<WorkflowEvaluateMutationUseCase<A, C>>,
        apply_mutation: Arc<engine::workflow::WorkflowApplyMutationUseCase<A, C>>,
        get_current: Arc<WorkflowGetCurrentUseCase<A>>,
        start_run: Arc<WorkflowStartRunUseCase<A, R, C, I>>,
        get_run: Arc<WorkflowGetRunUseCase<R>>,
        list_run_events: Arc<WorkflowListRunEventsUseCase<R>>,
    ) -> Self {
        Self { evaluate_mutation, apply_mutation, get_current, start_run, get_run, list_run_events }
    }
}

#[async_trait]
impl<A, R, C, I> AssistantWorkflowMutationEvaluatorInterface
    for DesktopAssistantWorkflowBridgeAdapterImpl<A, R, C, I>
where
    A: WorkflowAggregateRepositoryInterface + 'static,
    R: WorkflowRunRepositoryInterface + 'static,
    C: WorkflowClockInterface + 'static,
    I: WorkflowIdentityGeneratorInterface + 'static,
{
    async fn evaluate_assistant_workflow_mutations(
        &self,
        request: AssistantWorkflowEvaluationRequest,
    ) -> Result<AssistantWorkflowEvaluationResult, AssistantApplicationError> {
        let authorization = request.authorization;
        let (actions, aliases) =
            translate_proposals(authorization.change_id, &request.proposed_mutations)?;
        let base_revision =
            engine::workflow_graph::WorkflowRevision::new(request.base_workflow_revision.get())
                .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
        let result = self
            .evaluate_mutation
            .evaluate_workflow_mutation(WorkflowEvaluateMutationCommand {
                project_id: authorization.project_id,
                request_id: mutation_request_id(authorization.change_id)?,
                base_revision,
                actions: actions.clone(),
            })
            .await
            .map_err(map_workflow_error)?;
        Ok(AssistantWorkflowEvaluationResult {
            candidate: AssistantWorkflowChangeCandidate {
                id: authorization.change_id,
                project_id: authorization.project_id,
                session_id: authorization.session_id,
                base_workflow_revision: request.base_workflow_revision,
                ordered_mutations: canonical_mutations(&actions)?,
                stable_aliases: stable_aliases(aliases)?,
                readiness_issues: readiness_issues(result.readiness)?,
                mutation_digest: mutation_digest(&actions),
                resulting_workflow_fingerprint: AssistantWorkflowFingerprint::new(
                    result.workflow.canonical_graph_fingerprint(),
                ),
                lineage: authorization.lineage,
                approval_scope_id: authorization.approval_scope_id,
                expires_at: authorization.expires_at,
            },
        })
    }
}

#[async_trait]
impl<A, R, C, I> AssistantWorkflowMutationApplierInterface
    for DesktopAssistantWorkflowBridgeAdapterImpl<A, R, C, I>
where
    A: WorkflowAggregateRepositoryInterface + 'static,
    R: WorkflowRunRepositoryInterface + 'static,
    C: WorkflowClockInterface + 'static,
    I: WorkflowIdentityGeneratorInterface + 'static,
{
    async fn apply_assistant_workflow_change(
        &self,
        request: AssistantWorkflowApplyRequest,
    ) -> Result<AssistantWorkflowApplyReceiptBoundaryValue, AssistantApplicationError> {
        let change = request.change;
        let current = self
            .get_current
            .get_current_workflow(change.project_id())
            .await
            .map_err(map_workflow_error)?;
        let base_revision = WorkflowRevision::new(change.base_workflow_revision().get())
            .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
        let actions = change
            .ordered_mutations()
            .iter()
            .map(|value| WorkflowMutationAction::try_from_canonical_bytes(value.canonical_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
        let result = self
            .apply_mutation
            .apply_workflow_mutation(
                WorkflowApplyMutationCommand::try_new(
                    apply_request_id(change.id(), change.mutation_digest())?,
                    current.id,
                    base_revision,
                    actions,
                )
                .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
            )
            .await
            .map_err(map_workflow_error)?;
        if result.workflow.canonical_graph_fingerprint()
            != change.resulting_workflow_fingerprint().as_bytes()
        {
            return Err(AssistantApplicationError::CandidateFingerprintMismatch);
        }
        AssistantWorkflowApplyReceiptBoundaryValue::new(apply_receipt_bytes(&result.workflow))
            .map_err(|_| AssistantApplicationError::ProtocolViolation)
    }
}

#[async_trait]
impl<A, R, C, I> AssistantWorkflowRunStarterInterface
    for DesktopAssistantWorkflowBridgeAdapterImpl<A, R, C, I>
where
    A: WorkflowAggregateRepositoryInterface + 'static,
    R: WorkflowRunRepositoryInterface + 'static,
    C: WorkflowClockInterface + 'static,
    I: WorkflowIdentityGeneratorInterface + 'static,
{
    async fn start_assistant_workflow_run(
        &self,
        request: AssistantWorkflowRunRequest,
    ) -> Result<AssistantWorkflowRunBoundaryValue, AssistantApplicationError> {
        let receipt = decode_apply_receipt(request.applied_workflow_receipt.canonical_bytes())?;
        let current = self
            .get_current
            .get_current_workflow(request.project_id)
            .await
            .map_err(map_workflow_error)?;
        if current.id != receipt.workflow_id
            || current.revision != receipt.workflow_revision
            || current.canonical_fingerprint() != receipt.workflow_fingerprint
        {
            return Err(AssistantApplicationError::StaleWorkflowRevision);
        }
        let run = self
            .start_run
            .start_workflow_run(WorkflowStartRunCommand::new(
                run_request_id(request.workflow_change_id)?,
                receipt.workflow_id,
                receipt.workflow_revision,
                WorkflowRunScope::WholeWorkflow,
            ))
            .await
            .map_err(map_workflow_error)?;
        AssistantWorkflowRunBoundaryValue::new(run_boundary_bytes(&run))
            .map_err(|_| AssistantApplicationError::ProtocolViolation)
    }
}

#[async_trait]
impl<A, R, C, I> AssistantWorkflowRunReaderInterface
    for DesktopAssistantWorkflowBridgeAdapterImpl<A, R, C, I>
where
    A: WorkflowAggregateRepositoryInterface + 'static,
    R: WorkflowRunRepositoryInterface + 'static,
    C: WorkflowClockInterface + 'static,
    I: WorkflowIdentityGeneratorInterface + 'static,
{
    async fn read_assistant_workflow_run(
        &self,
        project_id: projects::project::domain::ProjectId,
        run_id: AssistantFailedWorkflowRunId,
    ) -> Result<Option<AssistantWorkflowRunBoundaryValue>, AssistantApplicationError> {
        let run_id =
            engine::node_capability::WorkflowRunId::from_uuid(uuid::Uuid::from_bytes(run_id.0))
                .ok_or(AssistantApplicationError::ProtocolViolation)?;
        let run = match self.get_run.get_workflow_run(project_id, run_id).await {
            Ok(run) => run,
            Err(WorkflowApplicationError::WorkflowRunNotFound) => return Ok(None),
            Err(error) => return Err(map_workflow_error(error)),
        };
        let mut events = Vec::new();
        let mut cursor = None;
        loop {
            let page = self
                .list_run_events
                .list_workflow_run_events(project_id, run_id, cursor, 500)
                .await
                .map_err(map_workflow_error)?;
            events.extend(page.events);
            if events.len() > 4_096 {
                return Err(AssistantApplicationError::BudgetExceeded);
            }
            match page.next_sequence {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        let bytes = run_with_events_boundary(&run, &events)
            .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
        AssistantWorkflowRunBoundaryValue::new(bytes)
            .map(Some)
            .map_err(|_| AssistantApplicationError::BudgetExceeded)
    }
}

fn canonical_mutations(
    actions: &[WorkflowMutationAction],
) -> Result<Vec<AssistantWorkflowMutation>, AssistantApplicationError> {
    actions
        .iter()
        .map(|action| {
            AssistantWorkflowMutation::new(action.canonical_bytes())
                .map_err(|_| AssistantApplicationError::ProtocolViolation)
        })
        .collect()
}

fn stable_aliases(
    aliases: std::collections::BTreeMap<String, WorkflowNodeId>,
) -> Result<AssistantWorkflowStableAliasSet, AssistantApplicationError> {
    let entries = aliases
        .into_iter()
        .map(|(alias, node_id)| {
            AssistantWorkflowStableAliasEntry::new(alias, node_id.as_uuid().into_bytes())
                .map_err(|_| AssistantApplicationError::ProtocolViolation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    AssistantWorkflowStableAliasSet::new(entries)
        .map_err(|_| AssistantApplicationError::ProtocolViolation)
}

fn readiness_issues(
    readiness: WorkflowReadinessResult,
) -> Result<Vec<AssistantWorkflowReadinessIssueBoundaryValue>, AssistantApplicationError> {
    match readiness {
        WorkflowReadinessResult::Ready => Ok(Vec::new()),
        WorkflowReadinessResult::Blocked { issues } => issues
            .into_iter()
            .map(|issue| {
                AssistantWorkflowReadinessIssueBoundaryValue::new(issue.canonical_bytes())
                    .map_err(|_| AssistantApplicationError::ProtocolViolation)
            })
            .collect(),
    }
}

fn mutation_digest(actions: &[WorkflowMutationAction]) -> AssistantWorkflowMutationDigest {
    let mut hash = Sha256::new();
    append_bytes(&mut hash, b"oh-my-dream/assistant-workflow-mutations/v1");
    hash.update((actions.len() as u32).to_be_bytes());
    for action in actions {
        append_bytes(&mut hash, &action.canonical_bytes());
    }
    AssistantWorkflowMutationDigest::new(hash.finalize().into())
}

fn mutation_request_id(
    change_id: assistant::domain::AssistantWorkflowChangeId,
) -> Result<WorkflowMutationRequestId, AssistantApplicationError> {
    let mut bytes: [u8; 16] = Sha256::new()
        .chain_update(b"oh-my-dream/assistant-workflow-evaluation/v1")
        .chain_update(change_id.as_uuid().as_bytes())
        .finalize()[..16]
        .try_into()
        .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    WorkflowMutationRequestId::from_uuid(uuid::Uuid::from_bytes(bytes))
        .map_err(|_| AssistantApplicationError::ProtocolViolation)
}

fn apply_request_id(
    change_id: assistant::domain::AssistantWorkflowChangeId,
    digest: AssistantWorkflowMutationDigest,
) -> Result<WorkflowMutationRequestId, AssistantApplicationError> {
    derived_workflow_uuid(
        b"oh-my-dream/assistant-workflow-apply/v1",
        change_id.as_uuid().as_bytes(),
        &digest.as_bytes(),
    )
    .and_then(|value| {
        WorkflowMutationRequestId::from_uuid(value)
            .map_err(|_| AssistantApplicationError::ProtocolViolation)
    })
}

fn run_request_id(
    change_id: assistant::domain::AssistantWorkflowChangeId,
) -> Result<engine::workflow::WorkflowRunRequestId, AssistantApplicationError> {
    derived_workflow_uuid(
        b"oh-my-dream/assistant-workflow-run/v1",
        change_id.as_uuid().as_bytes(),
        &[],
    )
    .and_then(|value| {
        engine::workflow::WorkflowRunRequestId::from_uuid(value)
            .ok_or(AssistantApplicationError::ProtocolViolation)
    })
}

fn derived_workflow_uuid(
    domain: &[u8],
    identity: &[u8],
    extra: &[u8],
) -> Result<uuid::Uuid, AssistantApplicationError> {
    let digest =
        Sha256::new().chain_update(domain).chain_update(identity).chain_update(extra).finalize();
    let mut bytes: [u8; 16] =
        digest[..16].try_into().map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(uuid::Uuid::from_bytes(bytes))
}

fn append_bytes(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u32).to_be_bytes());
    hash.update(value);
}

fn map_workflow_error(error: WorkflowApplicationError) -> AssistantApplicationError {
    match error {
        WorkflowApplicationError::WorkflowRevisionConflict => {
            AssistantApplicationError::StaleWorkflowRevision
        }
        WorkflowApplicationError::WorkflowNotFound { .. } => AssistantApplicationError::NotFound,
        _ => AssistantApplicationError::ExternalBoundaryFailed,
    }
}
