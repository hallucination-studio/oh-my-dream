mod operations;
mod row;

use std::sync::{Arc, Mutex};

use assets::asset::{
    application::*,
    domain::*,
    interfaces::{AssetIngestTransactionInterface, AssetRepositoryInterface},
};
use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::post_commit_effect::{
    DesktopPostCommitEffect, DesktopPostCommitEffectId, DesktopPostCommitTimestamp,
    insert_ready_post_commit_effect,
};
use operations::*;
use row::{AssetRow, FinalizationRow, decode_asset, decode_finalization, encode_asset};

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS assets (
    asset_id BLOB PRIMARY KEY NOT NULL CHECK(length(asset_id) = 16),
    project_id BLOB NOT NULL CHECK(length(project_id) = 16),
    media_kind INTEGER NOT NULL CHECK(media_kind BETWEEN 0 AND 2),
    content_state INTEGER NOT NULL CHECK(content_state BETWEEN 0 AND 2),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    body_json BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS assets_project_list
    ON assets(project_id, created_at DESC, asset_id DESC);
CREATE INDEX IF NOT EXISTS assets_available_recovery
    ON assets(content_state, created_at, asset_id);
CREATE TABLE IF NOT EXISTS asset_content_finalizations (
    finalization_id BLOB PRIMARY KEY NOT NULL CHECK(length(finalization_id) = 16),
    asset_id BLOB NOT NULL CHECK(length(asset_id) = 16),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    completed INTEGER NOT NULL CHECK(completed IN (0, 1)),
    body_json BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS asset_finalization_recovery
    ON asset_content_finalizations(completed, created_at, finalization_id);
CREATE TABLE IF NOT EXISTS asset_node_output_bindings (
    workflow_run_id BLOB NOT NULL CHECK(length(workflow_run_id) = 16),
    node_execution_id BLOB NOT NULL CHECK(length(node_execution_id) = 16),
    output_key TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0 AND ordinal <= 4294967295),
    asset_id BLOB NOT NULL CHECK(length(asset_id) = 16),
    PRIMARY KEY(workflow_run_id, node_execution_id, output_key, ordinal),
    UNIQUE(asset_id)
);
";

#[derive(Clone)]
pub struct SqliteAssetRepositoryAdapterImpl {
    connection: Arc<Mutex<Connection>>,
}

#[derive(Clone)]
pub struct SqliteAssetIngestTransactionAdapterImpl {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteAssetRepositoryAdapterImpl {
    pub fn try_new(connection: Arc<Mutex<Connection>>) -> Result<Self, AssetApplicationError> {
        initialize(&connection)?;
        Ok(Self { connection })
    }
}

impl SqliteAssetIngestTransactionAdapterImpl {
    pub fn try_new(connection: Arc<Mutex<Connection>>) -> Result<Self, AssetApplicationError> {
        initialize(&connection)?;
        Ok(Self { connection })
    }
}

fn initialize(connection: &Arc<Mutex<Connection>>) -> Result<(), AssetApplicationError> {
    connection
        .lock()
        .map_err(|_| storage())?
        .execute_batch(CREATE_SCHEMA)
        .map_err(|_| storage())?;
    Ok(())
}

async fn blocking<T: Send + 'static>(
    connection: Arc<Mutex<Connection>>,
    operation: impl FnOnce(&mut Connection) -> Result<T, AssetApplicationError> + Send + 'static,
) -> Result<T, AssetApplicationError> {
    tokio::task::spawn_blocking(move || {
        let mut connection = connection.lock().map_err(|_| storage())?;
        operation(&mut connection)
    })
    .await
    .map_err(|_| storage())?
}

#[async_trait]
impl AssetRepositoryInterface for SqliteAssetRepositoryAdapterImpl {
    async fn find_asset_by_id(
        &self,
        asset_id: AssetId,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| load_asset(connection, asset_id))
            .await
    }

    async fn find_asset_by_node_output_key(
        &self,
        output_key: AssetNodeOutputKey,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| {
            load_by_output_key(connection, &output_key)
        })
        .await
    }

    async fn list_project_assets(
        &self,
        query: AssetListQuery,
    ) -> Result<AssetListPage, AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| list_assets(connection, query))
            .await
    }

    async fn find_asset_content_finalization(
        &self,
        finalization_id: AssetContentFinalizationId,
    ) -> Result<Option<AssetContentFinalization>, AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| {
            load_finalization(connection, finalization_id).map(|value| value.map(|row| row.value))
        })
        .await
    }

    async fn list_unfinished_asset_content_finalizations(
        &self,
        cursor: Option<AssetFinalizationRecoveryCursor>,
        limit: AssetPageLimit,
    ) -> Result<AssetContentFinalizationRecoveryPage, AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| {
            list_finalizations(connection, cursor, limit)
        })
        .await
    }

    async fn list_available_assets_for_content_verification(
        &self,
        cursor: Option<AssetAvailableContentRecoveryCursor>,
        limit: AssetPageLimit,
    ) -> Result<AssetAvailableContentRecoveryPage, AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| {
            list_available(connection, cursor, limit)
        })
        .await
    }

    async fn is_asset_staged_content_referenced(
        &self,
        staged_content_ref: AssetStagedContentRef,
    ) -> Result<bool, AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| {
            let count: i64 = connection
                .query_row(
                    "SELECT count(*) FROM asset_content_finalizations
                     WHERE completed = 0 AND json_extract(body_json, '$.staged_ref') = ?1",
                    [hex(staged_content_ref.as_store_bytes())],
                    |row| row.get(0),
                )
                .map_err(|_| storage())?;
            Ok(count > 0)
        })
        .await
    }
}

#[async_trait]
impl AssetIngestTransactionInterface for SqliteAssetIngestTransactionAdapterImpl {
    async fn commit_imported_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<(), AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| {
            commit_pending(connection, command, None).map(|_| ())
        })
        .await
    }

    async fn commit_workflow_node_output_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<AssetCommitWorkflowNodeOutputPendingResult, AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| {
            let AssetOrigin::WorkflowNodeOutput(origin) = command.asset().origin() else {
                return Err(AssetApplicationError::IdentityConflict);
            };
            commit_pending(connection, command.clone(), Some(origin.output_key().clone()))
        })
        .await
    }

    async fn commit_finalized_asset_content_available(
        &self,
        command: AssetCommitFinalizedContentAvailableCommand,
    ) -> Result<(), AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| {
            commit_available(connection, command)
        })
        .await
    }

    async fn commit_asset_content_missing(
        &self,
        command: AssetCommitContentMissingCommand,
    ) -> Result<(), AssetApplicationError> {
        blocking(Arc::clone(&self.connection), move |connection| {
            commit_missing(connection, command)
        })
        .await
    }
}

fn storage() -> AssetApplicationError {
    AssetApplicationError::ManagedStorageFailed
}

fn identity() -> AssetApplicationError {
    AssetApplicationError::IdentityConflict
}

fn uuid(bytes: &[u8]) -> Result<Uuid, AssetApplicationError> {
    Uuid::from_slice(bytes).map_err(|_| storage())
}

fn same_immutable(left: &AssetAggregate, right: &AssetAggregate) -> bool {
    left.id() == right.id()
        && left.project_id() == right.project_id()
        && left.media_kind() == right.media_kind()
        && left.media_facts() == right.media_facts()
        && left.origin() == right.origin()
        && left.display_name() == right.display_name()
        && left.created_at() == right.created_at()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests;
