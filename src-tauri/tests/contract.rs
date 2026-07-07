use engine::{RunOutputs, Value, ValueMap};
use oh_my_dream_tauri::dto::{AssetDto, RunWorkflowResultDto};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn writes_frontend_contract_fixtures_with_frozen_dto_shapes() {
    let run_result = run_workflow_fixture();
    let asset = asset_fixture();

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
            "source_node_id": "video",
            "tags": [],
            "created_at": 0
        })
    );

    write_fixture("run_workflow_result.json", &run_result);
    write_fixture("asset.json", &asset);
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
        source_node_id: Some("video".to_owned()),
        tags: Vec::new(),
        created_at: 0,
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
