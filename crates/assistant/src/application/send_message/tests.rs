use std::sync::Arc;

use async_trait::async_trait;
use projects::project::domain::ProjectId;
use tokio::sync::{Barrier, Notify};
use uuid::Uuid;

use super::*;
use crate::interfaces::{AssistantModelTurnRequest, AssistantWorkspaceSnapshot};

#[derive(Clone)]
struct BarrierRunner {
    barrier: Arc<Barrier>,
    entered: Arc<Notify>,
}

#[async_trait]
impl AssistantModelRunnerInterface for BarrierRunner {
    async fn start_assistant_model_turn(
        &self,
        _request: AssistantModelTurnRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        self.entered.notify_one();
        self.barrier.wait().await;
        AssistantModelTurnResult::new(vec![1])
    }

    async fn resume_assistant_model_turn(
        &self,
        _request: crate::interfaces::AssistantModelResumeRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        Err(AssistantApplicationError::ProtocolViolation)
    }
}

#[derive(Clone, Copy)]
struct WorkspaceReaderFake;

#[async_trait]
impl AssistantWorkspaceSnapshotReaderInterface for WorkspaceReaderFake {
    async fn read_assistant_workspace_snapshot(
        &self,
        _request: AssistantWorkspaceSnapshotRequest,
    ) -> Result<AssistantWorkspaceSnapshot, AssistantApplicationError> {
        AssistantWorkspaceSnapshot::new(vec![1])
    }
}

#[tokio::test]
async fn same_project_session_rejects_a_concurrent_invocation_and_releases_after_completion() {
    let barrier = Arc::new(Barrier::new(2));
    let entered = Arc::new(Notify::new());
    let use_case = Arc::new(AssistantSendMessageUseCase::new(
        BarrierRunner { barrier: Arc::clone(&barrier), entered: Arc::clone(&entered) },
        WorkspaceReaderFake,
        AssistantActiveInvocationRegistry::default(),
    ));
    let first = tokio::spawn({
        let use_case = Arc::clone(&use_case);
        async move { use_case.send_message(command(1, 2, 3)).await }
    });
    entered.notified().await;
    assert_eq!(
        use_case.send_message(command(1, 2, 4)).await,
        Err(AssistantApplicationError::ConcurrentInvocation)
    );
    barrier.wait().await;
    assert!(first.await.unwrap().is_ok());

    let second = tokio::spawn({
        let use_case = Arc::clone(&use_case);
        async move { use_case.send_message(command(1, 2, 5)).await }
    });
    barrier.wait().await;
    assert!(second.await.unwrap().is_ok());
}

#[tokio::test]
async fn different_projects_may_invoke_the_same_session_identity_independently() {
    let barrier = Arc::new(Barrier::new(3));
    let entered = Arc::new(Notify::new());
    let use_case = Arc::new(AssistantSendMessageUseCase::new(
        BarrierRunner { barrier: Arc::clone(&barrier), entered },
        WorkspaceReaderFake,
        AssistantActiveInvocationRegistry::default(),
    ));
    let first = tokio::spawn({
        let use_case = Arc::clone(&use_case);
        async move { use_case.send_message(command(1, 2, 3)).await }
    });
    let second = tokio::spawn({
        let use_case = Arc::clone(&use_case);
        async move { use_case.send_message(command(9, 2, 4)).await }
    });
    barrier.wait().await;
    assert!(first.await.unwrap().is_ok());
    assert!(second.await.unwrap().is_ok());
}

fn command(project_seed: u8, session_seed: u8, invocation_seed: u8) -> AssistantSendMessageCommand {
    AssistantSendMessageCommand {
        workspace_request: AssistantWorkspaceSnapshotRequest::try_new(
            ProjectId::from_uuid(uuid(project_seed)).unwrap(),
            AssistantSessionId::from_uuid(uuid(session_seed)).unwrap(),
            None,
            Vec::new(),
            Vec::new(),
        )
        .unwrap(),
        invocation_id: AssistantModelInvocationId::from_uuid(uuid(invocation_seed)).unwrap(),
        intent: AssistantUserIntent::new("Create a scene").unwrap(),
    }
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
