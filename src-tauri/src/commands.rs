use crate::assistant_operations::RequestContext;
use crate::command_error::command_error;
use crate::dto::{AssetDto, OpenProjectResultDto, ProjectDto, ProviderDto, WorkflowHeadDto};
use crate::state::AppState;
use crate::workflow_patch_operation::{
    WorkflowApplyPatchError, WorkflowApplyPatchInput, WorkflowApplyPatchOutput,
    WorkflowPatchService,
};
use assets::{AssetKind, AssetQuery, AssetSort};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use tauri::State;
use tracing::info;

pub use crate::assistant::{
    get_assistant_config, get_assistant_config_with_state, set_assistant_config,
    set_assistant_config_with_state,
};
pub use crate::assistant_commands::{
    assistant_decide_approval, assistant_get_pending_approval, assistant_send,
};
pub use crate::capability_catalog::{
    get_capability_bundles, get_capability_bundles_with_state, get_capability_catalog,
    get_capability_catalog_with_state, search_capabilities, search_capabilities_with_state,
};
pub use crate::workflow_run_commands::{
    cancel_workflow_run, cancel_workflow_run_with_state, run_workflow, run_workflow_with_state,
    run_workflow_with_state_and_observer, start_workflow_run, start_workflow_run_with_state,
};

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

/// Legacy integration helper retained until V3 deletes the old Workflow authority.
pub fn list_projects_with_state(state: &AppState) -> Result<Vec<ProjectDto>, String> {
    let projects = state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .list_projects()
        .map_err(|source| command_error("list projects", source))?;
    Ok(projects.into_iter().map(ProjectDto::from).collect())
}

/// Legacy integration helper retained until V3 deletes the old Workflow authority.
pub fn create_project_with_state(name: String, state: &AppState) -> Result<ProjectDto, String> {
    let project = state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .create_project(&name)
        .map_err(|source| command_error("create project", source))?;
    Ok(ProjectDto::from(project))
}

/// Legacy integration helper retained until V3 deletes the old Workflow authority.
pub fn open_project_with_state(
    id: String,
    state: &AppState,
) -> Result<OpenProjectResultDto, String> {
    let project = state
        .store
        .lock()
        .map_err(|_| command_error("lock asset store", "asset store lock was poisoned"))?
        .get_project(&id)
        .map_err(|source| command_error("open project", source))?;
    let workflow_head = state
        .workflow_authority
        .load_head(&id)
        .map_err(|source| command_error("load Workflow head", source))?
        .map(WorkflowHeadDto::try_from)
        .transpose()
        .map_err(|source| command_error("serialize Workflow head", source))?;
    Ok(OpenProjectResultDto { project: ProjectDto::from(project), workflow_head })
}

/// Applies one UI Workflow patch through the shared authoritative service.
#[tauri::command(rename_all = "snake_case")]
pub fn workflow_apply_patch(
    project_id: String,
    request_id: String,
    input: WorkflowApplyPatchInput,
    state: State<'_, AppState>,
) -> Result<WorkflowApplyPatchOutput, WorkflowApplyPatchError> {
    workflow_apply_patch_with_state(project_id, request_id, input, &state)
}

/// Applies one UI Workflow patch against explicit managed state.
pub fn workflow_apply_patch_with_state(
    project_id: String,
    request_id: String,
    input: WorkflowApplyPatchInput,
    state: &AppState,
) -> Result<WorkflowApplyPatchOutput, WorkflowApplyPatchError> {
    let context = RequestContext::new(project_id, "ui", request_id, 1, None);
    WorkflowPatchService::from_state(state).apply(&context, input)
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

fn provider_catalog() -> [(&'static str, &'static str); 1] {
    [("mock", "Mock")]
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
