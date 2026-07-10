use assets::{AssetKind, AssetStore};
use engine::{
    EngineError, Executor, NodeParams, NodeRegistry, OutputRef, ResultCache, Value, Workflow,
    WorkflowNode,
};
use nodes::{
    GeneratedArtifact, GeneratedOutput, GenerationError, ImageToVideoGenerator,
    ImageToVideoRequest, InlineMedia, TextToAudioGenerator, TextToAudioRequest,
    TextToImageGenerator, TextToImageRequest,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn executes_full_generation_workflow_and_persists_video_asset() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let store = shared_store(temp_dir.path());
    store.lock().expect("store lock should succeed").create_project("Default").expect("project");
    let mut registry = NodeRegistry::new();
    register_test_generators(
        &mut registry,
        Arc::new(TestGenerators::succeeds()),
        Arc::clone(&store),
    );
    let workflow = full_workflow();

    let outputs = Executor::new(&registry)
        .execute(&workflow, &mut ResultCache::new())
        .expect("workflow should execute");

    let video = outputs
        .get("video")
        .and_then(|values| values.get("video"))
        .expect("video output should be produced");
    assert!(matches!(video, Value::Video(value) if value.starts_with("asset-")));

    let saved_assets =
        store.lock().expect("store lock should succeed").list(None).expect("assets should list");
    assert_eq!(saved_assets.len(), 2);
    assert_eq!(saved_assets[0].kind, AssetKind::Video);
    assert_eq!(saved_assets[0].prompt.as_deref(), Some("a small moonlit house"));
    assert_eq!(saved_assets[0].project_id.as_deref(), Some("project-0000000000000001"));
    assert_eq!(saved_assets[0].project_name.as_deref(), Some("Default"));
    assert_eq!(saved_assets[0].source_node_id.as_deref(), Some("video"));
    assert_eq!(saved_assets[0].source_node_type.as_deref(), Some("ImageToVideo"));
    assert_eq!(saved_assets[0].model.as_deref(), Some("mock-video"));
    assert_eq!(saved_assets[0].cost, Some(900));
    assert_eq!(saved_assets[0].workflow_snapshot["project_id"], "project-0000000000000001");

    let stored = store
        .lock()
        .expect("store lock should succeed")
        .get(&saved_assets[0].id)
        .expect("asset should be retrievable");
    assert_eq!(stored.source_node_id, Some("video".to_owned()));
    assert_eq!(
        Path::new(&stored.file_path).extension().and_then(|value| value.to_str()),
        Some("video-data")
    );
    assert!(
        fs::read(&stored.file_path)
            .expect("stored mock video artifact")
            .starts_with(b"OH_MY_DREAM_MOCK_VIDEO_V1\n")
    );
}

#[test]
fn executes_text_to_audio_and_persists_audio_asset() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let store = shared_store(temp_dir.path());
    store.lock().expect("store lock should succeed").create_project("Default").expect("project");
    let mut registry = NodeRegistry::new();
    register_test_generators(
        &mut registry,
        Arc::new(TestGenerators::succeeds()),
        Arc::clone(&store),
    );

    let outputs = Executor::new(&registry)
        .execute(&audio_workflow(), &mut ResultCache::new())
        .expect("workflow should execute");

    assert!(matches!(
        outputs.get("audio").and_then(|values| values.get("audio")),
        Some(Value::Audio(value)) if value.starts_with("asset-")
    ));
    let saved_assets = store
        .lock()
        .expect("store lock should succeed")
        .list(Some(AssetKind::Audio))
        .expect("assets");
    assert_eq!(saved_assets.len(), 1);
    assert_eq!(saved_assets[0].source_node_type.as_deref(), Some("TextToAudio"));
    assert_eq!(saved_assets[0].prompt.as_deref(), Some("rain on glass"));
    assert_eq!(saved_assets[0].cost, Some(125));
    let audio = fs::read(&saved_assets[0].file_path).expect("stored WAV fixture");
    assert_eq!(&audio[0..4], b"RIFF");
    assert_eq!(&audio[8..12], b"WAVE");
}

#[test]
fn save_asset_node_is_not_registered() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let store = shared_store(temp_dir.path());
    let mut registry = NodeRegistry::new();
    register_test_generators(&mut registry, Arc::new(TestGenerators::succeeds()), store);

    let error = Executor::new(&registry)
        .execute(&workflow_with_save_asset(), &mut ResultCache::new())
        .expect_err("SaveAsset should no longer be available");

    assert!(error.to_string().contains("SaveAsset"));
}

#[test]
fn failed_generation_task_surfaces_as_execution_error() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let store = shared_store(temp_dir.path());
    let mut registry = NodeRegistry::new();
    register_test_generators(
        &mut registry,
        Arc::new(TestGenerators::fails("provider failed")),
        store,
    );

    let error = Executor::new(&registry)
        .execute(&image_workflow(), &mut ResultCache::new())
        .expect_err("generation failure should fail workflow execution");

    assert!(matches!(
        error,
        EngineError::NodeExecution {
            ref node_id,
            ref type_id,
            ..
        } if node_id == "image" && type_id == "TextToImage"
    ));
    assert!(error.to_string().contains("provider failed"));
}

fn shared_store(path: &std::path::Path) -> nodes::SharedAssetStore {
    Arc::new(Mutex::new(AssetStore::open(path).expect("asset store should open")))
}

fn full_workflow() -> Workflow {
    let mut nodes = image_workflow().nodes;
    nodes.push(WorkflowNode {
        id: "video".to_owned(),
        type_id: "ImageToVideo".to_owned(),
        params: params(json!({
            "model": "mock-video",
            "duration": 4.0,
            "fps": 24
        })),
        inputs: BTreeMap::from([(
            "image".to_owned(),
            OutputRef("image".to_owned(), "image".to_owned()),
        )]),
        position: None,
    });
    Workflow { version: "1.0".to_owned(), project_id: "project-0000000000000001".to_owned(), nodes }
}

fn image_workflow() -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: "project-0000000000000001".to_owned(),
        nodes: vec![
            WorkflowNode {
                id: "prompt".to_owned(),
                type_id: "TextPrompt".to_owned(),
                params: params(json!({"text": "a small moonlit house"})),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "image".to_owned(),
                type_id: "TextToImage".to_owned(),
                params: params(json!({
                    "model": "mock-image",
                    "steps": 28,
                    "seed": 42
                })),
                inputs: BTreeMap::from([(
                    "prompt".to_owned(),
                    OutputRef("prompt".to_owned(), "text".to_owned()),
                )]),
                position: None,
            },
        ],
    }
}

fn audio_workflow() -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: "project-0000000000000001".to_owned(),
        nodes: vec![
            WorkflowNode {
                id: "prompt".to_owned(),
                type_id: "TextPrompt".to_owned(),
                params: params(json!({"text": "rain on glass"})),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "audio".to_owned(),
                type_id: "TextToAudio".to_owned(),
                params: params(json!({
                    "model": "mock-audio",
                    "seed": 7
                })),
                inputs: BTreeMap::from([(
                    "prompt".to_owned(),
                    OutputRef("prompt".to_owned(), "text".to_owned()),
                )]),
                position: None,
            },
        ],
    }
}

fn workflow_with_save_asset() -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: "project-0000000000000001".to_owned(),
        nodes: vec![WorkflowNode {
            id: "save".to_owned(),
            type_id: "SaveAsset".to_owned(),
            params: NodeParams::new(),
            inputs: BTreeMap::new(),
            position: None,
        }],
    }
}

fn params(value: serde_json::Value) -> NodeParams {
    match value {
        serde_json::Value::Object(params) => params,
        _ => NodeParams::new(),
    }
}

fn register_test_generators(
    registry: &mut NodeRegistry,
    generators: Arc<TestGenerators>,
    store: nodes::SharedAssetStore,
) {
    let image: Arc<dyn TextToImageGenerator> = generators.clone();
    let video: Arc<dyn ImageToVideoGenerator> = generators.clone();
    let audio: Arc<dyn TextToAudioGenerator> = generators;
    nodes::register_all(registry, image, video, audio, store);
}

struct TestGenerators {
    failure_reason: Option<String>,
}

impl TestGenerators {
    fn succeeds() -> Self {
        Self { failure_reason: None }
    }

    fn fails(reason: &str) -> Self {
        Self { failure_reason: Some(reason.to_owned()) }
    }

    fn result(&self, media: InlineMedia, cost: i64) -> Result<GeneratedOutput, GenerationError> {
        if let Some(reason) = &self.failure_reason {
            return Err(GenerationError::TaskFailed { reason: reason.clone() });
        }
        Ok(GeneratedOutput { artifact: GeneratedArtifact::InlineMedia(media), cost: Some(cost) })
    }
}

impl TextToImageGenerator for TestGenerators {
    fn generate(
        &self,
        _request: TextToImageRequest,
        on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        on_progress(0.25);
        on_progress(0.75);
        self.result(InlineMedia::png(MOCK_IMAGE_PNG.to_vec()), 250)
    }
}

impl ImageToVideoGenerator for TestGenerators {
    fn generate(
        &self,
        request: ImageToVideoRequest,
        on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        assert!(Path::new(&request.image).is_file(), "source image must resolve to a local asset");
        on_progress(0.25);
        on_progress(0.75);
        self.result(InlineMedia::opaque_video(b"OH_MY_DREAM_MOCK_VIDEO_V1\n".to_vec()), 900)
    }
}

impl TextToAudioGenerator for TestGenerators {
    fn generate(
        &self,
        _request: TextToAudioRequest,
        on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        on_progress(0.25);
        on_progress(0.75);
        self.result(InlineMedia::wav(silent_pcm_wave()), 125)
    }
}

const MOCK_IMAGE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn silent_pcm_wave() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(46);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&38_u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_000_u32.to_le_bytes());
    bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&[0, 0]);
    bytes
}
