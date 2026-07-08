use crate::dto::{
    AssetDto, NodeProgressEventDto, ProjectDto, ProjectWorkspaceDto, ProviderDto,
    RunWorkflowResultDto,
};
use crate::state::AppState;
use assets::{AssetKind, AssetQuery, AssetSort};
use engine::{Executor, NodeProgressEvent, ResultCache, Workflow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use tauri::{Emitter, State};
use tracing::{error, info};

/// Runs a workflow JSON payload to completion and returns final node outputs.
#[tauri::command(rename_all = "snake_case")]
pub fn run_workflow(
    workflow_json: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<RunWorkflowResultDto, String> {
    run_workflow_with_state_and_observer(workflow_json, &state, &mut |event| {
        if let Err(source) = app.emit("node_progress", NodeProgressEventDto::from(event.clone())) {
            error!(error = %source, "failed to emit node_progress event");
        }
    })
}

/// Runs a workflow through the command path against an explicit app state.
pub fn run_workflow_with_state(
    workflow_json: String,
    state: &AppState,
) -> Result<RunWorkflowResultDto, String> {
    run_workflow_with_state_and_observer(workflow_json, state, &mut |_event| {})
}

/// Runs a workflow through the command path with a testable observer hook.
pub fn run_workflow_with_state_and_observer(
    workflow_json: String,
    state: &AppState,
    observer: &mut impl FnMut(&NodeProgressEvent),
) -> Result<RunWorkflowResultDto, String> {
    info!("run_workflow command received");
    let workflow = serde_json::from_str::<Workflow>(&workflow_json)
        .map_err(|source| command_error("deserialize workflow", source))?;
    if workflow.project_id.is_empty() {
        return Err(command_error("validate workflow", "workflow project_id is empty"));
    }
    ensure_project_exists(state, &workflow.project_id)?;
    let mut cache = ResultCache::new();
    let outputs = Executor::new(&state.registry)
        .execute_with_observer(&workflow, &mut cache, observer)
        .map_err(|source| command_error("run workflow", source))?;
    info!(node_count = outputs.len(), "run_workflow command completed");
    Ok(RunWorkflowResultDto::from_outputs(&outputs))
}

/// Lists assets using optional library filters.
#[tauri::command(rename_all = "snake_case")]
pub fn list_assets(
    kind: Option<String>,
    project_id: Option<String>,
    model: Option<String>,
    prompt: Option<String>,
    sort: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<AssetDto>, String> {
    list_assets_with_state(kind, project_id, model, prompt, sort, &state)
}

/// Lists assets against an explicit app state.
pub fn list_assets_with_state(
    kind: Option<String>,
    project_id: Option<String>,
    model: Option<String>,
    prompt: Option<String>,
    sort: Option<String>,
    state: &AppState,
) -> Result<Vec<AssetDto>, String> {
    info!(kind = kind.as_deref().unwrap_or("all"), "list_assets command received");
    let kind = parse_asset_kind_filter(kind)
        .map_err(|source| command_error("parse asset kind", source))?;
    let sort =
        parse_asset_sort(sort).map_err(|source| command_error("parse asset sort", source))?;
    let assets = state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .list_with_query(&AssetQuery { kind, project_id, model, prompt, sort })
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

/// Lists projects.
#[tauri::command(rename_all = "snake_case")]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectDto>, String> {
    list_projects_with_state(&state)
}

/// Lists projects against an explicit app state.
pub fn list_projects_with_state(state: &AppState) -> Result<Vec<ProjectDto>, String> {
    let projects = state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .list_projects()
        .map_err(|source| command_error("list projects", source))?;
    Ok(projects.into_iter().map(ProjectDto::from).collect())
}

/// Creates a project.
#[tauri::command(rename_all = "snake_case")]
pub fn create_project(name: String, state: State<'_, AppState>) -> Result<ProjectDto, String> {
    create_project_with_state(name, &state)
}

/// Creates a project against an explicit app state.
pub fn create_project_with_state(name: String, state: &AppState) -> Result<ProjectDto, String> {
    let project = state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .create_project(&name)
        .map_err(|source| command_error("create project", source))?;
    Ok(ProjectDto::from(project))
}

/// Opens a project and its workflow.
#[tauri::command(rename_all = "snake_case")]
pub fn open_project(id: String, state: State<'_, AppState>) -> Result<ProjectWorkspaceDto, String> {
    open_project_with_state(id, &state)
}

/// Opens a project against an explicit app state.
pub fn open_project_with_state(
    id: String,
    state: &AppState,
) -> Result<ProjectWorkspaceDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?;
    let project = store.get_project(&id).map_err(|source| command_error("open project", source))?;
    let workflow_json = match store.load_workflow(&id) {
        Ok(workflow) => workflow,
        Err(assets::AssetError::NotFound { .. }) => default_workflow_json(&id),
        Err(source) => return Err(command_error("load workflow", source)),
    };
    Ok(ProjectWorkspaceDto { project: ProjectDto::from(project), workflow_json })
}

/// Saves workflow JSON by its embedded project id.
#[tauri::command(rename_all = "snake_case")]
pub fn save_workflow(workflow_json: String, state: State<'_, AppState>) -> Result<(), String> {
    save_workflow_with_state(workflow_json, &state)
}

/// Saves workflow JSON against an explicit app state.
pub fn save_workflow_with_state(workflow_json: String, state: &AppState) -> Result<(), String> {
    let workflow = serde_json::from_str::<Workflow>(&workflow_json)
        .map_err(|source| command_error("deserialize workflow", source))?;
    ensure_project_exists(state, &workflow.project_id)?;
    let value = serde_json::from_str::<serde_json::Value>(&workflow_json)
        .map_err(|source| command_error("deserialize workflow JSON", source))?;
    state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .save_workflow(&workflow.project_id, value)
        .map_err(|source| command_error("save workflow", source))
}

/// Loads workflow JSON by project id.
#[tauri::command(rename_all = "snake_case")]
pub fn load_workflow(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .load_workflow(&project_id)
        .map_err(|source| command_error("load workflow", source))
}

/// Returns provider summaries without raw keys.
#[tauri::command(rename_all = "snake_case")]
pub fn get_providers(state: State<'_, AppState>) -> Result<Vec<ProviderDto>, String> {
    get_providers_with_state(&state)
}

/// Returns provider summaries against an explicit app state.
pub fn get_providers_with_state(state: &AppState) -> Result<Vec<ProviderDto>, String> {
    let config = read_provider_config(state)?;
    Ok(provider_catalog()
        .into_iter()
        .map(|(id, name)| ProviderDto {
            id: id.to_owned(),
            name: name.to_owned(),
            active: config.active_provider == id,
            has_key: config.keys.get(id).is_some_and(|key| !key.is_empty()),
        })
        .collect())
}

/// Sets the active provider.
#[tauri::command(rename_all = "snake_case")]
pub fn set_active_provider(provider_id: String, state: State<'_, AppState>) -> Result<(), String> {
    set_active_provider_with_state(provider_id, &state)
}

/// Sets the active provider against an explicit app state.
pub fn set_active_provider_with_state(provider_id: String, state: &AppState) -> Result<(), String> {
    ensure_provider(&provider_id)?;
    let mut config = read_provider_config(state)?;
    config.active_provider = provider_id;
    write_provider_config(state, &config)
}

/// Stores a provider key locally.
#[tauri::command(rename_all = "snake_case")]
pub fn set_provider_key(
    provider_id: String,
    key: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_provider_key_with_state(provider_id, key, &state)
}

/// Stores a provider key locally against an explicit app state.
pub fn set_provider_key_with_state(
    provider_id: String,
    key: String,
    state: &AppState,
) -> Result<(), String> {
    ensure_provider(&provider_id)?;
    let mut config = read_provider_config(state)?;
    config.keys.insert(provider_id, key);
    write_provider_config(state, &config)
}

/// Parses a frontend asset kind filter.
pub fn parse_asset_kind_filter(kind: Option<String>) -> anyhow::Result<Option<AssetKind>> {
    match kind.as_deref() {
        None | Some("") => Ok(None),
        Some("image") => Ok(Some(AssetKind::Image)),
        Some("video") => Ok(Some(AssetKind::Video)),
        Some("audio") => Ok(Some(AssetKind::Audio)),
        Some(value) => anyhow::bail!("unsupported asset kind `{value}`"),
    }
}

/// Parses a frontend asset sort.
pub fn parse_asset_sort(sort: Option<String>) -> anyhow::Result<AssetSort> {
    match sort.as_deref() {
        None | Some("") | Some("newest") => Ok(AssetSort::Newest),
        Some("oldest") => Ok(AssetSort::Oldest),
        Some("cost_desc") => Ok(AssetSort::CostDesc),
        Some("cost_asc") => Ok(AssetSort::CostAsc),
        Some(value) => anyhow::bail!("unsupported asset sort `{value}`"),
    }
}

fn default_workflow_json(project_id: &str) -> serde_json::Value {
    serde_json::json!({ "version": "1.0", "project_id": project_id, "nodes": [] })
}

fn ensure_project_exists(state: &AppState, project_id: &str) -> Result<(), String> {
    state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .get_project(project_id)
        .map(|_| ())
        .map_err(|source| command_error("validate project", source))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderConfig {
    active_provider: String,
    #[serde(default)]
    keys: BTreeMap<String, String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self { active_provider: "mock".to_owned(), keys: BTreeMap::new() }
    }
}

fn provider_catalog() -> [(&'static str, &'static str); 3] {
    [("mock", "Mock"), ("fal", "fal.ai"), ("replicate", "Replicate")]
}

fn ensure_provider(provider_id: &str) -> Result<(), String> {
    provider_catalog().iter().any(|(id, _)| *id == provider_id).then_some(()).ok_or_else(|| {
        command_error("validate provider", format!("unknown provider `{provider_id}`"))
    })
}

fn read_provider_config(state: &AppState) -> Result<ProviderConfig, String> {
    let path = provider_config_path(state);
    if !path.exists() {
        return Ok(ProviderConfig::default());
    }
    let contents = fs::read_to_string(&path)
        .map_err(|source| command_error("read provider config", source))?;
    serde_json::from_str(&contents).map_err(|source| command_error("parse provider config", source))
}

fn write_provider_config(state: &AppState, config: &ProviderConfig) -> Result<(), String> {
    let path = provider_config_path(state);
    let json = serde_json::to_string_pretty(config)
        .map_err(|source| command_error("serialize provider config", source))?;
    fs::write(&path, format!("{json}\n"))
        .map_err(|source| command_error("write provider config", source))
}

fn provider_config_path(state: &AppState) -> std::path::PathBuf {
    state.config_root.join("provider_config.json")
}

fn command_error(operation: &str, error: impl std::fmt::Display) -> String {
    error!(operation, error = %error, "tauri command failed");
    format!("{operation}: {error}")
}
