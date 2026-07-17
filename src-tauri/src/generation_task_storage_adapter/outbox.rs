use async_trait::async_trait;
use rusqlite::{OptionalExtension, TransactionBehavior};
use tasks::generation_task::{application::*, domain::*, interfaces::*};

use super::repository::{SqliteGenerationTaskRepositoryAdapterImpl, storage};

#[async_trait]
impl GenerationTaskOutboxReaderInterface for SqliteGenerationTaskRepositoryAdapterImpl {
    async fn claim_next_generation_task_effect(
        &self,
        now: GenerationTaskTimestamp,
    ) -> Result<Option<GenerationTaskClaimedEffect>, GenerationTaskRepositoryError> {
        self.with_connection(|connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            let candidate = transaction
                .query_row(
                    "SELECT id, task_id, kind, available_at, delivery_attempts
                     FROM generation_task_outbox candidate
                     WHERE state = 'Ready' AND available_at <= ?1
                       AND NOT EXISTS (
                         SELECT 1 FROM generation_task_outbox active
                         WHERE active.task_id = candidate.task_id AND active.state = 'Claimed'
                       )
                     ORDER BY available_at, id LIMIT 1",
                    [now.as_utc_milliseconds()],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                        ))
                    },
                )
                .optional()
                .map_err(storage)?;
            let Some((id, task_id, kind, available_at, attempts)) = candidate else {
                transaction.commit().map_err(storage)?;
                return Ok(None);
            };
            let count = transaction
                .execute(
                    "UPDATE generation_task_outbox SET state = 'Claimed'
                     WHERE id = ?1 AND state = 'Ready'",
                    [id],
                )
                .map_err(storage)?;
            if count != 1 {
                return Err(GenerationTaskRepositoryError::EffectClaimConflict);
            }
            let claimed = decode_claimed(id, &task_id, &kind, available_at, attempts)?;
            transaction.commit().map_err(storage)?;
            Ok(Some(claimed))
        })
    }

    async fn reset_claimed_generation_task_effects(
        &self,
    ) -> Result<u64, GenerationTaskRepositoryError> {
        self.with_connection(|connection| {
            let count = connection
                .execute(
                    "UPDATE generation_task_outbox SET state = 'Ready' WHERE state = 'Claimed'",
                    [],
                )
                .map_err(storage)?;
            u64::try_from(count).map_err(|_| GenerationTaskRepositoryError::StorageFailure)
        })
    }
}

fn decode_claimed(
    id: i64,
    task_id: &[u8],
    kind: &str,
    available_at: i64,
    attempts: i64,
) -> Result<GenerationTaskClaimedEffect, GenerationTaskRepositoryError> {
    let id = GenerationTaskEffectId::try_new(
        u64::try_from(id).map_err(|_| GenerationTaskRepositoryError::Corruption)?,
    )
    .ok_or(GenerationTaskRepositoryError::Corruption)?;
    let task_id = GenerationTaskId::from_uuid(uuid::Uuid::from_bytes(
        task_id.try_into().map_err(|_| GenerationTaskRepositoryError::Corruption)?,
    ))
    .map_err(|_| GenerationTaskRepositoryError::Corruption)?;
    let effect = GenerationTaskEffect::restore(
        task_id,
        decode_effect_kind(kind)?,
        GenerationTaskTimestamp::from_utc_milliseconds(available_at)
            .map_err(|_| GenerationTaskRepositoryError::Corruption)?,
        u32::try_from(attempts).map_err(|_| GenerationTaskRepositoryError::Corruption)?,
    );
    Ok(GenerationTaskClaimedEffect::new(GenerationTaskEffectClaim::new(id), effect))
}

pub(super) const fn encode_effect_kind(kind: GenerationTaskEffectKind) -> &'static str {
    match kind {
        GenerationTaskEffectKind::SubmitTask => "SubmitTask",
        GenerationTaskEffectKind::PollTask => "PollTask",
        GenerationTaskEffectKind::CancelRemoteTask => "CancelRemoteTask",
        GenerationTaskEffectKind::NotifyWorkflow => "NotifyWorkflow",
    }
}

fn decode_effect_kind(
    value: &str,
) -> Result<GenerationTaskEffectKind, GenerationTaskRepositoryError> {
    match value {
        "SubmitTask" => Ok(GenerationTaskEffectKind::SubmitTask),
        "PollTask" => Ok(GenerationTaskEffectKind::PollTask),
        "CancelRemoteTask" => Ok(GenerationTaskEffectKind::CancelRemoteTask),
        "NotifyWorkflow" => Ok(GenerationTaskEffectKind::NotifyWorkflow),
        _ => Err(GenerationTaskRepositoryError::Corruption),
    }
}
