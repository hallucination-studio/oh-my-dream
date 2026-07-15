use crate::error::{NodesError, boxed, generation_error};
use crate::media::{AssetMetadata, store_generated_asset};
use crate::params::{
    canonicalize_mode, image_inputs, optional_param, reject_unknown_params, string_param,
    text_input,
};
use crate::ports::{output, required_input, required_many_input};
use crate::{
    AssetMediaKind, AssetReferenceRequest, AssetReferenceResolver, GenerationContext,
    ReferenceVideoGenerationRequest, ReferenceVideoGenerator, SharedAssetStore,
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

const TYPE_ID: &str = "ReferenceVideoGeneration";
const MODE: &str = "references";
const MAX_REFERENCES: usize = 16;

pub(crate) fn registration(
    generator: Arc<dyn ReferenceVideoGenerator>,
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
        vec![CapabilityPort::output("video", PortType::Video)],
        params_schema(),
        NodeParams::from_iter([
            ("mode".to_owned(), serde_json::Value::String(MODE.to_owned())),
            ("model".to_owned(), serde_json::Value::String("mock-reference-video".to_owned())),
        ]),
        vec![CapabilityEffect::External],
    );
    CapabilityRegistration::new(
        contract,
        CapabilityPresentation::new(
            "Reference Video Generation",
            "Generate a video from ordered image references and a prompt.",
            "video",
            vec!["video".to_owned(), "generation".to_owned(), "references".to_owned()],
        ),
        Box::new(normalize_params),
        Box::new(move |params| {
            ReferenceVideoNode::from_params(
                params,
                Arc::clone(&generator),
                Arc::clone(&store),
                Arc::clone(&resolver),
            )
            .map(|node| Box::new(node) as Box<dyn Node>)
            .map_err(boxed)
        }),
    )
    .with_selector(CapabilitySelector::new("Video", MODE))
}

fn params_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "mode": {"type": "string", "const": MODE, "default": MODE},
            "model": {"type": "string", "default": "mock-reference-video"},
            "duration": {"type": "number", "exclusiveMinimum": 0},
            "duration_seconds": {"type": "number", "exclusiveMinimum": 0},
            "aspect_ratio": {"type": "string"},
            "resolution": {"type": "string"},
            "fps": {"type": "integer", "minimum": 1}
        },
        "additionalProperties": false
    })
}

fn normalize_params(params: &NodeParams) -> Result<NodeParams, NodeRunError> {
    reject_unknown_params(
        params,
        &["mode", "model", "duration", "duration_seconds", "aspect_ratio", "resolution", "fps"],
    )
    .map_err(boxed)?;
    let model = string_param(params, &["model"], "mock-reference-video").map_err(boxed)?;
    let duration =
        optional_param::<f32>(params, &["duration", "duration_seconds"]).map_err(boxed)?;
    let aspect_ratio = optional_param::<String>(params, &["aspect_ratio"]).map_err(boxed)?;
    let resolution = optional_param::<String>(params, &["resolution"]).map_err(boxed)?;
    let fps = optional_param::<u32>(params, &["fps"]).map_err(boxed)?;
    validate_options(duration, fps)?;
    let mut normalized =
        NodeParams::from_iter([("model".to_owned(), serde_json::Value::String(model))]);
    canonicalize_mode(params, &mut normalized, MODE).map_err(boxed)?;
    insert_optional(&mut normalized, "duration", duration);
    insert_optional(&mut normalized, "aspect_ratio", aspect_ratio);
    insert_optional(&mut normalized, "resolution", resolution);
    insert_optional(&mut normalized, "fps", fps);
    Ok(normalized)
}

fn validate_options(duration: Option<f32>, fps: Option<u32>) -> Result<(), NodeRunError> {
    if duration.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return Err(boxed(NodesError::InvalidParam {
            name: "duration".to_owned(),
            reason: "must be a positive finite number".to_owned(),
        }));
    }
    if fps == Some(0) {
        return Err(boxed(NodesError::InvalidParam {
            name: "fps".to_owned(),
            reason: "must be at least 1".to_owned(),
        }));
    }
    Ok(())
}

fn insert_optional<T: serde::Serialize>(params: &mut NodeParams, name: &str, value: Option<T>) {
    if let Some(value) = value {
        params.insert(name.to_owned(), serde_json::json!(value));
    }
}

struct ReferenceVideoNode {
    generator: Arc<dyn ReferenceVideoGenerator>,
    store: SharedAssetStore,
    resolver: Arc<dyn AssetReferenceResolver>,
    model: String,
    duration_seconds: Option<f32>,
    aspect_ratio: Option<String>,
    resolution: Option<String>,
    fps: Option<u32>,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl ReferenceVideoNode {
    fn from_params(
        params: &NodeParams,
        generator: Arc<dyn ReferenceVideoGenerator>,
        store: SharedAssetStore,
        resolver: Arc<dyn AssetReferenceResolver>,
    ) -> Result<Self, NodesError> {
        Ok(Self {
            generator,
            store,
            resolver,
            model: string_param(params, &["model"], "mock-reference-video")?,
            duration_seconds: optional_param(params, &["duration", "duration_seconds"])?,
            aspect_ratio: optional_param(params, &["aspect_ratio"])?,
            resolution: optional_param(params, &["resolution"])?,
            fps: optional_param(params, &["fps"])?,
            inputs: vec![
                required_many_input("images", PortType::Image, 1, Some(MAX_REFERENCES)),
                required_input("prompt", PortType::String),
            ],
            outputs: vec![output("video", PortType::Video)],
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

impl Node for ReferenceVideoNode {
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
        let images = self
            .resolve_images(context.project_id(), image_inputs(inputs, "images").map_err(boxed)?)?;
        info!(type_id = TYPE_ID, reference_count = images.len(), "generating reference video");
        let output = self
            .generator
            .generate(
                ReferenceVideoGenerationRequest {
                    model: self.model.clone(),
                    images,
                    prompt: prompt.to_owned(),
                    duration_seconds: self.duration_seconds,
                    aspect_ratio: self.aspect_ratio.clone(),
                    resolution: self.resolution.clone(),
                    fps: self.fps,
                },
                context,
            )
            .map_err(|source| generation_error("generate reference video", source))?;
        GenerationContext::ensure_active(context)
            .map_err(|source| generation_error("generate reference video", source))?;
        let asset = store_generated_asset(
            &self.store,
            AssetKind::Video,
            &output,
            TYPE_ID,
            context,
            AssetMetadata {
                prompt: Some(prompt.to_owned()),
                model: Some(self.model.clone()),
                seed: None,
            },
        )
        .map_err(boxed)?;
        Ok(NodeRunResult {
            outputs: BTreeMap::from([("video".to_owned(), Value::Video(asset.id))]),
            cost: output.cost,
        })
    }
}
