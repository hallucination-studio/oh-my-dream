use std::sync::{Arc, Mutex};

use assistant::{
    application::AssistantApplyWorkflowChangeEffect,
    domain::*,
    interfaces::{
        AssistantClockInterface, AssistantModelContinuationEnvelope,
        AssistantModelContinuationStoreInterface, AssistantProductionPlanRepositoryInterface,
        AssistantStoredContinuation, AssistantWorkflowChangeRepositoryInterface,
    },
};
use projects::project::domain::ProjectId;
use rusqlite::Connection;
use uuid::Uuid;

use super::*;
use crate::post_commit_effect::{
    DesktopApplicationInstanceId, DesktopPostCommitEffect, DesktopPostCommitEffectOutboxInterface,
    DesktopPostCommitTimestamp, SqliteDesktopPostCommitEffectOutboxAdapterImpl,
};

#[tokio::test]
async fn production_plan_repository_round_trips_and_enforces_revision_cas() {
    let repository =
        SqliteAssistantProductionPlanRepositoryAdapterImpl::try_new(connection()).unwrap();
    let mut plan = plan();
    repository.compare_and_swap_assistant_production_plan(None, plan.clone()).await.unwrap();
    assert_eq!(
        repository
            .load_assistant_production_plan(plan.project_id(), plan.session_id())
            .await
            .unwrap(),
        Some(plan.clone())
    );
    let item_id = AssistantPlanItemId::new("draft").unwrap();
    plan.start_item(1, &item_id).unwrap();
    repository
        .compare_and_swap_assistant_production_plan(
            Some(AssistantProductionPlanRevision::initial()),
            plan.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        repository
            .compare_and_swap_assistant_production_plan(
                Some(AssistantProductionPlanRevision::initial()),
                plan,
            )
            .await,
        Err(assistant::interfaces::AssistantApplicationError::RevisionConflict)
    );
}

#[tokio::test]
async fn continuation_store_is_private_idempotent_and_single_use() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        LocalFilesystemAssistantModelContinuationStoreAdapterImpl::try_new(directory.path())
            .unwrap();
    let continuation = stored_continuation();
    store.store_assistant_model_continuation(continuation.clone()).await.unwrap();
    store.store_assistant_model_continuation(continuation.clone()).await.unwrap();
    assert_eq!(
        store.load_assistant_model_continuation(&continuation.continuation_ref).await.unwrap(),
        Some(continuation.clone())
    );
    assert_eq!(
        store.consume_assistant_model_continuation(&continuation.continuation_ref).await.unwrap(),
        Some(continuation.clone())
    );
    assert!(
        store
            .consume_assistant_model_continuation(&continuation.continuation_ref)
            .await
            .unwrap()
            .is_none()
    );
    let names = std::fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(names.iter().all(|name| !name.contains(continuation.continuation_ref.as_str())));
}

#[tokio::test]
async fn workflow_change_apply_decision_atomically_inserts_shared_outbox_effect() {
    let connection = connection();
    let outbox =
        SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let repository =
        SqliteAssistantWorkflowChangeRepositoryAdapterImpl::try_new(connection).unwrap();
    let mut change = awaiting_change(1, 2, 3);
    repository.insert_assistant_workflow_change(change.clone()).await.unwrap();
    change.begin_apply(decision_scope(&change), 20).unwrap();
    let effect = AssistantApplyWorkflowChangeEffect::new(change.id());
    repository
        .commit_assistant_workflow_change_apply_decision(
            AssistantWorkflowChangeState::AwaitingApproval,
            change.clone(),
            effect,
        )
        .await
        .unwrap();
    assert_eq!(
        repository.load_assistant_workflow_change(change.id()).await.unwrap(),
        Some(change.clone())
    );
    let claimed = outbox
        .claim_next_post_commit_effect(instance_id(9), timestamp(100))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.effect(), DesktopPostCommitEffect::Assistant(effect));
}

#[tokio::test]
async fn workflow_change_repository_enforces_one_pending_approval_per_project_session() {
    let repository =
        SqliteAssistantWorkflowChangeRepositoryAdapterImpl::try_new(connection()).unwrap();
    repository.insert_assistant_workflow_change(awaiting_change(1, 2, 3)).await.unwrap();
    assert_eq!(
        repository.insert_assistant_workflow_change(awaiting_change(1, 2, 4)).await,
        Err(assistant::interfaces::AssistantApplicationError::PendingApprovalExists)
    );
    repository.insert_assistant_workflow_change(awaiting_change(9, 2, 5)).await.unwrap();
}

#[test]
fn system_assistant_clock_returns_a_non_negative_timestamp() {
    assert!(SystemAssistantClockAdapterImpl.current_assistant_time().unwrap().epoch_ms() > 0);
}

fn connection() -> Arc<Mutex<Connection>> {
    Arc::new(Mutex::new(Connection::open_in_memory().unwrap()))
}

fn plan() -> AssistantProductionPlanAggregate {
    AssistantProductionPlanAggregate::new(
        AssistantProductionPlanId::from_uuid(uuid(3)).unwrap(),
        project_id(1),
        session_id(2),
        "Plan",
        vec![AssistantPlanItemEntity::new("draft", "Draft the scene").unwrap()],
    )
    .unwrap()
}

fn stored_continuation() -> AssistantStoredContinuation {
    AssistantStoredContinuation {
        continuation_ref: AssistantModelContinuationRef::new("assistant/secret-reference").unwrap(),
        project_id: project_id(1),
        session_id: session_id(2),
        invocation_id: invocation_id(3),
        envelope: AssistantModelContinuationEnvelope::new(vec![1, 2, 3]).unwrap(),
    }
}

fn awaiting_change(
    project_seed: u8,
    session_seed: u8,
    change_seed: u8,
) -> AssistantWorkflowChangeAggregate {
    let mut change = AssistantWorkflowChangeAggregate::new(AssistantWorkflowChangeCandidate {
        id: AssistantWorkflowChangeId::from_uuid(uuid(change_seed)).unwrap(),
        project_id: project_id(project_seed),
        session_id: session_id(session_seed),
        base_workflow_revision: WorkflowRevisionBoundaryValue::new(1).unwrap(),
        ordered_mutations: vec![AssistantWorkflowMutation::new(vec![1]).unwrap()],
        stable_aliases: AssistantWorkflowStableAliasSet::default(),
        readiness_issues: vec![],
        mutation_digest: AssistantWorkflowMutationDigest::new([7; 32]),
        resulting_workflow_fingerprint: AssistantWorkflowFingerprint::new([8; 32]),
        lineage: AssistantWorkflowChangeLineage::UserMessage {
            invocation_id: invocation_id(6),
            intent: AssistantUserIntent::new("Create a scene").unwrap(),
        },
        approval_scope_id: AssistantApprovalScopeId::from_uuid(uuid(7)).unwrap(),
        expires_at: AssistantWorkflowChangeExpiry::new(30_000).unwrap(),
    })
    .unwrap();
    change
        .accept_review(
            AssistantReviewReceipt::new(
                change.id(),
                change.mutation_digest(),
                AssistantContractEpoch::new(1).unwrap(),
                AssistantModelIdentity::new("reviewer@1").unwrap(),
                invocation_id(8),
                AssistantToolCallId::new("call_1").unwrap(),
                AssistantReviewVerdict::Pass,
                AssistantReviewedAt::new(10).unwrap(),
            ),
            AssistantModelContinuationRef::new("continuation").unwrap(),
        )
        .unwrap();
    change
}

fn decision_scope(
    change: &AssistantWorkflowChangeAggregate,
) -> AssistantWorkflowChangeDecisionScope {
    AssistantWorkflowChangeDecisionScope {
        project_id: change.project_id(),
        session_id: change.session_id(),
        change_id: change.id(),
        approval_scope_id: change.approval_scope_id(),
        mutation_digest: change.mutation_digest(),
    }
}

fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}
fn session_id(seed: u8) -> AssistantSessionId {
    AssistantSessionId::from_uuid(uuid(seed)).unwrap()
}
fn invocation_id(seed: u8) -> AssistantModelInvocationId {
    AssistantModelInvocationId::from_uuid(uuid(seed)).unwrap()
}
fn instance_id(seed: u8) -> DesktopApplicationInstanceId {
    DesktopApplicationInstanceId::from_uuid(uuid(seed)).unwrap()
}
fn timestamp(value: i64) -> DesktopPostCommitTimestamp {
    DesktopPostCommitTimestamp::from_epoch_millis(value).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
