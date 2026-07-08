use assets::AssetKind;
use engine::{NodeParams, OutputRef, Workflow, WorkflowNode};
use oh_my_dream_tauri::commands::{
    assets_root_with_state, enrich_save_asset_params, parse_asset_kind_filter,
};
use oh_my_dream_tauri::state::AppState;
use serde_json::json;
use std::collections::BTreeMap;
use tempfile::tempdir;

#[test]
fn enriches_save_asset_nodes_with_snapshot_and_source_node() {
    let workflow = workflow_with_save_asset();

    let enriched = enrich_save_asset_params(&workflow).expect("workflow should enrich");
    let save = enriched.nodes.iter().find(|node| node.id == "save").expect("save node exists");

    assert_eq!(save.params.get("source_node_id"), Some(&json!("video")));
    assert_eq!(save.params["workflow_snapshot"]["version"], json!("1.0"));
    assert_eq!(save.params["workflow_snapshot"]["nodes"][0]["id"], json!("video"));
}

#[test]
fn parses_asset_kind_filter_for_commands() {
    assert_eq!(parse_asset_kind_filter(None).expect("none should parse"), None);
    assert_eq!(
        parse_asset_kind_filter(Some("video".to_owned())).expect("video should parse"),
        Some(AssetKind::Video)
    );
    assert!(parse_asset_kind_filter(Some("audio".to_owned())).is_err());
}

#[test]
fn returns_configured_asset_root_for_commands() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let returned = assets_root_with_state(&state).expect("asset root should be returned");

    assert_eq!(returned, root.path().to_string_lossy());
}

fn workflow_with_save_asset() -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        nodes: vec![
            WorkflowNode {
                id: "video".to_owned(),
                type_id: "ImageToVideo".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "save".to_owned(),
                type_id: "SaveAsset".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::from([(
                    "media".to_owned(),
                    OutputRef("video".to_owned(), "video".to_owned()),
                )]),
                position: None,
            },
        ],
    }
}
