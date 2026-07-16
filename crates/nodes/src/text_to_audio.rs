use crate::error::{NodesError, boxed, generation_error};
use crate::media::{AssetMetadata, store_generated_asset};
use crate::params::{
    canonicalize_mode, optional_param, reject_unknown_params, string_param, text_input,
};
use crate::ports::{output, required_input};
use crate::{
    GenerationContextInterface, SharedAssetStore, TextToAudioGeneratorInterface, TextToAudioRequest,
};
use assets::AssetKind;
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilitySelector, InputPort, NodeInputs, NodeInterface, NodeParams,
    NodeRunContextImpl, NodeRunError, NodeRunResult, OutputPort, PortType, Value,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::info;

const TYPE_ID: &str = "TextToAudio";
const MODE: &str = "text";

pub(crate) fn registration(
    generator: Arc<dyn TextToAudioGeneratorInterface>,
    store: SharedAssetStore,
) -> CapabilityRegistration {
    let contract = CapabilityContract::new(
        CapabilityRef::new(TYPE_ID, engine::DEFAULT_CAPABILITY_VERSION),
        vec![CapabilityPort::input("prompt", PortType::String, true)],
        vec![CapabilityPort::output("audio", PortType::Audio)],
        serde_json::json!({
            "type": "object",
            "properties": {
                "mode": {"type": "string", "const": MODE, "default": MODE},
                "model": {"type": "string", "default": "mock-audio"},
                "seed": {"type": "integer", "minimum": 0}
            },
            "additionalProperties": false
        }),
        NodeParams::from_iter([
            ("mode".to_owned(), serde_json::Value::String(MODE.to_owned())),
            ("model".to_owned(), serde_json::Value::String("mock-audio".to_owned())),
        ]),
        vec![CapabilityEffect::External],
    );
    CapabilityRegistration::new(
        contract,
        CapabilityPresentation::new(
            "Text to Audio",
            "Generate an audio clip from a text prompt.",
            "audio",
            vec!["audio".to_owned(), "generation".to_owned(), "text to audio".to_owned()],
        ),
        Box::new(normalize_params),
        Box::new(move |params| {
            TextToAudioNodeImpl::from_params(params, Arc::clone(&generator), Arc::clone(&store))
                .map(boxed_node)
                .map_err(boxed)
        }),
    )
    .with_selector(CapabilitySelector::new("Audio", MODE))
}

fn normalize_params(params: &NodeParams) -> Result<NodeParams, NodeRunError> {
    reject_unknown_params(params, &["mode", "model", "seed"]).map_err(boxed)?;
    let model = string_param(params, &["model"], "mock-audio").map_err(boxed)?;
    let seed = optional_param::<u64>(params, &["seed"]).map_err(boxed)?;
    let mut normalized =
        NodeParams::from_iter([("model".to_owned(), serde_json::Value::String(model))]);
    canonicalize_mode(params, &mut normalized, MODE).map_err(boxed)?;
    if let Some(seed) = seed {
        normalized.insert("seed".to_owned(), serde_json::json!(seed));
    }
    Ok(normalized)
}

struct TextToAudioNodeImpl {
    generator: Arc<dyn TextToAudioGeneratorInterface>,
    store: SharedAssetStore,
    model: String,
    seed: Option<u64>,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl TextToAudioNodeImpl {
    fn from_params(
        params: &NodeParams,
        generator: Arc<dyn TextToAudioGeneratorInterface>,
        store: SharedAssetStore,
    ) -> Result<Self, NodesError> {
        Ok(Self {
            generator,
            store,
            model: string_param(params, &["model"], "mock-audio")?,
            seed: optional_param(params, &["seed"])?,
            inputs: vec![required_input("prompt", PortType::String)],
            outputs: vec![output("audio", PortType::Audio)],
        })
    }

    fn request(&self, prompt: &str) -> TextToAudioRequest {
        TextToAudioRequest { model: self.model.clone(), prompt: prompt.to_owned(), seed: self.seed }
    }
}

impl NodeInterface for TextToAudioNodeImpl {
    fn type_id(&self) -> &str {
        TYPE_ID
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn run(
        &self,
        inputs: &NodeInputs,
        context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, NodeRunError> {
        let prompt = text_input(inputs, "prompt").map_err(boxed)?;
        info!(type_id = TYPE_ID, "generating audio from text");
        let output = self
            .generator
            .generate(self.request(prompt), context)
            .map_err(|source| generation_error("generate audio", source))?;
        GenerationContextInterface::ensure_active(context)
            .map_err(|source| generation_error("generate audio", source))?;
        let asset = store_generated_asset(
            &self.store,
            AssetKind::Audio,
            &output,
            TYPE_ID,
            context,
            AssetMetadata {
                prompt: Some(prompt.to_owned()),
                model: Some(self.model.clone()),
                seed: self.seed,
            },
        )
        .map_err(boxed)?;
        Ok(NodeRunResult {
            outputs: BTreeMap::from([("audio".to_owned(), Value::Audio(asset.id))]),
            cost: output.cost,
        })
    }
}

fn boxed_node(node: TextToAudioNodeImpl) -> Box<dyn NodeInterface> {
    Box::new(node)
}
