use engine::{NodeExecutionState, NodeProgressEvent, RunOutputs, Value, ValueMap};
use oh_my_dream_tauri::dto::{
    AssetDto, AssistantConfigDto, AssistantSessionDto, AssistantSkillsDto, CapabilityDto,
    CapabilityManifestDto, NodeProgressEventDto, ProjectDto, RunWorkflowResultDto, SkillDto,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn writes_frontend_contract_fixtures_with_frozen_dto_shapes() {
    let run_result = run_workflow_fixture();
    let asset = asset_fixture();
    let project = project_fixture();
    let progress = progress_fixture();
    let assistant_config = assistant_config_fixture();
    let assistant_session = assistant_session_fixture();
    let capability_manifest = capability_manifest_fixture();
    let skill = skill_fixture();

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
            "seed": null,
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
            "has_key": false,
            "temperature": 0.3,
            "max_tool_iters": 20,
            "system_prompt_extra": null,
            "developer_mode": false,
            "skills": { "installed": ["portrait-helper"], "enabled": [] }
        })
    );
    assert_eq!(
        serde_json::to_value(&assistant_session).expect("serialize assistant session"),
        json!({ "port": 55123, "token": "abcdef0123456789abcdef0123456789" })
    );
    assert_eq!(
        serde_json::to_value(&capability_manifest).expect("serialize capability manifest"),
        json!({
            "capabilities": [{
                "name": "workflow.run",
                "description": "Run the current workflow.",
                "kind": "backend",
                "command": "run_workflow",
                "parameters": { "type": "object", "properties": {} },
                "returns": { "type": "object" },
                "confirm": true
            }]
        })
    );
    assert_eq!(
        serde_json::to_value(&skill).expect("serialize skill"),
        json!({
            "name": "portrait-helper",
            "version": "1.0.0",
            "description": "Portrait workflow helper",
            "enabled": false,
            "developer_mode_required": false,
            "status": "disabled"
        })
    );

    write_fixture("run_workflow_result.json", &run_result);
    write_fixture("asset.json", &asset);
    write_fixture("project.json", &project);
    write_fixture("node_progress_event.json", &progress);
    write_fixture("assistant_config.json", &assistant_config);
    write_fixture("assistant_session.json", &assistant_session);
    write_fixture("capability_manifest.json", &capability_manifest);
    write_fixture("skill.json", &skill);
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
        seed: None,
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
        temperature: 0.3,
        max_tool_iters: 20,
        system_prompt_extra: None,
        developer_mode: false,
        skills: AssistantSkillsDto {
            installed: vec!["portrait-helper".to_owned()],
            enabled: Vec::new(),
        },
    }
}

fn assistant_session_fixture() -> AssistantSessionDto {
    AssistantSessionDto { port: 55123, token: "abcdef0123456789abcdef0123456789".to_owned() }
}

fn capability_manifest_fixture() -> CapabilityManifestDto {
    CapabilityManifestDto {
        capabilities: vec![CapabilityDto {
            name: "workflow.run".to_owned(),
            description: "Run the current workflow.".to_owned(),
            kind: "backend".to_owned(),
            command: Some("run_workflow".to_owned()),
            parameters: json!({ "type": "object", "properties": {} }),
            returns: json!({ "type": "object" }),
            confirm: true,
        }],
    }
}

fn skill_fixture() -> SkillDto {
    SkillDto {
        name: "portrait-helper".to_owned(),
        version: "1.0.0".to_owned(),
        description: "Portrait workflow helper".to_owned(),
        enabled: false,
        developer_mode_required: false,
        status: "disabled".to_owned(),
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
