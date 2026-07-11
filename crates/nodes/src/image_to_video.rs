use crate::error::{NodesError, boxed, generation_error};
use crate::media::{AssetMetadata, ResolvedImageInput, resolve_image_input, store_generated_asset};
use crate::params::{image_input, optional_param, string_param};
use crate::ports::{output, required_input};
use crate::{GenerationContext, ImageToVideoGenerator, ImageToVideoRequest, SharedAssetStore};
use assets::AssetKind;
use engine::{
    InputPort, Node, NodeParams, NodeRegistry, NodeRunContext, NodeRunError, NodeRunResult,
    OutputPort, PortType, Value, ValueMap,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::info;

const TYPE_ID: &str = "ImageToVideo";

pub(crate) fn register(
    registry: &mut NodeRegistry,
    generator: Arc<dyn ImageToVideoGenerator>,
    store: SharedAssetStore,
) {
    registry.register(
        TYPE_ID,
        Box::new(move |params| {
            ImageToVideoNode::from_params(params, Arc::clone(&generator), Arc::clone(&store))
                .map(boxed_node)
                .map_err(boxed)
        }),
    );
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
