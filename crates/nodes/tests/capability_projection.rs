use assets::AssetStore;
use engine::{CapabilityEffect, CapabilityRef, NodeRegistry, PortCardinality};
use nodes::{GeneratedOutput, GenerationContext, GenerationError, ImageToVideoGenerator};
use nodes::{
    ImageToVideoRequest, ReferenceImageGenerationRequest, ReferenceImageGenerator,
    ReferenceVideoGenerationRequest, ReferenceVideoGenerator, SharedAssetStore,
    TextToAudioGenerator, TextToAudioRequest, TextToImageGenerator, TextToImageRequest,
};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn capability_projection_contains_exact_contract_and_presentation_pairs() {
    let (_directory, store) = store();
    let mut registry = NodeRegistry::new();
    register(&mut registry, store);

    let projections = nodes::project_capabilities(&registry).expect("project capabilities");
    let references = projections
        .iter()
        .map(|projection| projection.contract.reference.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        references,
        vec![
            CapabilityRef::new("AudioAssetSource", "1.0"),
            CapabilityRef::new("ImageAssetSource", "1.0"),
            CapabilityRef::new("ImageToVideo", "1.0"),
            CapabilityRef::new("ReferenceImageGeneration", "1.0"),
            CapabilityRef::new("ReferenceVideoGeneration", "1.0"),
            CapabilityRef::new("TextPrompt", "1.0"),
            CapabilityRef::new("TextToAudio", "1.0"),
            CapabilityRef::new("TextToImage", "1.0"),
            CapabilityRef::new("VideoAssetSource", "1.0"),
            CapabilityRef::new("VideoConcat", "1.0"),
        ]
    );

    let prompt = projections
        .iter()
        .find(|projection| projection.contract.reference.id == "TextPrompt")
        .expect("prompt projection");
    assert_eq!(prompt.contract.default_params.as_ref().expect("default params")["text"], "");
    assert_eq!(prompt.selector.type_id, "Text");
    assert_eq!(prompt.selector.mode, "literal");
    assert_eq!(prompt.contract.effects, vec![CapabilityEffect::Pure]);
    assert_eq!(prompt.presentation.label, "Text Prompt");
    assert!(prompt.presentation.search_terms.iter().any(|term| term == "prompt"));

    let concat = projections
        .iter()
        .find(|projection| projection.contract.reference.id == "VideoConcat")
        .expect("concat projection");
    assert_eq!(
        concat.contract.inputs[0].cardinality,
        PortCardinality::Many { minimum: 2, maximum: None }
    );
    assert_eq!(concat.presentation.category, "video");
}

#[test]
fn contract_and_presentation_round_trip_without_changing_identity() {
    let (_directory, store) = store();
    let mut registry = NodeRegistry::new();
    register(&mut registry, store);
    let projection = nodes::project_capabilities(&registry)
        .expect("project capabilities")
        .into_iter()
        .find(|projection| projection.contract.reference.id == "TextToImage")
        .expect("image projection");
    let reference = projection.contract.reference.clone();

    let contract_json = serde_json::to_value(&projection.contract).expect("serialize contract");
    let contract = serde_json::from_value::<engine::CapabilityContract>(contract_json)
        .expect("deserialize contract");
    assert_eq!(contract.reference, reference);
    assert_eq!(contract, projection.contract);

    let presentation_json =
        serde_json::to_value(&projection.presentation).expect("serialize presentation");
    let presentation = serde_json::from_value::<engine::CapabilityPresentation>(presentation_json)
        .expect("deserialize presentation");
    assert_eq!(presentation, projection.presentation);
    assert_eq!(contract.reference, reference);
}

fn store() -> (TempDir, SharedAssetStore) {
    let directory = TempDir::new().expect("asset root");
    let store = Arc::new(Mutex::new(AssetStore::open(directory.path()).expect("asset store")));
    (directory, store)
}

fn register(registry: &mut NodeRegistry, store: SharedAssetStore) {
    nodes::register_all(
        registry,
        nodes::GenerationAdapters::new(
            Arc::new(NoopGenerator),
            Arc::new(NoopGenerator),
            Arc::new(NoopGenerator),
            Arc::new(NoopGenerator),
            Arc::new(NoopGenerator),
        ),
        store,
        Arc::new(support::MissingResolver),
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

impl ReferenceImageGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: ReferenceImageGenerationRequest,
        _context: &mut dyn GenerationContext,
    ) -> Result<GeneratedOutput, GenerationError> {
        Err(GenerationError::OperationFailed {
            operation: "test",
            reason: "not executed".to_owned(),
        })
    }
}

impl ReferenceVideoGenerator for NoopGenerator {
    fn generate(
        &self,
        _request: ReferenceVideoGenerationRequest,
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
mod support;
