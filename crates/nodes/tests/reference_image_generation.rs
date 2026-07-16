use assets::{AssetKind, AssetStore};
use engine::{
    Executor, InputBinding, InputPort, NodeInputs, NodeInterface, NodeParams, NodeRegistry,
    NodeRunContextImpl, NodeRunError, NodeRunResult, OutputPort, OutputRef, PortType, ResultCache,
    Value, Workflow, WorkflowNode,
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
fn one_and_nine_references_execute_in_order_without_overwriting_sources() {
    for count in [1, 9] {
        execute_reference_workflow(count);
    }
}

fn execute_reference_workflow(count: usize) {
    let directory = TempDir::new().expect("temporary directory");
    let source_root = directory.path().join("sources");
    fs::create_dir(&source_root).expect("source directory");
    let asset_ids = (0..count).map(|index| format!("asset-{index}")).collect::<Vec<_>>();
    let expected_paths = create_sources(&source_root, &asset_ids);
    let store = Arc::new(Mutex::new(
        AssetStore::open(directory.path().join("assets")).expect("asset store"),
    ));
    let project = store.lock().expect("store lock").create_project("Default").expect("project");
    let resolver = Arc::new(RecordingResolverImpl::new(source_root, project.id.clone()));
    let generator = Arc::new(RecordingGeneratorImpl::default());
    let mut registry = NodeRegistry::new();
    register_capabilities(
        &mut registry,
        Arc::clone(&generator),
        Arc::clone(&store),
        resolver.clone(),
    );
    register_source(&mut registry);

    Executor::new(&registry)
        .execute(&workflow(project.id.clone(), &asset_ids), &mut ResultCache::new())
        .expect("reference image workflow");

    assert_eq!(resolver.asset_ids(), asset_ids);
    assert_eq!(
        generator.requests(),
        vec![ReferenceImageGenerationRequest {
            model: "reference-model".to_owned(),
            images: expected_paths.iter().map(|path| path.to_string_lossy().into_owned()).collect(),
            prompt: "combine the references".to_owned(),
            negative_prompt: Some("blur".to_owned()),
            steps: Some(12),
            seed: Some(7),
        }]
    );
    for (index, path) in expected_paths.iter().enumerate() {
        assert_eq!(fs::read(path).expect("source bytes"), vec![index as u8]);
    }
    let assets = store.lock().expect("store lock").list(None).expect("asset list");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, AssetKind::Image);
    assert_eq!(assets[0].project_id.as_deref(), Some(project.id.as_str()));
    assert_eq!(assets[0].prompt.as_deref(), Some("combine the references"));
    assert_eq!(assets[0].model.as_deref(), Some("reference-model"));
}

fn create_sources(root: &std::path::Path, asset_ids: &[String]) -> Vec<PathBuf> {
    asset_ids
        .iter()
        .enumerate()
        .map(|(index, asset_id)| {
            let path = root.join(format!("{asset_id}.png"));
            fs::write(&path, [index as u8]).expect("source image");
            path
        })
        .collect()
}

fn register_capabilities(
    registry: &mut NodeRegistry,
    generator: Arc<RecordingGeneratorImpl>,
    store: nodes::SharedAssetStore,
    resolver: Arc<RecordingResolverImpl>,
) {
    nodes::register_all(
        registry,
        nodes::GenerationAdapters::new(
            Arc::new(NoopGeneratorImpl),
            generator,
            Arc::new(NoopGeneratorImpl),
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

fn workflow(project_id: String, asset_ids: &[String]) -> Workflow {
    let mut nodes = asset_ids
        .iter()
        .enumerate()
        .map(|(index, asset_id)| source_node(index, asset_id))
        .collect::<Vec<_>>();
    nodes.push(WorkflowNode {
        id: "prompt".to_owned(),
        type_id: "TextPrompt".to_owned(),
        contract_version: "1.0".to_owned(),
        params: params(serde_json::json!({"text": "combine the references"})),
        inputs: BTreeMap::new(),
        position: None,
    });
    nodes.push(WorkflowNode {
        id: "generated".to_owned(),
        type_id: "ReferenceImageGeneration".to_owned(),
        contract_version: "1.0".to_owned(),
        params: params(serde_json::json!({
            "model": "reference-model",
            "negative_prompt": "blur",
            "steps": 12,
            "seed": 7
        })),
        inputs: BTreeMap::from([
            (
                "images".to_owned(),
                InputBinding::ordered_many(
                    (0..asset_ids.len())
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
        _inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, NodeRunError> {
        Ok(NodeRunResult::new(BTreeMap::from([(
            "image".to_owned(),
            Value::Image(self.asset_id.clone()),
        )])))
    }
}

struct RecordingResolverImpl {
    root: PathBuf,
    project_id: String,
    asset_ids: Mutex<Vec<String>>,
}

impl RecordingResolverImpl {
    fn new(root: PathBuf, project_id: String) -> Self {
        Self { root, project_id, asset_ids: Mutex::new(Vec::new()) }
    }

    fn asset_ids(&self) -> Vec<String> {
        self.asset_ids.lock().expect("resolver lock").clone()
    }
}

impl AssetReferenceResolverInterface for RecordingResolverImpl {
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
    requests: Mutex<Vec<ReferenceImageGenerationRequest>>,
}

impl RecordingGeneratorImpl {
    fn requests(&self) -> Vec<ReferenceImageGenerationRequest> {
        self.requests.lock().expect("generator lock").clone()
    }
}

impl ReferenceImageGeneratorInterface for RecordingGeneratorImpl {
    fn generate(
        &self,
        request: ReferenceImageGenerationRequest,
        _context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        self.requests
            .lock()
            .map_err(|_| GenerationError::OperationFailed {
                operation: "record request",
                reason: "request lock was poisoned".to_owned(),
            })?
            .push(request);
        Ok(GeneratedOutput {
            artifact: GeneratedArtifact::InlineMedia(InlineMedia::png(MOCK_IMAGE_PNG.to_vec())),
            cost: Some(400),
        })
    }
}

struct NoopGeneratorImpl;

const MOCK_IMAGE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

impl TextToImageGeneratorInterface for NoopGeneratorImpl {
    fn generate(
        &self,
        _: TextToImageRequest,
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

impl ReferenceVideoGeneratorInterface for NoopGeneratorImpl {
    fn generate(
        &self,
        _: ReferenceVideoGenerationRequest,
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
