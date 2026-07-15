//! Trusted local media import use case.

use std::sync::Arc;
use std::time::Instant;

use projects::project::domain::ProjectId;

use crate::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentFinalizationId, AssetCreatedAt,
    AssetDisplayName, AssetId, AssetImportId, AssetManagedContentId, AssetMediaKind, AssetOrigin,
    AssetOriginalFileName,
};
use crate::asset::interfaces::{
    AssetClockInterface, AssetIdentityGeneratorInterface, AssetIngestTransactionInterface,
    AssetManagedContentStoreInterface, AssetMediaInspectorInterface,
};

use super::{
    AssetApplicationError, AssetCommitPendingContentCommand, AssetContentFinalization,
    AssetFinalizeContentCommand, AssetFinalizeContentEffect, AssetFinalizeContentUseCase,
    AssetImportCommand, AssetInspectedMedia, AssetStagedContent, reject_elapsed_asset_deadline,
    run_asset_operation_before_deadline,
};

/// Validates and durably imports one already-open trusted local media source.
pub struct AssetImportUseCase {
    ingest_transaction: Arc<dyn AssetIngestTransactionInterface>,
    managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
    media_inspector: Arc<dyn AssetMediaInspectorInterface>,
    clock: Arc<dyn AssetClockInterface>,
    identity_generator: Arc<dyn AssetIdentityGeneratorInterface>,
    content_finalizer: Arc<AssetFinalizeContentUseCase>,
}

impl AssetImportUseCase {
    /// Wires the exact boundaries consumed by trusted local import.
    #[must_use]
    pub fn new(
        ingest_transaction: Arc<dyn AssetIngestTransactionInterface>,
        managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
        media_inspector: Arc<dyn AssetMediaInspectorInterface>,
        clock: Arc<dyn AssetClockInterface>,
        identity_generator: Arc<dyn AssetIdentityGeneratorInterface>,
        content_finalizer: Arc<AssetFinalizeContentUseCase>,
    ) -> Self {
        Self {
            ingest_transaction,
            managed_content_store,
            media_inspector,
            clock,
            identity_generator,
            content_finalizer,
        }
    }

    /// Imports one source and immediately attempts its committed finalization once.
    pub async fn import_asset(
        &self,
        command: AssetImportCommand,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        let deadline = command.deadline();
        reject_elapsed_asset_deadline(deadline)?;
        let request = self.prepare_import_request(&command)?;
        let source = command.into_source_lease();
        let staged_content = run_asset_operation_before_deadline(
            deadline,
            self.managed_content_store.stage_asset_content(
                source,
                request.expected_media_kind,
                request.created_at,
            ),
        )
        .await?;
        match self.commit_staged_import(&request, &staged_content).await {
            Ok((pending_asset, effect)) => {
                self.attempt_inline_finalization(pending_asset, effect, deadline).await
            }
            Err(error) => {
                self.remove_uncommitted_staging_best_effort(&staged_content, deadline).await;
                Err(error)
            }
        }
    }

    fn prepare_import_request(
        &self,
        command: &AssetImportCommand,
    ) -> Result<PreparedAssetImport, AssetApplicationError> {
        Ok(PreparedAssetImport {
            project_id: command.project_id(),
            expected_media_kind: command.expected_media_kind(),
            display_name: command.display_name().clone(),
            original_file_name: command.original_file_name().clone(),
            deadline: command.deadline(),
            created_at: self.clock.current_asset_time()?,
            asset_id: self.identity_generator.generate_asset_id()?,
            import_id: self.identity_generator.generate_asset_import_id()?,
            finalization_id: self.identity_generator.generate_asset_content_finalization_id()?,
        })
    }

    async fn commit_staged_import(
        &self,
        request: &PreparedAssetImport,
        staged_content: &AssetStagedContent,
    ) -> Result<(AssetAggregate, AssetFinalizeContentEffect), AssetApplicationError> {
        let inspected_media = self.inspect_staged_content(request, staged_content).await?;
        let descriptor = create_imported_content_descriptor(
            request.expected_media_kind,
            staged_content,
            inspected_media,
        )?;
        let pending_asset = create_pending_imported_asset(
            request,
            descriptor.clone(),
            inspected_media.media_facts(),
        )?;
        let finalization = AssetContentFinalization::new(
            request.finalization_id,
            request.asset_id,
            descriptor,
            staged_content.staged_content_ref().clone(),
            request.created_at,
        );
        let effect = AssetFinalizeContentEffect::new(request.finalization_id);
        let command =
            AssetCommitPendingContentCommand::try_new(pending_asset.clone(), finalization, effect)?;
        run_asset_operation_before_deadline(
            request.deadline,
            self.ingest_transaction.commit_imported_pending_asset(command),
        )
        .await?;
        Ok((pending_asset, effect))
    }

    async fn inspect_staged_content(
        &self,
        request: &PreparedAssetImport,
        staged_content: &AssetStagedContent,
    ) -> Result<AssetInspectedMedia, AssetApplicationError> {
        let source = run_asset_operation_before_deadline(
            request.deadline,
            self.managed_content_store.open_staged_asset_content(
                staged_content.staged_content_ref().clone(),
                request.deadline,
            ),
        )
        .await?
        .ok_or(AssetApplicationError::ContentMissing)?;
        run_asset_operation_before_deadline(
            request.deadline,
            self.media_inspector.inspect_asset_media(source, request.expected_media_kind),
        )
        .await
    }

    async fn attempt_inline_finalization(
        &self,
        pending_asset: AssetAggregate,
        effect: AssetFinalizeContentEffect,
        deadline: Instant,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        let command = AssetFinalizeContentCommand::new(effect, deadline);
        match self.content_finalizer.finalize_asset_content(command).await {
            Ok(asset) => Ok(asset),
            Err(
                error @ (AssetApplicationError::ManagedStorageFailed
                | AssetApplicationError::Cancelled
                | AssetApplicationError::DeadlineExceeded),
            ) => {
                tracing::warn!(
                    asset_id = %pending_asset.id(),
                    finalization_id = %effect.finalization_id(),
                    error = ?error,
                    "deferred committed Asset finalization"
                );
                Ok(pending_asset)
            }
            Err(error) => Err(error),
        }
    }

    async fn remove_uncommitted_staging_best_effort(
        &self,
        staged_content: &AssetStagedContent,
        deadline: Instant,
    ) {
        let removal = run_asset_operation_before_deadline(
            deadline,
            self.managed_content_store
                .remove_asset_staged_content(staged_content.staged_content_ref().clone(), deadline),
        )
        .await;
        if let Err(error) = removal {
            tracing::warn!(
                error = ?error,
                "failed to remove uncommitted Asset staging"
            );
        }
    }
}

struct PreparedAssetImport {
    project_id: ProjectId,
    expected_media_kind: AssetMediaKind,
    display_name: AssetDisplayName,
    original_file_name: AssetOriginalFileName,
    deadline: Instant,
    created_at: AssetCreatedAt,
    asset_id: AssetId,
    import_id: AssetImportId,
    finalization_id: AssetContentFinalizationId,
}

fn create_imported_content_descriptor(
    expected_media_kind: AssetMediaKind,
    staged_content: &AssetStagedContent,
    inspected_media: AssetInspectedMedia,
) -> Result<AssetContentDescriptor, AssetApplicationError> {
    AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(staged_content.digest()),
        staged_content.digest(),
        staged_content.byte_length(),
        inspected_media.mime_type(),
        expected_media_kind,
    )
    .map_err(|_| AssetApplicationError::InvalidMedia)
}

fn create_pending_imported_asset(
    request: &PreparedAssetImport,
    descriptor: AssetContentDescriptor,
    media_facts: crate::asset::domain::AssetMediaFacts,
) -> Result<AssetAggregate, AssetApplicationError> {
    AssetAggregate::try_new_pending(
        request.asset_id,
        request.project_id,
        request.expected_media_kind,
        descriptor,
        request.finalization_id,
        media_facts,
        AssetOrigin::imported(request.import_id, request.original_file_name.clone()),
        request.display_name.clone(),
        request.created_at,
    )
    .map_err(|_| AssetApplicationError::InvalidMedia)
}
