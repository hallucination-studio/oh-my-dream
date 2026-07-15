//! Project-isolated Asset query and content-access use cases.

use std::sync::Arc;

use crate::asset::domain::{AssetAggregate, AssetManagedContentState};
use crate::asset::interfaces::{
    AssetClockInterface, AssetIdentityGeneratorInterface, AssetManagedContentStoreInterface,
    AssetRepositoryInterface,
};

use super::{
    AssetApplicationError, AssetGetQuery, AssetIssuePreviewCommand, AssetListPage, AssetListQuery,
    AssetPreviewLease, AssetResolveContentQuery, AssetResolvedContent,
    reject_elapsed_asset_deadline, run_asset_operation_before_deadline,
};

/// Returns one Project-visible Asset without changing its state.
pub struct AssetGetUseCase {
    repository: Arc<dyn AssetRepositoryInterface>,
}

impl AssetGetUseCase {
    /// Wires the Asset read boundary.
    #[must_use]
    pub fn new(repository: Arc<dyn AssetRepositoryInterface>) -> Self {
        Self { repository }
    }

    /// Returns one Asset after exact Project visibility validation.
    pub async fn get_asset(
        &self,
        query: AssetGetQuery,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        find_visible_asset(self.repository.as_ref(), query.project_id(), query.asset_id()).await
    }
}

/// Returns stable repository-owned Project Asset pages.
pub struct AssetListUseCase {
    repository: Arc<dyn AssetRepositoryInterface>,
}

impl AssetListUseCase {
    /// Wires the Asset list boundary.
    #[must_use]
    pub fn new(repository: Arc<dyn AssetRepositoryInterface>) -> Self {
        Self { repository }
    }

    /// Returns the exact stable page for one validated query.
    pub async fn list_assets(
        &self,
        query: AssetListQuery,
    ) -> Result<AssetListPage, AssetApplicationError> {
        self.repository.list_project_assets(query).await
    }
}

/// Resolves one exact Available Asset to opaque managed bytes.
pub struct AssetResolveContentUseCase {
    repository: Arc<dyn AssetRepositoryInterface>,
    managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
}

impl AssetResolveContentUseCase {
    /// Wires Asset metadata and managed-content read boundaries.
    #[must_use]
    pub fn new(
        repository: Arc<dyn AssetRepositoryInterface>,
        managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
    ) -> Self {
        Self { repository, managed_content_store }
    }

    /// Returns one exact descriptor and matching one-shot content lease.
    pub async fn resolve_asset_content(
        &self,
        query: AssetResolveContentQuery,
    ) -> Result<AssetResolvedContent, AssetApplicationError> {
        reject_elapsed_asset_deadline(query.deadline())?;
        let asset =
            find_visible_asset(self.repository.as_ref(), query.project_id(), query.asset_id())
                .await?;
        if asset.media_kind() != query.expected_media_kind() {
            return Err(AssetApplicationError::MediaKindMismatch {
                expected: query.expected_media_kind(),
                observed: asset.media_kind(),
            });
        }
        let descriptor = available_descriptor(&asset)?.clone();
        let lease = run_asset_operation_before_deadline(
            query.deadline(),
            self.managed_content_store
                .open_managed_asset_content(descriptor.clone(), query.deadline()),
        )
        .await?
        .ok_or(AssetApplicationError::ContentMissing)?;
        Ok(AssetResolvedContent::new(descriptor, lease))
    }
}

/// Issues one five-minute preview permission for exact Available content.
pub struct AssetIssuePreviewUseCase {
    repository: Arc<dyn AssetRepositoryInterface>,
    clock: Arc<dyn AssetClockInterface>,
    identity_generator: Arc<dyn AssetIdentityGeneratorInterface>,
}

impl AssetIssuePreviewUseCase {
    /// Wires Asset lookup, time, and identity boundaries.
    #[must_use]
    pub fn new(
        repository: Arc<dyn AssetRepositoryInterface>,
        clock: Arc<dyn AssetClockInterface>,
        identity_generator: Arc<dyn AssetIdentityGeneratorInterface>,
    ) -> Self {
        Self { repository, clock, identity_generator }
    }

    /// Issues a permission bound to the Asset's exact current content identity.
    pub async fn issue_asset_preview(
        &self,
        command: AssetIssuePreviewCommand,
    ) -> Result<AssetPreviewLease, AssetApplicationError> {
        let asset =
            find_visible_asset(self.repository.as_ref(), command.project_id(), command.asset_id())
                .await?;
        let content_id = available_descriptor(&asset)?.content_id();
        let issued_at = self.clock.current_asset_time()?;
        let lease_id = self.identity_generator.generate_asset_preview_lease_id()?;
        AssetPreviewLease::try_new(
            lease_id,
            asset.project_id(),
            asset.id(),
            content_id,
            issued_at.as_utc_milliseconds(),
        )
    }
}

async fn find_visible_asset(
    repository: &dyn AssetRepositoryInterface,
    project_id: projects::project::domain::ProjectId,
    asset_id: crate::asset::domain::AssetId,
) -> Result<AssetAggregate, AssetApplicationError> {
    let asset =
        repository.find_asset_by_id(asset_id).await?.ok_or(AssetApplicationError::NotFound)?;
    if asset.project_id() != project_id {
        return Err(AssetApplicationError::NotVisible);
    }
    Ok(asset)
}

fn available_descriptor(
    asset: &AssetAggregate,
) -> Result<&crate::asset::domain::AssetContentDescriptor, AssetApplicationError> {
    match asset.content_state() {
        AssetManagedContentState::Available { descriptor } => Ok(descriptor),
        AssetManagedContentState::Pending { .. } => Err(AssetApplicationError::ContentPending),
        AssetManagedContentState::Missing { .. } => Err(AssetApplicationError::ContentMissing),
    }
}
