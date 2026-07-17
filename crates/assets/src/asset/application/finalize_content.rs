//! Idempotent exact managed-content finalization use case.

use std::sync::Arc;
use std::time::Instant;

use crate::asset::domain::{
    AssetAggregate, AssetContentFinalizationId, AssetContentMissingReason, AssetManagedContentState,
};
use crate::asset::interfaces::{
    AssetIngestTransactionInterface, AssetManagedContentStoreInterface, AssetRepositoryInterface,
};

use super::{
    AssetApplicationError, AssetCommitContentMissingCommand,
    AssetCommitFinalizedContentAvailableCommand, AssetContentFinalization,
    AssetFinalizeContentCommand, run_asset_operation_before_deadline,
};

/// Publishes or recovers one already-committed exact Asset finalization.
pub struct AssetFinalizeContentUseCase {
    repository: Arc<dyn AssetRepositoryInterface>,
    ingest_transaction: Arc<dyn AssetIngestTransactionInterface>,
    managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
}

impl AssetFinalizeContentUseCase {
    /// Wires the exact persistence and managed-byte boundaries used by finalization.
    #[must_use]
    pub fn new(
        repository: Arc<dyn AssetRepositoryInterface>,
        ingest_transaction: Arc<dyn AssetIngestTransactionInterface>,
        managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
    ) -> Self {
        Self { repository, ingest_transaction, managed_content_store }
    }

    /// Publishes or recovers one exact finalization idempotently before its deadline.
    pub async fn finalize_asset_content(
        &self,
        command: AssetFinalizeContentCommand,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        let deadline = command.deadline();
        let finalization_id = command.effect().finalization_id();
        let finalization = self.find_finalization(finalization_id, deadline).await?;
        let asset = self.find_finalization_asset(&finalization, deadline).await?;
        self.finalize_matching_asset(asset, finalization, deadline).await
    }

    async fn find_finalization(
        &self,
        finalization_id: AssetContentFinalizationId,
        deadline: Instant,
    ) -> Result<AssetContentFinalization, AssetApplicationError> {
        run_asset_operation_before_deadline(
            deadline,
            self.repository.find_asset_content_finalization(finalization_id),
        )
        .await?
        .ok_or(AssetApplicationError::NotFound)
    }

    async fn find_finalization_asset(
        &self,
        finalization: &AssetContentFinalization,
        deadline: Instant,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        let asset = run_asset_operation_before_deadline(
            deadline,
            self.repository.find_asset_by_id(finalization.asset_id()),
        )
        .await?
        .ok_or(AssetApplicationError::FinalizationFailed)?;
        if asset.id() != finalization.asset_id()
            || asset.content_state().descriptor() != finalization.descriptor()
        {
            return Err(AssetApplicationError::FinalizationFailed);
        }
        Ok(asset)
    }

    async fn finalize_matching_asset(
        &self,
        asset: AssetAggregate,
        finalization: AssetContentFinalization,
        deadline: Instant,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        match asset.content_state() {
            AssetManagedContentState::Available { .. }
            | AssetManagedContentState::Missing { .. } => Ok(asset),
            AssetManagedContentState::Pending { finalization_id, .. }
                if *finalization_id == finalization.finalization_id() =>
            {
                self.finalize_pending_asset(asset, finalization, deadline).await
            }
            AssetManagedContentState::Pending { .. } => {
                Err(AssetApplicationError::FinalizationFailed)
            }
        }
    }

    async fn finalize_pending_asset(
        &self,
        asset: AssetAggregate,
        finalization: AssetContentFinalization,
        deadline: Instant,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        let staged_exists = run_asset_operation_before_deadline(
            deadline,
            self.managed_content_store
                .open_staged_asset_content(finalization.staged_content_ref().clone(), deadline),
        )
        .await?
        .is_some();
        if staged_exists {
            self.publish_pending_asset(asset, finalization, deadline).await
        } else {
            self.recover_pending_asset_without_staging(asset, finalization, deadline).await
        }
    }

    async fn publish_pending_asset(
        &self,
        asset: AssetAggregate,
        finalization: AssetContentFinalization,
        deadline: Instant,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        run_asset_operation_before_deadline(
            deadline,
            self.managed_content_store.publish_staged_asset_content(
                finalization.staged_content_ref().clone(),
                finalization.descriptor().clone(),
                deadline,
            ),
        )
        .await?;
        let available = self.commit_pending_asset_available(asset, &finalization, deadline).await?;
        self.remove_published_staging_best_effort(&finalization, deadline).await;
        Ok(available)
    }

    async fn recover_pending_asset_without_staging(
        &self,
        asset: AssetAggregate,
        finalization: AssetContentFinalization,
        deadline: Instant,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        let managed_exists = run_asset_operation_before_deadline(
            deadline,
            self.managed_content_store
                .verify_managed_asset_content(finalization.descriptor().clone(), deadline),
        )
        .await?;
        if managed_exists {
            self.commit_pending_asset_available(asset, &finalization, deadline).await
        } else {
            self.commit_pending_asset_missing(asset, &finalization, deadline).await
        }
    }

    async fn commit_pending_asset_available(
        &self,
        mut asset: AssetAggregate,
        finalization: &AssetContentFinalization,
        deadline: Instant,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        asset
            .mark_pending_content_available(finalization.finalization_id())
            .map_err(|_| AssetApplicationError::FinalizationFailed)?;
        let command = AssetCommitFinalizedContentAvailableCommand::try_new(
            asset.clone(),
            finalization.finalization_id(),
        )?;
        run_asset_operation_before_deadline(
            deadline,
            self.ingest_transaction.commit_finalized_asset_content_available(command),
        )
        .await?;
        Ok(asset)
    }

    async fn commit_pending_asset_missing(
        &self,
        mut asset: AssetAggregate,
        finalization: &AssetContentFinalization,
        deadline: Instant,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        asset
            .mark_content_missing(AssetContentMissingReason::FinalizationSourceMissing)
            .map_err(|_| AssetApplicationError::FinalizationFailed)?;
        let command = AssetCommitContentMissingCommand::try_new(
            asset.clone(),
            Some(finalization.finalization_id()),
        )?;
        run_asset_operation_before_deadline(
            deadline,
            self.ingest_transaction.commit_asset_content_missing(command),
        )
        .await?;
        Ok(asset)
    }

    async fn remove_published_staging_best_effort(
        &self,
        finalization: &AssetContentFinalization,
        deadline: Instant,
    ) {
        let removal = run_asset_operation_before_deadline(
            deadline,
            self.managed_content_store
                .remove_asset_staged_content(finalization.staged_content_ref().clone(), deadline),
        )
        .await;
        if let Err(error) = removal {
            tracing::warn!(
                asset_id = %finalization.asset_id(),
                finalization_id = %finalization.finalization_id(),
                error = ?error,
                "failed to remove published Asset staging"
            );
        }
    }
}
