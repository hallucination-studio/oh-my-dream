use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use assets::asset::{
    application::{
        AssetApplicationError, AssetGetQuery, AssetGetUseCase, AssetListQuery, AssetListUseCase,
        AssetPageLimit,
    },
    domain::{AssetAggregate, AssetManagedContentState, AssetMediaKind},
};
use assistant::interfaces::{
    AssistantApplicationError, AssistantNodeCapabilityCatalogReaderInterface,
    AssistantNodeCapabilityCatalogRequest, AssistantNodeCapabilityCatalogSnapshot,
    AssistantWorkspaceSnapshot, AssistantWorkspaceSnapshotReaderInterface,
    AssistantWorkspaceSnapshotRequest,
};
use async_trait::async_trait;
use engine::{
    workflow::{
        WorkflowAggregateRepositoryInterface, WorkflowApplicationError, WorkflowGetCurrentUseCase,
        WorkflowListActiveRunsUseCase, WorkflowRunAggregate, WorkflowRunRepositoryInterface,
    },
    workflow_graph::WorkflowAggregate,
};
use nodes::{
    GenerationProfileAvailabilityState, GenerationProfileListForCapabilityQuery,
    GenerationProfileListForCapabilityUseCase, NodeCapabilityListUseCase,
};
use serde_json::{Value, json};

/// Bounded authoritative Assistant workspace projection over public application queries.
#[derive(Clone)]
pub struct DesktopAssistantWorkspaceBridgeAdapterImpl<A, R> {
    get_current: Arc<WorkflowGetCurrentUseCase<A>>,
    list_active_runs: Arc<WorkflowListActiveRunsUseCase<R>>,
    get_asset: Arc<AssetGetUseCase>,
    list_assets: Arc<AssetListUseCase>,
    list_capabilities: Arc<NodeCapabilityListUseCase>,
    list_profiles: Arc<GenerationProfileListForCapabilityUseCase>,
}

impl<A, R> DesktopAssistantWorkspaceBridgeAdapterImpl<A, R> {
    /// Wires only bounded public query use cases.
    #[must_use]
    pub const fn new(
        get_current: Arc<WorkflowGetCurrentUseCase<A>>,
        list_active_runs: Arc<WorkflowListActiveRunsUseCase<R>>,
        get_asset: Arc<AssetGetUseCase>,
        list_assets: Arc<AssetListUseCase>,
        list_capabilities: Arc<NodeCapabilityListUseCase>,
        list_profiles: Arc<GenerationProfileListForCapabilityUseCase>,
    ) -> Self {
        Self {
            get_current,
            list_active_runs,
            get_asset,
            list_assets,
            list_capabilities,
            list_profiles,
        }
    }
}

#[async_trait]
impl<A, R> AssistantWorkspaceSnapshotReaderInterface
    for DesktopAssistantWorkspaceBridgeAdapterImpl<A, R>
where
    A: WorkflowAggregateRepositoryInterface + 'static,
    R: WorkflowRunRepositoryInterface + 'static,
{
    async fn read_assistant_workspace_snapshot(
        &self,
        request: AssistantWorkspaceSnapshotRequest,
    ) -> Result<AssistantWorkspaceSnapshot, AssistantApplicationError> {
        let workflow = match self.get_current.get_current_workflow(request.project_id).await {
            Ok(value) => Some(value),
            Err(WorkflowApplicationError::WorkflowNotFound { .. }) => None,
            Err(_) => return Err(AssistantApplicationError::ExternalBoundaryFailed),
        };
        let selected_nodes = selected_nodes(workflow.as_ref(), &request);
        let selected_assets = self.selected_assets(&request).await?;
        let recent_assets = self.recent_assets(request.project_id).await?;
        let active_runs = self
            .list_active_runs
            .list_active_workflow_runs(request.project_id, 32)
            .await
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?;
        let catalog = self.catalog().await?;
        let snapshot = json!({
            "version": 1,
            "project_id": request.project_id.as_uuid().to_string(),
            "session_id": request.session_id.as_uuid().to_string(),
            "workflow": workflow.as_ref().map(workflow_summary),
            "observed_workflow_revision": request.observed_workflow_revision.map(|value| value.get()),
            "selected_nodes": selected_nodes,
            "selected_assets": selected_assets,
            "recent_assets": recent_assets,
            "active_runs": active_runs.iter().map(run_summary).collect::<Vec<_>>(),
            "catalog": catalog,
        });
        AssistantWorkspaceSnapshot::new(
            serde_json::to_vec(&snapshot)
                .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
        )
    }
}

#[async_trait]
impl<A, R> AssistantNodeCapabilityCatalogReaderInterface
    for DesktopAssistantWorkspaceBridgeAdapterImpl<A, R>
where
    A: WorkflowAggregateRepositoryInterface + 'static,
    R: WorkflowRunRepositoryInterface + 'static,
{
    async fn read_assistant_node_capability_catalog(
        &self,
        request: AssistantNodeCapabilityCatalogRequest,
    ) -> Result<AssistantNodeCapabilityCatalogSnapshot, AssistantApplicationError> {
        let catalog = self.catalog().await?;
        let selected = match request {
            AssistantNodeCapabilityCatalogRequest::List => catalog,
            AssistantNodeCapabilityCatalogRequest::Describe { contract_refs } => catalog
                .into_iter()
                .filter(|value| requested_contract(value, &contract_refs))
                .collect(),
        };
        AssistantNodeCapabilityCatalogSnapshot::new(
            serde_json::to_vec(&selected)
                .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
        )
    }
}

impl<A, R> DesktopAssistantWorkspaceBridgeAdapterImpl<A, R> {
    async fn selected_assets(
        &self,
        request: &AssistantWorkspaceSnapshotRequest,
    ) -> Result<Vec<Value>, AssistantApplicationError> {
        let mut values = Vec::with_capacity(request.selected_asset_ids.len());
        for selected in &request.selected_asset_ids {
            let id = assets::asset::domain::AssetId::from_uuid(uuid::Uuid::from_bytes(
                selected.as_bytes(),
            ))
            .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
            let value =
                match self.get_asset.get_asset(AssetGetQuery::new(request.project_id, id)).await {
                    Ok(asset) => asset_summary(&asset),
                    Err(AssetApplicationError::NotFound | AssetApplicationError::NotVisible) => {
                        json!({"asset_id": id.as_uuid().to_string(), "available": false})
                    }
                    Err(_) => return Err(AssistantApplicationError::ExternalBoundaryFailed),
                };
            values.push(value);
        }
        Ok(values)
    }

    async fn recent_assets(
        &self,
        project_id: projects::project::domain::ProjectId,
    ) -> Result<Vec<Value>, AssistantApplicationError> {
        let limit =
            AssetPageLimit::from_u16(16).ok_or(AssistantApplicationError::ProtocolViolation)?;
        let page = self
            .list_assets
            .list_assets(AssetListQuery::new(project_id, None, None, limit))
            .await
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?;
        Ok(page.assets().iter().map(asset_summary).collect())
    }

    async fn catalog(&self) -> Result<Vec<Value>, AssistantApplicationError> {
        let mut values = Vec::new();
        for contract in self.list_capabilities.list_node_capabilities() {
            let reference = contract.contract_ref().clone();
            let profiles = self
                .list_profiles
                .list_generation_profiles_for_capability(
                    GenerationProfileListForCapabilityQuery::new(
                        reference.clone(),
                        Instant::now() + Duration::from_secs(5),
                    ),
                )
                .await
                .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?;
            values.push(json!({
                "contract_ref": contract_ref(&reference),
                "profiles": profiles.iter().map(|item| json!({
                    "id": item.definition().profile_ref().id().as_str(),
                    "version": item.definition().profile_ref().version().get(),
                    "availability": availability_tag(item.availability().state()),
                })).collect::<Vec<_>>(),
            }));
        }
        Ok(values)
    }
}

fn selected_nodes(
    workflow: Option<&WorkflowAggregate>,
    request: &AssistantWorkspaceSnapshotRequest,
) -> Vec<Value> {
    request
        .selected_node_ids
        .iter()
        .map(|selected| {
            let id = engine::workflow_graph::WorkflowNodeId::from_uuid(uuid::Uuid::from_bytes(
                selected.as_bytes(),
            ));
            match (workflow, id.ok()) {
                (Some(workflow), Some(id)) => workflow.nodes().get(&id).map_or_else(
                    || json!({"node_id": id.as_uuid().to_string(), "available": false}),
                    |node| json!({
                        "node_id": id.as_uuid().to_string(),
                        "available": true,
                        "capability_ref": contract_ref(&node.capability_contract),
                    }),
                ),
                _ => json!({"node_id": uuid::Uuid::from_bytes(selected.as_bytes()).to_string(), "available": false}),
            }
        })
        .collect()
}

fn workflow_summary(workflow: &WorkflowAggregate) -> Value {
    json!({
        "workflow_id": workflow.id.as_uuid().to_string(),
        "revision": workflow.revision.get(),
        "node_count": workflow.nodes().len(),
        "binding_count": workflow.input_bindings().len(),
    })
}

fn asset_summary(asset: &AssetAggregate) -> Value {
    json!({
        "asset_id": asset.id().as_uuid().to_string(),
        "available": matches!(asset.content_state(), AssetManagedContentState::Available { .. }),
        "media_kind": media_kind(asset.media_kind()),
        "display_name": asset.display_name().as_str(),
        "created_at_epoch_ms": asset.created_at().as_utc_milliseconds(),
    })
}

fn run_summary(run: &WorkflowRunAggregate) -> Value {
    json!({
        "run_id": run.run_id().as_uuid().to_string(),
        "workflow_id": run.workflow_id().as_uuid().to_string(),
        "workflow_revision": run.workflow_revision().get(),
        "state": run_state(run.state()),
        "created_at_epoch_ms": run.created_at().as_utc_milliseconds(),
    })
}

fn contract_ref(value: &engine::node_capability::NodeCapabilityContractRef) -> Value {
    json!({
        "id": value.id().as_str(),
        "major": value.version().major(),
        "minor": value.version().minor(),
    })
}

const fn media_kind(value: AssetMediaKind) -> &'static str {
    match value {
        AssetMediaKind::Image => "image",
        AssetMediaKind::Video => "video",
        AssetMediaKind::Audio => "audio",
    }
}

const fn run_state(value: engine::workflow::WorkflowRunState) -> &'static str {
    match value {
        engine::workflow::WorkflowRunState::Queued => "queued",
        engine::workflow::WorkflowRunState::Running => "running",
        engine::workflow::WorkflowRunState::Succeeded => "succeeded",
        engine::workflow::WorkflowRunState::Failed => "failed",
        engine::workflow::WorkflowRunState::Cancelled => "cancelled",
    }
}

const fn availability_tag(value: &GenerationProfileAvailabilityState) -> &'static str {
    match value {
        GenerationProfileAvailabilityState::Available => "available",
        GenerationProfileAvailabilityState::Unavailable { .. } => "unavailable",
        GenerationProfileAvailabilityState::Indeterminate { .. } => "indeterminate",
    }
}

fn requested_contract(value: &Value, requested: &[String]) -> bool {
    let Some(reference) = value.get("contract_ref") else {
        return false;
    };
    let Some(id) = reference.get("id").and_then(Value::as_str) else {
        return false;
    };
    let Some(major) = reference.get("major").and_then(Value::as_u64) else {
        return false;
    };
    let Some(minor) = reference.get("minor").and_then(Value::as_u64) else {
        return false;
    };
    let canonical = format!("{id}@{major}.{minor}");
    requested.iter().any(|candidate| candidate == &canonical)
}
