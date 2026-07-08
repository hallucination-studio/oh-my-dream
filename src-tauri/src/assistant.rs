use crate::assistant_capabilities::backend_capabilities;
use crate::assistant_sidecar::create_assistant_session;
use crate::dto::{
    AssistantConfigDto, AssistantConfigInputDto, AssistantSessionDto, AssistantSkillsDto,
    CapabilityManifestDto, SkillDto,
};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
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
    let config = read_assistant_config(state)?;
    Ok(config.into_dto(installed_skill_names(state)?))
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
    config.temperature = input.temperature;
    config.max_tool_iters = input.max_tool_iters;
    config.system_prompt_extra = input.system_prompt_extra;
    config.developer_mode = input.developer_mode;
    config.skills.enabled = input.enabled_skills;
    if input.clear_api_key {
        config.api_key = None;
    } else if input.api_key.is_some() {
        config.api_key = input.api_key;
    }
    write_assistant_config(state, &config)
}

/// Returns the assistant capability manifest.
#[tauri::command(rename_all = "snake_case")]
pub fn get_capability_manifest(
    state: State<'_, AppState>,
) -> Result<CapabilityManifestDto, String> {
    get_capability_manifest_with_state(&state)
}

/// Returns the assistant capability manifest against an explicit app state.
pub fn get_capability_manifest_with_state(
    _state: &AppState,
) -> Result<CapabilityManifestDto, String> {
    Ok(CapabilityManifestDto { capabilities: backend_capabilities() })
}

/// Returns a stable local assistant session for this app state.
#[tauri::command(rename_all = "snake_case")]
pub fn get_assistant_session(state: State<'_, AppState>) -> Result<AssistantSessionDto, String> {
    get_assistant_session_with_state(&state)
}

/// Returns a stable local assistant session against an explicit app state.
pub fn get_assistant_session_with_state(state: &AppState) -> Result<AssistantSessionDto, String> {
    let mut session = state.assistant_session.lock().map_err(|_| {
        assistant_error("lock assistant session", "assistant session lock was poisoned")
    })?;
    if let Some(existing) = session.clone() {
        return Ok(existing);
    }
    let (created, process) = create_assistant_session(state)?;
    if let Some(process) = process {
        *state.assistant_process.lock().map_err(|_| {
            assistant_error("lock assistant process", "assistant process lock was poisoned")
        })? = Some(process);
    }
    *session = Some(created.clone());
    Ok(created)
}

/// Lists installed assistant skills.
#[tauri::command(rename_all = "snake_case")]
pub fn list_skills(state: State<'_, AppState>) -> Result<Vec<SkillDto>, String> {
    list_skills_with_state(&state)
}

/// Lists installed assistant skills against an explicit app state.
pub fn list_skills_with_state(state: &AppState) -> Result<Vec<SkillDto>, String> {
    let config = read_assistant_config(state)?;
    let enabled = config.skills.enabled.into_iter().collect::<BTreeSet<_>>();
    let mut skills = Vec::new();
    let root = skills_root(state);
    if !root.exists() {
        return Ok(skills);
    }
    for entry in fs::read_dir(&root).map_err(|source| assistant_error("read skills", source))? {
        let entry = entry.map_err(|source| assistant_error("read skill entry", source))?;
        if !entry.path().is_dir() {
            continue;
        }
        let manifest = read_skill_manifest(&entry.path())?;
        skills.push(SkillDto {
            enabled: enabled.contains(&manifest.name),
            developer_mode_required: entry.path().join("graph.py").exists(),
            status: if enabled.contains(&manifest.name) { "ready" } else { "disabled" }.to_owned(),
            name: manifest.name,
            version: manifest.version,
            description: manifest.description,
        });
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(skills)
}

/// Installs a declarative skill from a local directory or manifest path.
#[tauri::command(rename_all = "snake_case")]
pub fn install_skill(path: String, state: State<'_, AppState>) -> Result<SkillDto, String> {
    install_skill_with_state(path, &state)
}

/// Installs a declarative skill against an explicit app state.
pub fn install_skill_with_state(path: String, state: &AppState) -> Result<SkillDto, String> {
    let source = PathBuf::from(path);
    let source_dir = if source.is_dir() {
        source
    } else {
        source
            .parent()
            .ok_or_else(|| assistant_error("resolve skill package", "skill path has no parent"))?
            .to_path_buf()
    };
    let manifest = read_skill_manifest(&source_dir)?;
    validate_skill_name(&manifest.name)?;
    if !source_dir.join("prompt.md").exists() {
        return Err(assistant_error("validate skill package", "prompt.md is required"));
    }
    let destination = skills_root(state).join(&manifest.name);
    if destination.exists() {
        fs::remove_dir_all(&destination)
            .map_err(|source| assistant_error("replace skill package", source))?;
    }
    fs::create_dir_all(&destination)
        .map_err(|source| assistant_error("create skill directory", source))?;
    copy_skill_dir(&source_dir, &destination)?;
    Ok(SkillDto {
        name: manifest.name,
        version: manifest.version,
        description: manifest.description,
        enabled: false,
        developer_mode_required: destination.join("graph.py").exists(),
        status: "disabled".to_owned(),
    })
}

/// Enables or disables an installed skill.
#[tauri::command(rename_all = "snake_case")]
pub fn set_skill_enabled(
    name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_skill_enabled_with_state(name, enabled, &state)
}

/// Enables or disables an installed skill against an explicit app state.
pub fn set_skill_enabled_with_state(
    name: String,
    enabled: bool,
    state: &AppState,
) -> Result<(), String> {
    validate_skill_name(&name)?;
    if !skills_root(state).join(&name).exists() {
        return Err(assistant_error("enable skill", format!("unknown skill `{name}`")));
    }
    let mut config = read_assistant_config(state)?;
    config.skills.enabled.retain(|existing| existing != &name);
    if enabled {
        config.skills.enabled.push(name);
        config.skills.enabled.sort();
    }
    write_assistant_config(state, &config)
}

/// Uninstalls an assistant skill.
#[tauri::command(rename_all = "snake_case")]
pub fn uninstall_skill(name: String, state: State<'_, AppState>) -> Result<(), String> {
    uninstall_skill_with_state(name, &state)
}

/// Uninstalls an assistant skill against an explicit app state.
pub fn uninstall_skill_with_state(name: String, state: &AppState) -> Result<(), String> {
    validate_skill_name(&name)?;
    let path = skills_root(state).join(&name);
    if path.exists() {
        fs::remove_dir_all(&path).map_err(|source| assistant_error("remove skill", source))?;
    }
    let mut config = read_assistant_config(state)?;
    config.skills.enabled.retain(|existing| existing != &name);
    write_assistant_config(state, &config)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AssistantConfig {
    enabled: bool,
    base_url: String,
    model: String,
    api_key: Option<String>,
    temperature: f64,
    max_tool_iters: u32,
    system_prompt_extra: Option<String>,
    developer_mode: bool,
    skills: AssistantSkillsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AssistantSkillsConfig {
    #[serde(default)]
    enabled: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SkillManifest {
    name: String,
    version: String,
    description: String,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: DEFAULT_BASE_URL.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            api_key: None,
            temperature: 0.3,
            max_tool_iters: 20,
            system_prompt_extra: None,
            developer_mode: false,
            skills: AssistantSkillsConfig::default(),
        }
    }
}

impl AssistantConfig {
    fn into_dto(self, installed: Vec<String>) -> AssistantConfigDto {
        AssistantConfigDto {
            enabled: self.enabled,
            base_url: self.base_url,
            model: self.model,
            has_key: self.api_key.is_some_and(|key| !key.is_empty()),
            temperature: self.temperature,
            max_tool_iters: self.max_tool_iters,
            system_prompt_extra: self.system_prompt_extra,
            developer_mode: self.developer_mode,
            skills: AssistantSkillsDto { installed, enabled: self.skills.enabled },
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

fn installed_skill_names(state: &AppState) -> Result<Vec<String>, String> {
    Ok(list_skills_with_state(state)?.into_iter().map(|skill| skill.name).collect())
}

fn skills_root(state: &AppState) -> PathBuf {
    state.config_root.join("skills")
}

fn read_skill_manifest(path: &Path) -> Result<SkillManifest, String> {
    let contents = fs::read_to_string(path.join("skill.json"))
        .map_err(|source| assistant_error("read skill manifest", source))?;
    serde_json::from_str(&contents)
        .map_err(|source| assistant_error("parse skill manifest", source))
}

fn validate_skill_name(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        });
    valid.then_some(()).ok_or_else(|| {
        assistant_error(
            "validate skill name",
            "skill names must be ASCII letters, numbers, dashes, or underscores",
        )
    })
}

fn copy_skill_dir(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in
        fs::read_dir(source).map_err(|source| assistant_error("read skill package", source))?
    {
        let entry = entry.map_err(|source| assistant_error("read skill package entry", source))?;
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            fs::create_dir_all(&target)
                .map_err(|source| assistant_error("create skill subdirectory", source))?;
            copy_skill_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)
                .map_err(|source| assistant_error("copy skill file", source))?;
        }
    }
    Ok(())
}

fn assistant_error(operation: &str, error: impl std::fmt::Display) -> String {
    error!(operation, error = %error, "assistant command failed");
    format!("{operation}: {error}")
}
