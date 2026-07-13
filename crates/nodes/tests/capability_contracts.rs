use assets::AssetStore;
use engine::{CapabilityEffect, CapabilityRef, CapabilityRegistryError, NodeRegistry};
use nodes::{
    GeneratedOutput, GenerationContext, GenerationError, ImageToVideoGenerator,
    ImageToVideoRequest, SharedAssetStore, TextToAudioGenerator, TextToAudioRequest,
    TextToImageGenerator, TextToImageRequest,
};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn capability_registration_exposes_exact_refs_and_contracts() {
    let (_directory, store) = store();
    let mut registry = NodeRegistry::new();
    register(&mut registry, store);

    let refs = registry
        .capability_refs()
        .into_iter()
        .map(|reference| (reference.id.as_str(), reference.version.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        refs,
        vec![
            ("ImageToVideo", "1.0"),
            ("TextPrompt", "1.0"),
            ("TextToImage", "1.0"),
            ("VideoConcat", "1.0"),
        ]
    );

    let prompt = registry
        .capability(&CapabilityRef::new("TextPrompt", "1.0"))
        .expect("TextPrompt contract should resolve");
    assert_eq!(prompt.contract().reference.id, "TextPrompt");
    assert_eq!(prompt.contract().default_params["text"], "");
    assert_eq!(prompt.contract().effects, vec![CapabilityEffect::Pure]);

    let concat = registry
        .capability(&CapabilityRef::new("VideoConcat", "1.0"))
        .expect("VideoConcat contract should resolve");
    assert_eq!(concat.contract().inputs[0].name, "clips");
    assert_eq!(
        concat.contract().inputs[0].cardinality,
        engine::PortCardinality::Many { minimum: 2, maximum: None }
    );
}

#[test]
fn duplicate_capability_refs_are_rejected() {
    let (_directory, store) = store();
    let mut registry = NodeRegistry::new();
    register(&mut registry, Arc::clone(&store));

    let error = nodes::register_all(
        &mut registry,
        Arc::new(NoopGenerator),
        Arc::new(NoopGenerator),
        Arc::new(NoopGenerator),
        store,
    )
    .expect_err("duplicate capability refs must be rejected");
    assert!(matches!(error, CapabilityRegistryError::DuplicateReference { .. }));
}

#[test]
fn workflow_resolution_requires_the_exact_contract_version() {
    let (_directory, store) = store();
    let mut registry = NodeRegistry::new();
    register(&mut registry, store);
    let params = serde_json::Map::new();

    let node = registry
        .instantiate_workflow_node("prompt", "TextPrompt", "1.0", &params)
        .expect("registered version should instantiate");
    assert_eq!(node.type_id(), "TextPrompt");

    let result = registry.instantiate_workflow_node("prompt", "TextPrompt", "2.0", &params);
    assert!(matches!(
        result,
        Err(engine::EngineError::UnknownCapabilityVersion {
            node_id,
            type_id,
            contract_version
        }) if node_id == "prompt" && type_id == "TextPrompt" && contract_version == "2.0"
    ));
}

fn store() -> (TempDir, SharedAssetStore) {
    let directory = TempDir::new().expect("asset root");
    let store = Arc::new(Mutex::new(AssetStore::open(directory.path()).expect("asset store")));
    (directory, store)
}

fn register(registry: &mut NodeRegistry, store: SharedAssetStore) {
    nodes::register_all(
        registry,
        Arc::new(NoopGenerator),
        Arc::new(NoopGenerator),
        Arc::new(NoopGenerator),
        store,
    )
    .expect("capability registration");
}

struct NoopGenerator;

impl TextToImageGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: TextToImageRequest,
        _context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError> {
        Err(GenerationError::OperationFailed {
            operation: "test",
            reason: "not executed".to_owned(),
        })
    }
}

impl ImageToVideoGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: ImageToVideoRequest,
        _context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError> {
        Err(GenerationError::OperationFailed {
            operation: "test",
            reason: "not executed".to_owned(),
        })
    }
}

impl TextToAudioGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: TextToAudioRequest,
        _context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError> {
        Err(GenerationError::OperationFailed {
            operation: "test",
            reason: "not executed".to_owned(),
        })
    }
}
