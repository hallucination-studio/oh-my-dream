use std::sync::{Arc, Mutex};

use assets::asset::{application::AssetFinalizeContentEffect, domain::AssetContentFinalizationId};
use assistant::{
    application::AssistantApplyWorkflowChangeEffect, domain::AssistantWorkflowChangeId,
};
use async_trait::async_trait;
use engine::{node_capability::WorkflowRunId, workflow::WorkflowExecuteRunEffect};
use uuid::Uuid;

use super::*;
use crate::post_commit_effect::{
    DesktopPostCommitEffectId, DesktopPostCommitEffectOutboxError, DesktopPostCommitEffectRecord,
    DesktopPostCommitRecoveryCursor, DesktopPostCommitRecoveryPage,
};

struct FakeWorkflowRecoveryImpl {
    actions: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl DesktopWorkflowRestartRecoveryInterface for FakeWorkflowRecoveryImpl {
    async fn interrupt_all_non_terminal_workflow_runs(
        &self,
    ) -> Result<(), DesktopWorkflowRestartRecoveryError> {
        self.actions.lock().expect("actions").push("interrupt");
        Ok(())
    }

    async fn workflow_effect_abandon_reason(
        &self,
        _run_id: WorkflowRunId,
    ) -> Result<DesktopPostCommitEffectAbandonReason, DesktopWorkflowRestartRecoveryError> {
        self.actions.lock().expect("actions").push("observe_workflow");
        Ok(DesktopPostCommitEffectAbandonReason::WorkflowInterruptedByRestart)
    }
}

struct FakeOutboxImpl {
    records: Mutex<Option<Vec<DesktopPostCommitEffectRecord>>>,
    actions: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl DesktopPostCommitEffectOutboxInterface for FakeOutboxImpl {
    async fn claim_next_post_commit_effect(
        &self,
        _: DesktopApplicationInstanceId,
        _: DesktopPostCommitTimestamp,
    ) -> Result<Option<DesktopPostCommitEffectRecord>, DesktopPostCommitEffectOutboxError> {
        unreachable!("normal claims are not startup recovery")
    }

    async fn complete_claimed_post_commit_effect(
        &self,
        _: DesktopPostCommitEffectId,
        _: DesktopApplicationInstanceId,
        _: DesktopPostCommitTimestamp,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        unreachable!("normal completion is not startup recovery")
    }

    async fn release_claimed_post_commit_effect(
        &self,
        _: DesktopPostCommitEffectId,
        _: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        unreachable!("normal release is not startup recovery")
    }

    async fn abandon_claimed_post_commit_effect(
        &self,
        _: DesktopPostCommitEffectId,
        _: DesktopApplicationInstanceId,
        _: DesktopPostCommitTimestamp,
        _: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        unreachable!("normal abandonment is not startup recovery")
    }

    async fn list_recoverable_post_commit_effects(
        &self,
        _: DesktopApplicationInstanceId,
        _: Option<DesktopPostCommitRecoveryCursor>,
        limit: DesktopPostCommitRecoveryLimit,
    ) -> Result<DesktopPostCommitRecoveryPage, DesktopPostCommitEffectOutboxError> {
        self.actions.lock().expect("actions").push("list");
        DesktopPostCommitRecoveryPage::try_new(
            self.records.lock().expect("records").take().unwrap_or_default(),
            None,
            limit,
        )
    }

    async fn recover_replayable_post_commit_effect(
        &self,
        _: DesktopPostCommitEffectId,
        _: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.actions.lock().expect("actions").push("replay");
        Ok(())
    }

    async fn recover_abandoned_post_commit_effect(
        &self,
        _: DesktopPostCommitEffectId,
        _: DesktopPostCommitEffectState,
        _: DesktopPostCommitTimestamp,
        reason: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        assert_eq!(reason, DesktopPostCommitEffectAbandonReason::WorkflowInterruptedByRestart);
        self.actions.lock().expect("actions").push("abandon_workflow");
        Ok(())
    }
}

struct FakeClockImpl;

#[async_trait]
impl DesktopPostCommitWorkerClockInterface for FakeClockImpl {
    fn current_post_commit_timestamp(
        &self,
    ) -> Result<DesktopPostCommitTimestamp, DesktopPostCommitWorkerClockError> {
        Ok(timestamp(50))
    }

    async fn wait_after_transient_failure(&self) {}
}

#[tokio::test]
async fn interrupts_runs_before_replaying_only_asset_and_assistant_claims() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let prior = instance_id(80);
    let records = vec![
        record(
            1,
            DesktopPostCommitEffect::Workflow(WorkflowExecuteRunEffect {
                workflow_run_id: WorkflowRunId::from_uuid(uuid(21)).expect("run"),
            }),
            DesktopPostCommitEffectState::Ready,
        ),
        record(
            2,
            DesktopPostCommitEffect::Asset(AssetFinalizeContentEffect::new(
                AssetContentFinalizationId::from_uuid(uuid(22)).expect("asset"),
            )),
            claimed(prior),
        ),
        record(
            3,
            DesktopPostCommitEffect::Assistant(AssistantApplyWorkflowChangeEffect::new(
                AssistantWorkflowChangeId::from_uuid(uuid(23)).expect("change"),
            )),
            claimed(prior),
        ),
    ];
    let recovery = DesktopStartupRecovery::new(
        instance_id(81),
        Arc::new(FakeOutboxImpl {
            records: Mutex::new(Some(records)),
            actions: Arc::clone(&actions),
        }),
        Arc::new(FakeWorkflowRecoveryImpl { actions: Arc::clone(&actions) }),
        Arc::new(FakeClockImpl),
    );

    assert_eq!(recovery.recover_before_accepting_commands().await, Ok(()));
    assert_eq!(
        *actions.lock().expect("actions"),
        vec!["interrupt", "list", "observe_workflow", "abandon_workflow", "replay", "replay"]
    );
}

#[tokio::test]
async fn rejects_ready_non_workflow_effect_without_mutating_it() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let recovery = DesktopStartupRecovery::new(
        instance_id(82),
        Arc::new(FakeOutboxImpl {
            records: Mutex::new(Some(vec![record(
                4,
                DesktopPostCommitEffect::Asset(AssetFinalizeContentEffect::new(
                    AssetContentFinalizationId::from_uuid(uuid(24)).expect("asset"),
                )),
                DesktopPostCommitEffectState::Ready,
            )])),
            actions: Arc::clone(&actions),
        }),
        Arc::new(FakeWorkflowRecoveryImpl { actions }),
        Arc::new(FakeClockImpl),
    );

    assert_eq!(
        recovery.recover_before_accepting_commands().await,
        Err(DesktopStartupRecoveryError::InvalidRecord)
    );
}

fn record(
    seed: u128,
    effect: DesktopPostCommitEffect,
    state: DesktopPostCommitEffectState,
) -> DesktopPostCommitEffectRecord {
    DesktopPostCommitEffectRecord::new(effect_id(seed), effect, state, 1, timestamp(seed as i64))
}

fn claimed(instance_id: DesktopApplicationInstanceId) -> DesktopPostCommitEffectState {
    DesktopPostCommitEffectState::Claimed { instance_id, claimed_at: timestamp(5) }
}

fn timestamp(value: i64) -> DesktopPostCommitTimestamp {
    DesktopPostCommitTimestamp::from_epoch_millis(value).expect("timestamp")
}

fn effect_id(seed: u128) -> DesktopPostCommitEffectId {
    DesktopPostCommitEffectId::from_uuid(uuid(seed)).expect("effect")
}

fn instance_id(seed: u128) -> DesktopApplicationInstanceId {
    DesktopApplicationInstanceId::from_uuid(uuid(seed)).expect("instance")
}

fn uuid(seed: u128) -> Uuid {
    Uuid::from_u128(0x123e_4567_e89b_42d3_a456_4266_0000_0000 | seed)
}
