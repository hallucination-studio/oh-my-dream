use crate::error::{NodesError, boxed};
use crate::media::{AssetMetadata, store_generated_asset};
use crate::params::{optional_param, string_param, text_input};
use crate::ports::{output, required_input};
use crate::{SharedAssetStore, TextToImageGenerator, TextToImageRequest};
use assets::AssetKind;
use engine::{
    InputPort, Node, NodeParams, NodeRegistry, NodeRunContext, NodeRunError, NodeRunResult,
    OutputPort, PortType, Value, ValueMap,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::info;

const TYPE_ID: &str = "TextToImage";

pub(crate) fn register(
    registry: &mut NodeRegistry,
    generator: Arc<dyn TextToImageGenerator>,
    store: SharedAssetStore,
) {
    registry.register(
        TYPE_ID,
        Box::new(move |params| {
            TextToImageNode::from_params(params, Arc::clone(&generator), Arc::clone(&store))
                .map(boxed_node)
                .map_err(boxed)
        }),
    );
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
        let output = {
            let mut on_progress = |progress| context.progress(progress);
            self.generator.generate(self.request(prompt), &mut on_progress)
        }
        .map_err(|source| boxed(NodesError::Generation { operation: "generate image", source }))?;
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
