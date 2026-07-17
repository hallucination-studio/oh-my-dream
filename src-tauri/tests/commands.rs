use oh_my_dream_tauri::commands::{
    get_providers_with_state, set_active_provider_with_state, set_provider_key_with_state,
};
use oh_my_dream_tauri::state::AppState;
use tempfile::tempdir;

#[test]
fn unavailable_providers_cannot_be_selected_or_configured() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let selection_error = set_active_provider_with_state("replicate".to_owned(), &state)
        .expect_err("unavailable provider should not be selected");
    let key_error =
        set_provider_key_with_state("replicate".to_owned(), "secret-token".to_owned(), &state)
            .expect_err("unavailable provider should not accept credentials");

    let providers = get_providers_with_state(&state).expect("get providers");
    let config_path = state.config_root.join("provider_config.json");

    assert!(selection_error.contains("unknown provider `replicate`"));
    assert!(key_error.contains("unknown provider `replicate`"));
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].id, "mock");
    assert!(providers[0].active);
    assert!(!config_path.exists());
}
