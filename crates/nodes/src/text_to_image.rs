use crate::error::{NodesError, boxed};
use crate::params::{optional_param, string_param, text_input};
use crate::polling::wait_for_success;
use crate::ports::{output, required_input};
use backends::{InferenceBackend, TextToImageRequest};
use engine::{
    InputPort, Node, NodeParams, NodeRegistry, NodeRunError, OutputPort, PortType, Value, ValueMap,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::info;

const TYPE_ID: &str = "TextToImage";

pub(crate) fn register(registry: &mut NodeRegistry, backend: Arc<dyn InferenceBackend>) {
    registry.register(
        TYPE_ID,
        Box::new(move |params| {
            TextToImageNode::from_params(params, Arc::clone(&backend))
                .map(boxed_node)
                .map_err(boxed)
        }),
    );
}

struct TextToImageNode {
    backend: Arc<dyn InferenceBackend>,
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
        backend: Arc<dyn InferenceBackend>,
    ) -> Result<Self, NodesError> {
        Ok(Self {
            backend,
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

    fn run(&self, inputs: &ValueMap) -> Result<ValueMap, NodeRunError> {
        let prompt = text_input(inputs, "prompt").map_err(boxed)?;
        info!(type_id = TYPE_ID, backend = self.backend.name(), "submitting text-to-image task");
        let handle = pollster::block_on(self.backend.text_to_image(self.request(prompt))).map_err(
            |source| boxed(NodesError::Backend { operation: "submit text-to-image task", source }),
        )?;
        let output = wait_for_success(&self.backend, &handle).map_err(boxed)?;
        Ok(BTreeMap::from([("image".to_owned(), Value::Image(output))]))
    }
}

fn boxed_node(node: TextToImageNode) -> Box<dyn Node> {
    Box::new(node)
}
