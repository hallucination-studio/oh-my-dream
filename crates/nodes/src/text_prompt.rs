use crate::error::boxed;
use crate::params::string_param;
use crate::ports::output;
use engine::{
    InputPort, Node, NodeParams, NodeRegistry, NodeRunContext, NodeRunError, NodeRunResult,
    OutputPort, PortType, Value, ValueMap,
};
use std::collections::BTreeMap;
use tracing::info;

const TYPE_ID: &str = "TextPrompt";

pub(crate) fn register(registry: &mut NodeRegistry) {
    registry.register(
        TYPE_ID,
        Box::new(|params| TextPromptNode::from_params(params).map(boxed_node).map_err(boxed)),
    );
}

struct TextPromptNode {
    prompt: String,
    outputs: Vec<OutputPort>,
}

impl TextPromptNode {
    fn from_params(params: &NodeParams) -> Result<Self, crate::error::NodesError> {
        Ok(Self {
            prompt: string_param(params, &["text", "prompt"], "")?,
            outputs: vec![output("text", PortType::String)],
        })
    }
}

impl Node for TextPromptNode {
    fn type_id(&self) -> &str {
        TYPE_ID
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn run(
        &self,
        _inputs: &ValueMap,
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, NodeRunError> {
        info!(type_id = TYPE_ID, "text prompt node produced text");
        Ok(NodeRunResult::new(BTreeMap::from([(
            "text".to_owned(),
            Value::String(self.prompt.clone()),
        )])))
    }
}

fn boxed_node(node: TextPromptNode) -> Box<dyn Node> {
    Box::new(node)
}
