use async_trait::async_trait;
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::*;
use crate::domain::{
    AssistantModelInvocationId, AssistantProductionPlanAggregate, AssistantProductionPlanRevision,
    AssistantReviewedAt, AssistantSessionId,
};

struct BoundaryFaultFake;

#[async_trait]
impl AssistantModelRunnerInterface for BoundaryFaultFake {
    async fn start_assistant_model_turn(
        &self,
        _request: AssistantModelTurnRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
    async fn resume_assistant_model_turn(
        &self,
        _request: AssistantModelResumeRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[async_trait]
impl AssistantWorkspaceSnapshotReaderInterface for BoundaryFaultFake {
    async fn read_assistant_workspace_snapshot(
        &self,
        _request: AssistantWorkspaceSnapshotRequest,
    ) -> Result<AssistantWorkspaceSnapshot, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[async_trait]
impl AssistantNodeCapabilityCatalogReaderInterface for BoundaryFaultFake {
    async fn read_assistant_node_capability_catalog(
        &self,
        _request: AssistantNodeCapabilityCatalogRequest,
    ) -> Result<AssistantNodeCapabilityCatalogSnapshot, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[async_trait]
impl AssistantWorkflowMutationEvaluatorInterface for BoundaryFaultFake {
    async fn evaluate_assistant_workflow_mutations(
        &self,
        _request: AssistantWorkflowEvaluationRequest,
    ) -> Result<AssistantWorkflowEvaluationResult, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[async_trait]
impl AssistantWorkflowMutationApplierInterface for BoundaryFaultFake {
    async fn apply_assistant_workflow_change(
        &self,
        _request: AssistantWorkflowApplyRequest,
    ) -> Result<AssistantWorkflowApplyReceiptBoundaryValue, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[async_trait]
impl AssistantWorkflowRunStarterInterface for BoundaryFaultFake {
    async fn start_assistant_workflow_run(
        &self,
        _request: AssistantWorkflowRunRequest,
    ) -> Result<AssistantWorkflowRunBoundaryValue, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[async_trait]
impl AssistantWorkflowRunReaderInterface for BoundaryFaultFake {
    async fn read_assistant_workflow_run(
        &self,
        _project_id: ProjectId,
        _run_id: AssistantFailedWorkflowRunId,
    ) -> Result<Option<AssistantWorkflowRunBoundaryValue>, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[async_trait]
impl AssistantProductionPlanRepositoryInterface for BoundaryFaultFake {
    async fn load_assistant_production_plan(
        &self,
        _project_id: ProjectId,
        _session_id: AssistantSessionId,
    ) -> Result<Option<AssistantProductionPlanAggregate>, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
    async fn compare_and_swap_assistant_production_plan(
        &self,
        _expected_revision: Option<AssistantProductionPlanRevision>,
        _plan: AssistantProductionPlanAggregate,
    ) -> Result<(), AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
}

impl AssistantClockInterface for BoundaryFaultFake {
    fn current_assistant_time(&self) -> Result<AssistantReviewedAt, AssistantApplicationError> {
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[tokio::test]
async fn independent_boundary_faults_preserve_the_closed_category() {
    assert_interface_coverage::<BoundaryFaultFake>();
    let fake = BoundaryFaultFake;
    let project_id = ProjectId::from_uuid(uuid(1)).unwrap();
    let session_id = AssistantSessionId::from_uuid(uuid(2)).unwrap();
    let request = AssistantModelTurnRequest {
        project_id,
        session_id,
        invocation_id: AssistantModelInvocationId::from_uuid(uuid(3)).unwrap(),
        start: super::AssistantModelTurnStart::UserMessage(
            crate::domain::AssistantUserIntent::new("intent").unwrap(),
        ),
        workspace_snapshot: AssistantWorkspaceSnapshot::new(vec![1]).unwrap(),
    };
    assert_eq!(
        fake.start_assistant_model_turn(request).await,
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    );
    assert_eq!(
        fake.read_assistant_workspace_snapshot(
            AssistantWorkspaceSnapshotRequest::try_new(
                project_id,
                session_id,
                None,
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        )
        .await,
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    );
    assert_eq!(
        fake.current_assistant_time(),
        Err(AssistantApplicationError::ExternalBoundaryFailed)
    );
}

fn assert_interface_coverage<T>()
where
    T: AssistantModelRunnerInterface
        + AssistantWorkspaceSnapshotReaderInterface
        + AssistantNodeCapabilityCatalogReaderInterface
        + AssistantWorkflowMutationEvaluatorInterface
        + AssistantWorkflowMutationApplierInterface
        + AssistantWorkflowRunStarterInterface
        + AssistantWorkflowRunReaderInterface
        + AssistantProductionPlanRepositoryInterface
        + AssistantClockInterface,
{
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
