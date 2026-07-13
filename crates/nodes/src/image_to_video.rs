use crate::error::{NodesError, boxed, generation_error};
use crate::media::{AssetMetadata, ResolvedImageInput, resolve_image_input, store_generated_asset};
use crate::params::{image_input, optional_param, string_param};
use crate::ports::{output, required_input};
use crate::{GenerationContext, ImageToVideoGenerator, ImageToVideoRequest, SharedAssetStore};
use assets::AssetKind;
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityRef, CapabilityRegistration,
    InputPort, Node, NodeParams, NodeRunContext, NodeRunError, NodeRunResult, OutputPort, PortType,
    Value, ValueMap,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::info;

const TYPE_ID: &str = "ImageToVideo";

pub(crate) fn registration(
    generator: Arc<dyn ImageToVideoGenerator>,
    store: SharedAssetStore,
) -> CapabilityRegistration {
    let contract = CapabilityContract::new(
        CapabilityRef::new(TYPE_ID, engine::DEFAULT_CAPABILITY_VERSION),
        vec![CapabilityPort::input("image", PortType::Image, true)],
        vec![CapabilityPort::output("video", PortType::Video)],
        serde_json::json!({
            "type": "object",
            "properties": {
                "model": {"type": "string", "default": "mock-video"},
                "duration": {"type": "number", "exclusiveMinimum": 0},
                "duration_seconds": {"type": "number", "exclusiveMinimum": 0},
                "fps": {"type": "integer", "minimum": 1}
            },
            "additionalProperties": false
        }),
        NodeParams::from_iter([(
            "model".to_owned(),
            serde_json::Value::String("mock-video".to_owned()),
        )]),
        vec![CapabilityEffect::External],
    );
    CapabilityRegistration::new(
        contract,
        Box::new(normalize_params),
        Box::new(move |params| {
            ImageToVideoNode::from_params(params, Arc::clone(&generator), Arc::clone(&store))
                .map(boxed_node)
                .map_err(boxed)
        }),
    )
}

fn normalize_params(params: &NodeParams) -> Result<NodeParams, NodeRunError> {
    reject_unknown_params(params)?;
    let model = string_param(params, &["model"], "mock-video").map_err(boxed)?;
    let duration =
        optional_param::<f32>(params, &["duration", "duration_seconds"]).map_err(boxed)?;
    let fps = optional_param::<u32>(params, &["fps"]).map_err(boxed)?;
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
    let mut normalized =
        NodeParams::from_iter([("model".to_owned(), serde_json::Value::String(model))]);
    if let Some(value) = duration {
        normalized.insert("duration".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = fps {
        normalized.insert("fps".to_owned(), serde_json::json!(value));
    }
    Ok(normalized)
}

fn reject_unknown_params(params: &NodeParams) -> Result<(), NodeRunError> {
    let allowed = ["model", "duration", "duration_seconds", "fps"];
    if let Some(name) = params.keys().find(|name| !allowed.contains(&name.as_str())) {
        return Err(boxed(NodesError::InvalidParam {
            name: name.clone(),
            reason: "unknown parameter".to_owned(),
        }));
    }
    Ok(())
}

struct ImageToVideoNode {
    generator: Arc<dyn ImageToVideoGenerator>,
    store: SharedAssetStore,
    model: String,
    duration_seconds: Option<f32>,
    fps: Option<u32>,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl ImageToVideoNode {
    fn from_params(
        params: &NodeParams,
        generator: Arc<dyn ImageToVideoGenerator>,
        store: SharedAssetStore,
    ) -> Result<Self, NodesError> {
        Ok(Self {
            generator,
            store,
            model: string_param(params, &["model"], "mock-video")?,
            duration_seconds: optional_param(params, &["duration", "duration_seconds"])?,
            fps: optional_param(params, &["fps"])?,
            inputs: vec![required_input("image", PortType::Image)],
            outputs: vec![output("video", PortType::Video)],
        })
    }

    fn request(&self, image: String) -> ImageToVideoRequest {
        ImageToVideoRequest {
            model: self.model.clone(),
            image,
            duration_seconds: self.duration_seconds,
            fps: self.fps,
        }
    }
}

impl Node for ImageToVideoNode {
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
        let image = image_input(inputs, "image").map_err(boxed)?;
        let ResolvedImageInput { file_path, prompt } =
            resolve_image_input(&self.store, image).map_err(boxed)?;
        info!(type_id = TYPE_ID, "generating video from image");
        let output = self
            .generator
            .generate(self.request(file_path), context)
            .map_err(|source| generation_error("generate video", source))?;
        GenerationContext::ensure_active(context)
            .map_err(|source| generation_error("generate video", source))?;
        let asset = store_generated_asset(
            &self.store,
            AssetKind::Video,
            &output,
            TYPE_ID,
            context,
            AssetMetadata { prompt, model: Some(self.model.clone()), seed: None },
        )
        .map_err(boxed)?;
        Ok(NodeRunResult {
            outputs: BTreeMap::from([("video".to_owned(), Value::Video(asset.id))]),
            cost: output.cost,
        })
    }
}

fn boxed_node(node: ImageToVideoNode) -> Box<dyn Node> {
    Box::new(node)
}
