use crate::SharedAssetStore;
use crate::error::{NodesError, boxed};
use crate::media::{AssetMetadata, store_generated_asset};
use crate::params::{optional_param, string_param, text_input};
use crate::polling::wait_for_success;
use crate::ports::{output, required_input};
use assets::AssetKind;
use backends::{InferenceBackend, TextToAudioRequest};
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
    backend: Arc<dyn InferenceBackend>,
    store: SharedAssetStore,
) {
    registry.register(
        TYPE_ID,
        Box::new(move |params| {
            TextToAudioNode::from_params(params, Arc::clone(&backend), Arc::clone(&store))
                .map(boxed_node)
                .map_err(boxed)
        }),
    );
}

struct TextToAudioNode {
    backend: Arc<dyn InferenceBackend>,
    store: SharedAssetStore,
    model: String,
    seed: Option<u64>,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl TextToAudioNode {
    fn from_params(
        params: &NodeParams,
        backend: Arc<dyn InferenceBackend>,
        store: SharedAssetStore,
    ) -> Result<Self, NodesError> {
        Ok(Self {
            backend,
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
        info!(type_id = TYPE_ID, backend = self.backend.name(), "submitting text-to-audio task");
        let handle = pollster::block_on(self.backend.text_to_audio(self.request(prompt))).map_err(
            |source| boxed(NodesError::Backend { operation: "submit text-to-audio task", source }),
        )?;
        let output = wait_for_success(&self.backend, &handle, context).map_err(boxed)?;
        let asset = store_generated_asset(
            &self.store,
            AssetKind::Audio,
            &output.reference,
            TYPE_ID,
            context,
            AssetMetadata {
                prompt: Some(prompt.to_owned()),
                model: Some(self.model.clone()),
                seed: self.seed,
                cost: output.cost,
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
