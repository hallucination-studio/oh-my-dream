use crate::error::{NodesError, boxed, generation_error};
use crate::media::{AssetMetadata, store_generated_asset};
use crate::params::{optional_param, string_param, text_input};
use crate::ports::{output, required_input};
use crate::{GenerationContext, SharedAssetStore, TextToAudioGenerator, TextToAudioRequest};
use assets::AssetKind;
use engine::{
    InputPort, Node, NodeParams, NodeRegistry, NodeRunContext, NodeRunError, NodeRunResult,
    OutputPort, PortType, Value, ValueMap,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::info;

const TYPE_ID: &str = "TextToAudio";

pub(crate) fn register(
    registry: &mut NodeRegistry,
    generator: Arc<dyn TextToAudioGenerator>,
    store: SharedAssetStore,
) {
    registry.register(
        TYPE_ID,
        Box::new(move |params| {
            TextToAudioNode::from_params(params, Arc::clone(&generator), Arc::clone(&store))
                .map(boxed_node)
                .map_err(boxed)
        }),
    );
}

struct TextToAudioNode {
    generator: Arc<dyn TextToAudioGenerator>,
    store: SharedAssetStore,
    model: String,
    seed: Option<u64>,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl TextToAudioNode {
    fn from_params(
        params: &NodeParams,
        generator: Arc<dyn TextToAudioGenerator>,
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

impl Node for TextToAudioNode {
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
        info!(type_id = TYPE_ID, "generating audio from text");
        let output = self
            .generator
            .generate(self.request(prompt), context)
            .map_err(|source| generation_error("generate audio", source))?;
        GenerationContext::ensure_active(context)
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

fn boxed_node(node: TextToAudioNode) -> Box<dyn Node> {
    Box::new(node)
}
