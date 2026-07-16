use assets::{AssetKind, NewAsset};
use oh_my_dream_tauri::commands::{
    assets_root_with_state, get_providers_with_state, list_assets_with_state,
    parse_asset_kind_filter, parse_asset_sort, set_active_provider_with_state,
    set_provider_key_with_state,
};
use oh_my_dream_tauri::state::AppState;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn parses_asset_kind_filter_for_commands() {
    assert_eq!(parse_asset_kind_filter(None).expect("none should parse"), None);
    assert_eq!(
        parse_asset_kind_filter(Some("video".to_owned())).expect("video should parse"),
        Some(AssetKind::Video)
    );
    assert_eq!(
        parse_asset_kind_filter(Some("audio".to_owned())).expect("audio should parse"),
        Some(AssetKind::Audio)
    );
    assert_eq!(
        parse_asset_sort(None).expect("default sort should parse"),
        assets::AssetSort::Newest
    );
    assert_eq!(
        parse_asset_sort(Some("cost_desc".to_owned())).expect("cost sort should parse"),
        assets::AssetSort::CostDesc
    );
}

#[test]
fn returns_configured_asset_root_for_commands() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let returned = assets_root_with_state(&state).expect("asset root should be returned");

    assert_eq!(returned, root.path().to_string_lossy());
}

#[test]
fn list_assets_command_applies_filters_and_search() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    state
        .store
        .lock()
        .expect("store lock")
        .create_project_with_id("project-a", "A")
        .expect("project should be created");
    let image_path = root.path().join("source.png");
    write_png(&image_path);
    state
        .store
        .lock()
        .expect("store lock")
        .insert(NewAsset {
            kind: AssetKind::Image,
            file_path: image_path.to_string_lossy().into_owned(),
            workflow_snapshot: json!({}),
            prompt: Some("quiet ocean".to_owned()),
            project_id: Some("project-a".to_owned()),
            project_name: Some("A".to_owned()),
            source_node_id: Some("image".to_owned()),
            source_node_type: Some("TextToImage".to_owned()),
            model: Some("mock-image".to_owned()),
            seed: Some(4),
            cost: Some(250),
            tags: Vec::new(),
        })
        .expect("insert asset");

    let assets = list_assets_with_state(
        Some("image".to_owned()),
        Some("project-a".to_owned()),
        Some("mock-image".to_owned()),
        Some("ocean".to_owned()),
        Some("cost_desc".to_owned()),
        &state,
    )
    .expect("list assets");

    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].prompt.as_deref(), Some("quiet ocean"));
    assert_eq!(assets[0].cost, Some(250));
}

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

fn write_png(path: &std::path::Path) {
    const PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    fs::write(path, PNG).expect("png should write");
}
