//! Commands and result values for Asset ingest and recovery orchestration.

use std::time::Instant;

use projects::project::domain::ProjectId;

use crate::asset::domain::{
    AssetDisplayName, AssetMediaKind, AssetNodeOutputKey, AssetNodeOutputProduction,
    AssetOriginalFileName, AssetWorkflowNodeOrigin,
};

use super::{
    AssetApplicationError, AssetAvailableContentRecoveryCursor, AssetFinalizationRecoveryCursor,
    AssetFinalizeContentEffect, AssetImportSourceLease, AssetNodeOutputSourceLease, AssetPageLimit,
    AssetStagedContentRecoveryCursor,
};

/// Trusted translated input for recording one exact Workflow-node media output.
pub struct AssetRecordNodeOutputCommand {
    project_id: ProjectId,
    expected_media_kind: AssetMediaKind,
    display_name: AssetDisplayName,
    producer: AssetWorkflowNodeOrigin,
    production: AssetNodeOutputProduction,
    output_key: AssetNodeOutputKey,
    source: AssetNodeOutputSourceLease,
}

impl AssetRecordNodeOutputCommand {
    /// Creates a command only when producer and output-key coordinates agree.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        project_id: ProjectId,
        expected_media_kind: AssetMediaKind,
        display_name: AssetDisplayName,
        producer: AssetWorkflowNodeOrigin,
        production: AssetNodeOutputProduction,
        output_key: AssetNodeOutputKey,
        source: AssetNodeOutputSourceLease,
    ) -> Result<Self, AssetApplicationError> {
        if producer.workflow_run_id() != output_key.workflow_run_id()
            || producer.node_execution_id() != output_key.node_execution_id()
        {
            return Err(AssetApplicationError::IdentityConflict);
        }
        Ok(Self {
            project_id,
            expected_media_kind,
            display_name,
            producer,
            production,
            output_key,
            source,
        })
    }

    /// Returns the trusted owning Project.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    /// Returns the exact expected media kind.
    #[must_use]
    pub const fn expected_media_kind(&self) -> AssetMediaKind {
        self.expected_media_kind
    }

    /// Returns the user-visible output display name.
    #[must_use]
    pub const fn display_name(&self) -> &AssetDisplayName {
        &self.display_name
    }

    /// Returns the translated Workflow producer coordinates.
    #[must_use]
    pub const fn producer(&self) -> &AssetWorkflowNodeOrigin {
        &self.producer
    }

    /// Returns deterministic/provider production provenance.
    #[must_use]
    pub const fn production(&self) -> &AssetNodeOutputProduction {
        &self.production
    }

    /// Returns the durable node-output idempotency key.
    #[must_use]
    pub const fn output_key(&self) -> &AssetNodeOutputKey {
        &self.output_key
    }

    /// Returns the source's process-monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.source.deadline()
    }

    /// Consumes the command and returns its node-produced source lease.
    #[must_use]
    pub fn into_source_lease(self) -> AssetNodeOutputSourceLease {
        self.source
    }
}

/// Trusted input for importing one already-open local media source.
pub struct AssetImportCommand {
    project_id: ProjectId,
    expected_media_kind: AssetMediaKind,
    display_name: AssetDisplayName,
    original_file_name: AssetOriginalFileName,
    source: AssetImportSourceLease,
}

impl AssetImportCommand {
    /// Creates an import command from validated names and an opaque source lease.
    #[must_use]
    pub fn new(
        project_id: ProjectId,
        expected_media_kind: AssetMediaKind,
        display_name: AssetDisplayName,
        original_file_name: AssetOriginalFileName,
        source: AssetImportSourceLease,
    ) -> Self {
        Self { project_id, expected_media_kind, display_name, original_file_name, source }
    }

    /// Returns the trusted owning Project.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    /// Returns the media kind the imported bytes must satisfy.
    #[must_use]
    pub const fn expected_media_kind(&self) -> AssetMediaKind {
        self.expected_media_kind
    }

    /// Returns the validated user-visible display name.
    #[must_use]
    pub const fn display_name(&self) -> &AssetDisplayName {
        &self.display_name
    }

    /// Returns the validated final source file name.
    #[must_use]
    pub const fn original_file_name(&self) -> &AssetOriginalFileName {
        &self.original_file_name
    }

    /// Returns the source's process-monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.source.deadline()
    }

    /// Consumes the command and returns its one-shot opaque source lease.
    #[must_use]
    pub fn into_source_lease(self) -> AssetImportSourceLease {
        self.source
    }
}

/// Input for replaying one committed exact content finalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetFinalizeContentCommand {
    effect: AssetFinalizeContentEffect,
    deadline: Instant,
}

impl AssetFinalizeContentCommand {
    /// Creates a finalization command for one caller deadline.
    #[must_use]
    pub const fn new(effect: AssetFinalizeContentEffect, deadline: Instant) -> Self {
        Self { effect, deadline }
    }

    /// Returns the committed closed Asset effect.
    #[must_use]
    pub const fn effect(self) -> AssetFinalizeContentEffect {
        self.effect
    }

    /// Returns the caller's process-monotonic deadline.
    #[must_use]
    pub const fn deadline(self) -> Instant {
        self.deadline
    }
}

/// Bounded cursors, limit, and deadline for one reconciliation call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetReconcileContentCommand {
    deadline: Instant,
    finalization_cursor: Option<AssetFinalizationRecoveryCursor>,
    available_content_cursor: Option<AssetAvailableContentRecoveryCursor>,
    staged_content_cursor: Option<AssetStagedContentRecoveryCursor>,
    limit: AssetPageLimit,
}

impl AssetReconcileContentCommand {
    /// Creates a command and applies the frozen default limit when absent.
    #[must_use]
    pub fn new(
        deadline: Instant,
        finalization_cursor: Option<AssetFinalizationRecoveryCursor>,
        available_content_cursor: Option<AssetAvailableContentRecoveryCursor>,
        staged_content_cursor: Option<AssetStagedContentRecoveryCursor>,
        limit: Option<AssetPageLimit>,
    ) -> Self {
        Self {
            deadline,
            finalization_cursor,
            available_content_cursor,
            staged_content_cursor,
            limit: limit.unwrap_or_else(AssetPageLimit::reconciliation_default),
        }
    }

    /// Returns the caller's process-monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Returns the unfinished-finalization continuation position.
    #[must_use]
    pub const fn finalization_cursor(&self) -> Option<AssetFinalizationRecoveryCursor> {
        self.finalization_cursor
    }

    /// Returns the Available-content verification continuation position.
    #[must_use]
    pub const fn available_content_cursor(&self) -> Option<AssetAvailableContentRecoveryCursor> {
        self.available_content_cursor
    }

    /// Returns the stale staged-content continuation position.
    #[must_use]
    pub const fn staged_content_cursor(&self) -> Option<&AssetStagedContentRecoveryCursor> {
        self.staged_content_cursor.as_ref()
    }

    /// Returns the effective per-class page limit.
    #[must_use]
    pub const fn limit(&self) -> AssetPageLimit {
        self.limit
    }
}

/// Matching continuation positions after one successful reconciliation call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetReconcileContentResult {
    finalization_cursor: Option<AssetFinalizationRecoveryCursor>,
    available_content_cursor: Option<AssetAvailableContentRecoveryCursor>,
    staged_content_cursor: Option<AssetStagedContentRecoveryCursor>,
}

impl AssetReconcileContentResult {
    /// Creates a result from the three independently bounded pages.
    #[must_use]
    pub const fn new(
        finalization_cursor: Option<AssetFinalizationRecoveryCursor>,
        available_content_cursor: Option<AssetAvailableContentRecoveryCursor>,
        staged_content_cursor: Option<AssetStagedContentRecoveryCursor>,
    ) -> Self {
        Self { finalization_cursor, available_content_cursor, staged_content_cursor }
    }

    /// Returns the next unfinished-finalization cursor.
    #[must_use]
    pub const fn finalization_cursor(&self) -> Option<AssetFinalizationRecoveryCursor> {
        self.finalization_cursor
    }

    /// Returns the next Available-content verification cursor.
    #[must_use]
    pub const fn available_content_cursor(&self) -> Option<AssetAvailableContentRecoveryCursor> {
        self.available_content_cursor
    }

    /// Returns the next stale staged-content cursor.
    #[must_use]
    pub const fn staged_content_cursor(&self) -> Option<&AssetStagedContentRecoveryCursor> {
        self.staged_content_cursor.as_ref()
    }
}
