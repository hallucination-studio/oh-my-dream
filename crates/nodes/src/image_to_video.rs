use crate::error::{NodesError, boxed};
use crate::params::{image_input, optional_param, string_param};
use crate::polling::wait_for_success;
use crate::ports::{output, required_input};
use backends::{ImageToVideoRequest, InferenceBackend};
use engine::{
    InputPort, Node, NodeParams, NodeRegistry, NodeRunError, OutputPort, PortType, Value, ValueMap,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::info;

const TYPE_ID: &str = "ImageToVideo";

pub(crate) fn register(registry: &mut NodeRegistry, backend: Arc<dyn InferenceBackend>) {
    registry.register(
        TYPE_ID,
        Box::new(move |params| {
            ImageToVideoNode::from_params(params, Arc::clone(&backend))
                .map(boxed_node)
                .map_err(boxed)
        }),
    );
}

struct ImageToVideoNode {
    backend: Arc<dyn InferenceBackend>,
    model: String,
    duration_seconds: Option<f32>,
    fps: Option<u32>,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl ImageToVideoNode {
    fn from_params(
        params: &NodeParams,
        backend: Arc<dyn InferenceBackend>,
    ) -> Result<Self, NodesError> {
        Ok(Self {
            backend,
            model: string_param(params, &["model"], "mock-video")?,
            duration_seconds: optional_param(params, &["duration", "duration_seconds"])?,
            fps: optional_param(params, &["fps"])?,
            inputs: vec![required_input("image", PortType::Image)],
            outputs: vec![output("video", PortType::Video)],
        })
    }

    fn request(&self, image: &str) -> ImageToVideoRequest {
        ImageToVideoRequest {
            model: self.model.clone(),
            image: image.to_owned(),
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

    fn run(&self, inputs: &ValueMap) -> Result<ValueMap, NodeRunError> {
        let image = image_input(inputs, "image").map_err(boxed)?;
        info!(type_id = TYPE_ID, backend = self.backend.name(), "submitting image-to-video task");
        let handle = pollster::block_on(self.backend.image_to_video(self.request(image))).map_err(
            |source| boxed(NodesError::Backend { operation: "submit image-to-video task", source }),
        )?;
        let output = wait_for_success(&self.backend, &handle).map_err(boxed)?;
        Ok(BTreeMap::from([("video".to_owned(), Value::Video(output))]))
    }
}

fn boxed_node(node: ImageToVideoNode) -> Box<dyn Node> {
    Box::new(node)
}
