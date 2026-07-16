use assets::AssetStore;
use engine::{
    CancellationSignalInterface, EngineError, Executor, InputBinding, InputPort, NodeInterface,
    NodeParams, NodeRegistry, NodeRunContextImpl, NodeRunError, NodeRunResult, OutputPort,
    OutputRef, PortType, ResultCache, Value, Workflow, WorkflowNode,
};
use nodes::{
    AssetMediaKind, AssetReferenceError, AssetReferenceRequest, AssetReferenceResolverInterface,
    GeneratedArtifact, GeneratedOutput, GenerationContextInterface, GenerationError,
    ImageToVideoGeneratorInterface, ImageToVideoRequest, InlineMedia,
    ReferenceImageGenerationRequest, ReferenceImageGeneratorInterface,
    ReferenceVideoGenerationRequest, ReferenceVideoGeneratorInterface, ResolvedAssetReference,
    TextToAudioGeneratorInterface, TextToAudioRequest, TextToImageGeneratorInterface,
    TextToImageRequest,
};
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
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
    let reference = "asset-image-1";
    let directory = TempDir::new().expect("temp directory");
    let store = Arc::new(Mutex::new(AssetStore::open(directory.path()).expect("asset store")));
    let project = store.lock().expect("store lock").create_project("Default").expect("project");
    let fixed = Arc::new(FixedGeneratorsImpl {
        image_output: GeneratedOutput {
            artifact: GeneratedArtifact::RemoteUrl(String::new()),
            cost: None,
        },
    });
    let recorder = Arc::new(RecordingVideoGeneratorImpl::default());
    let image: Arc<dyn TextToImageGeneratorInterface> = fixed.clone();
    let reference_image: Arc<dyn ReferenceImageGeneratorInterface> = fixed.clone();
    let reference_video: Arc<dyn ReferenceVideoGeneratorInterface> = fixed.clone();
    let video: Arc<dyn ImageToVideoGeneratorInterface> = recorder.clone();
    let audio: Arc<dyn TextToAudioGeneratorInterface> = fixed;
    let mut registry = NodeRegistry::new();
    let resolver = Arc::new(FixedResolverImpl { path: local.path().to_path_buf() });
    nodes::register_all(
        &mut registry,
        nodes::GenerationAdapters::new(image, reference_image, reference_video, video, audio),
        Arc::clone(&store),
        resolver,
    )
    .expect("register workflow capabilities");
    register_local_image(&mut registry, reference);

    Executor::new(&registry)
        .execute(&local_image_workflow(project.id), &mut ResultCache::new())
        .expect("asset-prefixed local image workflow");

    let requests = recorder.requests.lock().expect("request lock");
    assert_eq!(
        requests.as_slice(),
        &[ImageToVideoRequest {
            model: "fixed-video".to_owned(),
            image: local.path().to_string_lossy().into_owned(),
            duration_seconds: None,
            fps: None,
        }]
    );
}

#[test]
fn cancellation_before_persistence_does_not_store_an_asset() {
    let directory = TempDir::new().expect("temp directory");
    let store = Arc::new(Mutex::new(AssetStore::open(directory.path()).expect("asset store")));
    let project = store.lock().expect("store lock").create_project("Default").expect("project");
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelling = Arc::new(CancellingImageGeneratorImpl { cancelled: Arc::clone(&cancelled) });
    let fixed = Arc::new(FixedGeneratorsImpl {
        image_output: GeneratedOutput {
            artifact: GeneratedArtifact::RemoteUrl(String::new()),
            cost: None,
        },
    });
    let image: Arc<dyn TextToImageGeneratorInterface> = cancelling;
    let reference_image: Arc<dyn ReferenceImageGeneratorInterface> = fixed.clone();
    let reference_video: Arc<dyn ReferenceVideoGeneratorInterface> = fixed.clone();
    let video: Arc<dyn ImageToVideoGeneratorInterface> = fixed.clone();
    let audio: Arc<dyn TextToAudioGeneratorInterface> = fixed;
    let mut registry = NodeRegistry::new();
    nodes::register_all(
        &mut registry,
        nodes::GenerationAdapters::new(image, reference_image, reference_video, video, audio),
        Arc::clone(&store),
        Arc::new(support::MissingResolverImpl),
    )
    .expect("register workflow capabilities");
    let signal = AtomicCancellationImpl { cancelled };

    let error = Executor::new(&registry)
        .execute_interruptible(
            &text_to_image_workflow(project.id),
            &mut ResultCache::new(),
            &signal,
            &mut |_| {},
        )
        .expect_err("workflow should be cancelled before persistence");

    assert!(matches!(error, EngineError::Cancelled));
    assert!(store.lock().expect("store lock").list(None).expect("asset list").is_empty());
}

fn execute_text_to_image(
    output: GeneratedOutput,
) -> (engine::Result<engine::RunOutputs>, nodes::SharedAssetStore, TempDir) {
    let directory = TempDir::new().expect("temp directory");
    let store = Arc::new(Mutex::new(AssetStore::open(directory.path()).expect("asset store")));
    let project = store.lock().expect("store lock").create_project("Default").expect("project");
    let generators = Arc::new(FixedGeneratorsImpl { image_output: output });
    let image: Arc<dyn TextToImageGeneratorInterface> = generators.clone();
    let reference_image: Arc<dyn ReferenceImageGeneratorInterface> = generators.clone();
    let reference_video: Arc<dyn ReferenceVideoGeneratorInterface> = generators.clone();
    let video: Arc<dyn ImageToVideoGeneratorInterface> = generators.clone();
    let audio: Arc<dyn TextToAudioGeneratorInterface> = generators;
    let mut registry = NodeRegistry::new();
    nodes::register_all(
        &mut registry,
        nodes::GenerationAdapters::new(image, reference_image, reference_video, video, audio),
        Arc::clone(&store),
        Arc::new(support::MissingResolverImpl),
    )
    .expect("register workflow capabilities");
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
                contract_version: "1.0".to_owned(),
                params: params(serde_json::json!({"text": "a bright sky"})),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "image".to_owned(),
                type_id: "TextToImage".to_owned(),
                contract_version: "1.0".to_owned(),
                params: params(serde_json::json!({"model": "fixed-output"})),
                inputs: BTreeMap::from([(
                    "prompt".to_owned(),
                    InputBinding::single(engine::OutputRef("prompt".to_owned(), "text".to_owned())),
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
                contract_version: "1.0".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "video".to_owned(),
                type_id: "ImageToVideo".to_owned(),
                contract_version: "1.0".to_owned(),
                params: params(serde_json::json!({"model": "fixed-video"})),
                inputs: BTreeMap::from([(
                    "image".to_owned(),
                    InputBinding::single(OutputRef("source".to_owned(), "image".to_owned())),
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
        Box::new(move |_| Ok(Box::new(LocalImageNodeImpl::new(reference.clone())))),
    );
}

fn params(value: serde_json::Value) -> NodeParams {
    match value {
        serde_json::Value::Object(params) => params,
        _ => NodeParams::new(),
    }
}

struct FixedGeneratorsImpl {
    image_output: GeneratedOutput,
}

struct CancellingImageGeneratorImpl {
    cancelled: Arc<AtomicBool>,
}

impl TextToImageGeneratorInterface for CancellingImageGeneratorImpl {
    fn generate(
        &self,
        _request: TextToImageRequest,
        _context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        self.cancelled.store(true, Ordering::SeqCst);
        Ok(GeneratedOutput {
            artifact: GeneratedArtifact::InlineMedia(InlineMedia::png(vec![1, 2, 3])),
            cost: None,
        })
    }
}

struct AtomicCancellationImpl {
    cancelled: Arc<AtomicBool>,
}

impl CancellationSignalInterface for AtomicCancellationImpl {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

struct LocalImageNodeImpl {
    reference: String,
    outputs: Vec<OutputPort>,
}

impl LocalImageNodeImpl {
    fn new(reference: String) -> Self {
        Self {
            reference,
            outputs: vec![OutputPort { name: "image".to_owned(), port_type: PortType::Image }],
        }
    }
}

impl NodeInterface for LocalImageNodeImpl {
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
        _inputs: &engine::NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, NodeRunError> {
        Ok(NodeRunResult::new(BTreeMap::from([(
            "image".to_owned(),
            Value::Image(self.reference.clone()),
        )])))
    }
}

#[derive(Default)]
struct RecordingVideoGeneratorImpl {
    requests: Mutex<Vec<ImageToVideoRequest>>,
}

struct FixedResolverImpl {
    path: std::path::PathBuf,
}

impl AssetReferenceResolverInterface for FixedResolverImpl {
    fn resolve(
        &self,
        request: AssetReferenceRequest<'_>,
    ) -> Result<ResolvedAssetReference, AssetReferenceError> {
        assert_eq!(request.expected_kind, AssetMediaKind::Image);
        assert_eq!(request.asset_id, "asset-image-1");
        Ok(ResolvedAssetReference {
            asset_id: request.asset_id.to_owned(),
            local_path: self.path.clone(),
            prompt: None,
        })
    }
}

impl ImageToVideoGeneratorInterface for RecordingVideoGeneratorImpl {
    fn generate(
        &self,
        request: ImageToVideoRequest,
        _context: &mut dyn GenerationContextInterface,
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

impl TextToImageGeneratorInterface for FixedGeneratorsImpl {
    fn generate(
        &self,
        _request: TextToImageRequest,
        _context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        Ok(self.image_output.clone())
    }
}

impl ReferenceImageGeneratorInterface for FixedGeneratorsImpl {
    fn generate(
        &self,
        _request: ReferenceImageGenerationRequest,
        _context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        Ok(self.image_output.clone())
    }
}

impl ReferenceVideoGeneratorInterface for FixedGeneratorsImpl {
    fn generate(
        &self,
        _request: ReferenceVideoGenerationRequest,
        _context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        Ok(GeneratedOutput {
            artifact: GeneratedArtifact::InlineMedia(InlineMedia::webm(Vec::new())),
            cost: None,
        })
    }
}

impl ImageToVideoGeneratorInterface for FixedGeneratorsImpl {
    fn generate(
        &self,
        _request: ImageToVideoRequest,
        _context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        Ok(GeneratedOutput {
            artifact: GeneratedArtifact::InlineMedia(InlineMedia::opaque_video(Vec::new())),
            cost: None,
        })
    }
}

impl TextToAudioGeneratorInterface for FixedGeneratorsImpl {
    fn generate(
        &self,
        _request: TextToAudioRequest,
        _context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        Ok(GeneratedOutput {
            artifact: GeneratedArtifact::InlineMedia(InlineMedia::wav(Vec::new())),
            cost: None,
        })
    }
}
mod support;
