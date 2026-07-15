//! Idempotent Workflow-node produced media recording use case.

use std::sync::Arc;
use std::time::Instant;

use projects::project::domain::ProjectId;

use crate::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentFinalizationId, AssetCreatedAt,
    AssetDisplayName, AssetId, AssetManagedContentId, AssetManagedContentState, AssetMediaKind,
    AssetNodeOutputKey, AssetOrigin,
};
use crate::asset::interfaces::{
    AssetClockInterface, AssetIdentityGeneratorInterface, AssetIngestTransactionInterface,
    AssetManagedContentStoreInterface, AssetMediaInspectorInterface, AssetRepositoryInterface,
};

use super::{
    AssetApplicationError, AssetCommitPendingContentCommand,
    AssetCommitWorkflowNodeOutputPendingResult, AssetContentFinalization,
    AssetFinalizeContentCommand, AssetFinalizeContentEffect, AssetFinalizeContentUseCase,
    AssetInspectedMedia, AssetRecordNodeOutputCommand, AssetStagedContent,
    reject_elapsed_asset_deadline, run_asset_operation_before_deadline,
};

/// Records or replays one exact Workflow-node produced media output.
pub struct AssetRecordNodeOutputUseCase {
    repository: Arc<dyn AssetRepositoryInterface>,
    ingest_transaction: Arc<dyn AssetIngestTransactionInterface>,
    managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
    media_inspector: Arc<dyn AssetMediaInspectorInterface>,
    clock: Arc<dyn AssetClockInterface>,
    identity_generator: Arc<dyn AssetIdentityGeneratorInterface>,
    content_finalizer: Arc<AssetFinalizeContentUseCase>,
}

impl AssetRecordNodeOutputUseCase {
    /// Wires the exact boundaries consumed by node-output recording.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        repository: Arc<dyn AssetRepositoryInterface>,
        ingest_transaction: Arc<dyn AssetIngestTransactionInterface>,
        managed_content_store: Arc<dyn AssetManagedContentStoreInterface>,
        media_inspector: Arc<dyn AssetMediaInspectorInterface>,
        clock: Arc<dyn AssetClockInterface>,
        identity_generator: Arc<dyn AssetIdentityGeneratorInterface>,
        content_finalizer: Arc<AssetFinalizeContentUseCase>,
    ) -> Self {
        Self {
            repository,
            ingest_transaction,
            managed_content_store,
            media_inspector,
            clock,
            identity_generator,
            content_finalizer,
        }
    }

    /// Returns only an Available Asset for one exact output key and produced stream.
    pub async fn record_asset_node_output(
        &self,
        command: AssetRecordNodeOutputCommand,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        let deadline = command.deadline();
        reject_elapsed_asset_deadline(deadline)?;
        let request = PreparedNodeOutputRequest::try_from_command(
            &command,
            self.clock.current_asset_time()?,
        )?;
        let staged_content = run_asset_operation_before_deadline(
            deadline,
            self.managed_content_store.stage_node_output_asset_content(
                command.into_source_lease(),
                request.expected_media_kind,
                request.created_at,
            ),
        )
        .await?;

        match self.find_existing_output(&request, deadline).await {
            Ok(Some(existing)) => {
                return self.replay_existing_output(existing, &request, &staged_content).await;
            }
            Ok(None) => {}
            Err(error) => {
                self.remove_node_output_staging_best_effort(&staged_content, deadline).await;
                return Err(error);
            }
        }

        match self.commit_new_node_output(&request, &staged_content).await {
            Ok(NodeOutputCommitDecision::Committed(effect)) => {
                self.finalize_available_only(effect, deadline).await
            }
            Ok(NodeOutputCommitDecision::Existing(existing)) => {
                self.replay_existing_output(*existing, &request, &staged_content).await
            }
            Err(error) => {
                self.remove_node_output_staging_best_effort(&staged_content, deadline).await;
                Err(error)
            }
        }
    }

    async fn find_existing_output(
        &self,
        request: &PreparedNodeOutputRequest,
        deadline: Instant,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError> {
        run_asset_operation_before_deadline(
            deadline,
            self.repository.find_asset_by_node_output_key(request.output_key.clone()),
        )
        .await
    }

    async fn commit_new_node_output(
        &self,
        request: &PreparedNodeOutputRequest,
        staged_content: &AssetStagedContent,
    ) -> Result<NodeOutputCommitDecision, AssetApplicationError> {
        let inspected_media = self.inspect_staged_node_output(request, staged_content).await?;
        let descriptor = create_node_output_descriptor(
            request.expected_media_kind,
            staged_content,
            inspected_media,
        )?;
        let asset_id = self.identity_generator.generate_asset_id()?;
        let finalization_id = self.identity_generator.generate_asset_content_finalization_id()?;
        let pending_asset = create_pending_node_output_asset(
            request,
            asset_id,
            finalization_id,
            descriptor.clone(),
            inspected_media,
        )?;
        let finalization = AssetContentFinalization::new(
            finalization_id,
            asset_id,
            descriptor,
            staged_content.staged_content_ref().clone(),
            request.created_at,
        );
        let effect = AssetFinalizeContentEffect::new(finalization_id);
        let command =
            AssetCommitPendingContentCommand::try_new(pending_asset.clone(), finalization, effect)?;
        let result = run_asset_operation_before_deadline(
            request.deadline,
            self.ingest_transaction.commit_workflow_node_output_pending_asset(command),
        )
        .await?;
        Ok(match result {
            AssetCommitWorkflowNodeOutputPendingResult::Committed => {
                NodeOutputCommitDecision::Committed(effect)
            }
            AssetCommitWorkflowNodeOutputPendingResult::OutputKeyAlreadyBound { asset } => {
                NodeOutputCommitDecision::Existing(asset)
            }
        })
    }

    async fn inspect_staged_node_output(
        &self,
        request: &PreparedNodeOutputRequest,
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

    async fn replay_existing_output(
        &self,
        existing: AssetAggregate,
        request: &PreparedNodeOutputRequest,
        staged_content: &AssetStagedContent,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        let exact_replay = existing.project_id() == request.project_id
            && existing.media_kind() == request.expected_media_kind
            && existing.origin() == &request.expected_origin
            && existing.content_state().descriptor().digest() == staged_content.digest()
            && existing.content_state().descriptor().byte_length() == staged_content.byte_length();
        self.remove_node_output_staging_best_effort(staged_content, request.deadline).await;
        if !exact_replay {
            return Err(AssetApplicationError::NodeOutputConflict);
        }
        match existing.content_state() {
            AssetManagedContentState::Available { .. } => Ok(existing),
            AssetManagedContentState::Missing { .. } => Err(AssetApplicationError::ContentMissing),
            AssetManagedContentState::Pending { finalization_id, .. } => {
                let effect = AssetFinalizeContentEffect::new(*finalization_id);
                self.finalize_available_only(effect, request.deadline).await
            }
        }
    }

    async fn finalize_available_only(
        &self,
        effect: AssetFinalizeContentEffect,
        deadline: Instant,
    ) -> Result<AssetAggregate, AssetApplicationError> {
        let finalized = self
            .content_finalizer
            .finalize_asset_content(AssetFinalizeContentCommand::new(effect, deadline))
            .await?;
        match finalized.content_state() {
            AssetManagedContentState::Available { .. } => Ok(finalized),
            AssetManagedContentState::Pending { .. } => Err(AssetApplicationError::ContentPending),
            AssetManagedContentState::Missing { .. } => Err(AssetApplicationError::ContentMissing),
        }
    }

    async fn remove_node_output_staging_best_effort(
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
            tracing::warn!(error = ?error, "failed to remove replayed Asset node-output staging");
        }
    }
}

struct PreparedNodeOutputRequest {
    project_id: ProjectId,
    expected_media_kind: AssetMediaKind,
    display_name: AssetDisplayName,
    output_key: AssetNodeOutputKey,
    expected_origin: AssetOrigin,
    created_at: AssetCreatedAt,
    deadline: Instant,
}

impl PreparedNodeOutputRequest {
    fn try_from_command(
        command: &AssetRecordNodeOutputCommand,
        created_at: AssetCreatedAt,
    ) -> Result<Self, AssetApplicationError> {
        let expected_origin = AssetOrigin::workflow_node_output(
            command.producer().clone(),
            command.production().clone(),
            command.output_key().clone(),
        )
        .map_err(|_| AssetApplicationError::IdentityConflict)?;
        Ok(Self {
            project_id: command.project_id(),
            expected_media_kind: command.expected_media_kind(),
            display_name: command.display_name().clone(),
            output_key: command.output_key().clone(),
            expected_origin,
            created_at,
            deadline: command.deadline(),
        })
    }
}

enum NodeOutputCommitDecision {
    Committed(AssetFinalizeContentEffect),
    Existing(Box<AssetAggregate>),
}

fn create_node_output_descriptor(
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

fn create_pending_node_output_asset(
    request: &PreparedNodeOutputRequest,
    asset_id: AssetId,
    finalization_id: AssetContentFinalizationId,
    descriptor: AssetContentDescriptor,
    inspected_media: AssetInspectedMedia,
) -> Result<AssetAggregate, AssetApplicationError> {
    AssetAggregate::try_new_pending(
        asset_id,
        request.project_id,
        request.expected_media_kind,
        descriptor,
        finalization_id,
        inspected_media.media_facts(),
        request.expected_origin.clone(),
        request.display_name.clone(),
        request.created_at,
    )
    .map_err(|_| AssetApplicationError::InvalidMedia)
}
