//! Desktop translation between Generation Task media results and Asset application use cases.

#[cfg(test)]
mod tests;

use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use assets::asset::application::{
    AssetApplicationError, AssetNodeOutputRecovery, AssetNodeOutputSourceLease,
    AssetRecordNodeOutputCommand, AssetRecordNodeOutputUseCase, AssetRecoverNodeOutputUseCase,
};
use assets::asset::domain::{
    AssetDisplayName, AssetManagedContentState, AssetMediaKind, AssetNodeOutputKey,
    AssetNodeOutputProduction, AssetOriginGenerationProfileRef,
    AssetOriginNodeCapabilityContractRef, AssetOriginNodeOutputKey, AssetOriginSourceAssetId,
    AssetOriginSourceAssetIds, AssetOriginWorkflowId, AssetOriginWorkflowNodeExecutionId,
    AssetOriginWorkflowNodeId, AssetOriginWorkflowRevision, AssetOriginWorkflowRunId,
    AssetWorkflowNodeOrigin,
};
use async_trait::async_trait;
use tasks::generation_task::application::{
    GenerationTaskAssetKey, GenerationTaskAssetRecovery, GenerationTaskAvailableAsset,
    GenerationTaskBoundaryError, GenerationTaskProviderResult, GenerationTaskStoreAssetCommand,
};
use tasks::generation_task::domain::{
    GenerationTaskAssetResult, GenerationTaskRequest, GenerationTaskRequestKind,
    GenerationTaskResult,
};
use tasks::generation_task::interfaces::GenerationTaskAssetSinkInterface;

/// Asset-backed media sink consumed by Generation Task effect application.
pub struct DesktopGenerationTaskAssetSinkAdapterImpl {
    recover_node_output: Arc<AssetRecoverNodeOutputUseCase>,
    record_node_output: Arc<AssetRecordNodeOutputUseCase>,
}

impl DesktopGenerationTaskAssetSinkAdapterImpl {
    /// Wires only Asset-owned application use cases into the Task boundary.
    #[must_use]
    pub const fn new(
        recover_node_output: Arc<AssetRecoverNodeOutputUseCase>,
        record_node_output: Arc<AssetRecordNodeOutputUseCase>,
    ) -> Self {
        Self { recover_node_output, record_node_output }
    }
}

#[async_trait]
impl GenerationTaskAssetSinkInterface for DesktopGenerationTaskAssetSinkAdapterImpl {
    async fn recover_generation_task_asset(
        &self,
        key: GenerationTaskAssetKey,
    ) -> Result<GenerationTaskAssetRecovery, GenerationTaskBoundaryError> {
        match self
            .recover_node_output
            .recover_asset_node_output(output_key(key)?)
            .await
            .map_err(map_asset_error)?
        {
            AssetNodeOutputRecovery::Available(asset) => {
                available_asset(&asset, expected_media_kind(key.request_kind())?)
                    .map(GenerationTaskAssetRecovery::Available)
            }
            AssetNodeOutputRecovery::Pending { .. } => Ok(GenerationTaskAssetRecovery::Pending),
            AssetNodeOutputRecovery::SourceRequired => {
                Ok(GenerationTaskAssetRecovery::SourceRequired)
            }
        }
    }

    async fn store_generation_task_asset(
        &self,
        command: GenerationTaskStoreAssetCommand,
    ) -> Result<GenerationTaskAvailableAsset, GenerationTaskBoundaryError> {
        let record_command = record_command(&command)?;
        let asset = self
            .record_node_output
            .record_asset_node_output(record_command)
            .await
            .map_err(map_asset_error)?;
        available_asset(&asset, expected_media_kind(command.key().request_kind())?)
    }
}

fn record_command(
    command: &GenerationTaskStoreAssetCommand,
) -> Result<AssetRecordNodeOutputCommand, GenerationTaskBoundaryError> {
    let key = output_key(command.key())?;
    let origin = command.origin();
    let profile = AssetOriginGenerationProfileRef::try_new(
        command.target().generation_profile_ref().id().as_str(),
        command.target().generation_profile_ref().version().get(),
    )
    .map_err(|_| permanent())?;
    let (media_kind, display_name, bytes, production) =
        match (command.request(), command.provider_result()) {
            (GenerationTaskRequest::Image(_), GenerationTaskProviderResult::Image(value)) => (
                AssetMediaKind::Image,
                "Generated Image",
                value.clone().into_bytes(),
                AssetNodeOutputProduction::ProviderGenerated { generation_profile_ref: profile },
            ),
            (GenerationTaskRequest::Voice(_), GenerationTaskProviderResult::Voice(value)) => (
                AssetMediaKind::Audio,
                "Synthesized Speech",
                value.clone().into_bytes(),
                AssetNodeOutputProduction::ProviderGenerated { generation_profile_ref: profile },
            ),
            (GenerationTaskRequest::Video(_), GenerationTaskProviderResult::Video(value)) => (
                AssetMediaKind::Video,
                "Generated Video",
                value.clone().into_bytes(),
                video_production(command.request(), profile)?,
            ),
            _ => return Err(permanent()),
        };
    let deadline = monotonic_deadline(command)?;
    AssetRecordNodeOutputCommand::try_new(
        origin.project_id(),
        media_kind,
        AssetDisplayName::try_new(display_name).map_err(|_| permanent())?,
        AssetWorkflowNodeOrigin::new(
            AssetOriginWorkflowId::from_uuid(origin.workflow_id().as_uuid())
                .map_err(|_| permanent())?,
            AssetOriginWorkflowRevision::new(origin.workflow_revision().get())
                .map_err(|_| permanent())?,
            AssetOriginWorkflowRunId::from_uuid(origin.workflow_run_id().as_uuid())
                .map_err(|_| permanent())?,
            AssetOriginWorkflowNodeId::from_uuid(origin.workflow_node_id().as_uuid())
                .map_err(|_| permanent())?,
            AssetOriginWorkflowNodeExecutionId::from_uuid(
                origin.workflow_node_execution_id().as_uuid(),
            )
            .map_err(|_| permanent())?,
            AssetOriginNodeCapabilityContractRef::try_new(
                origin.capability_contract_ref().id().as_str(),
                origin.capability_contract_ref().version().major(),
                origin.capability_contract_ref().version().minor(),
            )
            .map_err(|_| permanent())?,
        ),
        production,
        key,
        AssetNodeOutputSourceLease::new(deadline, Box::pin(Cursor::new(bytes))),
    )
    .map_err(map_asset_error)
}

fn output_key(
    key: GenerationTaskAssetKey,
) -> Result<AssetNodeOutputKey, GenerationTaskBoundaryError> {
    let output = match key.request_kind() {
        GenerationTaskRequestKind::Image => "image",
        GenerationTaskRequestKind::Video => "video",
        GenerationTaskRequestKind::Voice => "audio",
        GenerationTaskRequestKind::Text => return Err(permanent()),
    };
    Ok(AssetNodeOutputKey::new(
        AssetOriginWorkflowRunId::from_uuid(key.workflow_run_id().as_uuid())
            .map_err(|_| permanent())?,
        AssetOriginWorkflowNodeExecutionId::from_uuid(key.workflow_node_execution_id().as_uuid())
            .map_err(|_| permanent())?,
        AssetOriginNodeOutputKey::try_new(output).map_err(|_| permanent())?,
        0,
    ))
}

const fn expected_media_kind(
    request_kind: GenerationTaskRequestKind,
) -> Result<AssetMediaKind, GenerationTaskBoundaryError> {
    match request_kind {
        GenerationTaskRequestKind::Image => Ok(AssetMediaKind::Image),
        GenerationTaskRequestKind::Video => Ok(AssetMediaKind::Video),
        GenerationTaskRequestKind::Voice => Ok(AssetMediaKind::Audio),
        GenerationTaskRequestKind::Text => Err(GenerationTaskBoundaryError::Permanent),
    }
}

fn video_production(
    request: &GenerationTaskRequest,
    generation_profile_ref: AssetOriginGenerationProfileRef,
) -> Result<AssetNodeOutputProduction, GenerationTaskBoundaryError> {
    let GenerationTaskRequest::Video(spec) = request else {
        return Err(permanent());
    };
    let source_asset_ids =
        AssetOriginSourceAssetIds::try_new(vec![AssetOriginSourceAssetId::from_asset_id(
            spec.input_image().asset_id(),
        )])
        .map_err(|_| permanent())?;
    Ok(AssetNodeOutputProduction::ProviderDerived { source_asset_ids, generation_profile_ref })
}

fn monotonic_deadline(
    command: &GenerationTaskStoreAssetCommand,
) -> Result<Instant, GenerationTaskBoundaryError> {
    let remaining = command
        .provider_deadline_at()
        .as_utc_milliseconds()
        .checked_sub(command.observed_at().as_utc_milliseconds())
        .filter(|value| *value > 0)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(transient)?;
    Instant::now().checked_add(Duration::from_millis(remaining)).ok_or_else(permanent)
}

fn available_asset(
    asset: &assets::asset::domain::AssetAggregate,
    expected_media_kind: AssetMediaKind,
) -> Result<GenerationTaskAvailableAsset, GenerationTaskBoundaryError> {
    if asset.media_kind() != expected_media_kind {
        return Err(permanent());
    }
    if !matches!(asset.content_state(), AssetManagedContentState::Available { .. }) {
        return Err(transient());
    }
    GenerationTaskAvailableAsset::try_new(GenerationTaskResult::Asset(
        GenerationTaskAssetResult::new(asset.id(), asset.media_kind()),
    ))
    .map_err(|_| permanent())
}

const fn map_asset_error(error: AssetApplicationError) -> GenerationTaskBoundaryError {
    match error {
        AssetApplicationError::ContentPending
        | AssetApplicationError::ManagedStorageFailed
        | AssetApplicationError::Cancelled
        | AssetApplicationError::DeadlineExceeded => GenerationTaskBoundaryError::Transient,
        _ => GenerationTaskBoundaryError::Permanent,
    }
}

const fn transient() -> GenerationTaskBoundaryError {
    GenerationTaskBoundaryError::Transient
}

const fn permanent() -> GenerationTaskBoundaryError {
    GenerationTaskBoundaryError::Permanent
}
