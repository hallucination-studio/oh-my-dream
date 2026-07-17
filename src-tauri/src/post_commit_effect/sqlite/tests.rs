use assets::asset::{application::AssetFinalizeContentEffect, domain::AssetContentFinalizationId};
use assistant::{
    application::AssistantApplyWorkflowChangeEffect, domain::AssistantWorkflowChangeId,
};
use engine::{node_capability::WorkflowRunId, workflow::WorkflowExecuteRunEffect};
use uuid::Uuid;

use super::*;

fn setup() -> (Arc<Mutex<Connection>>, SqliteDesktopPostCommitEffectOutboxAdapterImpl) {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let adapter =
        SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    (connection, adapter)
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
fn effect_id(seed: u8) -> DesktopPostCommitEffectId {
    DesktopPostCommitEffectId::from_uuid(uuid(seed)).unwrap()
}
fn instance_id(seed: u8) -> DesktopApplicationInstanceId {
    DesktopApplicationInstanceId::from_uuid(uuid(seed)).unwrap()
}
fn time(value: i64) -> DesktopPostCommitTimestamp {
    DesktopPostCommitTimestamp::from_epoch_millis(value).unwrap()
}
fn workflow(seed: u8) -> DesktopPostCommitEffect {
    DesktopPostCommitEffect::Workflow(WorkflowExecuteRunEffect {
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed)).unwrap(),
    })
}
fn asset(seed: u8) -> DesktopPostCommitEffect {
    DesktopPostCommitEffect::Asset(AssetFinalizeContentEffect::new(
        AssetContentFinalizationId::from_uuid(uuid(seed)).unwrap(),
    ))
}
fn assistant(seed: u8) -> DesktopPostCommitEffect {
    DesktopPostCommitEffect::Assistant(AssistantApplyWorkflowChangeEffect::new(
        AssistantWorkflowChangeId::from_uuid(uuid(seed)).unwrap(),
    ))
}

fn insert(
    connection: &Arc<Mutex<Connection>>,
    id: u8,
    created_at: i64,
    effect: DesktopPostCommitEffect,
) {
    let mut connection = connection.lock().unwrap();
    let transaction = connection.transaction().unwrap();
    insert_ready_post_commit_effect(&transaction, effect_id(id), effect, time(created_at)).unwrap();
    transaction.commit().unwrap();
}

#[tokio::test]
async fn sqlite_outbox_claims_oldest_and_enforces_claim_cas() {
    let (connection, adapter) = setup();
    insert(&connection, 2, 20, asset(2));
    insert(&connection, 1, 10, workflow(1));
    let instance = instance_id(8);
    let claimed = adapter.claim_next_post_commit_effect(instance, time(30)).await.unwrap().unwrap();
    assert_eq!(claimed.effect_id(), effect_id(1));
    assert_eq!(claimed.attempt_count(), 1);
    assert_eq!(
        adapter.complete_claimed_post_commit_effect(effect_id(1), instance_id(9), time(40)).await,
        Err(DesktopPostCommitEffectOutboxError::StateConflict)
    );
    adapter.complete_claimed_post_commit_effect(effect_id(1), instance, time(40)).await.unwrap();
}

#[tokio::test]
async fn sqlite_outbox_recovers_prior_asset_claim_and_ready_workflow() {
    let (connection, adapter) = setup();
    insert(&connection, 1, 10, asset(1));
    insert(&connection, 2, 20, workflow(2));
    let prior = instance_id(7);
    adapter.claim_next_post_commit_effect(prior, time(11)).await.unwrap();
    let page = adapter
        .list_recoverable_post_commit_effects(
            instance_id(8),
            None,
            DesktopPostCommitRecoveryLimit::from_u8(100).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(page.records().len(), 2);
    adapter.recover_replayable_post_commit_effect(effect_id(1), prior).await.unwrap();
    adapter
        .recover_abandoned_post_commit_effect(
            effect_id(2),
            DesktopPostCommitEffectState::Ready,
            time(30),
            DesktopPostCommitEffectAbandonReason::WorkflowInterruptedByRestart,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn sqlite_outbox_round_trips_and_releases_assistant_effects() {
    let (connection, adapter) = setup();
    insert(&connection, 3, 10, assistant(3));
    let instance = instance_id(8);

    let claimed = adapter.claim_next_post_commit_effect(instance, time(20)).await.unwrap().unwrap();
    assert!(matches!(claimed.effect(), DesktopPostCommitEffect::Assistant(_)));
    adapter.release_claimed_post_commit_effect(effect_id(3), instance).await.unwrap();

    let reclaimed =
        adapter.claim_next_post_commit_effect(instance, time(30)).await.unwrap().unwrap();
    assert_eq!(reclaimed.attempt_count(), 2);
}
