use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use assets::asset::application::{
    AssetApplicationError, AssetAvailableContentRecoveryCursor, AssetAvailableContentRecoveryPage,
    AssetCommitContentMissingCommand, AssetCommitFinalizedContentAvailableCommand,
    AssetCommitPendingContentCommand, AssetCommitWorkflowNodeOutputPendingResult,
    AssetContentFinalization, AssetContentFinalizationRecoveryPage,
    AssetFinalizationRecoveryCursor, AssetFinalizeContentUseCase, AssetImportCommand,
    AssetImportSourceLease, AssetImportUseCase, AssetInspectedMedia, AssetListPage, AssetListQuery,
    AssetManagedContentLease, AssetPageLimit, AssetStagedContent, AssetStagedContentRecoveryCursor,
    AssetStagedContentRecoveryPage, AssetStagedContentRef,
};
use assets::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentDigest, AssetContentFinalizationId,
    AssetCreatedAt, AssetDisplayName, AssetId, AssetImportId, AssetMediaFacts, AssetMediaKind,
    AssetMediaMimeType, AssetNodeOutputKey, AssetOriginalFileName, AssetPreviewLeaseId,
};
use assets::asset::interfaces::{
    AssetClockInterface, AssetIdentityGeneratorInterface, AssetIngestTransactionInterface,
    AssetManagedContentStoreInterface, AssetMediaInspectorInterface, AssetRepositoryInterface,
};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use uuid::Uuid;

pub struct AssetImportFixtureFakeImpl {
    committed_asset: Mutex<Option<AssetAggregate>>,
    finalization: Mutex<Option<AssetContentFinalization>>,
    inspection_fails: AtomicBool,
    publish_fails: AtomicBool,
    finalization_hidden: AtomicBool,
    staged_open_count: AtomicUsize,
    events: Mutex<Vec<&'static str>>,
}

impl AssetImportFixtureFakeImpl {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            committed_asset: Mutex::new(None),
            finalization: Mutex::new(None),
            inspection_fails: AtomicBool::new(false),
            publish_fails: AtomicBool::new(false),
            finalization_hidden: AtomicBool::new(false),
            staged_open_count: AtomicUsize::new(0),
            events: Mutex::new(Vec::new()),
        })
    }

    pub fn import_use_case(self: &Arc<Self>) -> AssetImportUseCase {
        let finalizer =
            Arc::new(AssetFinalizeContentUseCase::new(self.clone(), self.clone(), self.clone()));
        AssetImportUseCase::new(
            self.clone(),
            self.clone(),
            self.clone(),
            self.clone(),
            self.clone(),
            finalizer,
        )
    }

    pub fn import_command(&self) -> AssetImportCommand {
        AssetImportCommand::new(
            project_id(),
            AssetMediaKind::Image,
            AssetDisplayName::try_new("cover").unwrap(),
            AssetOriginalFileName::try_new("cover.png").unwrap(),
            AssetImportSourceLease::new(
                Instant::now() + Duration::from_secs(60),
                Box::pin(Cursor::new(vec![1; 10])),
            ),
        )
    }

    pub fn fail_inspection(&self) {
        self.inspection_fails.store(true, Ordering::Relaxed);
    }

    pub fn fail_publish(&self) {
        self.publish_fails.store(true, Ordering::Relaxed);
    }

    pub fn hide_committed_finalization(&self) {
        self.finalization_hidden.store(true, Ordering::Relaxed);
    }

    pub fn events(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }

    pub fn committed_asset(&self) -> Option<AssetAggregate> {
        self.committed_asset.lock().unwrap().clone()
    }

    fn record(&self, event: &'static str) {
        self.events.lock().unwrap().push(event);
    }
}

#[async_trait]
impl AssetRepositoryInterface for AssetImportFixtureFakeImpl {
    async fn find_asset_by_id(
        &self,
        asset_id: AssetId,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError> {
        Ok(self.committed_asset().filter(|asset| asset.id() == asset_id))
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
        if self.finalization_hidden.load(Ordering::Relaxed) {
            return Ok(None);
        }
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
impl AssetIngestTransactionInterface for AssetImportFixtureFakeImpl {
    async fn commit_imported_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<(), AssetApplicationError> {
        self.record("commit_pending");
        *self.committed_asset.lock().unwrap() = Some(command.asset().clone());
        *self.finalization.lock().unwrap() = Some(command.finalization().clone());
        Ok(())
    }

    async fn commit_workflow_node_output_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<AssetCommitWorkflowNodeOutputPendingResult, AssetApplicationError> {
        *self.committed_asset.lock().unwrap() = Some(command.asset().clone());
        *self.finalization.lock().unwrap() = Some(command.finalization().clone());
        Ok(AssetCommitWorkflowNodeOutputPendingResult::Committed)
    }

    async fn commit_finalized_asset_content_available(
        &self,
        command: AssetCommitFinalizedContentAvailableCommand,
    ) -> Result<(), AssetApplicationError> {
        self.record("commit_available");
        *self.committed_asset.lock().unwrap() = Some(command.asset().clone());
        Ok(())
    }

    async fn commit_asset_content_missing(
        &self,
        command: AssetCommitContentMissingCommand,
    ) -> Result<(), AssetApplicationError> {
        *self.committed_asset.lock().unwrap() = Some(command.asset().clone());
        Ok(())
    }
}

#[async_trait]
impl AssetManagedContentStoreInterface for AssetImportFixtureFakeImpl {
    async fn stage_asset_content(
        &self,
        _source: AssetImportSourceLease,
        _expected_media_kind: AssetMediaKind,
        created_at: AssetCreatedAt,
    ) -> Result<AssetStagedContent, AssetApplicationError> {
        self.record("stage");
        AssetStagedContent::try_new(staged_ref(), digest(), 10, created_at)
    }

    async fn open_staged_asset_content(
        &self,
        _staged_content_ref: AssetStagedContentRef,
        deadline: Instant,
    ) -> Result<Option<AssetImportSourceLease>, AssetApplicationError> {
        let count = self.staged_open_count.fetch_add(1, Ordering::Relaxed);
        self.record(if count == 0 {
            "open_staged_for_inspection"
        } else {
            "open_staged_for_finalization"
        });
        Ok(Some(AssetImportSourceLease::new(deadline, Box::pin(Cursor::new(vec![1; 10])))))
    }

    async fn publish_staged_asset_content(
        &self,
        _staged_content_ref: AssetStagedContentRef,
        _descriptor: AssetContentDescriptor,
        _deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        self.record("publish");
        if self.publish_fails.load(Ordering::Relaxed) {
            return Err(AssetApplicationError::ManagedStorageFailed);
        }
        Ok(())
    }

    async fn open_managed_asset_content(
        &self,
        _descriptor: AssetContentDescriptor,
        _deadline: Instant,
    ) -> Result<Option<AssetManagedContentLease>, AssetApplicationError> {
        Ok(None)
    }

    async fn verify_managed_asset_content(
        &self,
        _descriptor: AssetContentDescriptor,
        _deadline: Instant,
    ) -> Result<bool, AssetApplicationError> {
        Ok(false)
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
        self.record("remove_staging");
        Ok(())
    }
}

#[async_trait]
impl AssetMediaInspectorInterface for AssetImportFixtureFakeImpl {
    async fn inspect_asset_media(
        &self,
        _source: AssetImportSourceLease,
        _expected_media_kind: AssetMediaKind,
    ) -> Result<AssetInspectedMedia, AssetApplicationError> {
        self.record("inspect");
        if self.inspection_fails.load(Ordering::Relaxed) {
            return Err(AssetApplicationError::InspectionFailed);
        }
        AssetInspectedMedia::try_new(
            AssetMediaMimeType::ImagePng,
            AssetMediaFacts::try_image(32, 32).unwrap(),
        )
    }
}

impl AssetClockInterface for AssetImportFixtureFakeImpl {
    fn current_asset_time(&self) -> Result<AssetCreatedAt, AssetApplicationError> {
        self.record("clock");
        Ok(created_at())
    }
}

impl AssetIdentityGeneratorInterface for AssetImportFixtureFakeImpl {
    fn generate_asset_id(&self) -> Result<AssetId, AssetApplicationError> {
        self.record("generate_asset_id");
        Ok(asset_id())
    }

    fn generate_asset_import_id(&self) -> Result<AssetImportId, AssetApplicationError> {
        self.record("generate_import_id");
        AssetImportId::from_uuid(uuid(4)).map_err(|_| AssetApplicationError::IdentityConflict)
    }

    fn generate_asset_content_finalization_id(
        &self,
    ) -> Result<AssetContentFinalizationId, AssetApplicationError> {
        self.record("generate_finalization_id");
        Ok(finalization_id())
    }

    fn generate_asset_preview_lease_id(
        &self,
    ) -> Result<AssetPreviewLeaseId, AssetApplicationError> {
        self.record("generate_preview_lease_id");
        AssetPreviewLeaseId::from_uuid(uuid(5)).map_err(|_| AssetApplicationError::IdentityConflict)
    }
}

fn digest() -> AssetContentDigest {
    AssetContentDigest::from_bytes([7; 32])
}

fn staged_ref() -> AssetStagedContentRef {
    AssetStagedContentRef::try_from_store_bytes(vec![1]).unwrap()
}

fn asset_id() -> AssetId {
    AssetId::from_uuid(uuid(1)).unwrap()
}

fn project_id() -> ProjectId {
    ProjectId::from_uuid(uuid(2)).unwrap()
}

fn finalization_id() -> AssetContentFinalizationId {
    AssetContentFinalizationId::from_uuid(uuid(3)).unwrap()
}

fn created_at() -> AssetCreatedAt {
    AssetCreatedAt::from_utc_milliseconds(10).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
