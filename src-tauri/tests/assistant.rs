use oh_my_dream_tauri::commands::{
    get_assistant_config_with_state, set_assistant_config_with_state,
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
    assert!(
        !serde_json::to_string(&saved).expect("serialize dto").contains("secret-assistant-key")
    );
    assert!(config.contains("secret-assistant-key"));
    assert!(!config_path.starts_with(&state.root));
}

#[test]
fn app_state_wires_the_framed_stdio_command() {
    let root = tempdir().expect("create asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    assert!(
        format!("{:?}", state.assistant_sidecar_command()).contains("assistant.protocol_v1_app")
    );
}
