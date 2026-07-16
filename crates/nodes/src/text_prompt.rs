use crate::error::boxed;
use crate::params::{canonicalize_mode, reject_unknown_params, string_param};
use crate::ports::output;
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilitySelector, InputPort, NodeInputs, NodeInterface, NodeParams,
    NodeRunContextImpl, NodeRunError, NodeRunResult, OutputPort, PortType, WorkflowNodeValue,
};
use std::collections::BTreeMap;
use tracing::info;

const TYPE_ID: &str = "TextPrompt";
const MODE: &str = "literal";

pub(crate) fn registration() -> CapabilityRegistration {
    let contract = CapabilityContract::new(
        CapabilityRef::new(TYPE_ID, engine::DEFAULT_CAPABILITY_VERSION),
        vec![],
        vec![CapabilityPort::output("text", PortType::String)],
        serde_json::json!({
            "type": "object",
            "properties": {
                "mode": {"type": "string", "const": MODE, "default": MODE},
                "text": {"type": "string", "default": ""},
                "prompt": {"type": "string"}
            },
            "additionalProperties": false
        }),
        NodeParams::from_iter([
            ("mode".to_owned(), serde_json::Value::String(MODE.to_owned())),
            ("text".to_owned(), serde_json::Value::String(String::new())),
        ]),
        vec![CapabilityEffect::Pure],
    );
    CapabilityRegistration::new(
        contract,
        CapabilityPresentation::new(
            "Text Prompt",
            "Provide a reusable text prompt to downstream nodes.",
            "input",
            vec!["prompt".to_owned(), "text".to_owned()],
        ),
        Box::new(normalize_params),
        Box::new(|params| TextPromptNodeImpl::from_params(params).map(boxed_node).map_err(boxed)),
    )
    .with_selector(CapabilitySelector::new("Text", MODE))
}

fn normalize_params(params: &NodeParams) -> Result<NodeParams, NodeRunError> {
    let text = string_param(params, &["text", "prompt"], "").map_err(boxed)?;
    reject_unknown_params(params, &["mode", "text", "prompt"]).map_err(boxed)?;
    let mut normalized =
        NodeParams::from_iter([("text".to_owned(), serde_json::Value::String(text))]);
    canonicalize_mode(params, &mut normalized, MODE).map_err(boxed)?;
    Ok(normalized)
}

struct TextPromptNodeImpl {
    prompt: String,
    outputs: Vec<OutputPort>,
}

impl TextPromptNodeImpl {
    fn from_params(params: &NodeParams) -> Result<Self, crate::error::NodesError> {
        Ok(Self {
            prompt: string_param(params, &["text", "prompt"], "")?,
            outputs: vec![output("text", PortType::String)],
        })
    }
}

impl NodeInterface for TextPromptNodeImpl {
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
        _inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, NodeRunError> {
        info!(type_id = TYPE_ID, "text prompt node produced text");
        Ok(NodeRunResult::new(BTreeMap::from([(
            "text".to_owned(),
            WorkflowNodeValue::String(self.prompt.clone()),
        )])))
    }
}

fn boxed_node(node: TextPromptNodeImpl) -> Box<dyn NodeInterface> {
    Box::new(node)
}
