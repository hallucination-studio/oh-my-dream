use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use assistant::{
    application::AssistantApplyWorkflowChangeEffect,
    domain::*,
    interfaces::{AssistantApplicationError, AssistantWorkflowChangeRepositoryInterface},
};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::post_commit_effect::{
    DesktopPostCommitEffect, DesktopPostCommitEffectId, DesktopPostCommitTimestamp,
    insert_ready_post_commit_effect,
};

mod row;

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS assistant_workflow_changes (
    change_id BLOB PRIMARY KEY NOT NULL CHECK(length(change_id) = 16),
    project_id BLOB NOT NULL CHECK(length(project_id) = 16),
    session_id BLOB NOT NULL CHECK(length(session_id) = 16),
    state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 7),
    body_json BLOB NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS assistant_one_pending_approval
    ON assistant_workflow_changes(project_id, session_id) WHERE state = 2;
";

#[derive(Clone)]
pub struct SqliteAssistantWorkflowChangeRepositoryAdapterImpl {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteAssistantWorkflowChangeRepositoryAdapterImpl {
    pub fn try_new(connection: Arc<Mutex<Connection>>) -> Result<Self, AssistantApplicationError> {
        connection
            .lock()
            .map_err(|_| storage())?
            .execute_batch(CREATE_SCHEMA)
            .map_err(|_| storage())?;
        Ok(Self { connection })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, AssistantApplicationError> + Send + 'static,
    ) -> Result<T, AssistantApplicationError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection = connection.lock().map_err(|_| storage())?;
            operation(&mut connection)
        })
        .await
        .map_err(|_| storage())?
    }
}

#[async_trait]
impl AssistantWorkflowChangeRepositoryInterface
    for SqliteAssistantWorkflowChangeRepositoryAdapterImpl
{
    async fn load_assistant_workflow_change(
        &self,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        self.blocking(move |connection| load_by_id(connection, change_id)).await
    }

    async fn load_pending_assistant_workflow_change(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        self.blocking(move |connection| load_pending(connection, project_id, session_id)).await
    }

    async fn insert_assistant_workflow_change(
        &self,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        self.blocking(move |connection| insert(connection, change)).await
    }

    async fn commit_assistant_workflow_change_transition(
        &self,
        expected_state: AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        self.blocking(move |connection| update(connection, expected_state, change)).await
    }

    async fn commit_assistant_workflow_change_apply_decision(
        &self,
        expected_state: AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
        effect: AssistantApplyWorkflowChangeEffect,
    ) -> Result<(), AssistantApplicationError> {
        self.blocking(move |connection| apply_decision(connection, expected_state, change, effect))
            .await
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChangeRow {
    base_revision: u64,
    mutations: Vec<Vec<u8>>,
    aliases: Vec<(String, [u8; 16])>,
    readiness: Vec<Vec<u8>>,
    mutation_digest: [u8; 32],
    fingerprint: [u8; 32],
    lineage: LineageRow,
    review: Option<ReviewRow>,
    approval_scope_id: [u8; 16],
    continuation_ref: Option<String>,
    expires_at: i64,
    applied_receipt: Option<Vec<u8>>,
    admitted_run: Option<Vec<u8>>,
    continuation_outcome: u8,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub(super) enum LineageRow {
    UserMessage { invocation_id: [u8; 16], intent: String },
    ReviewedRepair { activation_id: [u8; 16], failed_run_id: [u8; 16] },
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReviewRow {
    contract_epoch: u32,
    model: String,
    invocation_id: [u8; 16],
    tool_call_id: String,
    verdict: u8,
    reviewed_at: i64,
}

fn load_by_id(
    connection: &Connection,
    change_id: AssistantWorkflowChangeId,
) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
    let raw = connection
        .query_row(
            "SELECT project_id, session_id, state, body_json FROM assistant_workflow_changes
             WHERE change_id = ?1",
            [change_id.as_uuid().as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|_| storage())?;
    raw.map(|(project, session, state, body)| {
        decode(
            change_id,
            ProjectId::from_uuid(uuid(project)?).ok_or_else(corrupt)?,
            AssistantSessionId::from_uuid(uuid(session)?).map_err(|_| corrupt())?,
            state,
            body,
        )
    })
    .transpose()
}

fn load_pending(
    connection: &Connection,
    project_id: ProjectId,
    session_id: AssistantSessionId,
) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
    let raw = connection
        .query_row(
            "SELECT change_id, state, body_json FROM assistant_workflow_changes
             WHERE project_id = ?1 AND session_id = ?2 AND state = 2",
            params![project_id.as_uuid().as_bytes(), session_id.as_uuid().as_bytes()],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?, row.get::<_, Vec<u8>>(2)?)),
        )
        .optional()
        .map_err(|_| storage())?;
    raw.map(|(id, state, body)| {
        decode(
            AssistantWorkflowChangeId::from_uuid(uuid(id)?).map_err(|_| corrupt())?,
            project_id,
            session_id,
            state,
            body,
        )
    })
    .transpose()
}

fn insert(
    connection: &Connection,
    change: AssistantWorkflowChangeAggregate,
) -> Result<(), AssistantApplicationError> {
    if load_by_id(connection, change.id())?.is_some() {
        return Err(AssistantApplicationError::InvalidTransition);
    }
    connection
        .execute(
            "INSERT INTO assistant_workflow_changes
             (change_id, project_id, session_id, state, body_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                change.id().as_uuid().as_bytes(),
                change.project_id().as_uuid().as_bytes(),
                change.session_id().as_uuid().as_bytes(),
                encode_state(change.state()),
                encode(&change)?,
            ],
        )
        .map(|_| ())
        .map_err(map_write_error)
}

fn update(
    connection: &Connection,
    expected_state: AssistantWorkflowChangeState,
    change: AssistantWorkflowChangeAggregate,
) -> Result<(), AssistantApplicationError> {
    let changed = connection
        .execute(
            "UPDATE assistant_workflow_changes SET state = ?1, body_json = ?2
             WHERE change_id = ?3 AND state = ?4",
            params![
                encode_state(change.state()),
                encode(&change)?,
                change.id().as_uuid().as_bytes(),
                encode_state(expected_state),
            ],
        )
        .map_err(|_| storage())?;
    if changed == 1 { Ok(()) } else { Err(AssistantApplicationError::InvalidTransition) }
}

fn apply_decision(
    connection: &mut Connection,
    expected_state: AssistantWorkflowChangeState,
    change: AssistantWorkflowChangeAggregate,
    effect: AssistantApplyWorkflowChangeEffect,
) -> Result<(), AssistantApplicationError> {
    if effect.workflow_change_id() != change.id() {
        return Err(AssistantApplicationError::InvalidTransition);
    }
    let transaction = connection.transaction().map_err(|_| storage())?;
    update(&transaction, expected_state, change.clone())?;
    let timestamp = now()?;
    insert_ready_post_commit_effect(
        &transaction,
        DesktopPostCommitEffectId::from_uuid(change.id().as_uuid()).map_err(|_| storage())?,
        DesktopPostCommitEffect::Assistant(effect),
        timestamp,
    )
    .map_err(|_| storage())?;
    transaction.commit().map_err(|_| storage())
}

fn encode(change: &AssistantWorkflowChangeAggregate) -> Result<Vec<u8>, AssistantApplicationError> {
    serde_json::to_vec(&ChangeRow::from_domain(change)).map_err(|_| storage())
}

fn decode(
    change_id: AssistantWorkflowChangeId,
    project_id: ProjectId,
    session_id: AssistantSessionId,
    state: i64,
    body: Vec<u8>,
) -> Result<AssistantWorkflowChangeAggregate, AssistantApplicationError> {
    let row: ChangeRow = serde_json::from_slice(&body).map_err(|_| corrupt())?;
    row.into_domain(change_id, project_id, session_id, decode_state(state)?)
}

fn encode_state(state: AssistantWorkflowChangeState) -> i64 {
    match state {
        AssistantWorkflowChangeState::Proposed => 0,
        AssistantWorkflowChangeState::ReviewRejected => 1,
        AssistantWorkflowChangeState::AwaitingApproval => 2,
        AssistantWorkflowChangeState::Rejected => 3,
        AssistantWorkflowChangeState::Applying => 4,
        AssistantWorkflowChangeState::Applied => 5,
        AssistantWorkflowChangeState::ApplyFailed => 6,
        AssistantWorkflowChangeState::Expired => 7,
    }
}

fn decode_state(value: i64) -> Result<AssistantWorkflowChangeState, AssistantApplicationError> {
    match value {
        0 => Ok(AssistantWorkflowChangeState::Proposed),
        1 => Ok(AssistantWorkflowChangeState::ReviewRejected),
        2 => Ok(AssistantWorkflowChangeState::AwaitingApproval),
        3 => Ok(AssistantWorkflowChangeState::Rejected),
        4 => Ok(AssistantWorkflowChangeState::Applying),
        5 => Ok(AssistantWorkflowChangeState::Applied),
        6 => Ok(AssistantWorkflowChangeState::ApplyFailed),
        7 => Ok(AssistantWorkflowChangeState::Expired),
        _ => Err(corrupt()),
    }
}

fn uuid(bytes: Vec<u8>) -> Result<Uuid, AssistantApplicationError> {
    Uuid::from_slice(&bytes).map_err(|_| corrupt())
}

fn now() -> Result<DesktopPostCommitTimestamp, AssistantApplicationError> {
    let milliseconds =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| storage())?.as_millis();
    DesktopPostCommitTimestamp::from_epoch_millis(
        i64::try_from(milliseconds).map_err(|_| storage())?,
    )
    .map_err(|_| storage())
}

fn map_write_error(error: rusqlite::Error) -> AssistantApplicationError {
    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
        AssistantApplicationError::PendingApprovalExists
    } else {
        storage()
    }
}

fn storage() -> AssistantApplicationError {
    AssistantApplicationError::ExternalBoundaryFailed
}

fn corrupt() -> AssistantApplicationError {
    AssistantApplicationError::ExternalBoundaryFailed
}
