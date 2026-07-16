use crate::command_error::command_error;
use crate::dto::ProviderDto;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use tauri::State;

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
