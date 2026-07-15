use crate::error::{NodesError, boxed, generation_error};
use crate::media::{AssetMetadata, store_generated_asset};
use crate::params::{
    canonicalize_mode, image_inputs, optional_param, reject_unknown_params, string_param,
    text_input,
};
use crate::ports::{output, required_input, required_many_input};
use crate::{
    AssetMediaKind, AssetReferenceRequest, AssetReferenceResolver, GenerationContext,
    ReferenceImageGenerationRequest, ReferenceImageGenerator, SharedAssetStore,
};
use assets::AssetKind;
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilitySelector, InputPort, Node, NodeInputs, NodeParams,
    NodeRunContext, NodeRunError, NodeRunResult, OutputPort, PortCardinality, PortType, Value,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::info;

const TYPE_ID: &str = "ReferenceImageGeneration";
const MODE: &str = "references";
const MAX_REFERENCES: usize = 16;

pub(crate) fn registration(
    generator: Arc<dyn ReferenceImageGenerator>,
    store: SharedAssetStore,
    resolver: Arc<dyn AssetReferenceResolver>,
) -> CapabilityRegistration {
    let references = PortCardinality::Many { minimum: 1, maximum: Some(MAX_REFERENCES) };
    let contract = CapabilityContract::new(
        CapabilityRef::new(TYPE_ID, engine::DEFAULT_CAPABILITY_VERSION),
        vec![
            CapabilityPort::input("images", PortType::Image, true).with_cardinality(references),
            CapabilityPort::input("prompt", PortType::String, true),
        ],
        vec![CapabilityPort::output("image", PortType::Image)],
        params_schema(),
        NodeParams::from_iter([
            ("mode".to_owned(), serde_json::Value::String(MODE.to_owned())),
            ("model".to_owned(), serde_json::Value::String("mock-reference-image".to_owned())),
        ]),
        vec![CapabilityEffect::External],
    );
    CapabilityRegistration::new(
        contract,
        CapabilityPresentation::new(
            "Reference Image Generation",
            "Generate an image from ordered image references and a prompt.",
            "image",
            vec!["image".to_owned(), "generation".to_owned(), "references".to_owned()],
        ),
        Box::new(normalize_params),
        Box::new(move |params| {
            ReferenceImageNode::from_params(
                params,
                Arc::clone(&generator),
                Arc::clone(&store),
                Arc::clone(&resolver),
            )
            .map(|node| Box::new(node) as Box<dyn Node>)
            .map_err(boxed)
        }),
    )
    .with_selector(CapabilitySelector::new("Image", MODE))
}

fn params_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "mode": {"type": "string", "const": MODE, "default": MODE},
            "model": {"type": "string", "default": "mock-reference-image"},
            "negative_prompt": {"type": "string"},
            "steps": {"type": "integer", "minimum": 1},
            "seed": {"type": "integer", "minimum": 0}
        },
        "additionalProperties": false
    })
}

fn normalize_params(params: &NodeParams) -> Result<NodeParams, NodeRunError> {
    reject_unknown_params(params, &["mode", "model", "negative_prompt", "steps", "seed"])
        .map_err(boxed)?;
    let model = string_param(params, &["model"], "mock-reference-image").map_err(boxed)?;
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
    insert_optional(&mut normalized, "negative_prompt", negative_prompt);
    insert_optional(&mut normalized, "steps", steps);
    insert_optional(&mut normalized, "seed", seed);
    Ok(normalized)
}

fn insert_optional<T: serde::Serialize>(params: &mut NodeParams, name: &str, value: Option<T>) {
    if let Some(value) = value {
        params.insert(name.to_owned(), serde_json::json!(value));
    }
}

struct ReferenceImageNode {
    generator: Arc<dyn ReferenceImageGenerator>,
    store: SharedAssetStore,
    resolver: Arc<dyn AssetReferenceResolver>,
    model: String,
    negative_prompt: Option<String>,
    steps: Option<u32>,
    seed: Option<u64>,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl ReferenceImageNode {
    fn from_params(
        params: &NodeParams,
        generator: Arc<dyn ReferenceImageGenerator>,
        store: SharedAssetStore,
        resolver: Arc<dyn AssetReferenceResolver>,
    ) -> Result<Self, NodesError> {
        Ok(Self {
            generator,
            store,
            resolver,
            model: string_param(params, &["model"], "mock-reference-image")?,
            negative_prompt: optional_param(params, &["negative_prompt"])?,
            steps: optional_param(params, &["steps"])?,
            seed: optional_param(params, &["seed"])?,
            inputs: vec![
                required_many_input("images", PortType::Image, 1, Some(MAX_REFERENCES)),
                required_input("prompt", PortType::String),
            ],
            outputs: vec![output("image", PortType::Image)],
        })
    }

    fn resolve_images(
        &self,
        project_id: &str,
        images: Vec<&str>,
    ) -> Result<Vec<String>, NodeRunError> {
        images
            .into_iter()
            .map(|asset_id| {
                self.resolver
                    .resolve(AssetReferenceRequest {
                        project_id,
                        asset_id,
                        expected_kind: AssetMediaKind::Image,
                    })
                    .map(|resolved| resolved.local_path.to_string_lossy().into_owned())
                    .map_err(|source| Box::new(source) as NodeRunError)
            })
            .collect()
    }
}

impl Node for ReferenceImageNode {
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
        context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, NodeRunError> {
        let prompt = text_input(inputs, "prompt").map_err(boxed)?;
        let images = image_inputs(inputs, "images").map_err(boxed)?;
        let images = self.resolve_images(context.project_id(), images)?;
        info!(type_id = TYPE_ID, reference_count = images.len(), "generating reference image");
        let output = self
            .generator
            .generate(
                ReferenceImageGenerationRequest {
                    model: self.model.clone(),
                    images,
                    prompt: prompt.to_owned(),
                    negative_prompt: self.negative_prompt.clone(),
                    steps: self.steps,
                    seed: self.seed,
                },
                context,
            )
            .map_err(|source| generation_error("generate reference image", source))?;
        GenerationContext::ensure_active(context)
            .map_err(|source| generation_error("generate reference image", source))?;
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
