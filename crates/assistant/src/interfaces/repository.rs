use async_trait::async_trait;
use projects::project::domain::ProjectId;

use super::{
    AssistantApplicationError, AssistantFailedWorkflowRunId, AssistantRepairActivation,
    AssistantRepairActivationRecordResult,
};
use crate::{
    application::AssistantApplyWorkflowChangeEffect,
    domain::{
        AssistantProductionPlanAggregate, AssistantProductionPlanRevision,
        AssistantRepairActivationId, AssistantReviewedAt, AssistantSessionId,
        AssistantWorkflowChangeAggregate, AssistantWorkflowChangeId,
    },
};

/// Revision-CAS persistence for one Project/Session production plan.
#[async_trait]
pub trait AssistantProductionPlanRepositoryInterface: Send + Sync {
    async fn load_assistant_production_plan(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
    ) -> Result<Option<AssistantProductionPlanAggregate>, AssistantApplicationError>;

    async fn compare_and_swap_assistant_production_plan(
        &self,
        expected_revision: Option<AssistantProductionPlanRevision>,
        plan: AssistantProductionPlanAggregate,
    ) -> Result<(), AssistantApplicationError>;
}

/// Transition persistence and pending-approval query for Workflow Changes.
#[async_trait]
pub trait AssistantWorkflowChangeRepositoryInterface: Send + Sync {
    async fn load_assistant_workflow_change(
        &self,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError>;

    async fn load_pending_assistant_workflow_change(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError>;

    async fn insert_assistant_workflow_change(
        &self,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError>;

    async fn commit_assistant_workflow_change_transition(
        &self,
        expected_state: crate::domain::AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError>;

    async fn commit_assistant_workflow_change_apply_decision(
        &self,
        expected_state: crate::domain::AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
        effect: AssistantApplyWorkflowChangeEffect,
    ) -> Result<(), AssistantApplicationError>;
}

/// Unique factual failed-Run repair activation persistence.
#[async_trait]
pub trait AssistantRepairActivationRepositoryInterface: Send + Sync {
    async fn record_or_get_repair_activation(
        &self,
        activation: AssistantRepairActivation,
    ) -> Result<AssistantRepairActivationRecordResult, AssistantApplicationError>;

    async fn load_repair_activation(
        &self,
        project_id: ProjectId,
        activation_id: AssistantRepairActivationId,
    ) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError>;

    async fn load_repair_activation_for_failed_run(
        &self,
        project_id: ProjectId,
        failed_workflow_run_id: AssistantFailedWorkflowRunId,
    ) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError>;
}

/// Deterministic Assistant wall-clock boundary.
pub trait AssistantClockInterface: Send + Sync {
    fn current_assistant_time(&self) -> Result<AssistantReviewedAt, AssistantApplicationError>;
}
