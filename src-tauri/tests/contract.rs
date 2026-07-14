use engine::{NodeExecutionState, NodeProgressEvent, PortType, RunOutputs, Value, ValueMap};
use oh_my_dream_tauri::dto::{
    AssetDto, AssistantConfigDto, CapabilityCatalogDto, NodeProgressEventDto, OpenProjectResultDto,
    ProjectDto, RunWorkflowResultDto, WorkflowHeadDto,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[path = "contract/assistant_operation_contract.rs"]
mod assistant_operation_contract;

#[test]
fn writes_frontend_contract_fixtures_with_frozen_dto_shapes() {
    let run_result = run_workflow_fixture();
    let asset = asset_fixture();
    let project = project_fixture();
    let open_project = open_project_fixture();
    let progress = progress_fixture();
    let assistant_config = assistant_config_fixture();
    let capability_catalog = capability_catalog_fixture();
    let node_contracts = node_contract_fixture();
    let assistant_operations = assistant_operation_contract::fixture();

    assert_eq!(
        serde_json::to_value(&run_result).expect("serialize run workflow result"),
        json!({
            "outputs": {
                "video": {
                    "video": {
                        "kind": "video",
                        "value": "mock://mock/image-to-video/task-2"
                    }
                }
            }
        })
    );
    assert_eq!(
        serde_json::to_value(&asset).expect("serialize asset"),
        json!({
            "id": "asset-0000000000000001",
            "kind": "video",
            "file_path": "/tmp/oh-my-dream/assets/files/asset-0000000000000001.mp4",
            "thumbnail_path": "/tmp/oh-my-dream/assets/thumbnails/asset-0000000000000001.png",
            "workflow_snapshot": {},
            "prompt": "a red fox",
            "project_id": "project-0000000000000001",
            "project_name": "Default",
            "source_node_id": "video",
            "source_node_type": "ImageToVideo",
            "model": "mock-video",
            "seed": "18446744073709551615",
            "cost": 900,
            "tags": [],
            "created_at": 0
        })
    );
    assert_eq!(
        serde_json::to_value(&project).expect("serialize project"),
        json!({
            "id": "project-0000000000000001",
            "name": "Default",
            "created_at": 0
        })
    );
    assert_eq!(
        serde_json::to_value(&open_project).expect("serialize open project result"),
        json!({
            "project": {
                "id": "project-0000000000000001",
                "name": "Default",
                "created_at": 0
            },
            "workflow_head": {
                "project_id": "project-0000000000000001",
                "revision": 1,
                "workflow": {
                    "version": "1.0",
                    "project_id": "project-0000000000000001",
                    "nodes": [{
                        "id": "prompt",
                        "type": "TextPrompt",
                        "contract_version": "1.0",
                        "params": {"text": "hello"},
                        "inputs": {},
                        "position": null
                    }]
                }
            }
        })
    );
    assert_eq!(
        serde_json::to_value(&progress).expect("serialize progress"),
        json!({
            "node_id": "video",
            "state": "done",
            "progress": 1.0,
            "cost": 900
        })
    );
    assert_eq!(
        serde_json::to_value(&assistant_config).expect("serialize assistant config"),
        json!({
            "enabled": true,
            "base_url": "https://api.openai.com/v1",
            "model": "gpt-5.4",
            "has_key": false
        })
    );
    assistant_operation_contract::assert_fixture(&assistant_operations);
    write_fixture("run_workflow_result.json", &run_result);
    write_fixture("asset.json", &asset);
    write_fixture("project.json", &project);
    write_fixture("open_project.json", &open_project);
    write_fixture("node_progress_event.json", &progress);
    write_fixture("assistant_config.json", &assistant_config);
    write_fixture("capability_catalog.json", &capability_catalog);
    write_fixture("node_contracts.json", &node_contracts);
    write_fixture("assistant_operations.json", &assistant_operations);
}

fn capability_catalog_fixture() -> CapabilityCatalogDto {
    let root = tempdir().expect("create capability catalog asset root");
    let state = oh_my_dream_tauri::state::AppState::from_asset_root(root.path())
        .expect("build capability catalog app state");
    oh_my_dream_tauri::commands::get_capability_catalog_with_state(&state)
        .expect("project capability catalog")
}

#[derive(serde::Serialize)]
struct NodeContractsFixture {
    port_types: Vec<PortType>,
    compatible: Vec<(PortType, PortType)>,
    nodes: Vec<NodeContractFixture>,
}

#[derive(serde::Serialize)]
struct NodeContractFixture {
    type_id: String,
    inputs: Vec<InputContractFixture>,
    outputs: Vec<PortContractFixture>,
}

#[derive(serde::Serialize)]
struct InputContractFixture {
    name: String,
    port_type: PortType,
    required: bool,
}

#[derive(serde::Serialize)]
struct PortContractFixture {
    name: String,
    port_type: PortType,
}

fn node_contract_fixture() -> NodeContractsFixture {
    let root = tempdir().expect("create node contract asset root");
    let state = oh_my_dream_tauri::state::AppState::from_asset_root(root.path())
        .expect("build node contract app state");
    let nodes = state
        .registry
        .capability_refs()
        .into_iter()
        .map(|reference| {
            let contract = state
                .registry
                .capability(reference)
                .expect("load node contract")
                .contract();
            NodeContractFixture {
                type_id: reference.id.clone(),
                inputs: contract
                    .inputs
                    .iter()
                    .map(|port| InputContractFixture {
                        name: port.name.clone(),
                        port_type: port.port_type,
                        required: port.required,
                    })
                    .collect(),
                outputs: contract
                    .outputs
                    .iter()
                    .map(|port| PortContractFixture {
                        name: port.name.clone(),
                        port_type: port.port_type,
                    })
                    .collect(),
            }
        })
        .collect();
    let compatible = PortType::ALL
        .into_iter()
        .flat_map(|from| {
            PortType::ALL
                .into_iter()
                .filter(move |to| from.is_compatible_with(*to))
                .map(move |to| (from, to))
        })
        .collect();
    NodeContractsFixture { port_types: PortType::ALL.to_vec(), compatible, nodes }
}

fn run_workflow_fixture() -> RunWorkflowResultDto {
    let outputs = RunOutputs::from([(
        "video".to_owned(),
        ValueMap::from([(
            "video".to_owned(),
            Value::Video("mock://mock/image-to-video/task-2".to_owned()),
        )]),
    )]);
    RunWorkflowResultDto::from_outputs(&outputs)
}

fn asset_fixture() -> AssetDto {
    AssetDto {
        id: "asset-0000000000000001".to_owned(),
        kind: "video".to_owned(),
        file_path: "/tmp/oh-my-dream/assets/files/asset-0000000000000001.mp4".to_owned(),
        thumbnail_path: Some(
            "/tmp/oh-my-dream/assets/thumbnails/asset-0000000000000001.png".to_owned(),
        ),
        workflow_snapshot: json!(BTreeMap::<String, serde_json::Value>::new()),
        prompt: Some("a red fox".to_owned()),
        project_id: Some("project-0000000000000001".to_owned()),
        project_name: Some("Default".to_owned()),
        source_node_id: Some("video".to_owned()),
        source_node_type: Some("ImageToVideo".to_owned()),
        model: Some("mock-video".to_owned()),
        seed: Some("18446744073709551615".to_owned()),
        cost: Some(900),
        tags: Vec::new(),
        created_at: 0,
    }
}

fn project_fixture() -> ProjectDto {
    ProjectDto {
        id: "project-0000000000000001".to_owned(),
        name: "Default".to_owned(),
        created_at: 0,
    }
}

fn open_project_fixture() -> OpenProjectResultDto {
    OpenProjectResultDto {
        project: project_fixture(),
        workflow_head: Some(WorkflowHeadDto {
            project_id: "project-0000000000000001".to_owned(),
            revision: 1,
            workflow: json!({
                "version": "1.0",
                "project_id": "project-0000000000000001",
                "nodes": [{
                    "id": "prompt",
                    "type": "TextPrompt",
                    "contract_version": "1.0",
                    "params": {"text": "hello"},
                    "inputs": {},
                    "position": null
                }]
            }),
        }),
    }
}

fn progress_fixture() -> NodeProgressEventDto {
    NodeProgressEventDto::from(NodeProgressEvent {
        node_id: "video".to_owned(),
        state: NodeExecutionState::Done,
        progress: Some(1.0),
        cost: Some(900),
    })
}

fn assistant_config_fixture() -> AssistantConfigDto {
    AssistantConfigDto {
        enabled: true,
        base_url: "https://api.openai.com/v1".to_owned(),
        model: "gpt-5.4".to_owned(),
        has_key: false,
    }
}

fn write_fixture<T: serde::Serialize>(file_name: &str, value: &T) {
    let directory = fixture_directory();
    fs::create_dir_all(&directory).expect("create frontend fixture directory");
    let json = serde_json::to_string_pretty(value).expect("serialize fixture");
    fs::write(directory.join(file_name), format!("{json}\n")).expect("write fixture");
}

fn fixture_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("ui/src/__fixtures__")
}
