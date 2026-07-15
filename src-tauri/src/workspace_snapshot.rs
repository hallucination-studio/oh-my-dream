//! Authoritative, bounded, project-scoped workspace read operation.

mod schema;

use crate::assistant_operations::{
    OperationEffect, OperationHandlerError, OperationInputSchemaMode, OperationOutputSchemaMode,
    OperationRegistration, OperationRegistrationError, RequestContext,
};
use crate::dto::{
    CapabilityRefDto, ProjectDto, WorkflowHeadDto, WorkspaceAssetSummaryDto,
    WorkspaceNodeSummaryDto, WorkspaceRunSummaryDto, WorkspaceScopeDto,
};
use crate::state::AppState;
use crate::workflow_authority::{WorkflowAuthority, WorkflowHead};
use crate::workflow_runs::WorkflowRuns;
use assets::{AssetError, Project};
use engine::{NodeRegistry, validate_workflow};
use nodes::SharedAssetStore;
use std::sync::Arc;
use thiserror::Error;

pub use crate::dto::{WorkspaceSnapshotInput, WorkspaceSnapshotOutput};

/// Maximum newest-first Project Asset summaries returned by one snapshot.
pub const MAX_WORKSPACE_ASSET_SUMMARIES: usize = 8;
/// Maximum selected Asset or node ids accepted in each input collection.
pub const MAX_WORKSPACE_SELECTIONS: usize = 16;
/// Maximum active Run summaries returned for one Project.
pub const MAX_WORKSPACE_RUN_SUMMARIES: usize = 1;
/// Maximum serialized bytes returned to one Agent invocation.
pub const MAX_WORKSPACE_SNAPSHOT_BYTES: usize = 512 * 1024;

/// Structured workspace read failure.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{code} at {pointer}: {constraint}")]
pub struct WorkspaceSnapshotError {
    /// Stable machine-readable code.
    pub code: String,
    /// JSON Pointer into input or the current Workflow.
    pub pointer: String,
    /// Boundary-safe constraint detail.
    pub constraint: String,
}

impl WorkspaceSnapshotError {
    fn new(
        code: impl Into<String>,
        pointer: impl Into<String>,
        constraint: impl Into<String>,
    ) -> Self {
        Self { code: code.into(), pointer: pointer.into(), constraint: constraint.into() }
    }
}

/// Application service composing project, Workflow, Asset, and Run projections.
pub struct WorkspaceSnapshotService {
    registry: Arc<NodeRegistry>,
    authority: Arc<WorkflowAuthority>,
    runs: Arc<WorkflowRuns>,
    store: SharedAssetStore,
}

impl WorkspaceSnapshotService {
    /// Creates a service over explicit application-owned dependencies.
    #[must_use]
    pub fn new(
        registry: Arc<NodeRegistry>,
        authority: Arc<WorkflowAuthority>,
        runs: Arc<WorkflowRuns>,
        store: SharedAssetStore,
    ) -> Self {
        Self { registry, authority, runs, store }
    }

    /// Creates a service from the application composition root.
    #[must_use]
    pub fn from_state(state: &AppState) -> Self {
        Self::new(
            Arc::clone(&state.registry),
            Arc::clone(&state.workflow_authority),
            Arc::clone(&state.workflow_runs),
            Arc::clone(&state.store),
        )
    }

    /// Reads one bounded snapshot using only trusted context for Project scope.
    pub fn get_snapshot(
        &self,
        context: &RequestContext,
        _input: WorkspaceSnapshotInput,
    ) -> Result<WorkspaceSnapshotOutput, WorkspaceSnapshotError> {
        validate_input_bounds(context)?;
        let store_state = self.load_store_state(context)?;
        let head = self.authority.load_head(context.project_id()).map_err(|error| {
            WorkspaceSnapshotError::new("WORKFLOW_READ_FAILED", "/", error.to_string())
        })?;
        let (readiness_blockers, selected_nodes) =
            self.workflow_projection(head.as_ref(), context.selected_node_ids())?;
        let run = self.runs.active_run_id(context.project_id()).map_err(|error| {
            WorkspaceSnapshotError::new("RUN_READ_FAILED", "/runs", error.to_string())
        })?;
        let workflow_head = head.map(WorkflowHeadDto::try_from).transpose().map_err(|error| {
            WorkspaceSnapshotError::new(
                "WORKFLOW_SERIALIZATION_FAILED",
                "/workflow_head",
                error.to_string(),
            )
        })?;
        let output = WorkspaceSnapshotOutput {
            scope: WorkspaceScopeDto {
                project_id: context.project_id().to_owned(),
                session_id: context.session_id().to_owned(),
                request_id: context.request_id().to_owned(),
            },
            project: ProjectDto::from(store_state.project),
            workflow_head,
            selected_assets: store_state.selected_assets,
            selected_nodes,
            readiness_blockers,
            assets: store_state.assets,
            runs: run
                .map(|run_id| WorkspaceRunSummaryDto {
                    run_id: run_id.as_str().to_owned(),
                    status: "active".to_owned(),
                })
                .into_iter()
                .collect(),
        };
        enforce_output_size(&output)?;
        Ok(output)
    }

    /// Builds the strict model-facing workspace read operation.
    pub fn operation_registration(
        self: Arc<Self>,
    ) -> Result<OperationRegistration, OperationRegistrationError> {
        let service = Arc::clone(&self);
        OperationRegistration::new_with_output_mode::<
            WorkspaceSnapshotInput,
            WorkspaceSnapshotOutput,
            _,
        >(
            "workspace_get_snapshot",
            1,
            "Read one bounded authoritative snapshot of the current Project workspace.",
            OperationEffect::LocalRead,
            OperationInputSchemaMode::Strict,
            OperationOutputSchemaMode::WorkflowDocument,
            move |context: &RequestContext, _input: WorkspaceSnapshotInput| {
                let context = context.clone();
                let service = Arc::clone(&service);
                async move {
                    service.get_snapshot(&context, WorkspaceSnapshotInput {}).map_err(|error| {
                        OperationHandlerError::new(error.code.clone(), error.to_string())
                    })
                }
            },
        )
    }

    fn load_store_state(
        &self,
        context: &RequestContext,
    ) -> Result<StoreState, WorkspaceSnapshotError> {
        let project_id = context.project_id();
        let store = self.store.lock().map_err(|_| {
            WorkspaceSnapshotError::new(
                "ASSET_STORE_UNAVAILABLE",
                "/",
                "asset store lock is unavailable",
            )
        })?;
        let project = store.get_project(project_id).map_err(project_error)?;
        let assets = crate::managed_asset_access::list_visible(
            &store,
            project_id,
            MAX_WORKSPACE_ASSET_SUMMARIES,
        )
        .map_err(store_error)?
        .into_iter()
        .map(WorkspaceAssetSummaryDto::from)
        .collect();
        let selected_assets = context
            .selected_asset_ids()
            .iter()
            .enumerate()
            .map(|(index, id)| selected_asset(&store, project_id, id, index))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(StoreState { project, assets, selected_assets })
    }

    fn workflow_projection(
        &self,
        head: Option<&WorkflowHead>,
        selected_ids: &[String],
    ) -> Result<
        (Vec<engine::WorkflowReadinessBlocker>, Vec<WorkspaceNodeSummaryDto>),
        WorkspaceSnapshotError,
    > {
        let Some(head) = head else {
            if selected_ids.is_empty() {
                return Ok((Vec::new(), Vec::new()));
            }
            return Err(selected_node_error(0));
        };
        let report = validate_workflow(&self.registry, &head.workflow);
        if let Some(error) = report.persistence_errors.first() {
            return Err(WorkspaceSnapshotError::new(
                error.code.clone(),
                error.pointer.clone(),
                error.constraint.clone(),
            ));
        }
        let selected = selected_ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let node = head
                    .workflow
                    .nodes
                    .iter()
                    .find(|node| node.id == *id)
                    .ok_or_else(|| selected_node_error(index))?;
                Ok(WorkspaceNodeSummaryDto {
                    id: node.id.clone(),
                    capability: CapabilityRefDto {
                        id: node.type_id.clone(),
                        version: node.contract_version.clone(),
                    },
                })
            })
            .collect::<Result<Vec<_>, WorkspaceSnapshotError>>()?;
        Ok((report.readiness_blockers, selected))
    }
}

struct StoreState {
    project: Project,
    assets: Vec<WorkspaceAssetSummaryDto>,
    selected_assets: Vec<WorkspaceAssetSummaryDto>,
}

fn selected_asset(
    store: &assets::AssetStore,
    project_id: &str,
    id: &str,
    index: usize,
) -> Result<WorkspaceAssetSummaryDto, WorkspaceSnapshotError> {
    let asset = match store.get(id) {
        Ok(asset) => asset,
        Err(AssetError::NotFound { .. }) => return Err(selected_asset_error(index)),
        Err(error) => return Err(store_error(error)),
    };
    if asset.project_id.as_deref().is_some_and(|owner| owner != project_id) {
        return Err(selected_asset_error(index));
    }
    Ok(WorkspaceAssetSummaryDto::from(asset))
}

fn selected_asset_error(index: usize) -> WorkspaceSnapshotError {
    WorkspaceSnapshotError::new(
        "SELECTED_ASSET_OUT_OF_SCOPE",
        format!("/selected_asset_ids/{index}"),
        "selected Asset is not available in the trusted Project scope",
    )
}

fn selected_node_error(index: usize) -> WorkspaceSnapshotError {
    WorkspaceSnapshotError::new(
        "SELECTED_NODE_OUT_OF_SCOPE",
        format!("/selected_node_ids/{index}"),
        "selected node is not present in the trusted Project Workflow",
    )
}

fn project_error(error: AssetError) -> WorkspaceSnapshotError {
    match error {
        AssetError::NotFound { .. } => {
            WorkspaceSnapshotError::new("PROJECT_NOT_FOUND", "/", "trusted Project was not found")
        }
        other => store_error(other),
    }
}

fn store_error(error: AssetError) -> WorkspaceSnapshotError {
    WorkspaceSnapshotError::new("ASSET_STORE_UNAVAILABLE", "/assets", error.to_string())
}

fn enforce_output_size(output: &WorkspaceSnapshotOutput) -> Result<(), WorkspaceSnapshotError> {
    let size = serde_json::to_vec(output)
        .map_err(|error| {
            WorkspaceSnapshotError::new("SNAPSHOT_SERIALIZATION_FAILED", "/", error.to_string())
        })?
        .len();
    if size <= MAX_WORKSPACE_SNAPSHOT_BYTES {
        return Ok(());
    }
    Err(WorkspaceSnapshotError::new(
        "SNAPSHOT_SIZE_LIMIT",
        "/",
        format!("snapshot must be at most {MAX_WORKSPACE_SNAPSHOT_BYTES} bytes"),
    ))
}

fn validate_input_bounds(context: &RequestContext) -> Result<(), WorkspaceSnapshotError> {
    for (pointer, count) in [
        ("/selected_asset_ids", context.selected_asset_ids().len()),
        ("/selected_node_ids", context.selected_node_ids().len()),
    ] {
        if count > MAX_WORKSPACE_SELECTIONS {
            return Err(WorkspaceSnapshotError::new(
                "SNAPSHOT_SELECTION_LIMIT",
                pointer,
                format!("at most {MAX_WORKSPACE_SELECTIONS} selected ids are allowed"),
            ));
        }
    }
    Ok(())
}
