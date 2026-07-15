use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use assets::asset::application::{
    AssetApplicationError, AssetAvailableContentRecoveryCursor, AssetAvailableContentRecoveryPage,
    AssetCommitContentMissingCommand, AssetCommitFinalizedContentAvailableCommand,
    AssetCommitPendingContentCommand, AssetCommitWorkflowNodeOutputPendingResult,
    AssetContentFinalization, AssetContentFinalizationRecoveryPage,
    AssetFinalizationRecoveryCursor, AssetFinalizeContentUseCase, AssetImportSourceLease,
    AssetListPage, AssetListQuery, AssetManagedContentLease, AssetPageLimit,
    AssetReconcileContentCommand, AssetReconcileContentUseCase, AssetStagedContent,
    AssetStagedContentRecoveryCursor, AssetStagedContentRecoveryPage, AssetStagedContentRef,
};
use assets::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentDigest, AssetContentFinalizationId,
    AssetCreatedAt, AssetDisplayName, AssetId, AssetImportId, AssetManagedContentId,
    AssetMediaFacts, AssetMediaKind, AssetMediaMimeType, AssetNodeOutputKey, AssetOrigin,
    AssetOriginalFileName,
};
use assets::asset::interfaces::{
    AssetClockInterface, AssetIngestTransactionInterface, AssetManagedContentStoreInterface,
    AssetRepositoryInterface,
};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use uuid::Uuid;

pub struct AssetReconcileFixtureFakeImpl {
    pending_asset: Mutex<AssetAggregate>,
    available_asset: Mutex<AssetAggregate>,
    finalization: AssetContentFinalization,
    staged_content: AssetStagedContent,
    staging_referenced: AtomicBool,
    available_verification_fails: AtomicBool,
    verification_count: AtomicUsize,
    stale_cutoff: Mutex<Option<AssetCreatedAt>>,
    events: Mutex<Vec<&'static str>>,
}

impl AssetReconcileFixtureFakeImpl {
    pub fn new() -> Arc<Self> {
        let pending = pending_asset(1, 3, 7);
        let mut available = pending_asset(11, 13, 17);
        available.mark_pending_content_available(finalization_id(17)).unwrap();
        let finalization = AssetContentFinalization::new(
            finalization_id(7),
            asset_id(1),
            descriptor(3),
            staged_ref(1),
            created_at(10),
        );
        let staged_content =
            AssetStagedContent::try_new(staged_ref(9), digest(9), 10, created_at(1)).unwrap();
        Arc::new(Self {
            pending_asset: Mutex::new(pending),
            available_asset: Mutex::new(available),
            finalization,
            staged_content,
            staging_referenced: AtomicBool::new(false),
            available_verification_fails: AtomicBool::new(false),
            verification_count: AtomicUsize::new(0),
            stale_cutoff: Mutex::new(None),
            events: Mutex::new(Vec::new()),
        })
    }

    pub fn reconcile_use_case(self: &Arc<Self>) -> AssetReconcileContentUseCase {
        let finalizer =
            Arc::new(AssetFinalizeContentUseCase::new(self.clone(), self.clone(), self.clone()));
        AssetReconcileContentUseCase::new(
            self.clone(),
            self.clone(),
            self.clone(),
            self.clone(),
            finalizer,
        )
    }

    pub fn reconcile_command(&self) -> AssetReconcileContentCommand {
        AssetReconcileContentCommand::new(
            Instant::now() + Duration::from_secs(60),
            None,
            None,
            None,
            None,
        )
    }

    pub fn mark_staging_referenced(&self) {
        self.staging_referenced.store(true, Ordering::Relaxed);
    }

    pub fn fail_available_verification(&self) {
        self.available_verification_fails.store(true, Ordering::Relaxed);
    }

    pub fn events(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }

    pub fn pending_asset(&self) -> AssetAggregate {
        self.pending_asset.lock().unwrap().clone()
    }

    pub fn available_asset(&self) -> AssetAggregate {
        self.available_asset.lock().unwrap().clone()
    }

    pub fn stale_cutoff_utc_milliseconds(&self) -> Option<i64> {
        self.stale_cutoff.lock().unwrap().map(AssetCreatedAt::as_utc_milliseconds)
    }

    fn record(&self, event: &'static str) {
        self.events.lock().unwrap().push(event);
    }
}

#[async_trait]
impl AssetRepositoryInterface for AssetReconcileFixtureFakeImpl {
    async fn find_asset_by_id(
        &self,
        asset_id: AssetId,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError> {
        self.record("find_finalization_asset");
        let asset = self.pending_asset.lock().unwrap().clone();
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
        self.record("find_finalization");
        Ok((self.finalization.finalization_id() == finalization_id)
            .then(|| self.finalization.clone()))
    }

    async fn list_unfinished_asset_content_finalizations(
        &self,
        _cursor: Option<AssetFinalizationRecoveryCursor>,
        _limit: AssetPageLimit,
    ) -> Result<AssetContentFinalizationRecoveryPage, AssetApplicationError> {
        self.record("list_finalizations");
        Ok(AssetContentFinalizationRecoveryPage::new(vec![self.finalization.clone()], None))
    }

    async fn list_available_assets_for_content_verification(
        &self,
        _cursor: Option<AssetAvailableContentRecoveryCursor>,
        _limit: AssetPageLimit,
    ) -> Result<AssetAvailableContentRecoveryPage, AssetApplicationError> {
        self.record("list_available");
        Ok(AssetAvailableContentRecoveryPage::new(
            vec![self.available_asset.lock().unwrap().clone()],
            None,
        ))
    }

    async fn is_asset_staged_content_referenced(
        &self,
        _staged_content_ref: AssetStagedContentRef,
    ) -> Result<bool, AssetApplicationError> {
        self.record("check_staging_reference");
        Ok(self.staging_referenced.load(Ordering::Relaxed))
    }
}

#[async_trait]
impl AssetIngestTransactionInterface for AssetReconcileFixtureFakeImpl {
    async fn commit_imported_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<(), AssetApplicationError> {
        *self.pending_asset.lock().unwrap() = command.asset().clone();
        Ok(())
    }

    async fn commit_workflow_node_output_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<AssetCommitWorkflowNodeOutputPendingResult, AssetApplicationError> {
        *self.pending_asset.lock().unwrap() = command.asset().clone();
        Ok(AssetCommitWorkflowNodeOutputPendingResult::Committed)
    }

    async fn commit_finalized_asset_content_available(
        &self,
        command: AssetCommitFinalizedContentAvailableCommand,
    ) -> Result<(), AssetApplicationError> {
        *self.pending_asset.lock().unwrap() = command.asset().clone();
        Ok(())
    }

    async fn commit_asset_content_missing(
        &self,
        command: AssetCommitContentMissingCommand,
    ) -> Result<(), AssetApplicationError> {
        let is_finalization = command.finalization_id().is_some();
        self.record(if is_finalization {
            "commit_finalization_missing"
        } else {
            "commit_available_missing"
        });
        if is_finalization {
            *self.pending_asset.lock().unwrap() = command.asset().clone();
        } else {
            *self.available_asset.lock().unwrap() = command.asset().clone();
        }
        Ok(())
    }
}

#[async_trait]
impl AssetManagedContentStoreInterface for AssetReconcileFixtureFakeImpl {
    async fn stage_asset_content(
        &self,
        _source: AssetImportSourceLease,
        _expected_media_kind: AssetMediaKind,
        created_at: AssetCreatedAt,
    ) -> Result<AssetStagedContent, AssetApplicationError> {
        AssetStagedContent::try_new(staged_ref(8), digest(8), 10, created_at)
    }

    async fn open_staged_asset_content(
        &self,
        _staged_content_ref: AssetStagedContentRef,
        _deadline: Instant,
    ) -> Result<Option<AssetImportSourceLease>, AssetApplicationError> {
        self.record("open_finalization_staging");
        Ok(None)
    }

    async fn publish_staged_asset_content(
        &self,
        _staged_content_ref: AssetStagedContentRef,
        _descriptor: AssetContentDescriptor,
        _deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
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
        let count = self.verification_count.fetch_add(1, Ordering::Relaxed);
        if count == 0 {
            self.record("verify_finalization_managed");
            return Ok(false);
        }
        self.record("verify_available_managed");
        if self.available_verification_fails.load(Ordering::Relaxed) {
            return Err(AssetApplicationError::ManagedStorageFailed);
        }
        Ok(false)
    }

    async fn list_stale_asset_staged_content(
        &self,
        exclusive_created_before: AssetCreatedAt,
        _cursor: Option<AssetStagedContentRecoveryCursor>,
        _limit: AssetPageLimit,
    ) -> Result<AssetStagedContentRecoveryPage, AssetApplicationError> {
        self.record("list_stale_staging");
        *self.stale_cutoff.lock().unwrap() = Some(exclusive_created_before);
        Ok(AssetStagedContentRecoveryPage::new(vec![self.staged_content.clone()], None))
    }

    async fn remove_asset_staged_content(
        &self,
        _staged_content_ref: AssetStagedContentRef,
        _deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        self.record("remove_stale_staging");
        Ok(())
    }
}

impl AssetClockInterface for AssetReconcileFixtureFakeImpl {
    fn current_asset_time(&self) -> Result<AssetCreatedAt, AssetApplicationError> {
        self.record("clock");
        Ok(created_at(100_000_000))
    }
}

fn pending_asset(asset_seed: u8, digest_seed: u8, finalization_seed: u8) -> AssetAggregate {
    AssetAggregate::try_new_pending(
        asset_id(asset_seed),
        project_id(),
        AssetMediaKind::Image,
        descriptor(digest_seed),
        finalization_id(finalization_seed),
        AssetMediaFacts::try_image(32, 32).unwrap(),
        AssetOrigin::imported(
            AssetImportId::from_uuid(uuid(asset_seed + 20)).unwrap(),
            AssetOriginalFileName::try_new("image.png").unwrap(),
        ),
        AssetDisplayName::try_new("image").unwrap(),
        created_at(10),
    )
    .unwrap()
}

fn descriptor(seed: u8) -> AssetContentDescriptor {
    AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(digest(seed)),
        digest(seed),
        10,
        AssetMediaMimeType::ImagePng,
        AssetMediaKind::Image,
    )
    .unwrap()
}

fn digest(seed: u8) -> AssetContentDigest {
    AssetContentDigest::from_bytes([seed; 32])
}

fn staged_ref(seed: u8) -> AssetStagedContentRef {
    AssetStagedContentRef::try_from_store_bytes(vec![seed]).unwrap()
}

fn asset_id(seed: u8) -> AssetId {
    AssetId::from_uuid(uuid(seed)).unwrap()
}

fn project_id() -> ProjectId {
    ProjectId::from_uuid(uuid(2)).unwrap()
}

fn finalization_id(seed: u8) -> AssetContentFinalizationId {
    AssetContentFinalizationId::from_uuid(uuid(seed)).unwrap()
}

fn created_at(value: i64) -> AssetCreatedAt {
    AssetCreatedAt::from_utc_milliseconds(value).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
