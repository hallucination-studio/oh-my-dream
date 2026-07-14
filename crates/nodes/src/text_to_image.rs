use crate::error::{NodesError, boxed, generation_error};
use crate::media::{AssetMetadata, store_generated_asset};
use crate::params::{
    canonicalize_mode, optional_param, reject_unknown_params, string_param, text_input,
};
use crate::ports::{output, required_input};
use crate::{GenerationContext, SharedAssetStore, TextToImageGenerator, TextToImageRequest};
use assets::AssetKind;
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilitySelector, InputPort, Node, NodeParams, NodeRunContext, NodeRunError,
    NodeRunResult, OutputPort, PortType, Value, ValueMap,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::info;

const TYPE_ID: &str = "TextToImage";
const MODE: &str = "text";

pub(crate) fn registration(
    generator: Arc<dyn TextToImageGenerator>,
    store: SharedAssetStore,
) -> CapabilityRegistration {
    let contract = CapabilityContract::new(
        CapabilityRef::new(TYPE_ID, engine::DEFAULT_CAPABILITY_VERSION),
        vec![CapabilityPort::input("prompt", PortType::String, true)],
        vec![CapabilityPort::output("image", PortType::Image)],
        serde_json::json!({
            "type": "object",
            "properties": {
                "mode": {"type": "string", "const": MODE, "default": MODE},
                "model": {"type": "string", "default": "mock-image"},
                "negative_prompt": {"type": "string"},
                "steps": {"type": "integer", "minimum": 1},
                "seed": {"type": "integer", "minimum": 0}
            },
            "additionalProperties": false
        }),
        NodeParams::from_iter([
            ("mode".to_owned(), serde_json::Value::String(MODE.to_owned())),
            ("model".to_owned(), serde_json::Value::String("mock-image".to_owned())),
        ]),
        vec![CapabilityEffect::External],
    );
    CapabilityRegistration::new(
        contract,
        CapabilityPresentation::new(
            "Text to Image",
            "Generate an image from a text prompt.",
            "image",
            vec!["image".to_owned(), "generation".to_owned(), "text to image".to_owned()],
        ),
        Box::new(normalize_params),
        Box::new(move |params| {
            TextToImageNode::from_params(params, Arc::clone(&generator), Arc::clone(&store))
                .map(boxed_node)
                .map_err(boxed)
        }),
    )
    .with_selector(CapabilitySelector::new("Image", MODE))
}

fn normalize_params(params: &NodeParams) -> Result<NodeParams, NodeRunError> {
    reject_unknown_params(params, &["mode", "model", "negative_prompt", "steps", "seed"])
        .map_err(boxed)?;
    let model = string_param(params, &["model"], "mock-image").map_err(boxed)?;
    let negative_prompt = optional_param::<String>(params, &["negative_prompt"]).map_err(boxed)?;
    let steps = optional_param::<u32>(params, &["steps"]).map_err(boxed)?;
    let seed = optional_param::<u64>(params, &["seed"]).map_err(boxed)?;
    if steps == Some(0) {
        return Err(boxed(NodesError::InvalidParam {
            name: "steps".to_owned(),
            reason: "must be at least 1".to_owned(),
        }));
    }
    let mut normalized =
        NodeParams::from_iter([("model".to_owned(), serde_json::Value::String(model))]);
    canonicalize_mode(params, &mut normalized, MODE).map_err(boxed)?;
    if let Some(value) = negative_prompt {
        normalized.insert("negative_prompt".to_owned(), serde_json::Value::String(value));
    }
    if let Some(value) = steps {
        normalized.insert("steps".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = seed {
        normalized.insert("seed".to_owned(), serde_json::json!(value));
    }
    Ok(normalized)
}

struct TextToImageNode {
    generator: Arc<dyn TextToImageGenerator>,
    store: SharedAssetStore,
    model: String,
    negative_prompt: Option<String>,
    steps: Option<u32>,
    seed: Option<u64>,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl TextToImageNode {
    fn from_params(
        params: &NodeParams,
        generator: Arc<dyn TextToImageGenerator>,
        store: SharedAssetStore,
    ) -> Result<Self, NodesError> {
        Ok(Self {
            generator,
            store,
            model: string_param(params, &["model"], "mock-image")?,
            negative_prompt: optional_param(params, &["negative_prompt"])?,
            steps: optional_param(params, &["steps"])?,
            seed: optional_param(params, &["seed"])?,
            inputs: vec![required_input("prompt", PortType::String)],
            outputs: vec![output("image", PortType::Image)],
        })
    }

    fn request(&self, prompt: &str) -> TextToImageRequest {
        TextToImageRequest {
            model: self.model.clone(),
            prompt: prompt.to_owned(),
            negative_prompt: self.negative_prompt.clone(),
            steps: self.steps,
            seed: self.seed,
        }
    }
}

impl Node for TextToImageNode {
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
        inputs: &ValueMap,
        context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, NodeRunError> {
        let prompt = text_input(inputs, "prompt").map_err(boxed)?;
        info!(type_id = TYPE_ID, "generating image from text");
        let output = self
            .generator
            .generate(self.request(prompt), context)
            .map_err(|source| generation_error("generate image", source))?;
        GenerationContext::ensure_active(context)
            .map_err(|source| generation_error("generate image", source))?;
        let asset = store_generated_asset(
            &self.store,
            AssetKind::Image,
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
            outputs: BTreeMap::from([("image".to_owned(), Value::Image(asset.id))]),
            cost: output.cost,
        })
    }
}

fn boxed_node(node: TextToImageNode) -> Box<dyn Node> {
    Box::new(node)
}
