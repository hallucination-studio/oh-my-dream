//! SQLite adapter for the closed Desktop post-commit effect outbox.

mod row;
#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
#[cfg(test)]
use rusqlite::Transaction;
use rusqlite::{Connection, OptionalExtension, params};

use super::*;
#[cfg(test)]
use row::encode_effect;
use row::{decode_record, encode_reason, read_record};

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS desktop_post_commit_effects (
    effect_id BLOB PRIMARY KEY NOT NULL CHECK(length(effect_id) = 16),
    effect_kind INTEGER NOT NULL CHECK(effect_kind IN (1, 2, 3)),
    owner_id BLOB NOT NULL CHECK(length(owner_id) = 16),
    state INTEGER NOT NULL CHECK(state IN (0, 1, 2, 3)),
    attempt_count INTEGER NOT NULL CHECK(attempt_count >= 0 AND attempt_count <= 4294967295),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    claiming_instance_id BLOB CHECK(claiming_instance_id IS NULL OR length(claiming_instance_id) = 16),
    claimed_at INTEGER CHECK(claimed_at IS NULL OR claimed_at >= 0),
    completed_at INTEGER CHECK(completed_at IS NULL OR completed_at >= 0),
    abandoned_at INTEGER CHECK(abandoned_at IS NULL OR abandoned_at >= 0),
    abandon_reason INTEGER CHECK(abandon_reason IS NULL OR abandon_reason IN (1, 2)),
    CHECK(
        (state = 0 AND claiming_instance_id IS NULL AND claimed_at IS NULL AND completed_at IS NULL AND abandoned_at IS NULL AND abandon_reason IS NULL) OR
        (state = 1 AND claiming_instance_id IS NOT NULL AND claimed_at IS NOT NULL AND completed_at IS NULL AND abandoned_at IS NULL AND abandon_reason IS NULL) OR
        (state = 2 AND claiming_instance_id IS NULL AND claimed_at IS NULL AND completed_at IS NOT NULL AND abandoned_at IS NULL AND abandon_reason IS NULL) OR
        (state = 3 AND claiming_instance_id IS NULL AND claimed_at IS NULL AND completed_at IS NULL AND abandoned_at IS NOT NULL AND abandon_reason IS NOT NULL)
    )
);
CREATE INDEX IF NOT EXISTS desktop_post_commit_effect_ready_order
    ON desktop_post_commit_effects(state, created_at, effect_id);
";

/// SQLite implementation of the Desktop effect outbox boundary.
#[derive(Clone)]
pub struct SqliteDesktopPostCommitEffectOutboxAdapterImpl {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteDesktopPostCommitEffectOutboxAdapterImpl {
    /// Initializes the closed outbox table on the shared metadata connection.
    pub fn try_new(
        connection: Arc<Mutex<Connection>>,
    ) -> Result<Self, DesktopPostCommitEffectOutboxError> {
        connection
            .lock()
            .map_err(|_| storage())?
            .execute_batch(CREATE_SCHEMA)
            .map_err(|_| storage())?;
        Ok(Self { connection })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, DesktopPostCommitEffectOutboxError>
        + Send
        + 'static,
    ) -> Result<T, DesktopPostCommitEffectOutboxError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection = connection.lock().map_err(|_| storage())?;
            operation(&mut connection)
        })
        .await
        .map_err(|_| storage())?
    }
}

/// Inserts one Ready effect inside an owning business transaction.
#[cfg(test)]
pub(crate) fn insert_ready_post_commit_effect(
    transaction: &Transaction<'_>,
    effect_id: DesktopPostCommitEffectId,
    effect: DesktopPostCommitEffect,
    created_at: DesktopPostCommitTimestamp,
) -> Result<(), DesktopPostCommitEffectOutboxError> {
    let (kind, owner_id) = encode_effect(effect);
    transaction
        .execute(
            "INSERT INTO desktop_post_commit_effects
             (effect_id, effect_kind, owner_id, state, attempt_count, created_at)
             VALUES (?1, ?2, ?3, 0, 0, ?4)",
            params![
                effect_id.as_uuid().as_bytes().as_slice(),
                kind,
                owner_id.as_slice(),
                created_at.epoch_millis()
            ],
        )
        .map_err(|_| storage())?;
    Ok(())
}

#[async_trait]
impl DesktopPostCommitEffectOutboxInterface for SqliteDesktopPostCommitEffectOutboxAdapterImpl {
    async fn claim_next_post_commit_effect(
        &self,
        instance_id: DesktopApplicationInstanceId,
        claimed_at: DesktopPostCommitTimestamp,
    ) -> Result<Option<DesktopPostCommitEffectRecord>, DesktopPostCommitEffectOutboxError> {
        self.blocking(move |connection| claim_next(connection, instance_id, claimed_at)).await
    }

    async fn complete_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
        completed_at: DesktopPostCommitTimestamp,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.blocking(move |connection| {
            update_current_claim(
                connection,
                effect_id,
                instance_id,
                2,
                Some(completed_at.epoch_millis()),
                None,
                None,
            )
        })
        .await
    }

    async fn release_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.blocking(move |connection| {
            update_current_claim(connection, effect_id, instance_id, 0, None, None, None)
        })
        .await
    }

    async fn abandon_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        instance_id: DesktopApplicationInstanceId,
        abandoned_at: DesktopPostCommitTimestamp,
        reason: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.blocking(move |connection| {
            update_current_claim(
                connection,
                effect_id,
                instance_id,
                3,
                None,
                Some(abandoned_at.epoch_millis()),
                Some(encode_reason(reason)),
            )
        })
        .await
    }

    async fn list_recoverable_post_commit_effects(
        &self,
        current_instance_id: DesktopApplicationInstanceId,
        cursor: Option<DesktopPostCommitRecoveryCursor>,
        limit: DesktopPostCommitRecoveryLimit,
    ) -> Result<DesktopPostCommitRecoveryPage, DesktopPostCommitEffectOutboxError> {
        self.blocking(move |connection| {
            list_recoverable(connection, current_instance_id, cursor, limit)
        })
        .await
    }

    async fn recover_replayable_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        prior_instance_id: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.blocking(move |connection| {
            changed_one(connection.execute(
                "UPDATE desktop_post_commit_effects
                 SET state = 0, claiming_instance_id = NULL, claimed_at = NULL
                 WHERE effect_id = ?1 AND effect_kind IN (2, 3) AND state = 1
                   AND claiming_instance_id = ?2",
                params![
                    effect_id.as_uuid().as_bytes().as_slice(),
                    prior_instance_id.as_uuid().as_bytes().as_slice()
                ],
            ))
        })
        .await
    }

    async fn recover_abandoned_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        expected_state: DesktopPostCommitEffectState,
        abandoned_at: DesktopPostCommitTimestamp,
        reason: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.blocking(move |connection| {
            recover_abandoned(connection, effect_id, expected_state, abandoned_at, reason)
        })
        .await
    }
}

fn claim_next(
    connection: &mut Connection,
    instance_id: DesktopApplicationInstanceId,
    claimed_at: DesktopPostCommitTimestamp,
) -> Result<Option<DesktopPostCommitEffectRecord>, DesktopPostCommitEffectOutboxError> {
    let transaction = connection.transaction().map_err(|_| storage())?;
    let candidate = transaction
        .query_row(
            "SELECT effect_id FROM desktop_post_commit_effects
             WHERE state = 0 ORDER BY created_at, effect_id LIMIT 1",
            [],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(|_| storage())?;
    let Some(effect_id) = candidate else {
        transaction.commit().map_err(|_| storage())?;
        return Ok(None);
    };
    let changed = transaction
        .execute(
            "UPDATE desktop_post_commit_effects
             SET state = 1, attempt_count = attempt_count + 1,
                 claiming_instance_id = ?2, claimed_at = ?3
             WHERE effect_id = ?1 AND state = 0 AND attempt_count < 4294967295",
            params![
                effect_id,
                instance_id.as_uuid().as_bytes().as_slice(),
                claimed_at.epoch_millis()
            ],
        )
        .map_err(|_| storage())?;
    if changed != 1 {
        return Err(DesktopPostCommitEffectOutboxError::StateConflict);
    }
    let record = load_record_by_bytes(&transaction, &effect_id)?.ok_or_else(storage)?;
    transaction.commit().map_err(|_| storage())?;
    Ok(Some(record))
}

fn update_current_claim(
    connection: &Connection,
    effect_id: DesktopPostCommitEffectId,
    instance_id: DesktopApplicationInstanceId,
    target_state: i64,
    completed_at: Option<i64>,
    abandoned_at: Option<i64>,
    reason: Option<i64>,
) -> Result<(), DesktopPostCommitEffectOutboxError> {
    changed_one(connection.execute(
        "UPDATE desktop_post_commit_effects
         SET state = ?3, claiming_instance_id = NULL, claimed_at = NULL,
             completed_at = ?4, abandoned_at = ?5, abandon_reason = ?6
         WHERE effect_id = ?1 AND state = 1 AND claiming_instance_id = ?2",
        params![
            effect_id.as_uuid().as_bytes().as_slice(),
            instance_id.as_uuid().as_bytes().as_slice(),
            target_state,
            completed_at,
            abandoned_at,
            reason
        ],
    ))
}

fn list_recoverable(
    connection: &Connection,
    current_instance_id: DesktopApplicationInstanceId,
    cursor: Option<DesktopPostCommitRecoveryCursor>,
    limit: DesktopPostCommitRecoveryLimit,
) -> Result<DesktopPostCommitRecoveryPage, DesktopPostCommitEffectOutboxError> {
    let (cursor_time, cursor_id) = cursor
        .map(|cursor| {
            (cursor.created_at().epoch_millis(), *cursor.effect_id().as_uuid().as_bytes())
        })
        .unwrap_or((-1, [0_u8; 16]));
    let fetch_limit = i64::from(limit.get()) + 1;
    let mut statement = connection
        .prepare(
            "SELECT effect_id, effect_kind, owner_id, state, attempt_count, created_at,
                    claiming_instance_id, claimed_at, completed_at, abandoned_at, abandon_reason
             FROM desktop_post_commit_effects
             WHERE ((state = 1 AND claiming_instance_id <> ?1) OR (state = 0 AND effect_kind = 1))
               AND (created_at > ?2 OR (created_at = ?2 AND effect_id > ?3))
             ORDER BY created_at, effect_id LIMIT ?4",
        )
        .map_err(|_| storage())?;
    let rows = statement
        .query_map(
            params![
                current_instance_id.as_uuid().as_bytes().as_slice(),
                cursor_time,
                cursor_id.as_slice(),
                fetch_limit
            ],
            read_record,
        )
        .map_err(|_| storage())?;
    let mut records = rows
        .map(|row| row.map_err(|_| storage()).and_then(decode_record))
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = records.len() > usize::from(limit.get());
    records.truncate(usize::from(limit.get()));
    let next_cursor = has_more.then(|| records.last().copied()).flatten().map(|record| {
        DesktopPostCommitRecoveryCursor::new(record.created_at(), record.effect_id())
    });
    DesktopPostCommitRecoveryPage::try_new(records, next_cursor, limit)
}

fn recover_abandoned(
    connection: &Connection,
    effect_id: DesktopPostCommitEffectId,
    expected_state: DesktopPostCommitEffectState,
    abandoned_at: DesktopPostCommitTimestamp,
    reason: DesktopPostCommitEffectAbandonReason,
) -> Result<(), DesktopPostCommitEffectOutboxError> {
    let (state, instance_id, claimed_at) = match expected_state {
        DesktopPostCommitEffectState::Ready => (0, None, None),
        DesktopPostCommitEffectState::Claimed { instance_id, claimed_at } => {
            (1, Some(*instance_id.as_uuid().as_bytes()), Some(claimed_at.epoch_millis()))
        }
        _ => return Err(DesktopPostCommitEffectOutboxError::StateConflict),
    };
    changed_one(connection.execute(
        "UPDATE desktop_post_commit_effects
         SET state = 3, claiming_instance_id = NULL, claimed_at = NULL,
             abandoned_at = ?5, abandon_reason = ?6
         WHERE effect_id = ?1 AND effect_kind = 1 AND state = ?2
           AND claiming_instance_id IS ?3 AND claimed_at IS ?4",
        params![
            effect_id.as_uuid().as_bytes().as_slice(),
            state,
            instance_id.map(|bytes| bytes.to_vec()),
            claimed_at,
            abandoned_at.epoch_millis(),
            encode_reason(reason)
        ],
    ))
}

fn load_record_by_bytes(
    connection: &Connection,
    effect_id: &[u8],
) -> Result<Option<DesktopPostCommitEffectRecord>, DesktopPostCommitEffectOutboxError> {
    connection
        .query_row(
            "SELECT effect_id, effect_kind, owner_id, state, attempt_count, created_at,
                    claiming_instance_id, claimed_at, completed_at, abandoned_at, abandon_reason
             FROM desktop_post_commit_effects WHERE effect_id = ?1",
            [effect_id],
            read_record,
        )
        .optional()
        .map_err(|_| storage())?
        .map(decode_record)
        .transpose()
}

fn changed_one(result: rusqlite::Result<usize>) -> Result<(), DesktopPostCommitEffectOutboxError> {
    match result.map_err(|_| storage())? {
        1 => Ok(()),
        _ => Err(DesktopPostCommitEffectOutboxError::StateConflict),
    }
}

const fn storage() -> DesktopPostCommitEffectOutboxError {
    DesktopPostCommitEffectOutboxError::StorageFailure
}
