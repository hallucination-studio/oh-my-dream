use std::{collections::BTreeMap, sync::Mutex};

use assistant::{
    application::AssistantApplyWorkflowChangeEffect, domain::AssistantWorkflowChangeId,
};
use async_trait::async_trait;
use engine::{node_capability::WorkflowRunId, workflow::WorkflowExecuteRunEffect};
use oh_my_dream_tauri::post_commit_effect::*;
use uuid::Uuid;

#[derive(Default)]
struct DesktopPostCommitEffectOutboxContractFake {
    records: Mutex<BTreeMap<DesktopPostCommitEffectId, DesktopPostCommitEffectRecord>>,
}

impl DesktopPostCommitEffectOutboxContractFake {
    fn insert(&self, record: DesktopPostCommitEffectRecord) {
        self.records.lock().unwrap().insert(record.effect_id(), record);
    }

    fn record(&self, effect_id: DesktopPostCommitEffectId) -> DesktopPostCommitEffectRecord {
        *self.records.lock().unwrap().get(&effect_id).unwrap()
    }
}

#[async_trait]
impl DesktopPostCommitEffectOutboxInterface for DesktopPostCommitEffectOutboxContractFake {
    async fn claim_next_post_commit_effect(
        &self,
        instance_id: DesktopApplicationInstanceId,
        claimed_at: DesktopPostCommitTimestamp,
    ) -> Result<Option<DesktopPostCommitEffectRecord>, DesktopPostCommitEffectOutboxError> {
        let mut records = self.records.lock().unwrap();
        let candidate = records
            .values()
            .filter(|record| record.state() == DesktopPostCommitEffectState::Ready)
            .min_by_key(|record| (record.created_at(), record.effect_id()))
            .copied();
        let Some(record) = candidate else { return Ok(None) };
        let attempt = record
            .attempt_count()
            .checked_add(1)
            .filter(|attempt| *attempt != 0)
            .ok_or(DesktopPostCommitEffectOutboxError::StorageFailure)?;
        let claimed = DesktopPostCommitEffectRecord::new(
            record.effect_id(),
            record.effect(),
            DesktopPostCommitEffectState::Claimed { instance_id, claimed_at },
            attempt,
            record.created_at(),
        );
        records.insert(record.effect_id(), claimed);
        Ok(Some(claimed))
    }

    async fn complete_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
        completed_at: DesktopPostCommitTimestamp,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.replace_claim(
            effect_id,
            instance_id,
            DesktopPostCommitEffectState::Completed { completed_at },
        )
    }

    async fn release_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.replace_claim(effect_id, instance_id, DesktopPostCommitEffectState::Ready)
    }

    async fn abandon_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
        abandoned_at: DesktopPostCommitTimestamp,
        reason: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.replace_claim(
            effect_id,
            instance_id,
            DesktopPostCommitEffectState::Abandoned { abandoned_at, reason },
        )
    }

    async fn list_recoverable_post_commit_effects(
        &self,
        current_instance_id: DesktopApplicationInstanceId,
        cursor: Option<DesktopPostCommitRecoveryCursor>,
        limit: DesktopPostCommitRecoveryLimit,
    ) -> Result<DesktopPostCommitRecoveryPage, DesktopPostCommitEffectOutboxError> {
        let after = cursor.map(|value| (value.created_at(), value.effect_id()));
        let mut records = self
            .records
            .lock()
            .unwrap()
            .values()
            .filter(|record| match record.state() {
                DesktopPostCommitEffectState::Claimed { instance_id, .. } => {
                    instance_id != current_instance_id
                }
                DesktopPostCommitEffectState::Ready => {
                    matches!(record.effect(), DesktopPostCommitEffect::Workflow(_))
                }
                _ => false,
            })
            .filter(|record| {
                after.is_none_or(|after| (record.created_at(), record.effect_id()) > after)
            })
            .copied()
            .collect::<Vec<_>>();
        records.sort_by_key(|record| (record.created_at(), record.effect_id()));
        let has_more = records.len() > usize::from(limit.get());
        records.truncate(usize::from(limit.get()));
        let next_cursor = has_more.then(|| {
            let last = records.last().copied().unwrap();
            DesktopPostCommitRecoveryCursor::new(last.created_at(), last.effect_id())
        });
        DesktopPostCommitRecoveryPage::try_new(records, next_cursor, limit)
    }

    async fn recover_replayable_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        prior_instance_id: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        let effect = self.record(effect_id).effect();
        if matches!(effect, DesktopPostCommitEffect::Workflow(_)) {
            return Err(DesktopPostCommitEffectOutboxError::StateConflict);
        }
        self.replace_claim(effect_id, prior_instance_id, DesktopPostCommitEffectState::Ready)
    }

    async fn recover_abandoned_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        expected_state: DesktopPostCommitEffectState,
        abandoned_at: DesktopPostCommitTimestamp,
        reason: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        let mut records = self.records.lock().unwrap();
        let record = records
            .get(&effect_id)
            .copied()
            .ok_or(DesktopPostCommitEffectOutboxError::StateConflict)?;
        if record.state() != expected_state
            || !matches!(record.effect(), DesktopPostCommitEffect::Workflow(_))
            || !matches!(
                expected_state,
                DesktopPostCommitEffectState::Ready | DesktopPostCommitEffectState::Claimed { .. }
            )
        {
            return Err(DesktopPostCommitEffectOutboxError::StateConflict);
        }
        records.insert(
            effect_id,
            DesktopPostCommitEffectRecord::new(
                effect_id,
                record.effect(),
                DesktopPostCommitEffectState::Abandoned { abandoned_at, reason },
                record.attempt_count(),
                record.created_at(),
            ),
        );
        Ok(())
    }
}

impl DesktopPostCommitEffectOutboxContractFake {
    fn replace_claim(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
        state: DesktopPostCommitEffectState,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        let mut records = self.records.lock().unwrap();
        let record = records
            .get(&effect_id)
            .copied()
            .ok_or(DesktopPostCommitEffectOutboxError::StateConflict)?;
        if !matches!(record.state(), DesktopPostCommitEffectState::Claimed { instance_id: observed, .. } if observed == instance_id)
        {
            return Err(DesktopPostCommitEffectOutboxError::StateConflict);
        }
        records.insert(
            effect_id,
            DesktopPostCommitEffectRecord::new(
                effect_id,
                record.effect(),
                state,
                record.attempt_count(),
                record.created_at(),
            ),
        );
        Ok(())
    }
}

#[tokio::test]
async fn outbox_contract_claims_in_order_and_requires_the_claiming_instance() {
    let fake = DesktopPostCommitEffectOutboxContractFake::default();
    let first = record(1, 10, workflow_effect(1), DesktopPostCommitEffectState::Ready);
    let second = record(2, 20, asset_effect(2), DesktopPostCommitEffectState::Ready);
    fake.insert(second);
    fake.insert(first);
    let instance = instance_id(8);

    let claimed =
        fake.claim_next_post_commit_effect(instance, timestamp(30)).await.unwrap().unwrap();
    assert_eq!(claimed.effect_id(), effect_id(1));
    assert_eq!(claimed.attempt_count(), 1);
    assert_eq!(
        fake.complete_claimed_post_commit_effect(effect_id(1), instance_id(9), timestamp(40)).await,
        Err(DesktopPostCommitEffectOutboxError::StateConflict)
    );
    fake.complete_claimed_post_commit_effect(effect_id(1), instance, timestamp(40)).await.unwrap();
}

#[tokio::test]
async fn outbox_contract_recovers_only_prior_claims_and_ready_workflows() {
    let fake = DesktopPostCommitEffectOutboxContractFake::default();
    let prior = instance_id(7);
    let current = instance_id(8);
    fake.insert(record(
        1,
        10,
        assistant_effect(1),
        DesktopPostCommitEffectState::Claimed { instance_id: prior, claimed_at: timestamp(11) },
    ));
    fake.insert(record(2, 20, workflow_effect(2), DesktopPostCommitEffectState::Ready));
    fake.insert(record(3, 30, asset_effect(3), DesktopPostCommitEffectState::Ready));

    let page = fake
        .list_recoverable_post_commit_effects(
            current,
            None,
            DesktopPostCommitRecoveryLimit::from_u8(100).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(page.records().len(), 2);
    fake.recover_replayable_post_commit_effect(effect_id(1), prior).await.unwrap();
    fake.recover_abandoned_post_commit_effect(
        effect_id(2),
        DesktopPostCommitEffectState::Ready,
        timestamp(40),
        DesktopPostCommitEffectAbandonReason::WorkflowInterruptedByRestart,
    )
    .await
    .unwrap();
    assert_eq!(fake.record(effect_id(1)).state(), DesktopPostCommitEffectState::Ready);
    assert!(matches!(
        fake.record(effect_id(2)).state(),
        DesktopPostCommitEffectState::Abandoned { .. }
    ));
}

fn record(
    seed: u8,
    created_at: i64,
    effect: DesktopPostCommitEffect,
    state: DesktopPostCommitEffectState,
) -> DesktopPostCommitEffectRecord {
    DesktopPostCommitEffectRecord::new(effect_id(seed), effect, state, 0, timestamp(created_at))
}
fn workflow_effect(seed: u8) -> DesktopPostCommitEffect {
    DesktopPostCommitEffect::Workflow(WorkflowExecuteRunEffect {
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed)).unwrap(),
    })
}
fn asset_effect(seed: u8) -> DesktopPostCommitEffect {
    DesktopPostCommitEffect::Asset(assets::asset::application::AssetFinalizeContentEffect::new(
        assets::asset::domain::AssetContentFinalizationId::from_uuid(uuid(seed)).unwrap(),
    ))
}
fn assistant_effect(seed: u8) -> DesktopPostCommitEffect {
    DesktopPostCommitEffect::Assistant(AssistantApplyWorkflowChangeEffect::new(
        AssistantWorkflowChangeId::from_uuid(uuid(seed)).unwrap(),
    ))
}
fn effect_id(seed: u8) -> DesktopPostCommitEffectId {
    DesktopPostCommitEffectId::from_uuid(uuid(seed)).unwrap()
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
