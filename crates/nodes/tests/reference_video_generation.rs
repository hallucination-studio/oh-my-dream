use assets::{AssetKind, AssetStore};
use engine::{
    InputBinding, InputPort, NodeInputs, NodeInterface, NodeParams, NodeRegistry,
    NodeRunContextImpl, NodeRunError, NodeRunResult, OutputPort, OutputRef, PortType, ResultCache,
    Workflow, WorkflowGraphExecutor, WorkflowNode, WorkflowNodeValue,
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
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn ordered_references_and_video_options_reach_the_generator() {
    let directory = TempDir::new().expect("temporary directory");
    let source_root = directory.path().join("sources");
    fs::create_dir(&source_root).expect("source directory");
    let asset_ids = ["asset-second".to_owned(), "asset-first".to_owned()];
    for asset_id in &asset_ids {
        fs::write(source_root.join(format!("{asset_id}.png")), asset_id).expect("source image");
    }
    let store = Arc::new(Mutex::new(
        AssetStore::open(directory.path().join("assets")).expect("asset store"),
    ));
    let project = store.lock().expect("store lock").create_project("Default").expect("project");
    let resolver = Arc::new(ResolverImpl::new(source_root, project.id.clone()));
    let generator = Arc::new(RecordingGeneratorImpl::default());
    let mut registry = NodeRegistry::new();
    register(&mut registry, Arc::clone(&generator), Arc::clone(&store), resolver.clone());
    register_source(&mut registry);

    WorkflowGraphExecutor::new(&registry)
        .execute(&workflow(project.id.clone(), &asset_ids), &mut ResultCache::new())
        .expect("reference video workflow");

    assert_eq!(resolver.asset_ids(), asset_ids);
    assert_eq!(
        generator.request(),
        ReferenceVideoGenerationRequest {
            model: "reference-video-model".to_owned(),
            images: asset_ids
                .iter()
                .map(|asset_id| resolver
                    .root
                    .join(format!("{asset_id}.png"))
                    .to_string_lossy()
                    .into_owned())
                .collect(),
            prompt: "move through the scene".to_owned(),
            duration_seconds: Some(3.5),
            aspect_ratio: Some("16:9".to_owned()),
            resolution: Some("720p".to_owned()),
            fps: Some(24),
        }
    );
    let assets = store.lock().expect("store lock").list(None).expect("asset list");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, AssetKind::Video);
    assert_eq!(assets[0].project_id.as_deref(), Some(project.id.as_str()));
    assert!(assets[0].file_path.ends_with(".webm"));
    assert_eq!(assets[0].prompt.as_deref(), Some("move through the scene"));
}

fn register(
    registry: &mut NodeRegistry,
    generator: Arc<RecordingGeneratorImpl>,
    store: nodes::SharedAssetStore,
    resolver: Arc<ResolverImpl>,
) {
    nodes::register_all(
        registry,
        nodes::GenerationAdapters::new(
            Arc::new(NoopGeneratorImpl),
            Arc::new(NoopGeneratorImpl),
            generator,
            Arc::new(NoopGeneratorImpl),
            Arc::new(NoopGeneratorImpl),
        ),
        store,
        resolver,
    )
    .expect("capability registration");
}

fn register_source(registry: &mut NodeRegistry) {
    registry.register(
        "TestImageSourceImpl",
        Box::new(|params| {
            let asset_id = params
                .get("asset_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "missing asset_id".to_owned())?;
            Ok(Box::new(TestImageSourceImpl::new(asset_id.to_owned())))
        }),
    );
}

fn workflow(project_id: String, asset_ids: &[String; 2]) -> Workflow {
    let mut nodes = asset_ids
        .iter()
        .enumerate()
        .map(|(index, asset_id)| source_node(index, asset_id))
        .collect::<Vec<_>>();
    nodes.push(WorkflowNode {
        id: "prompt".to_owned(),
        type_id: "TextPrompt".to_owned(),
        contract_version: "1.0".to_owned(),
        params: params(serde_json::json!({"text": "move through the scene"})),
        inputs: BTreeMap::new(),
        position: None,
    });
    nodes.push(WorkflowNode {
        id: "video".to_owned(),
        type_id: "ReferenceVideoGeneration".to_owned(),
        contract_version: "1.0".to_owned(),
        params: params(serde_json::json!({
            "model": "reference-video-model", "duration": 3.5,
            "aspect_ratio": "16:9", "resolution": "720p", "fps": 24
        })),
        inputs: BTreeMap::from([
            (
                "images".to_owned(),
                InputBinding::ordered_many(
                    (0..2)
                        .map(|index| OutputRef(format!("source-{index}"), "image".to_owned()))
                        .collect(),
                ),
            ),
            (
                "prompt".to_owned(),
                InputBinding::single(OutputRef("prompt".to_owned(), "text".to_owned())),
            ),
        ]),
        position: None,
    });
    Workflow { version: "1.0".to_owned(), project_id, nodes }
}

fn source_node(index: usize, asset_id: &str) -> WorkflowNode {
    WorkflowNode {
        id: format!("source-{index}"),
        type_id: "TestImageSourceImpl".to_owned(),
        contract_version: "1.0".to_owned(),
        params: params(serde_json::json!({"asset_id": asset_id})),
        inputs: BTreeMap::new(),
        position: None,
    }
}

fn params(value: serde_json::Value) -> NodeParams {
    value.as_object().cloned().unwrap_or_default()
}

struct TestImageSourceImpl {
    asset_id: String,
    outputs: Vec<OutputPort>,
}

impl TestImageSourceImpl {
    fn new(asset_id: String) -> Self {
        Self {
            asset_id,
            outputs: vec![OutputPort { name: "image".to_owned(), port_type: PortType::Image }],
        }
    }
}

impl NodeInterface for TestImageSourceImpl {
    fn type_id(&self) -> &str {
        "TestImageSourceImpl"
    }
    fn inputs(&self) -> &[InputPort] {
        &[]
    }
    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }
    fn run(
        &self,
        _: &NodeInputs,
        _: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, NodeRunError> {
        Ok(NodeRunResult::new(BTreeMap::from([(
            "image".to_owned(),
            WorkflowNodeValue::Image(self.asset_id.clone()),
        )])))
    }
}

struct ResolverImpl {
    root: PathBuf,
    project_id: String,
    asset_ids: Mutex<Vec<String>>,
}

impl ResolverImpl {
    fn new(root: PathBuf, project_id: String) -> Self {
        Self { root, project_id, asset_ids: Mutex::new(Vec::new()) }
    }
    fn asset_ids(&self) -> Vec<String> {
        self.asset_ids.lock().expect("resolver lock").clone()
    }
}

impl AssetReferenceResolverInterface for ResolverImpl {
    fn resolve(
        &self,
        request: AssetReferenceRequest<'_>,
    ) -> Result<ResolvedAssetReference, AssetReferenceError> {
        assert_eq!(request.project_id, self.project_id);
        assert_eq!(request.expected_kind, AssetMediaKind::Image);
        let local_path = self.root.join(format!("{}.png", request.asset_id));
        if !local_path.is_file() {
            return Err(AssetReferenceError::MissingContent);
        }
        self.asset_ids
            .lock()
            .map_err(|_| AssetReferenceError::StorageUnavailable)?
            .push(request.asset_id.to_owned());
        Ok(ResolvedAssetReference {
            asset_id: request.asset_id.to_owned(),
            local_path,
            prompt: None,
        })
    }
}

#[derive(Default)]
struct RecordingGeneratorImpl {
    request: Mutex<Option<ReferenceVideoGenerationRequest>>,
}

impl RecordingGeneratorImpl {
    fn request(&self) -> ReferenceVideoGenerationRequest {
        self.request.lock().expect("generator lock").clone().expect("recorded request")
    }
}

impl ReferenceVideoGeneratorInterface for RecordingGeneratorImpl {
    fn generate(
        &self,
        request: ReferenceVideoGenerationRequest,
        _: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        *self.request.lock().map_err(|_| GenerationError::OperationFailed {
            operation: "record request",
            reason: "request lock was poisoned".to_owned(),
        })? = Some(request);
        Ok(GeneratedOutput {
            artifact: GeneratedArtifact::InlineMedia(InlineMedia::webm(b"webm".to_vec())),
            cost: Some(1_200),
        })
    }
}

struct NoopGeneratorImpl;
impl TextToImageGeneratorInterface for NoopGeneratorImpl {
    fn generate(
        &self,
        _: TextToImageRequest,
        _: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        unreachable!()
    }
}
impl ReferenceImageGeneratorInterface for NoopGeneratorImpl {
    fn generate(
        &self,
        _: ReferenceImageGenerationRequest,
        _: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        unreachable!()
    }
}
impl ImageToVideoGeneratorInterface for NoopGeneratorImpl {
    fn generate(
        &self,
        _: ImageToVideoRequest,
        _: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        unreachable!()
    }
}
impl TextToAudioGeneratorInterface for NoopGeneratorImpl {
    fn generate(
        &self,
        _: TextToAudioRequest,
        _: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        unreachable!()
    }
}
