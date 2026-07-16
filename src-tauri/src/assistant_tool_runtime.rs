//! Rust-trusted Assistant tool context and dispatcher adapters.

use std::sync::Arc;

use assistant::{
    application::{AssistantToolDispatcherImpl, AssistantToolExecutionContext},
    domain::{
        AssistantApprovalScopeId, AssistantModelInvocationId, AssistantProductionPlanId,
        AssistantSessionId, AssistantWorkflowChangeExpiry, AssistantWorkflowChangeId,
        AssistantWorkflowChangeLineage,
    },
    interfaces::{
        AssistantApplicationError, AssistantClockInterface, AssistantModelResumeRequest,
        AssistantModelTurnRequest, AssistantModelTurnStart,
        AssistantNodeCapabilityCatalogReaderInterface, AssistantProductionPlanRepositoryInterface,
        AssistantWorkflowChangeRepositoryInterface, AssistantWorkflowMutationEvaluatorInterface,
        AssistantWorkspaceSnapshotReaderInterface, AssistantWorkspaceSnapshotRequest,
    },
};
use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::assistant_model_runner::{
    AssistantProtocolToolExecutorInterface, AssistantToolExecutionContextFactoryInterface,
};

/// Builds deterministic per-invocation trusted tool identities and expiry.
#[derive(Clone)]
pub struct DesktopAssistantToolExecutionContextFactoryAdapterImpl {
    clock: Arc<dyn AssistantClockInterface>,
    approval_expiry_ms: u64,
}

impl DesktopAssistantToolExecutionContextFactoryAdapterImpl {
    #[must_use]
    pub const fn new(clock: Arc<dyn AssistantClockInterface>, approval_expiry_ms: u64) -> Self {
        Self { clock, approval_expiry_ms }
    }
}

impl AssistantToolExecutionContextFactoryInterface
    for DesktopAssistantToolExecutionContextFactoryAdapterImpl
{
    fn create_assistant_tool_execution_context(
        &self,
        request: &AssistantModelTurnRequest,
    ) -> Result<AssistantToolExecutionContext, AssistantApplicationError> {
        let lineage = match &request.start {
            AssistantModelTurnStart::UserMessage(intent) => {
                AssistantWorkflowChangeLineage::UserMessage {
                    invocation_id: request.invocation_id,
                    intent: intent.clone(),
                }
            }
            AssistantModelTurnStart::RepairActivation(activation) => {
                AssistantWorkflowChangeLineage::ReviewedRepair {
                    activation_id: activation.id(),
                    failed_workflow_run_id: activation.failed_workflow_run_id().0,
                }
            }
        };
        self.context(request.invocation_id, lineage, request.workspace_request.clone())
    }

    fn create_resumed_assistant_tool_execution_context(
        &self,
        request: &AssistantModelResumeRequest,
    ) -> Result<AssistantToolExecutionContext, AssistantApplicationError> {
        let workspace_request = AssistantWorkspaceSnapshotRequest::try_new(
            request.project_id,
            request.session_id,
            Some(request.observed_workflow_revision),
            Vec::new(),
            Vec::new(),
        )?;
        self.context(request.invocation_id, request.lineage.clone(), workspace_request)
    }
}

impl DesktopAssistantToolExecutionContextFactoryAdapterImpl {
    fn context(
        &self,
        invocation_id: AssistantModelInvocationId,
        lineage: AssistantWorkflowChangeLineage,
        workspace_request: AssistantWorkspaceSnapshotRequest,
    ) -> Result<AssistantToolExecutionContext, AssistantApplicationError> {
        let now = self.clock.current_assistant_time()?.epoch_ms();
        let expiry = i64::try_from(self.approval_expiry_ms)
            .ok()
            .and_then(|duration| now.checked_add(duration))
            .ok_or(AssistantApplicationError::ProtocolViolation)?;
        Ok(AssistantToolExecutionContext {
            project_id: workspace_request.project_id,
            session_id: workspace_request.session_id,
            production_plan_id: AssistantProductionPlanId::from_uuid(derived_id(
                invocation_id,
                b"production-plan",
            ))
            .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
            workflow_change_id: AssistantWorkflowChangeId::from_uuid(derived_id(
                invocation_id,
                b"workflow-change",
            ))
            .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
            approval_scope_id: AssistantApprovalScopeId::from_uuid(derived_id(
                invocation_id,
                b"approval-scope",
            ))
            .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
            lineage,
            workflow_change_expires_at: AssistantWorkflowChangeExpiry::new(expiry)
                .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
            workspace_request,
        })
    }
}

/// Adapts the canonical eleven-tool dispatcher to the sidecar runner boundary.
#[derive(Clone)]
pub struct DesktopAssistantProtocolToolExecutorAdapterImpl<W, C, P, E, R> {
    dispatcher: AssistantToolDispatcherImpl<W, C, P, E, R>,
}

impl<W, C, P, E, R> DesktopAssistantProtocolToolExecutorAdapterImpl<W, C, P, E, R> {
    #[must_use]
    pub const fn new(dispatcher: AssistantToolDispatcherImpl<W, C, P, E, R>) -> Self {
        Self { dispatcher }
    }
}

#[async_trait]
impl<W, C, P, E, R> AssistantProtocolToolExecutorInterface
    for DesktopAssistantProtocolToolExecutorAdapterImpl<W, C, P, E, R>
where
    W: AssistantWorkspaceSnapshotReaderInterface,
    C: AssistantNodeCapabilityCatalogReaderInterface,
    P: AssistantProductionPlanRepositoryInterface,
    E: AssistantWorkflowMutationEvaluatorInterface,
    R: AssistantWorkflowChangeRepositoryInterface,
{
    async fn execute_assistant_protocol_tool(
        &self,
        context: AssistantToolExecutionContext,
        tool_id: &str,
        arguments: Value,
    ) -> Result<Value, AssistantApplicationError> {
        self.dispatcher.execute(context, tool_id, arguments).await
    }
}

/// Stable Project-scoped Session identity supplied only by Desktop.
pub fn assistant_session_id(
    project_id: projects::project::domain::ProjectId,
) -> Result<AssistantSessionId, AssistantApplicationError> {
    let mut hasher = Sha256::new();
    hasher.update(b"oh-my-dream/assistant-session/v1");
    hasher.update(project_id.as_uuid().as_bytes());
    AssistantSessionId::from_uuid(v4_from_digest(hasher.finalize().into()))
        .map_err(|_| AssistantApplicationError::ProtocolViolation)
}

fn derived_id(invocation_id: AssistantModelInvocationId, label: &[u8]) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"oh-my-dream/assistant-invocation/v1");
    hasher.update(invocation_id.as_uuid().as_bytes());
    hasher.update(label);
    v4_from_digest(hasher.finalize().into())
}

fn v4_from_digest(digest: [u8; 32]) -> Uuid {
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
