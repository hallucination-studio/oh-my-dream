use crate::error::boxed;
use crate::params::canonicalize_mode;
use crate::ports::{output, required_many_input};
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilitySelector, Node, NodeParams, NodeRunContext, NodeRunError,
    NodeRunResult, OutputPort, PortCardinality, PortType, Value, ValueMap,
};
use std::collections::BTreeMap;

const TYPE_ID: &str = "VideoConcat";
const CONTRACT_VERSION: &str = engine::DEFAULT_CAPABILITY_VERSION;
const MODE: &str = "concat";

pub(crate) fn registration() -> CapabilityRegistration {
    let reference = CapabilityRef::new(TYPE_ID, CONTRACT_VERSION);
    let contract = CapabilityContract::new(
        reference,
        vec![
            CapabilityPort::input("clips", PortType::Video, true)
                .with_cardinality(PortCardinality::Many { minimum: 2, maximum: None }),
        ],
        vec![CapabilityPort::output("video", PortType::Video)],
        serde_json::json!({
            "type": "object",
            "properties": {
                "mode": {"type": "string", "const": MODE, "default": MODE}
            },
            "additionalProperties": false
        }),
        NodeParams::from_iter([("mode".to_owned(), serde_json::Value::String(MODE.to_owned()))]),
        vec![CapabilityEffect::Pure],
    );
    CapabilityRegistration::new(
        contract,
        CapabilityPresentation::new(
            "Video Concat",
            "Join ordered video clips into one sequence.",
            "video",
            vec!["concat".to_owned(), "sequence".to_owned(), "video".to_owned()],
        ),
        Box::new(normalize_params),
        Box::new(|_| Ok(Box::new(VideoConcatNode::new()))),
    )
    .with_selector(CapabilitySelector::new("Video", MODE))
}

fn normalize_params(params: &NodeParams) -> Result<NodeParams, NodeRunError> {
    if let Some(name) = params.keys().find(|name| name.as_str() != "mode") {
        return Err(boxed(crate::error::NodesError::InvalidParam {
            name: name.clone(),
            reason: "VideoConcat does not accept params".to_owned(),
        }));
    }
    let mut normalized = NodeParams::new();
    canonicalize_mode(params, &mut normalized, MODE).map_err(boxed)?;
    Ok(normalized)
}

struct VideoConcatNode {
    inputs: Vec<engine::InputPort>,
    outputs: Vec<OutputPort>,
}

impl VideoConcatNode {
    fn new() -> Self {
        Self {
            inputs: vec![required_many_input("clips", PortType::Video, 2, None)],
            outputs: vec![output("video", PortType::Video)],
        }
    }
}

impl Node for VideoConcatNode {
    fn type_id(&self) -> &str {
        TYPE_ID
    }

    fn inputs(&self) -> &[engine::InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn run(
        &self,
        inputs: &ValueMap,
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, NodeRunError> {
        let Some(Value::Video(reference)) = inputs.get("clips") else {
            return Err(boxed(crate::error::NodesError::WrongInputType {
                name: "clips".to_owned(),
                expected: "video",
            }));
        };
        Ok(NodeRunResult::new(BTreeMap::from([(
            "video".to_owned(),
            Value::Video(format!("concat://{reference}")),
        )])))
    }
}
