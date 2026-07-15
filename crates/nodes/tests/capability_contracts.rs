use assets::AssetStore;
use engine::{
    CapabilityEffect, CapabilityRef, CapabilityRegistryError, CapabilitySelector, NodeRegistry,
};
use nodes::{
    GeneratedOutput, GenerationContext, GenerationError, ImageToVideoGenerator,
    ImageToVideoRequest, ReferenceImageGenerationRequest, ReferenceImageGenerator,
    SharedAssetStore, TextToAudioGenerator, TextToAudioRequest, TextToImageGenerator,
    TextToImageRequest,
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
            ("AudioAssetSource", "1.0"),
            ("ImageAssetSource", "1.0"),
            ("ImageToVideo", "1.0"),
            ("ReferenceImageGeneration", "1.0"),
            ("TextPrompt", "1.0"),
            ("TextToAudio", "1.0"),
            ("TextToImage", "1.0"),
            ("VideoAssetSource", "1.0"),
            ("VideoConcat", "1.0"),
        ]
    );

    let prompt = registry
        .capability(&CapabilityRef::new("TextPrompt", "1.0"))
        .expect("TextPrompt contract should resolve");
    assert_eq!(prompt.contract().reference.id, "TextPrompt");
    assert_eq!(prompt.contract().default_params.as_ref().expect("default params")["text"], "");
    assert_eq!(prompt.contract().effects, vec![CapabilityEffect::Pure]);

    let concat = registry
        .capability(&CapabilityRef::new("VideoConcat", "1.0"))
        .expect("VideoConcat contract should resolve");
    assert_eq!(concat.contract().inputs[0].name, "clips");
    assert_eq!(
        concat.contract().inputs[0].cardinality,
        engine::PortCardinality::Many { minimum: 2, maximum: None }
    );

    let reference_image = registry
        .capability(&CapabilityRef::new("ReferenceImageGeneration", "1.0"))
        .expect("ReferenceImageGeneration contract should resolve");
    assert_eq!(reference_image.contract().inputs[0].name, "images");
    assert_eq!(reference_image.contract().inputs[0].port_type, engine::PortType::Image);
    assert_eq!(
        reference_image.contract().inputs[0].cardinality,
        engine::PortCardinality::Many { minimum: 1, maximum: Some(16) }
    );
    assert_eq!(reference_image.contract().inputs[1].name, "prompt");
}

#[test]
fn registrations_own_their_selector_mode_defaults_and_schema() {
    let (_directory, store) = store();
    let mut registry = NodeRegistry::new();
    register(&mut registry, store);
    let expected = [
        ("TextPrompt", "Text", "literal"),
        ("TextToImage", "Image", "text"),
        ("ImageToVideo", "Video", "image"),
        ("ReferenceImageGeneration", "Image", "references"),
        ("VideoConcat", "Video", "concat"),
        ("TextToAudio", "Audio", "text"),
    ];

    for (id, type_id, mode) in expected {
        let registration = registry
            .capability(&CapabilityRef::new(id, "1.0"))
            .expect("exact registration should resolve");

        assert_eq!(registration.selector(), Some(&CapabilitySelector::new(type_id, mode)));
        assert_eq!(
            registration.contract().default_params.as_ref().expect("default params")["mode"],
            mode
        );
        assert_eq!(registration.contract().params_schema["properties"]["mode"]["const"], mode);
        assert_eq!(
            registration
                .normalize_params(&serde_json::Map::new())
                .expect("missing mode should normalize")["mode"],
            mode
        );
    }
}

#[test]
fn registration_rejects_non_string_or_mismatched_mode() {
    let (_directory, store) = store();
    let mut registry = NodeRegistry::new();
    register(&mut registry, store);
    let registration = registry
        .capability(&CapabilityRef::new("ImageToVideo", "1.0"))
        .expect("video registration should resolve");

    assert!(
        registration
            .normalize_params(&serde_json::Map::from_iter([(
                "mode".to_owned(),
                serde_json::json!(42),
            )]))
            .is_err()
    );
    assert!(
        registration
            .normalize_params(&serde_json::Map::from_iter([(
                "mode".to_owned(),
                serde_json::json!("concat"),
            )]))
            .is_err()
    );
}

#[test]
fn current_discovery_and_direct_instantiation_use_selectors() {
    let (_directory, store) = store();
    let mut registry = NodeRegistry::new();
    register(&mut registry, store);

    assert_eq!(
        registry.current_capability_refs(),
        vec![
            CapabilityRef::new("AudioAssetSource", "1.0"),
            CapabilityRef::new("ImageAssetSource", "1.0"),
            CapabilityRef::new("ImageToVideo", "1.0"),
            CapabilityRef::new("ReferenceImageGeneration", "1.0"),
            CapabilityRef::new("TextPrompt", "1.0"),
            CapabilityRef::new("TextToAudio", "1.0"),
            CapabilityRef::new("TextToImage", "1.0"),
            CapabilityRef::new("VideoAssetSource", "1.0"),
            CapabilityRef::new("VideoConcat", "1.0"),
        ]
    );

    let node = registry
        .instantiate(
            "image",
            "Image",
            &serde_json::Map::from_iter([("mode".to_owned(), serde_json::json!("text"))]),
        )
        .expect("selector-shaped node should instantiate");
    assert_eq!(node.type_id(), "TextToImage");
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
        Arc::new(NoopGenerator),
        store,
        Arc::new(support::MissingResolver),
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

    let audio = registry
        .instantiate_workflow_node("audio", "TextToAudio", "1.0", &params)
        .expect("legacy exact-id audio should instantiate without mode");
    assert_eq!(audio.type_id(), "TextToAudio");
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
        Arc::new(NoopGenerator),
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
