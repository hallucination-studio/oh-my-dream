//! Consumer-owned Asset substitution boundaries.

use std::time::Instant;

use async_trait::async_trait;

use crate::asset::application::{
    AssetApplicationError, AssetAvailableContentRecoveryCursor, AssetAvailableContentRecoveryPage,
    AssetCommitContentMissingCommand, AssetCommitFinalizedContentAvailableCommand,
    AssetCommitPendingContentCommand, AssetCommitWorkflowNodeOutputPendingResult,
    AssetContentFinalization, AssetContentFinalizationRecoveryPage,
    AssetFinalizationRecoveryCursor, AssetImportSourceLease, AssetInspectedMedia, AssetListPage,
    AssetListQuery, AssetManagedContentLease, AssetNodeOutputSourceLease, AssetPageLimit,
    AssetStagedContent, AssetStagedContentRecoveryCursor, AssetStagedContentRecoveryPage,
    AssetStagedContentRef,
};
use crate::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentFinalizationId, AssetCreatedAt, AssetId,
    AssetImportId, AssetMediaKind, AssetNodeOutputKey, AssetPreviewLeaseId,
};

/// Read-only persistence boundary consumed by Asset use cases.
#[async_trait]
pub trait AssetRepositoryInterface: Send + Sync {
    /// Finds one Asset globally by identity without applying Project visibility.
    async fn find_asset_by_id(
        &self,
        asset_id: AssetId,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError>;

    /// Finds the Asset bound to one exact Workflow-node output key.
    async fn find_asset_by_node_output_key(
        &self,
        output_key: AssetNodeOutputKey,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError>;

    /// Lists one stable descending Project Asset page.
    async fn list_project_assets(
        &self,
        query: AssetListQuery,
    ) -> Result<AssetListPage, AssetApplicationError>;

    /// Finds one exact durable content finalization.
    async fn find_asset_content_finalization(
        &self,
        finalization_id: AssetContentFinalizationId,
    ) -> Result<Option<AssetContentFinalization>, AssetApplicationError>;

    /// Lists one ascending page of unfinished content finalizations.
    async fn list_unfinished_asset_content_finalizations(
        &self,
        cursor: Option<AssetFinalizationRecoveryCursor>,
        limit: AssetPageLimit,
    ) -> Result<AssetContentFinalizationRecoveryPage, AssetApplicationError>;

    /// Lists one ascending page of Available Assets requiring content verification.
    async fn list_available_assets_for_content_verification(
        &self,
        cursor: Option<AssetAvailableContentRecoveryCursor>,
        limit: AssetPageLimit,
    ) -> Result<AssetAvailableContentRecoveryPage, AssetApplicationError>;

    /// Reports whether an unfinished finalization owns the exact staged object.
    async fn is_asset_staged_content_referenced(
        &self,
        staged_content_ref: AssetStagedContentRef,
    ) -> Result<bool, AssetApplicationError>;
}

/// Atomic Asset, finalization, output-key, and closed-effect write boundary.
#[async_trait]
pub trait AssetIngestTransactionInterface: Send + Sync {
    /// Atomically commits one imported Pending Asset, finalization, and Asset effect.
    async fn commit_imported_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<(), AssetApplicationError>;

    /// Atomically commits or returns one exact Workflow-node output-key binding.
    async fn commit_workflow_node_output_pending_asset(
        &self,
        command: AssetCommitPendingContentCommand,
    ) -> Result<AssetCommitWorkflowNodeOutputPendingResult, AssetApplicationError>;

    /// Atomically persists exact availability and completes its finalization.
    async fn commit_finalized_asset_content_available(
        &self,
        command: AssetCommitFinalizedContentAvailableCommand,
    ) -> Result<(), AssetApplicationError>;

    /// Atomically persists one approved Missing transition and optional finalization completion.
    async fn commit_asset_content_missing(
        &self,
        command: AssetCommitContentMissingCommand,
    ) -> Result<(), AssetApplicationError>;
}

/// Opaque staging and immutable managed-byte boundary consumed by Asset use cases.
#[async_trait]
pub trait AssetManagedContentStoreInterface: Send + Sync {
    /// Stages one source once while calculating its exact digest and length.
    async fn stage_imported_asset_content(
        &self,
        source: AssetImportSourceLease,
        expected_media_kind: AssetMediaKind,
        created_at: AssetCreatedAt,
    ) -> Result<AssetStagedContent, AssetApplicationError>;

    /// Stages one node-produced source once while calculating its exact digest and length.
    async fn stage_node_output_asset_content(
        &self,
        source: AssetNodeOutputSourceLease,
        expected_media_kind: AssetMediaKind,
        created_at: AssetCreatedAt,
    ) -> Result<AssetStagedContent, AssetApplicationError>;

    /// Opens one exact staged object as a deadline-bounded one-shot stream.
    async fn open_staged_asset_content(
        &self,
        staged_content_ref: AssetStagedContentRef,
        deadline: Instant,
    ) -> Result<Option<AssetImportSourceLease>, AssetApplicationError>;

    /// Idempotently verifies and publishes one staged object as exact managed content.
    async fn publish_staged_asset_content(
        &self,
        staged_content_ref: AssetStagedContentRef,
        descriptor: AssetContentDescriptor,
        deadline: Instant,
    ) -> Result<(), AssetApplicationError>;

    /// Opens exact managed bytes without disclosing their storage location.
    async fn open_managed_asset_content(
        &self,
        descriptor: AssetContentDescriptor,
        deadline: Instant,
    ) -> Result<Option<AssetManagedContentLease>, AssetApplicationError>;

    /// Verifies whether managed bytes match one exact descriptor.
    async fn verify_managed_asset_content(
        &self,
        descriptor: AssetContentDescriptor,
        deadline: Instant,
    ) -> Result<bool, AssetApplicationError>;

    /// Lists one ascending page of staged objects older than an exclusive cutoff.
    async fn list_stale_asset_staged_content(
        &self,
        exclusive_created_before: AssetCreatedAt,
        cursor: Option<AssetStagedContentRecoveryCursor>,
        limit: AssetPageLimit,
    ) -> Result<AssetStagedContentRecoveryPage, AssetApplicationError>;

    /// Idempotently removes one exact stale staged object.
    async fn remove_asset_staged_content(
        &self,
        staged_content_ref: AssetStagedContentRef,
        deadline: Instant,
    ) -> Result<(), AssetApplicationError>;
}

/// Verified media-inspection boundary consumed by Asset ingest use cases.
#[async_trait]
pub trait AssetMediaInspectorInterface: Send + Sync {
    /// Sniffs MIME and extracts facts from one deadline-bounded staged stream.
    async fn inspect_asset_media(
        &self,
        source: AssetImportSourceLease,
        expected_media_kind: AssetMediaKind,
    ) -> Result<AssetInspectedMedia, AssetApplicationError>;
}

/// Deterministic Asset time source.
pub trait AssetClockInterface: Send + Sync {
    /// Observes one validated non-negative Asset creation time.
    fn current_asset_time(&self) -> Result<AssetCreatedAt, AssetApplicationError>;
}

/// Authoritative source of all Asset-owned UUIDv4 identities.
pub trait AssetIdentityGeneratorInterface: Send + Sync {
    /// Generates one logical Asset identity.
    fn generate_asset_id(&self) -> Result<AssetId, AssetApplicationError>;

    /// Generates one trusted import identity.
    fn generate_asset_import_id(&self) -> Result<AssetImportId, AssetApplicationError>;

    /// Generates one exact content-finalization identity.
    fn generate_asset_content_finalization_id(
        &self,
    ) -> Result<AssetContentFinalizationId, AssetApplicationError>;

    /// Generates one short-lived preview permission identity.
    fn generate_asset_preview_lease_id(&self)
    -> Result<AssetPreviewLeaseId, AssetApplicationError>;
}
