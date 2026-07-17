use async_trait::async_trait;
use projects::project::domain::ProjectId;

use super::{
    AssistantApplicationError, AssistantNodeCapabilityCatalogRequest,
    AssistantNodeCapabilityCatalogSnapshot, AssistantWorkflowApplyReceiptBoundaryValue,
    AssistantWorkflowApplyRequest, AssistantWorkflowEvaluationRequest,
    AssistantWorkflowEvaluationResult, AssistantWorkflowRunBoundaryValue,
    AssistantWorkflowRunRequest, AssistantWorkspaceSnapshot,
};

/// Authoritative bounded workspace projection reader.
#[async_trait]
pub trait AssistantWorkspaceSnapshotReaderInterface: Send + Sync {
    async fn read_assistant_workspace_snapshot(
        &self,
        request: super::AssistantWorkspaceSnapshotRequest,
    ) -> Result<AssistantWorkspaceSnapshot, AssistantApplicationError>;
}

/// Active capability/profile catalog projection reader.
#[async_trait]
pub trait AssistantNodeCapabilityCatalogReaderInterface: Send + Sync {
    async fn read_assistant_node_capability_catalog(
        &self,
        request: AssistantNodeCapabilityCatalogRequest,
    ) -> Result<AssistantNodeCapabilityCatalogSnapshot, AssistantApplicationError>;
}

/// Non-committing canonical Workflow candidate evaluator.
#[async_trait]
pub trait AssistantWorkflowMutationEvaluatorInterface: Send + Sync {
    async fn evaluate_assistant_workflow_mutations(
        &self,
        request: AssistantWorkflowEvaluationRequest,
    ) -> Result<AssistantWorkflowEvaluationResult, AssistantApplicationError>;
}

/// Canonical Workflow mutation use-case bridge.
#[async_trait]
pub trait AssistantWorkflowMutationApplierInterface: Send + Sync {
    async fn apply_assistant_workflow_change(
        &self,
        request: AssistantWorkflowApplyRequest,
    ) -> Result<AssistantWorkflowApplyReceiptBoundaryValue, AssistantApplicationError>;
}

/// Canonical Workflow Run admission bridge.
#[async_trait]
pub trait AssistantWorkflowRunStarterInterface: Send + Sync {
    async fn start_assistant_workflow_run(
        &self,
        request: AssistantWorkflowRunRequest,
    ) -> Result<AssistantWorkflowRunBoundaryValue, AssistantApplicationError>;
}

/// Canonical committed Workflow Run/event projection reader.
#[async_trait]
pub trait AssistantWorkflowRunReaderInterface: Send + Sync {
    async fn read_assistant_workflow_run(
        &self,
        project_id: ProjectId,
        run_id: super::AssistantFailedWorkflowRunId,
    ) -> Result<Option<AssistantWorkflowRunBoundaryValue>, AssistantApplicationError>;
}
