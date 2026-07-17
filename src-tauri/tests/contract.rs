use engine::{NodeExecutionState, NodeProgressEvent, PortType};
use oh_my_dream_tauri::dto::{CapabilityCatalogDto, NodeProgressEventDto};
use oh_my_dream_tauri::project_commands::{
    ProjectDto, ProjectWorkflowReadinessDto, ProjectWorkflowSummaryDto, ProjectWorkspaceDto,
};
use oh_my_dream_tauri::{
    asset_import_source_picker::{
        DesktopAssetImportSourcePickerError, DesktopAssetImportSourcePickerInterface,
        DesktopPickedAssetImportSource,
    },
    composition::{DesktopApplicationPaths, DesktopCompositionRoot},
    generation_provider_settings_commands::{
        GenerationProviderSettingsBindingDto, GenerationProviderSettingsDto,
        GenerationProviderSettingsProfileDto, GenerationProviderSettingsProviderChoiceDto,
        GenerationProviderSettingsRouteChoiceDto,
    },
    generation_task_command_dto::{
        GenerationTaskDto, GenerationTaskFailureDto, GenerationTaskFailureKindDto,
        GenerationTaskListPageDto, GenerationTaskRequestKindDto, GenerationTaskResultDto,
        GenerationTaskStatusDto, GenerationTaskSummaryDto,
    },
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
    let capability_catalog = capability_catalog_fixture();
    let node_contracts = node_contract_fixture();
    let assistant_operations = assistant_operation_contract::fixture();
    let assistant_approval = assistant_approval_contract::fixture();
    let (node_capabilities, generation_profiles) = node_capability_fixtures();
    let generation_provider_settings = generation_provider_settings_fixture();
    let generation_task = generation_task_fixture();
    let generation_tasks = GenerationTaskListPageDto {
        tasks: vec![generation_task.summary.clone(), failed_task_summary(&generation_task.summary)],
        next_cursor: Some("AQIDBAUGBwgJCgsMDQ4PEA".to_owned()),
    };

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
    let task_json = serde_json::to_string(&generation_task).expect("serialize task");
    for prohibited in ["route_id", "remote_task_id", "credential", "signed_url", "raw_payload"] {
        assert!(!task_json.contains(prohibited), "Task leaked {prohibited}");
    }
    assistant_operation_contract::assert_fixture(&assistant_operations);
    write_fixture("workflow.json", &workflow);
    write_fixture("workflow_run.json", &workflow_run);
    write_fixture("workflow_run_events.json", &workflow_events);
    write_fixture("asset.json", &asset);
    write_fixture("project.json", &project);
    write_fixture("open_project.json", &open_project);
    write_fixture("node_progress_event.json", &progress);
    write_fixture("capability_catalog.json", &capability_catalog);
    write_fixture("node_contracts.json", &node_contracts);
    write_fixture("assistant_operations.json", &assistant_operations);
    write_fixture("assistant_approval.json", &assistant_approval);
    write_fixture("node_capabilities.json", &node_capabilities);
    write_fixture("generation_profiles.json", &generation_profiles);
    write_fixture("generation_provider_settings.json", &generation_provider_settings);
    write_fixture("generation_task.json", &generation_task);
    write_fixture("generation_tasks.json", &generation_tasks);
}

fn failed_task_summary(summary: &GenerationTaskSummaryDto) -> GenerationTaskSummaryDto {
    let mut failed = summary.clone();
    failed.status = GenerationTaskStatusDto::Failed;
    failed.preview_asset_id = None;
    failed.has_result = false;
    failed.failure = Some(GenerationTaskFailureDto {
        kind: GenerationTaskFailureKindDto::ProviderRejected,
        code: "CONTENT_POLICY".to_owned(),
        message: "The provider rejected this request.".to_owned(),
    });
    failed.updated_at_epoch_ms = "2100".to_owned();
    failed.completed_at_epoch_ms = Some("2100".to_owned());
    failed
}

fn generation_task_fixture() -> GenerationTaskDto {
    let summary = GenerationTaskSummaryDto {
        id: "123e4567-e89b-42d3-a456-426600000030".to_owned(),
        project_id: "123e4567-e89b-42d3-a456-426600000001".to_owned(),
        workflow_id: "123e4567-e89b-42d3-a456-426600000002".to_owned(),
        workflow_run_id: "123e4567-e89b-42d3-a456-426600000031".to_owned(),
        workflow_node_id: "123e4567-e89b-42d3-a456-426600000032".to_owned(),
        workflow_node_execution_id: "123e4567-e89b-42d3-a456-426600000033".to_owned(),
        request_kind: GenerationTaskRequestKindDto::Image,
        status: GenerationTaskStatusDto::Succeeded,
        progress_percent: None,
        generation_profile_ref: "image.high_quality_general@1".to_owned(),
        provider_id: "mock".to_owned(),
        provider_display_name: Some("Mock".to_owned()),
        prompt_preview: Some("A lighthouse above a stormy sea".to_owned()),
        preview_asset_id: Some("123e4567-e89b-42d3-a456-426600000020".to_owned()),
        has_result: true,
        failure: None,
        created_at_epoch_ms: "1000".to_owned(),
        updated_at_epoch_ms: "2000".to_owned(),
        completed_at_epoch_ms: Some("2000".to_owned()),
    };
    GenerationTaskDto {
        summary,
        result: Some(GenerationTaskResultDto::Asset {
            asset_id: "123e4567-e89b-42d3-a456-426600000020".to_owned(),
            media_kind: "image".to_owned(),
        }),
    }
}

fn generation_provider_settings_fixture() -> GenerationProviderSettingsDto {
    GenerationProviderSettingsDto {
        settings_revision: "1".to_owned(),
        profiles: [
            (
                "image.high_quality_general@1",
                "image",
                "mock.image.high-quality-general.v1",
                "High Quality General Image",
            ),
            (
                "speech.multilingual_narration@1",
                "voice",
                "mock.voice.multilingual-narration.v1",
                "Multilingual Narration",
            ),
            (
                "video.cinematic_image_animation@1",
                "video",
                "mock.video.cinematic-image-animation.v1",
                "Cinematic Image Animation",
            ),
        ]
        .into_iter()
        .map(|(profile_ref, generation_kind, route_id, route_name)| {
            GenerationProviderSettingsProfileDto {
                profile_ref: profile_ref.to_owned(),
                generation_kind: generation_kind.to_owned(),
                selected_binding: Some(GenerationProviderSettingsBindingDto {
                    provider_id: "mock".to_owned(),
                    route_id: route_id.to_owned(),
                }),
                provider_choices: vec![GenerationProviderSettingsProviderChoiceDto {
                    provider_id: "mock".to_owned(),
                    display_name: "Mock".to_owned(),
                    routes: vec![GenerationProviderSettingsRouteChoiceDto {
                        route_id: route_id.to_owned(),
                        display_name: route_name.to_owned(),
                    }],
                }],
            }
        })
        .collect(),
    }
}

fn node_capability_fixtures() -> (serde_json::Value, serde_json::Value) {
    let directory = tempdir().expect("node capability fixture root");
    tauri::async_runtime::block_on(async {
        let dependencies = DesktopCompositionRoot::compose_activated_commands_with_emitter(
            DesktopApplicationPaths::from_application_data_root(directory.path()),
            Arc::new(ContractEventEmitterImpl),
            Arc::new(CancelledAssetPickerImpl),
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

struct ContractEventEmitterImpl;

struct CancelledAssetPickerImpl;

#[async_trait::async_trait]
impl DesktopAssetImportSourcePickerInterface for CancelledAssetPickerImpl {
    async fn pick_asset_import_source(
        &self,
        _expected_media_kind: assets::asset::domain::AssetMediaKind,
    ) -> Result<Option<DesktopPickedAssetImportSource>, DesktopAssetImportSourcePickerError> {
        Ok(None)
    }
}

impl DesktopEventEmitterInterface for ContractEventEmitterImpl {
    fn emit_desktop_event(
        &self,
        _event_name: &str,
        _payload: serde_json::Value,
    ) -> Result<(), DesktopEventEmissionError> {
        Ok(())
    }
}

fn capability_catalog_fixture() -> CapabilityCatalogDto {
    let root = tempdir().expect("create capability catalog roots");
    let state = oh_my_dream_tauri::state::AppState::from_roots(
        root.path().join("assets"),
        root.path().join("config"),
    )
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
    let root = tempdir().expect("create node contract roots");
    let state = oh_my_dream_tauri::state::AppState::from_roots(
        root.path().join("assets"),
        root.path().join("config"),
    )
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
