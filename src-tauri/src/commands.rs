use crate::dto::{AssetDto, RunWorkflowResultDto};
use crate::state::AppState;
use assets::AssetKind;
use engine::{Executor, ResultCache, Workflow};
use serde_json::json;
use tauri::State;
use tracing::{error, info};

/// Runs a workflow JSON payload to completion and returns final node outputs.
#[tauri::command(rename_all = "snake_case")]
pub fn run_workflow(
    workflow_json: String,
    state: State<'_, AppState>,
) -> Result<RunWorkflowResultDto, String> {
    run_workflow_with_state(workflow_json, &state)
}

/// Runs a workflow through the command path against an explicit app state.
pub fn run_workflow_with_state(
    workflow_json: String,
    state: &AppState,
) -> Result<RunWorkflowResultDto, String> {
    info!("run_workflow command received");
    let workflow = serde_json::from_str::<Workflow>(&workflow_json)
        .map_err(|source| command_error("deserialize workflow", source))?;
    let workflow = enrich_save_asset_params(&workflow)
        .map_err(|source| command_error("prepare workflow", source))?;
    let mut cache = ResultCache::new();
    let outputs = Executor::new(&state.registry)
        .execute(&workflow, &mut cache)
        .map_err(|source| command_error("run workflow", source))?;
    info!(node_count = outputs.len(), "run_workflow command completed");
    Ok(RunWorkflowResultDto::from_outputs(&outputs))
}

/// Lists assets, optionally filtered by kind.
#[tauri::command(rename_all = "snake_case")]
pub fn list_assets(
    kind: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<AssetDto>, String> {
    info!(kind = kind.as_deref().unwrap_or("all"), "list_assets command received");
    let kind = parse_asset_kind_filter(kind)
        .map_err(|source| command_error("parse asset kind", source))?;
    let assets = state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .list(kind)
        .map_err(|source| command_error("list assets", source))?;
    Ok(assets.into_iter().map(AssetDto::from).collect())
}

/// Returns a single asset by id.
#[tauri::command(rename_all = "snake_case")]
pub fn get_asset(id: String, state: State<'_, AppState>) -> Result<AssetDto, String> {
    info!(asset_id = %id, "get_asset command received");
    let asset = state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .get(&id)
        .map_err(|source| command_error("get asset", source))?;
    Ok(AssetDto::from(asset))
}

/// Returns the local asset store root path.
#[tauri::command(rename_all = "snake_case")]
pub fn assets_root(state: State<'_, AppState>) -> Result<String, String> {
    info!(asset_root = %state.root.display(), "assets_root command received");
    assets_root_with_state(&state)
}

/// Returns the configured asset root for tests and command adapters.
pub fn assets_root_with_state(state: &AppState) -> Result<String, String> {
    state
        .root
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| command_error("resolve asset root", "asset root path is not valid UTF-8"))
}

/// Adds execution-only metadata needed by `SaveAsset` nodes.
pub fn enrich_save_asset_params(workflow: &Workflow) -> anyhow::Result<Workflow> {
    let snapshot = serde_json::to_value(workflow)?;
    let mut enriched = workflow.clone();
    for node in &mut enriched.nodes {
        if node.type_id != "SaveAsset" {
            continue;
        }
        node.params.insert("workflow_snapshot".to_owned(), snapshot.clone());
        if let Some(source) = node.inputs.get("media") {
            node.params.insert("source_node_id".to_owned(), json!(source.node_id()));
        }
    }
    Ok(enriched)
}

/// Parses a frontend asset kind filter.
pub fn parse_asset_kind_filter(kind: Option<String>) -> anyhow::Result<Option<AssetKind>> {
    match kind.as_deref() {
        None | Some("") => Ok(None),
        Some("image") => Ok(Some(AssetKind::Image)),
        Some("video") => Ok(Some(AssetKind::Video)),
        Some(value) => anyhow::bail!("unsupported asset kind `{value}`"),
    }
}

fn command_error(operation: &str, error: impl std::fmt::Display) -> String {
    error!(operation, error = %error, "tauri command failed");
    format!("{operation}: {error}")
}
