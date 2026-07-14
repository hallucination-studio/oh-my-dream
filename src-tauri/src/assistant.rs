use crate::dto::{AssistantConfigDto, AssistantConfigInputDto};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::State;
use tracing::error;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-5.4";

/// Returns assistant configuration without exposing the raw API key.
#[tauri::command(rename_all = "snake_case")]
pub fn get_assistant_config(state: State<'_, AppState>) -> Result<AssistantConfigDto, String> {
    get_assistant_config_with_state(&state)
}

/// Returns assistant configuration against an explicit app state.
pub fn get_assistant_config_with_state(state: &AppState) -> Result<AssistantConfigDto, String> {
    Ok(read_assistant_config(state)?.into_dto())
}

/// Updates assistant configuration while preserving secrets unless replaced.
#[tauri::command(rename_all = "snake_case")]
pub fn set_assistant_config(
    input: AssistantConfigInputDto,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_assistant_config_with_state(input, &state)
}

/// Updates assistant configuration against an explicit app state.
pub fn set_assistant_config_with_state(
    input: AssistantConfigInputDto,
    state: &AppState,
) -> Result<(), String> {
    let mut config = read_assistant_config(state)?;
    config.enabled = input.enabled;
    config.base_url = input.base_url;
    config.model = input.model;
    if input.clear_api_key {
        config.api_key = None;
    } else if input.api_key.is_some() {
        config.api_key = input.api_key;
    }
    write_assistant_config(state, &config)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AssistantConfig {
    enabled: bool,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: DEFAULT_BASE_URL.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            api_key: None,
        }
    }
}

impl AssistantConfig {
    fn into_dto(self) -> AssistantConfigDto {
        AssistantConfigDto {
            enabled: self.enabled,
            base_url: self.base_url,
            model: self.model,
            has_key: self.api_key.is_some_and(|key| !key.is_empty()),
        }
    }
}

fn read_assistant_config(state: &AppState) -> Result<AssistantConfig, String> {
    let path = assistant_config_path(state);
    if !path.exists() {
        return Ok(AssistantConfig::default());
    }
    let contents = fs::read_to_string(&path)
        .map_err(|source| assistant_error("read assistant config", source))?;
    serde_json::from_str(&contents)
        .map_err(|source| assistant_error("parse assistant config", source))
}

fn write_assistant_config(state: &AppState, config: &AssistantConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(config)
        .map_err(|source| assistant_error("serialize assistant config", source))?;
    fs::write(assistant_config_path(state), format!("{json}\n"))
        .map_err(|source| assistant_error("write assistant config", source))
}

fn assistant_config_path(state: &AppState) -> PathBuf {
    state.config_root.join("assistant_config.json")
}

fn assistant_error(operation: &str, error: impl std::fmt::Display) -> String {
    error!(operation, error = %error, "assistant command failed");
    format!("{operation}: {error}")
}
