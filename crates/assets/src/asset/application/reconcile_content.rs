//! Bounded startup and worker Asset content reconciliation.

use std::sync::Arc;
use std::time::Instant;

use crate::asset::domain::{
    AssetAggregate, AssetContentMissingReason, AssetCreatedAt, AssetManagedContentState,
};
use crate::asset::interfaces::{
    AssetClockInterface, AssetIngestTransactionInterface, AssetManagedContentStoreInterface,
    AssetRepositoryInterface,
};

use super::{
    AssetApplicationError, AssetCommitContentMissingCommand, AssetContentFinalization,
    AssetFinalizeContentCommand, AssetFinalizeContentEffect, AssetFinalizeContentUseCase,
    AssetReconcileContentCommand, AssetReconcileContentResult, AssetStagedContent,
    reject_elapsed_asset_deadline, run_asset_operation_before_deadline,
};

const STALE_STAGING_AGE_MILLISECONDS: i64 = 24 * 60 * 60 * 1_000;

/// Reconciles bounded interrupted Asset publication and stale staging work.
pub struct AssetReconcileContentUseCase {
    repository: Arc<dyn AssetRepositoryInterface>,
    ingest_transaction: Arc<dyn AssetIngestTransactionInterface>,
    managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
    clock: Arc<dyn AssetClockInterface>,
    content_finalizer: Arc<AssetFinalizeContentUseCase>,
}

impl AssetReconcileContentUseCase {
    /// Wires the exact boundaries consumed by bounded content reconciliation.
    #[must_use]
    pub fn new(
        repository: Arc<dyn AssetRepositoryInterface>,
        ingest_transaction: Arc<dyn AssetIngestTransactionInterface>,
        managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
        clock: Arc<dyn AssetClockInterface>,
        content_finalizer: Arc<AssetFinalizeContentUseCase>,
    ) -> Self {
        Self { repository, ingest_transaction, managed_content_store, clock, content_finalizer }
    }

    /// Processes one bounded page from each recovery class in frozen order.
    pub async fn reconcile_asset_content(
        &self,
        command: AssetReconcileContentCommand,
    ) -> Result<AssetReconcileContentResult, AssetApplicationError> {
        let deadline = command.deadline();
        reject_elapsed_asset_deadline(deadline)?;
        let stale_cutoff = stale_staging_cutoff(self.clock.current_asset_time()?)?;

        let finalization_page = run_asset_operation_before_deadline(
            deadline,
            self.repository.list_unfinished_asset_content_finalizations(
                command.finalization_cursor(),
                command.limit(),
            ),
        )
        .await?;
        self.reconcile_unfinished_finalizations(finalization_page.finalizations(), deadline)
            .await?;

        let available_page = run_asset_operation_before_deadline(
            deadline,
            self.repository.list_available_assets_for_content_verification(
                command.available_content_cursor(),
                command.limit(),
            ),
        )
        .await?;
        self.reconcile_available_managed_content(available_page.assets(), deadline).await?;

        let staged_page = run_asset_operation_before_deadline(
            deadline,
            self.managed_content_store.list_stale_asset_staged_content(
                stale_cutoff,
                command.staged_content_cursor().cloned(),
                command.limit(),
            ),
        )
        .await?;
        self.reconcile_stale_staged_content(staged_page.staged_contents(), deadline).await?;

        Ok(AssetReconcileContentResult::new(
            finalization_page.next_cursor(),
            available_page.next_cursor(),
            staged_page.next_cursor().cloned(),
        ))
    }

    async fn reconcile_unfinished_finalizations(
        &self,
        finalizations: &[AssetContentFinalization],
        deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        for finalization in finalizations {
            reject_elapsed_asset_deadline(deadline)?;
            let command = AssetFinalizeContentCommand::new(
                AssetFinalizeContentEffect::new(finalization.finalization_id()),
                deadline,
            );
            self.content_finalizer.finalize_asset_content(command).await?;
        }
        Ok(())
    }

    async fn reconcile_available_managed_content(
        &self,
        assets: &[AssetAggregate],
        deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        for asset in assets {
            reject_elapsed_asset_deadline(deadline)?;
            if !matches!(asset.content_state(), AssetManagedContentState::Available { .. }) {
                return Err(AssetApplicationError::FinalizationFailed);
            }
            let exists = run_asset_operation_before_deadline(
                deadline,
                self.managed_content_store.verify_managed_asset_content(
                    asset.content_state().descriptor().clone(),
                    deadline,
                ),
            )
            .await?;
            if !exists {
                self.commit_available_asset_missing(asset, deadline).await?;
            }
        }
        Ok(())
    }

    async fn commit_available_asset_missing(
        &self,
        asset: &AssetAggregate,
        deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        let mut missing = asset.clone();
        missing
            .mark_content_missing(AssetContentMissingReason::ManagedContentMissing)
            .map_err(|_| AssetApplicationError::FinalizationFailed)?;
        let command = AssetCommitContentMissingCommand::try_new(missing, None)?;
        run_asset_operation_before_deadline(
            deadline,
            self.ingest_transaction.commit_asset_content_missing(command),
        )
        .await
    }

    async fn reconcile_stale_staged_content(
        &self,
        staged_contents: &[AssetStagedContent],
        deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        for staged_content in staged_contents {
            reject_elapsed_asset_deadline(deadline)?;
            let is_referenced = run_asset_operation_before_deadline(
                deadline,
                self.repository.is_asset_staged_content_referenced(
                    staged_content.staged_content_ref().clone(),
                ),
            )
            .await?;
            if !is_referenced {
                run_asset_operation_before_deadline(
                    deadline,
                    self.managed_content_store.remove_asset_staged_content(
                        staged_content.staged_content_ref().clone(),
                        deadline,
                    ),
                )
                .await?;
            }
        }
        Ok(())
    }
}

fn stale_staging_cutoff(now: AssetCreatedAt) -> Result<AssetCreatedAt, AssetApplicationError> {
    let cutoff = now.as_utc_milliseconds().saturating_sub(STALE_STAGING_AGE_MILLISECONDS).max(0);
    AssetCreatedAt::from_utc_milliseconds(cutoff)
        .map_err(|_| AssetApplicationError::IdentityConflict)
}
