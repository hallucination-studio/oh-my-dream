use assets::{AssetKind, NewAsset};
use engine::{CapabilityRef, NodeProgressEvent, WorkflowPatchOperation};
use oh_my_dream_tauri::commands::{
    assets_root_with_state, create_project_with_state, get_providers_with_state,
    list_assets_with_state, list_projects_with_state, open_project_with_state,
    parse_asset_kind_filter, parse_asset_sort, run_workflow_with_state_and_observer,
    set_active_provider_with_state, set_provider_key_with_state, workflow_apply_patch_with_state,
};
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_authority::WorkflowCommitRequest;
use oh_my_dream_tauri::workflow_patch_operation::WorkflowApplyPatchInput;
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
fn project_commands_create_list_and_open_without_a_workflow_head() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let project = create_project_with_state("Launch".to_owned(), &state).expect("create project");
    let projects = list_projects_with_state(&state).expect("list projects");
    let workspace = open_project_with_state(project.id.clone(), &state).expect("open project");

    assert_eq!(
        projects.iter().map(|project| project.name.as_str()).collect::<Vec<_>>(),
        vec!["Launch"]
    );
    assert_eq!(workspace.project.id, project.id);
    assert!(workspace.workflow_head.is_none());
}

#[test]
fn open_project_returns_the_authoritative_workflow_head() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let project = create_project_with_state("Headed".to_owned(), &state).expect("create project");
    let workflow = serde_json::from_value(json!({
        "version": "1.0",
        "project_id": project.id,
        "nodes": [{
            "id": "prompt",
            "type": "TextPrompt",
            "contract_version": "1.0",
            "params": {"text": "hello"},
            "inputs": {},
            "position": null
        }]
    }))
    .expect("build empty Workflow document");
    state
        .workflow_authority
        .apply(WorkflowCommitRequest::new(
            project.id.clone(),
            None,
            "request-1",
            "hash-1",
            workflow,
        ))
        .expect("persist Workflow head");

    let opened = open_project_with_state(project.id, &state).expect("open project");
    let head = opened.workflow_head.expect("head should be returned");
    assert_eq!(head.revision, 1);
    assert_eq!(head.workflow["nodes"][0]["id"], "prompt");
}

#[test]
fn ui_patch_command_uses_the_authoritative_workflow_service() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let project = create_project_with_state("Patched".to_owned(), &state).expect("create project");

    let output = workflow_apply_patch_with_state(
        project.id.clone(),
        "ui-request-1".to_owned(),
        WorkflowApplyPatchInput {
            expected_revision: None,
            operations: vec![WorkflowPatchOperation::AddNode {
                alias: "prompt".to_owned(),
                capability: CapabilityRef::new("TextPrompt", "1.0"),
                params: serde_json::Map::from_iter([("text".to_owned(), json!("hello"))]),
                position: Some([20.0, 30.0]),
            }],
        },
        &state,
    )
    .expect("apply UI patch");

    let head = output.workflow_head.expect("patch creates a head");
    let opened = open_project_with_state(project.id, &state).expect("open project");
    assert_eq!(head.revision, 1);
    assert_eq!(opened.workflow_head, Some(head));
}

#[test]
fn workflow_commands_reject_unknown_project_ids() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let workflow = json!({
        "version": "1.0",
        "project_id": "missing-project",
        "nodes": []
    })
    .to_string();

    let run_error = run_workflow_with_state_and_observer(workflow, &state, &mut |_event| {})
        .expect_err("run should fail");

    assert!(run_error.contains("validate project"));
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

#[test]
fn app_state_does_not_seed_a_default_project() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");

    let projects = list_projects_with_state(&state).expect("list projects");

    assert!(projects.is_empty());
}

#[test]
fn run_workflow_helper_forwards_node_progress_events() {
    let root = tempdir().expect("create temp asset root");
    let state = AppState::from_asset_root(root.path()).expect("build app state");
    let project = create_project_with_state("Default".to_owned(), &state).expect("create project");
    let workflow = json!({
        "version": "1.0",
        "project_id": project.id,
        "nodes": [
            { "id": "prompt", "type": "TextPrompt", "params": { "text": "a red fox" }, "inputs": {} },
            { "id": "image", "type": "TextToImage", "params": {}, "inputs": { "prompt": ["prompt", "text"] } }
        ]
    });
    let mut events = Vec::<NodeProgressEvent>::new();

    run_workflow_with_state_and_observer(workflow.to_string(), &state, &mut |event| {
        events.push(event.clone());
    })
    .expect("workflow should run");

    assert!(events.iter().any(|event| event.node_id == "image" && event.progress == Some(0.25)));
    assert!(events.iter().any(|event| event.node_id == "image" && event.cost == Some(250)));
}

fn write_png(path: &std::path::Path) {
    const PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    fs::write(path, PNG).expect("png should write");
}
