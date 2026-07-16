//! Private SQLite row translation for Desktop post-commit effects.

use assets::asset::{application::AssetFinalizeContentEffect, domain::AssetContentFinalizationId};
use assistant::{
    application::AssistantApplyWorkflowChangeEffect, domain::AssistantWorkflowChangeId,
};
use engine::{node_capability::WorkflowRunId, workflow::WorkflowExecuteRunEffect};
use uuid::Uuid;

use super::{DesktopPostCommitEffectOutboxError, storage};
use crate::post_commit_effect::*;

pub(super) type RawRecord = (
    Vec<u8>,
    i64,
    Vec<u8>,
    i64,
    i64,
    i64,
    Option<Vec<u8>>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

pub(super) fn read_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawRecord> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
    ))
}

pub(super) fn decode_record(
    raw: RawRecord,
) -> Result<DesktopPostCommitEffectRecord, DesktopPostCommitEffectOutboxError> {
    let (
        effect_id,
        kind,
        owner_id,
        state,
        attempts,
        created_at,
        instance,
        claimed_at,
        completed_at,
        abandoned_at,
        reason,
    ) = raw;
    let effect_id =
        DesktopPostCommitEffectId::from_uuid(decode_uuid(&effect_id)?).map_err(|_| storage())?;
    let state = decode_state(state, instance, claimed_at, completed_at, abandoned_at, reason)?;
    Ok(DesktopPostCommitEffectRecord::new(
        effect_id,
        decode_effect(kind, decode_uuid(&owner_id)?)?,
        state,
        u32::try_from(attempts).map_err(|_| storage())?,
        timestamp(created_at)?,
    ))
}

fn decode_state(
    state: i64,
    instance: Option<Vec<u8>>,
    claimed_at: Option<i64>,
    completed_at: Option<i64>,
    abandoned_at: Option<i64>,
    reason: Option<i64>,
) -> Result<DesktopPostCommitEffectState, DesktopPostCommitEffectOutboxError> {
    match state {
        0 => Ok(DesktopPostCommitEffectState::Ready),
        1 => Ok(DesktopPostCommitEffectState::Claimed {
            instance_id: DesktopApplicationInstanceId::from_uuid(decode_uuid(
                instance.as_deref().ok_or_else(storage)?,
            )?)
            .map_err(|_| storage())?,
            claimed_at: timestamp(claimed_at.ok_or_else(storage)?)?,
        }),
        2 => Ok(DesktopPostCommitEffectState::Completed {
            completed_at: timestamp(completed_at.ok_or_else(storage)?)?,
        }),
        3 => Ok(DesktopPostCommitEffectState::Abandoned {
            abandoned_at: timestamp(abandoned_at.ok_or_else(storage)?)?,
            reason: decode_reason(reason.ok_or_else(storage)?)?,
        }),
        _ => Err(storage()),
    }
}

pub(super) fn encode_effect(effect: DesktopPostCommitEffect) -> (i64, [u8; 16]) {
    match effect {
        DesktopPostCommitEffect::Workflow(effect) => {
            (1, *effect.workflow_run_id.as_uuid().as_bytes())
        }
        DesktopPostCommitEffect::Asset(effect) => {
            (2, *effect.finalization_id().as_uuid().as_bytes())
        }
        DesktopPostCommitEffect::Assistant(effect) => {
            (3, *effect.workflow_change_id().as_uuid().as_bytes())
        }
    }
}

fn decode_effect(
    kind: i64,
    owner_id: Uuid,
) -> Result<DesktopPostCommitEffect, DesktopPostCommitEffectOutboxError> {
    match kind {
        1 => WorkflowRunId::from_uuid(owner_id)
            .map(|workflow_run_id| {
                DesktopPostCommitEffect::Workflow(WorkflowExecuteRunEffect { workflow_run_id })
            })
            .ok_or_else(storage),
        2 => AssetContentFinalizationId::from_uuid(owner_id)
            .map(AssetFinalizeContentEffect::new)
            .map(DesktopPostCommitEffect::Asset)
            .map_err(|_| storage()),
        3 => AssistantWorkflowChangeId::from_uuid(owner_id)
            .map(AssistantApplyWorkflowChangeEffect::new)
            .map(DesktopPostCommitEffect::Assistant)
            .map_err(|_| storage()),
        _ => Err(storage()),
    }
}

fn decode_uuid(bytes: &[u8]) -> Result<Uuid, DesktopPostCommitEffectOutboxError> {
    Uuid::from_slice(bytes).map_err(|_| storage())
}
fn timestamp(value: i64) -> Result<DesktopPostCommitTimestamp, DesktopPostCommitEffectOutboxError> {
    DesktopPostCommitTimestamp::from_epoch_millis(value).map_err(|_| storage())
}

pub(super) const fn encode_reason(reason: DesktopPostCommitEffectAbandonReason) -> i64 {
    match reason {
        DesktopPostCommitEffectAbandonReason::WorkflowInterruptedByRestart => 1,
        DesktopPostCommitEffectAbandonReason::OwningStateAlreadyTerminal => 2,
    }
}

fn decode_reason(
    value: i64,
) -> Result<DesktopPostCommitEffectAbandonReason, DesktopPostCommitEffectOutboxError> {
    match value {
        1 => Ok(DesktopPostCommitEffectAbandonReason::WorkflowInterruptedByRestart),
        2 => Ok(DesktopPostCommitEffectAbandonReason::OwningStateAlreadyTerminal),
        _ => Err(storage()),
    }
}
