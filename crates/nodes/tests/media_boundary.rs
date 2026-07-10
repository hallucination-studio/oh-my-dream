use assets::AssetStore;
use engine::{
    Executor, InputPort, Node, NodeParams, NodeRegistry, NodeRunContext, NodeRunError,
    NodeRunResult, OutputPort, OutputRef, PortType, ResultCache, Value, ValueMap, Workflow,
    WorkflowNode,
};
use nodes::{
    GeneratedArtifact, GeneratedOutput, GenerationError, ImageToVideoGenerator,
    ImageToVideoRequest, InlineMedia, TextToAudioGenerator, TextToAudioRequest,
    TextToImageGenerator, TextToImageRequest,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn remote_url_output_fails_without_persisting_an_asset_or_credentials() {
    let reference = "https://media.example/image.png?token=secret";
    let (result, store, _directory) = execute_text_to_image(GeneratedOutput {
        artifact: GeneratedArtifact::RemoteUrl(reference.to_owned()),
        cost: None,
    });

    let error = result.expect_err("remote output must require a resolver");
    assert!(error.to_string().contains("remote media output requires a resolver"));
    assert!(!error.to_string().contains(reference));
    assert!(!error.to_string().contains("secret"));
    assert!(store.lock().expect("store lock").list(None).expect("asset list").is_empty());
}

#[test]
fn inline_kind_mismatch_fails_without_persisting_an_asset() {
    let output = GeneratedOutput {
        artifact: GeneratedArtifact::InlineMedia(InlineMedia::wav(b"not used".to_vec())),
        cost: None,
    };
    let (result, store, _directory) = execute_text_to_image(output);

    let error = result.expect_err("audio inline media cannot satisfy an image output");
    assert!(error.to_string().contains("inline media kind `audio`"));
    assert!(error.to_string().contains("`image` asset"));
    assert!(store.lock().expect("store lock").list(None).expect("asset list").is_empty());
}

#[test]
fn asset_prefixed_local_image_reaches_the_video_generator() {
    let local = tempfile::Builder::new()
        .prefix("asset-preview-")
        .suffix(".png")
        .tempfile_in(".")
        .expect("asset-prefixed local image");
    let reference = local.path().file_name().and_then(|name| name.to_str()).expect("file name");
    let directory = TempDir::new().expect("temp directory");
    let store = Arc::new(Mutex::new(AssetStore::open(directory.path()).expect("asset store")));
    let project = store.lock().expect("store lock").create_project("Default").expect("project");
    let fixed = Arc::new(FixedGenerators {
        image_output: GeneratedOutput {
            artifact: GeneratedArtifact::RemoteUrl(String::new()),
            cost: None,
        },
    });
    let recorder = Arc::new(RecordingVideoGenerator::default());
    let image: Arc<dyn TextToImageGenerator> = fixed.clone();
    let video: Arc<dyn ImageToVideoGenerator> = recorder.clone();
    let audio: Arc<dyn TextToAudioGenerator> = fixed;
    let mut registry = NodeRegistry::new();
    nodes::register_all(&mut registry, image, video, audio, Arc::clone(&store));
    register_local_image(&mut registry, reference);

    Executor::new(&registry)
        .execute(&local_image_workflow(project.id), &mut ResultCache::new())
        .expect("asset-prefixed local image workflow");

    let requests = recorder.requests.lock().expect("request lock");
    assert_eq!(
        requests.as_slice(),
        &[ImageToVideoRequest {
            model: "fixed-video".to_owned(),
            image: reference.to_owned(),
            duration_seconds: None,
            fps: None,
        }]
    );
}

fn execute_text_to_image(
    output: GeneratedOutput,
) -> (engine::Result<engine::RunOutputs>, nodes::SharedAssetStore, TempDir) {
    let directory = TempDir::new().expect("temp directory");
    let store = Arc::new(Mutex::new(AssetStore::open(directory.path()).expect("asset store")));
    let project = store.lock().expect("store lock").create_project("Default").expect("project");
    let generators = Arc::new(FixedGenerators { image_output: output });
    let image: Arc<dyn TextToImageGenerator> = generators.clone();
    let video: Arc<dyn ImageToVideoGenerator> = generators.clone();
    let audio: Arc<dyn TextToAudioGenerator> = generators;
    let mut registry = NodeRegistry::new();
    nodes::register_all(&mut registry, image, video, audio, Arc::clone(&store));
    let workflow = text_to_image_workflow(project.id);
    let result = Executor::new(&registry).execute(&workflow, &mut ResultCache::new());
    (result, store, directory)
}

fn text_to_image_workflow(project_id: String) -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id,
        nodes: vec![
            WorkflowNode {
                id: "prompt".to_owned(),
                type_id: "TextPrompt".to_owned(),
                params: params(serde_json::json!({"text": "a bright sky"})),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "image".to_owned(),
                type_id: "TextToImage".to_owned(),
                params: params(serde_json::json!({"model": "fixed-output"})),
                inputs: BTreeMap::from([(
                    "prompt".to_owned(),
                    engine::OutputRef("prompt".to_owned(), "text".to_owned()),
                )]),
                position: None,
            },
        ],
    }
}

fn local_image_workflow(project_id: String) -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id,
        nodes: vec![
            WorkflowNode {
                id: "source".to_owned(),
                type_id: "LocalImage".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "video".to_owned(),
                type_id: "ImageToVideo".to_owned(),
                params: params(serde_json::json!({"model": "fixed-video"})),
                inputs: BTreeMap::from([(
                    "image".to_owned(),
                    OutputRef("source".to_owned(), "image".to_owned()),
                )]),
                position: None,
            },
        ],
    }
}

fn register_local_image(registry: &mut NodeRegistry, reference: &str) {
    let reference = reference.to_owned();
    registry.register(
        "LocalImage",
        Box::new(move |_| Ok(Box::new(LocalImageNode::new(reference.clone())))),
    );
}

fn params(value: serde_json::Value) -> NodeParams {
    match value {
        serde_json::Value::Object(params) => params,
        _ => NodeParams::new(),
    }
}

struct FixedGenerators {
    image_output: GeneratedOutput,
}

struct LocalImageNode {
    reference: String,
    outputs: Vec<OutputPort>,
}

impl LocalImageNode {
    fn new(reference: String) -> Self {
        Self {
            reference,
            outputs: vec![OutputPort { name: "image".to_owned(), port_type: PortType::Image }],
        }
    }
}

impl Node for LocalImageNode {
    fn type_id(&self) -> &str {
        "LocalImage"
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn run(
        &self,
        _inputs: &ValueMap,
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, NodeRunError> {
        Ok(NodeRunResult::new(BTreeMap::from([(
            "image".to_owned(),
            Value::Image(self.reference.clone()),
        )])))
    }
}

#[derive(Default)]
struct RecordingVideoGenerator {
    requests: Mutex<Vec<ImageToVideoRequest>>,
}

impl ImageToVideoGenerator for RecordingVideoGenerator {
    fn generate(
        &self,
        request: ImageToVideoRequest,
        _on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        self.requests.lock().expect("request lock").push(request);
        Ok(GeneratedOutput {
            artifact: GeneratedArtifact::InlineMedia(InlineMedia::opaque_video(
                b"OH_MY_DREAM_MOCK_VIDEO_V1\n".to_vec(),
            )),
            cost: None,
        })
    }
}

impl TextToImageGenerator for FixedGenerators {
    fn generate(
        &self,
        _request: TextToImageRequest,
        _on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        Ok(self.image_output.clone())
    }
}

impl ImageToVideoGenerator for FixedGenerators {
    fn generate(
        &self,
        _request: ImageToVideoRequest,
        _on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        Ok(GeneratedOutput {
            artifact: GeneratedArtifact::InlineMedia(InlineMedia::opaque_video(Vec::new())),
            cost: None,
        })
    }
}

impl TextToAudioGenerator for FixedGenerators {
    fn generate(
        &self,
        _request: TextToAudioRequest,
        _on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        Ok(GeneratedOutput {
            artifact: GeneratedArtifact::InlineMedia(InlineMedia::wav(Vec::new())),
            cost: None,
        })
    }
}
