use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::*;
use crate::{
    domain::AssistantWorkflowRunBoundaryValue,
    interfaces::{
        AssistantModelResumeRequest, AssistantRepairActivationRecordResult,
        AssistantWorkspaceSnapshot, AssistantWorkspaceSnapshotRequest,
    },
};

#[derive(Clone, Default)]
struct RepairRepositoryFakeImpl {
    values:
        Arc<Mutex<BTreeMap<(ProjectId, AssistantFailedWorkflowRunId), AssistantRepairActivation>>>,
}

#[async_trait]
impl AssistantRepairActivationRepositoryInterface for RepairRepositoryFakeImpl {
    async fn record_or_get_repair_activation(
        &self,
        activation: AssistantRepairActivation,
    ) -> Result<AssistantRepairActivationRecordResult, AssistantApplicationError> {
        let key = (activation.project_id(), activation.failed_workflow_run_id());
        let mut values =
            self.values.lock().map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?;
        if let Some(existing) = values.get(&key) {
            return Ok(AssistantRepairActivationRecordResult::Existing(existing.clone()));
        }
        values.insert(key, activation.clone());
        Ok(AssistantRepairActivationRecordResult::Created(activation))
    }

    async fn load_repair_activation(
        &self,
        project_id: ProjectId,
        activation_id: AssistantRepairActivationId,
    ) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError> {
        Ok(self
            .values
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?
            .values()
            .find(|value| value.project_id() == project_id && value.id() == activation_id)
            .cloned())
    }

    async fn load_repair_activation_for_failed_run(
        &self,
        project_id: ProjectId,
        failed_workflow_run_id: AssistantFailedWorkflowRunId,
    ) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError> {
        Ok(self
            .values
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?
            .get(&(project_id, failed_workflow_run_id))
            .cloned())
    }
}

#[derive(Clone, Default)]
struct RunnerFakeImpl {
    requests: Arc<Mutex<Vec<AssistantModelTurnRequest>>>,
}

#[async_trait]
impl AssistantModelRunnerInterface for RunnerFakeImpl {
    async fn start_assistant_model_turn(
        &self,
        request: AssistantModelTurnRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        self.requests
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?
            .push(request);
        AssistantModelTurnResult::new(vec![1])
    }

    async fn resume_assistant_model_turn(
        &self,
        _request: AssistantModelResumeRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        Err(AssistantApplicationError::ProtocolViolation)
    }
}

#[derive(Clone, Copy)]
struct RunReaderFakeImpl;

#[async_trait]
impl AssistantWorkflowRunReaderInterface for RunReaderFakeImpl {
    async fn read_assistant_workflow_run(
        &self,
        _project_id: ProjectId,
        _run_id: AssistantFailedWorkflowRunId,
    ) -> Result<Option<AssistantWorkflowRunBoundaryValue>, AssistantApplicationError> {
        Ok(Some(AssistantWorkflowRunBoundaryValue::new(b"failed-run-facts".to_vec()).unwrap()))
    }
}

#[derive(Clone, Copy)]
struct WorkspaceReaderFakeImpl;

#[async_trait]
impl AssistantWorkspaceSnapshotReaderInterface for WorkspaceReaderFakeImpl {
    async fn read_assistant_workspace_snapshot(
        &self,
        _request: AssistantWorkspaceSnapshotRequest,
    ) -> Result<AssistantWorkspaceSnapshot, AssistantApplicationError> {
        AssistantWorkspaceSnapshot::new(vec![2])
    }
}

#[tokio::test]
async fn only_created_activation_starts_a_repair_turn_with_canonical_failed_run_facts() {
    let repository = RepairRepositoryFakeImpl::default();
    let runner = RunnerFakeImpl::default();
    let use_case = AssistantActivateRepairUseCase::new(
        repository,
        RunReaderFakeImpl,
        runner.clone(),
        WorkspaceReaderFakeImpl,
        AssistantActiveInvocationRegistry::default(),
    );

    assert!(use_case.activate_repair(command(3, 4)).await.unwrap().is_some());
    assert!(use_case.activate_repair(command(5, 6)).await.unwrap().is_none());

    let requests = runner.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let AssistantModelTurnStart::RepairActivation(activation) = &requests[0].start else {
        panic!("repair activation expected");
    };
    assert_eq!(activation.exact_failed_run_facts(), b"failed-run-facts");
}

fn command(activation_seed: u8, invocation_seed: u8) -> AssistantActivateRepairCommand {
    AssistantActivateRepairCommand {
        project_id: ProjectId::from_uuid(uuid(1)).unwrap(),
        session_id: AssistantSessionId::from_uuid(uuid(2)).unwrap(),
        activation_id: AssistantRepairActivationId::from_uuid(uuid(activation_seed)).unwrap(),
        invocation_id: AssistantModelInvocationId::from_uuid(uuid(invocation_seed)).unwrap(),
        failed_workflow_run_id: AssistantFailedWorkflowRunId(uuid(7).into_bytes()),
        created_at_epoch_ms: 8,
    }
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
