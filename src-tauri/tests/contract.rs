use engine::{NodeExecutionState, NodeProgressEvent, PortType};
use oh_my_dream_tauri::dto::{AssistantConfigDto, CapabilityCatalogDto, NodeProgressEventDto};
use oh_my_dream_tauri::project_commands::{
    ProjectDto, ProjectWorkflowReadinessDto, ProjectWorkflowSummaryDto, ProjectWorkspaceDto,
};
use oh_my_dream_tauri::{
    asset_import_source_picker::{
        DesktopAssetImportSourcePickerError, DesktopAssetImportSourcePickerInterface,
        DesktopPickedAssetImportSource,
    },
    composition::{DesktopApplicationPaths, DesktopCompositionRoot},
    node_capability_commands::{
        GenerationProfileListForCapabilityRequestDto, generation_profile_list_with_dependencies,
        node_capability_list_with_dependencies,
    },
    workflow_command_dto::{
        WorkflowCanvasPositionDto, WorkflowDto, WorkflowNodeDto, WorkflowParameterDto,
        WorkflowRunDto, WorkflowRunEventPageDto, WorkflowRunNodeExecutionDto,
    },
    workflow_readiness_dto::{WorkflowReadinessDto, WorkflowWithReadinessDto},
    workflow_run_event_publisher::{DesktopEventEmissionError, DesktopEventEmitterInterface},
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::tempdir;

#[path = "contract/asset_contract.rs"]
mod asset_contract;
#[path = "contract/assistant_approval_contract.rs"]
mod assistant_approval_contract;
#[path = "contract/assistant_operation_contract.rs"]
mod assistant_operation_contract;

#[test]
fn writes_frontend_contract_fixtures_with_frozen_dto_shapes() {
    let workflow = workflow_fixture();
    let workflow_run = workflow_run_fixture();
    let workflow_events = workflow_event_fixture();
    let asset = asset_contract::fixture();
    let project = project_fixture();
    let open_project = open_project_fixture();
    let progress = progress_fixture();
    let assistant_config = assistant_config_fixture();
    let capability_catalog = capability_catalog_fixture();
    let node_contracts = node_contract_fixture();
    let assistant_operations = assistant_operation_contract::fixture();
    let assistant_approval = assistant_approval_contract::fixture();
    let (node_capabilities, generation_profiles) = node_capability_fixtures();

    assert_eq!(workflow.workflow.workflow_id, "123e4567-e89b-42d3-a456-426600000010");
    assert_eq!(workflow_run.workflow_revision, "1");
    assert_eq!(workflow_events.next_sequence, None);
    assert_eq!(
        serde_json::to_value(&asset).expect("serialize asset"),
        json!({
            "asset_id": "123e4567-e89b-42d3-a456-426600000020",
            "project_id": "123e4567-e89b-42d3-a456-426600000001",
            "media_kind": "video",
            "content_state": "available",
            "display_name": "A red fox",
            "created_at_epoch_ms": "0",
            "content": {
                "content_fingerprint_hex": "00".repeat(32),
                "byte_length": "1024",
                "mime_type": "video/mp4"
            },
            "media_facts": {
                "kind": "video",
                "width": 1920,
                "height": 1080,
                "duration_ms": "1000",
                "has_audio": true
            },
            "origin": {
                "kind": "imported",
                "original_file_name": "fox.mp4"
            }
        })
    );
    assert_eq!(
        serde_json::to_value(&project).expect("serialize project"),
        json!({
            "id": "123e4567-e89b-42d3-a456-426600000001",
            "name": "Default",
            "revision": "1",
            "created_at_epoch_ms": "0",
            "updated_at_epoch_ms": "0"
        })
    );
    assert_eq!(
        serde_json::to_value(&open_project).expect("serialize open project result"),
        json!({
            "project": {
                "id": "123e4567-e89b-42d3-a456-426600000001",
                "name": "Default",
                "revision": "1",
                "created_at_epoch_ms": "0",
                "updated_at_epoch_ms": "0"
            },
            "current_workflow_summary": {
                "workflow_id": "123e4567-e89b-42d3-a456-426600000002",
                "workflow_revision": "1",
                "readiness": "ready"
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
    write_fixture("workflow.json", &workflow);
    write_fixture("workflow_run.json", &workflow_run);
    write_fixture("workflow_run_events.json", &workflow_events);
    write_fixture("asset.json", &asset);
    write_fixture("project.json", &project);
    write_fixture("open_project.json", &open_project);
    write_fixture("node_progress_event.json", &progress);
    write_fixture("assistant_config.json", &assistant_config);
    write_fixture("capability_catalog.json", &capability_catalog);
    write_fixture("node_contracts.json", &node_contracts);
    write_fixture("assistant_operations.json", &assistant_operations);
    write_fixture("assistant_approval.json", &assistant_approval);
    write_fixture("node_capabilities.json", &node_capabilities);
    write_fixture("generation_profiles.json", &generation_profiles);
}

fn node_capability_fixtures() -> (serde_json::Value, serde_json::Value) {
    let directory = tempdir().expect("node capability fixture root");
    tauri::async_runtime::block_on(async {
        let dependencies = DesktopCompositionRoot::compose_activated_commands_with_emitter(
            DesktopApplicationPaths::from_application_data_root(directory.path()),
            Arc::new(ContractEventEmitter),
            Arc::new(CancelledAssetPicker),
        )
        .await
        .expect("compose activated commands");
        let contracts = node_capability_list_with_dependencies(&dependencies);
        let profiles = generation_profile_list_with_dependencies(
            GenerationProfileListForCapabilityRequestDto {
                capability_id: "image.generate_from_text".to_owned(),
                capability_version: "1.0".to_owned(),
            },
            &dependencies,
        )
        .await
        .expect("list profiles");
        let mut profiles = serde_json::to_value(profiles).expect("serialize profiles");
        for item in profiles.as_array_mut().expect("profile fixture array") {
            item["availability"]["observed_at_epoch_ms"] = json!("0");
            item["availability"]["expires_at_epoch_ms"] = json!("30000");
        }
        (serde_json::to_value(contracts).expect("serialize contracts"), profiles)
    })
}

struct ContractEventEmitter;

struct CancelledAssetPicker;

#[async_trait::async_trait]
impl DesktopAssetImportSourcePickerInterface for CancelledAssetPicker {
    async fn pick_asset_import_source(
        &self,
        _expected_media_kind: assets::asset::domain::AssetMediaKind,
    ) -> Result<Option<DesktopPickedAssetImportSource>, DesktopAssetImportSourcePickerError> {
        Ok(None)
    }
}

impl DesktopEventEmitterInterface for ContractEventEmitter {
    fn emit_desktop_event(
        &self,
        _event_name: &str,
        _payload: serde_json::Value,
    ) -> Result<(), DesktopEventEmissionError> {
        Ok(())
    }
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
            let contract =
                state.registry.capability(reference).expect("load node contract").contract();
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

fn workflow_fixture() -> WorkflowWithReadinessDto {
    WorkflowWithReadinessDto {
        workflow: WorkflowDto {
            schema_version: 1,
            workflow_id: "123e4567-e89b-42d3-a456-426600000010".to_owned(),
            project_id: "123e4567-e89b-42d3-a456-426600000001".to_owned(),
            revision: "1".to_owned(),
            created_at_epoch_ms: "0".to_owned(),
            updated_at_epoch_ms: "0".to_owned(),
            nodes: vec![WorkflowNodeDto {
                node_id: "123e4567-e89b-42d3-a456-426600000011".to_owned(),
                capability_id: "text.provide_literal".to_owned(),
                capability_version: "1.0".to_owned(),
                parameters: vec![WorkflowParameterDto {
                    key: "text".to_owned(),
                    value: json!({"kind":"text","value":"hello"}),
                }],
                canvas_position: WorkflowCanvasPositionDto { x: 100.0, y: 120.0 },
            }],
            input_bindings: Vec::new(),
        },
        readiness: WorkflowReadinessDto::Ready,
    }
}

fn workflow_run_fixture() -> WorkflowRunDto {
    WorkflowRunDto {
        workflow_run_id: "123e4567-e89b-42d3-a456-426600000012".to_owned(),
        project_id: "123e4567-e89b-42d3-a456-426600000001".to_owned(),
        workflow_id: "123e4567-e89b-42d3-a456-426600000010".to_owned(),
        workflow_revision: "1".to_owned(),
        scope: json!({"kind":"whole_workflow"}),
        state: "queued".to_owned(),
        created_at_epoch_ms: "1".to_owned(),
        updated_at_epoch_ms: "1".to_owned(),
        node_executions: vec![WorkflowRunNodeExecutionDto {
            node_id: "123e4567-e89b-42d3-a456-426600000011".to_owned(),
            node_execution_id: "123e4567-e89b-42d3-a456-426600000013".to_owned(),
            state: "pending".to_owned(),
            progress_basis_points: None,
        }],
    }
}

fn workflow_event_fixture() -> WorkflowRunEventPageDto {
    WorkflowRunEventPageDto {
        events: vec![json!({
            "workflow_run_id":"123e4567-e89b-42d3-a456-426600000012",
            "sequence":"1",
            "occurred_at_epoch_ms":"1",
            "payload":{"type":"run_queued"},
        })],
        next_sequence: None,
    }
}

fn project_fixture() -> ProjectDto {
    ProjectDto {
        id: "123e4567-e89b-42d3-a456-426600000001".to_owned(),
        name: "Default".to_owned(),
        revision: "1".to_owned(),
        created_at_epoch_ms: "0".to_owned(),
        updated_at_epoch_ms: "0".to_owned(),
    }
}

fn open_project_fixture() -> ProjectWorkspaceDto {
    ProjectWorkspaceDto {
        project: project_fixture(),
        current_workflow_summary: Some(ProjectWorkflowSummaryDto {
            workflow_id: "123e4567-e89b-42d3-a456-426600000002".to_owned(),
            workflow_revision: "1".to_owned(),
            readiness: ProjectWorkflowReadinessDto::Ready,
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
