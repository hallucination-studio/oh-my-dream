use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use assets::asset::application::{
    AssetApplicationError, AssetAvailableContentRecoveryCursor, AssetAvailableContentRecoveryPage,
    AssetCommitContentMissingCommand, AssetCommitFinalizedContentAvailableCommand,
    AssetCommitPendingContentCommand, AssetCommitWorkflowNodeOutputPendingResult,
    AssetContentFinalization, AssetContentFinalizationRecoveryPage,
    AssetFinalizationRecoveryCursor, AssetFinalizeContentUseCase, AssetImportSourceLease,
    AssetListPage, AssetListQuery, AssetManagedContentLease, AssetPageLimit, AssetStagedContent,
    AssetStagedContentRecoveryCursor, AssetStagedContentRecoveryPage, AssetStagedContentRef,
};
use assets::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentFinalizationId, AssetCreatedAt, AssetId,
    AssetManagedContentState, AssetMediaKind, AssetNodeOutputKey,
};
use assets::asset::interfaces::{
    AssetIngestTransactionInterface, AssetManagedContentStoreInterface, AssetRepositoryInterface,
};
use async_trait::async_trait;

#[path = "support/asset_finalize_fault_tests.rs"]
mod asset_finalize_fault_tests;
#[path = "support/asset_finalize_values.rs"]
mod asset_finalize_values;

use asset_finalize_values::{
    command, content_finalization, digest, finalization_id, pending_asset, staged_ref,
};

#[tokio::test]
async fn staged_finalization_publishes_commits_available_and_removes_staging() {
    let fixture = FinalizationFixtureFakeImpl::new(true, false);
    let use_case = fixture.use_case();

    let asset = use_case.finalize_asset_content(command()).await.unwrap();

    assert!(matches!(asset.content_state(), AssetManagedContentState::Available { .. }));
    assert_eq!(fixture.events(), vec!["open_staged", "publish", "commit_available", "remove"]);
}

#[tokio::test]
async fn absent_staging_and_managed_bytes_commits_missing() {
    let fixture = FinalizationFixtureFakeImpl::new(false, false);
    let asset = fixture.use_case().finalize_asset_content(command()).await.unwrap();

    assert!(matches!(asset.content_state(), AssetManagedContentState::Missing { .. }));
    assert_eq!(fixture.events(), vec!["open_staged", "verify_managed", "commit_missing"]);
}

#[tokio::test]
async fn absent_staging_with_exact_managed_bytes_commits_available() {
    let fixture = FinalizationFixtureFakeImpl::new(false, true);
    let asset = fixture.use_case().finalize_asset_content(command()).await.unwrap();

    assert!(matches!(asset.content_state(), AssetManagedContentState::Available { .. }));
    assert_eq!(fixture.events(), vec!["open_staged", "verify_managed", "commit_available"]);
}

#[tokio::test]
async fn already_available_finalization_returns_without_store_mutation() {
    let fixture = FinalizationFixtureFakeImpl::new(true, false);
    fixture.make_asset_available();

    let asset = fixture.use_case().finalize_asset_content(command()).await.unwrap();

    assert!(matches!(asset.content_state(), AssetManagedContentState::Available { .. }));
    assert!(fixture.events().is_empty());
}

#[tokio::test]
async fn absent_finalization_returns_not_found() {
    let fixture = FinalizationFixtureFakeImpl::new(true, false);
    fixture.remove_finalization();

    let result = fixture.use_case().finalize_asset_content(command()).await;

    assert_eq!(result.unwrap_err(), AssetApplicationError::NotFound);
    assert!(fixture.events().is_empty());
}

#[tokio::test]
async fn publish_failure_propagates_without_committing_available() {
    let fixture = FinalizationFixtureFakeImpl::new(true, false);
    fixture.fail_publish();

    let result = fixture.use_case().finalize_asset_content(command()).await;

    assert_eq!(result.unwrap_err(), AssetApplicationError::ManagedStorageFailed);
    assert_eq!(fixture.events(), vec!["open_staged", "publish"]);
    assert!(matches!(
        fixture.asset.lock().unwrap().content_state(),
        AssetManagedContentState::Pending { .. }
    ));
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinalizationFailurePoint {
    OpenStaged,
    Publish,
    CommitAvailable,
    VerifyManaged,
    CommitMissing,
    RemoveStaging,
}

pub(crate) struct FinalizationFixtureFakeImpl {
    asset: Mutex<AssetAggregate>,
    finalization: Mutex<Option<AssetContentFinalization>>,
    staged_exists: bool,
    managed_exists: bool,
    failure_point: Mutex<Option<FinalizationFailurePoint>>,
    events: Mutex<Vec<&'static str>>,
}

impl FinalizationFixtureFakeImpl {
    fn new(staged_exists: bool, managed_exists: bool) -> Arc<Self> {
        Arc::new(Self {
            asset: Mutex::new(pending_asset()),
            finalization: Mutex::new(Some(content_finalization())),
            staged_exists,
            managed_exists,
            failure_point: Mutex::new(None),
            events: Mutex::new(Vec::new()),
        })
    }

    fn use_case(self: &Arc<Self>) -> AssetFinalizeContentUseCase {
        AssetFinalizeContentUseCase::new(self.clone(), self.clone(), self.clone())
    }

    fn events(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }

    fn make_asset_available(&self) {
        self.asset.lock().unwrap().mark_pending_content_available(finalization_id()).unwrap();
    }

    fn remove_finalization(&self) {
        *self.finalization.lock().unwrap() = None;
    }

    fn fail_publish(&self) {
        self.inject_finalization_failure_at(FinalizationFailurePoint::Publish);
    }

    pub(crate) fn inject_finalization_failure_at(&self, failure_point: FinalizationFailurePoint) {
        *self.failure_point.lock().unwrap() = Some(failure_point);
    }

    pub(crate) fn pending_content_remains(&self) -> bool {
        matches!(
            self.asset.lock().unwrap().content_state(),
            AssetManagedContentState::Pending { .. }
        )
    }

    fn fails_at(&self, failure_point: FinalizationFailurePoint) -> bool {
        *self.failure_point.lock().unwrap() == Some(failure_point)
    }

    fn record(&self, event: &'static str) {
        self.events.lock().unwrap().push(event);
    }
}

#[async_trait]
impl AssetRepositoryInterface for FinalizationFixtureFakeImpl {
    async fn find_asset_by_id(
        &self,
        asset_id: AssetId,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError> {
        let asset = self.asset.lock().unwrap().clone();
        Ok((asset.id() == asset_id).then_some(asset))
    }

    async fn find_asset_by_node_output_key(
        &self,
        _output_key: AssetNodeOutputKey,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError> {
        Ok(None)
    }

    async fn list_project_assets(
        &self,
        _query: AssetListQuery,
    ) -> Result<AssetListPage, AssetApplicationError> {
        Ok(AssetListPage::new(Vec::new(), None))
    }

    async fn find_asset_content_finalization(
        &self,
        finalization_id: AssetContentFinalizationId,
    ) -> Result<Option<AssetContentFinalization>, AssetApplicationError> {
        Ok(self
            .finalization
            .lock()
            .unwrap()
            .clone()
            .filter(|value| value.finalization_id() == finalization_id))
    }

    async fn list_unfinished_asset_content_finalizations(
        &self,
        _cursor: Option<AssetFinalizationRecoveryCursor>,
        _limit: AssetPageLimit,
    ) -> Result<AssetContentFinalizationRecoveryPage, AssetApplicationError> {
        Ok(AssetContentFinalizationRecoveryPage::new(Vec::new(), None))
    }

    async fn list_available_assets_for_content_verification(
        &self,
        _cursor: Option<AssetAvailableContentRecoveryCursor>,
        _limit: AssetPageLimit,
    ) -> Result<AssetAvailableContentRecoveryPage, AssetApplicationError> {
        Ok(AssetAvailableContentRecoveryPage::new(Vec::new(), None))
    }

    async fn is_asset_staged_content_referenced(
        &self,
        _staged_content_ref: AssetStagedContentRef,
    ) -> Result<bool, AssetApplicationError> {
        Ok(false)
    }
}

#[async_trait]
impl AssetIngestTransactionInterface for FinalizationFixtureFakeImpl {
    async fn commit_imported_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<(), AssetApplicationError> {
        *self.asset.lock().unwrap() = command.asset().clone();
        Ok(())
    }

    async fn commit_workflow_node_output_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<AssetCommitWorkflowNodeOutputPendingResult, AssetApplicationError> {
        *self.asset.lock().unwrap() = command.asset().clone();
        Ok(AssetCommitWorkflowNodeOutputPendingResult::Committed)
    }

    async fn commit_finalized_asset_content_available(
        &self,
        command: AssetCommitFinalizedContentAvailableCommand,
    ) -> Result<(), AssetApplicationError> {
        self.record("commit_available");
        if self.fails_at(FinalizationFailurePoint::CommitAvailable) {
            return Err(AssetApplicationError::IdentityConflict);
        }
        *self.asset.lock().unwrap() = command.asset().clone();
        Ok(())
    }

    async fn commit_asset_content_missing(
        &self,
        command: AssetCommitContentMissingCommand,
    ) -> Result<(), AssetApplicationError> {
        self.record("commit_missing");
        if self.fails_at(FinalizationFailurePoint::CommitMissing) {
            return Err(AssetApplicationError::IdentityConflict);
        }
        *self.asset.lock().unwrap() = command.asset().clone();
        Ok(())
    }
}

#[async_trait]
impl AssetManagedContentStoreInterface for FinalizationFixtureFakeImpl {
    async fn stage_imported_asset_content(
        &self,
        _source: AssetImportSourceLease,
        _expected_media_kind: AssetMediaKind,
        created_at: AssetCreatedAt,
    ) -> Result<AssetStagedContent, AssetApplicationError> {
        AssetStagedContent::try_new(staged_ref(), digest(), 10, created_at)
    }

    async fn stage_node_output_asset_content(
        &self,
        _source: assets::asset::application::AssetNodeOutputSourceLease,
        _expected_media_kind: AssetMediaKind,
        created_at: AssetCreatedAt,
    ) -> Result<AssetStagedContent, AssetApplicationError> {
        AssetStagedContent::try_new(staged_ref(), digest(), 10, created_at)
    }

    async fn open_staged_asset_content(
        &self,
        _staged_content_ref: AssetStagedContentRef,
        deadline: Instant,
    ) -> Result<Option<AssetImportSourceLease>, AssetApplicationError> {
        self.record("open_staged");
        if self.fails_at(FinalizationFailurePoint::OpenStaged) {
            return Err(AssetApplicationError::ManagedStorageFailed);
        }
        Ok(self
            .staged_exists
            .then(|| AssetImportSourceLease::new(deadline, Box::pin(Cursor::new(vec![1; 10])))))
    }

    async fn publish_staged_asset_content(
        &self,
        _staged_content_ref: AssetStagedContentRef,
        _descriptor: AssetContentDescriptor,
        _deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        self.record("publish");
        if self.fails_at(FinalizationFailurePoint::Publish) {
            return Err(AssetApplicationError::ManagedStorageFailed);
        }
        Ok(())
    }

    async fn open_managed_asset_content(
        &self,
        descriptor: AssetContentDescriptor,
        deadline: Instant,
    ) -> Result<Option<AssetManagedContentLease>, AssetApplicationError> {
        Ok(self.managed_exists.then(|| {
            AssetManagedContentLease::new(
                descriptor.content_id(),
                descriptor.byte_length(),
                deadline,
                Box::pin(Cursor::new(vec![1; 10])),
            )
        }))
    }

    async fn verify_managed_asset_content(
        &self,
        _descriptor: AssetContentDescriptor,
        _deadline: Instant,
    ) -> Result<bool, AssetApplicationError> {
        self.record("verify_managed");
        if self.fails_at(FinalizationFailurePoint::VerifyManaged) {
            return Err(AssetApplicationError::ManagedStorageFailed);
        }
        Ok(self.managed_exists)
    }

    async fn list_stale_asset_staged_content(
        &self,
        _exclusive_created_before: AssetCreatedAt,
        _cursor: Option<AssetStagedContentRecoveryCursor>,
        _limit: AssetPageLimit,
    ) -> Result<AssetStagedContentRecoveryPage, AssetApplicationError> {
        Ok(AssetStagedContentRecoveryPage::new(Vec::new(), None))
    }

    async fn remove_asset_staged_content(
        &self,
        _staged_content_ref: AssetStagedContentRef,
        _deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        self.record("remove");
        if self.fails_at(FinalizationFailurePoint::RemoveStaging) {
            return Err(AssetApplicationError::ManagedStorageFailed);
        }
        Ok(())
    }
}
