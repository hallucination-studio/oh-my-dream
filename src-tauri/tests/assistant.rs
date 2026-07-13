use oh_my_dream_tauri::commands::{
    get_assistant_config_with_state, get_assistant_session_with_state,
    get_capability_manifest_with_state, install_skill_with_state, list_skills_with_state,
    set_assistant_config_with_state, set_skill_enabled_with_state, uninstall_skill_with_state,
};
use oh_my_dream_tauri::dto::AssistantConfigInputDto;
use oh_my_dream_tauri::state::AppState;
use std::fs;
use tempfile::tempdir;

#[test]
fn assistant_config_persists_without_returning_raw_api_key() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let defaults = get_assistant_config_with_state(&state).expect("read default config");
    assert_eq!(defaults.model, "gpt-5.4");
    assert!(!defaults.has_key);

    set_assistant_config_with_state(
        AssistantConfigInputDto {
            enabled: true,
            base_url: "https://example.test/v1".to_owned(),
            model: "gpt-5.4".to_owned(),
            api_key: Some("secret-assistant-key".to_owned()),
            clear_api_key: false,
            temperature: 0.2,
            max_tool_iters: 12,
            system_prompt_extra: Some("Prefer concise edits.".to_owned()),
            developer_mode: false,
            enabled_skills: vec![],
        },
        &state,
    )
    .expect("write assistant config");

    let saved = get_assistant_config_with_state(&state).expect("read assistant config");
    let config_path = state.config_root.join("assistant_config.json");
    let config = fs::read_to_string(&config_path).expect("assistant config file");

    assert!(saved.enabled);
    assert!(saved.has_key);
    assert_eq!(saved.base_url, "https://example.test/v1");
    assert_eq!(saved.max_tool_iters, 12);
    assert!(
        !serde_json::to_string(&saved).expect("serialize dto").contains("secret-assistant-key")
    );
    assert!(config.contains("secret-assistant-key"));
    assert!(!config_path.starts_with(&state.root));
}

#[test]
fn capability_manifest_exposes_backend_commands_with_descriptions() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let manifest = get_capability_manifest_with_state(&state).expect("capability manifest");

    let names =
        manifest.capabilities.iter().map(|capability| capability.name.as_str()).collect::<Vec<_>>();
    assert!(names.contains(&"workflow.run"));
    assert!(names.contains(&"asset.list"));
    assert!(names.contains(&"project.create"));
    assert!(names.contains(&"provider.set_key"));
    assert!(names.contains(&"assistant.set_config"));
    assert!(manifest.capabilities.iter().all(|capability| !capability.description.is_empty()));
    assert!(
        manifest.capabilities.iter().all(|capability| capability.parameters.get("type").is_some())
    );
}

#[test]
fn assistant_session_returns_stable_port_and_secret_token_shape() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let first = get_assistant_session_with_state(&state).expect("first assistant session");
    let second = get_assistant_session_with_state(&state).expect("second assistant session");

    assert_eq!(first.port, second.port);
    assert_eq!(first.token, second.token);
    assert!(first.port > 0);
    assert!(first.token.len() >= 32);
    assert!(!first.token.contains(' '));
}

#[test]
fn app_state_wires_the_framed_stdio_command() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    assert!(format!("{:?}", state.assistant_sidecar_command()).contains("assistant.stdio_app"));
}

#[test]
fn declarative_skills_install_enable_list_and_uninstall() {
    let root = tempdir().expect("create asset root");
    let source = tempdir().expect("create skill package");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    fs::write(
        source.path().join("skill.json"),
        r#"{"name":"portrait-helper","version":"1.0.0","description":"Portrait workflow helper","capabilities":["workflow.add_node"],"requires":{}}"#,
    )
    .expect("write skill manifest");
    fs::write(source.path().join("prompt.md"), "Help build portrait workflows.\n")
        .expect("write prompt");

    let installed = install_skill_with_state(source.path().to_string_lossy().into_owned(), &state)
        .expect("install skill");
    set_skill_enabled_with_state("portrait-helper".to_owned(), true, &state).expect("enable skill");
    let skills = list_skills_with_state(&state).expect("list skills");

    assert_eq!(installed.name, "portrait-helper");
    assert!(skills.iter().any(|skill| skill.name == "portrait-helper" && skill.enabled));
    assert!(state.config_root.join("skills/portrait-helper/skill.json").exists());

    uninstall_skill_with_state("portrait-helper".to_owned(), &state).expect("uninstall skill");
    assert!(list_skills_with_state(&state).expect("list after uninstall").is_empty());
    assert!(!state.config_root.join("skills/portrait-helper").exists());
}
