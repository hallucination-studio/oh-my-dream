use crate::error::boxed;
use crate::params::canonicalize_mode;
use crate::ports::{output, required_many_input};
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilitySelector, InputValue, NodeInputs, NodeInterface, NodeParams,
    NodeRunContextImpl, NodeRunError, NodeRunResult, OutputPort, PortCardinality, PortType, Value,
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
        Box::new(|_| Ok(Box::new(VideoConcatNodeImpl::new()))),
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

struct VideoConcatNodeImpl {
    inputs: Vec<engine::InputPort>,
    outputs: Vec<OutputPort>,
}

impl VideoConcatNodeImpl {
    fn new() -> Self {
        Self {
            inputs: vec![required_many_input("clips", PortType::Video, 2, None)],
            outputs: vec![output("video", PortType::Video)],
        }
    }
}

impl NodeInterface for VideoConcatNodeImpl {
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
        inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, NodeRunError> {
        let Some(InputValue::OrderedMany(clips)) = inputs.get("clips") else {
            return Err(boxed(crate::error::NodesError::WrongInputType {
                name: "clips".to_owned(),
                expected: "video",
            }));
        };
        let references = clips
            .iter()
            .map(|clip| match clip {
                Value::Video(reference) => Ok(reference.as_str()),
                _ => Err(boxed(crate::error::NodesError::WrongInputType {
                    name: "clips".to_owned(),
                    expected: "video",
                })),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(NodeRunResult::new(BTreeMap::from([(
            "video".to_owned(),
            Value::Video(concat_reference(&references)),
        )])))
    }
}

fn concat_reference(references: &[&str]) -> String {
    let mut encoded = String::from("concat://");
    for reference in references {
        encoded.push_str(&reference.len().to_string());
        encoded.push(':');
        encoded.push_str(reference);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::concat_reference;

    #[test]
    fn placeholder_reference_preserves_order_and_boundaries() {
        assert_ne!(concat_reference(&["a", "b|c"]), concat_reference(&["a|b", "c"]));
        assert_ne!(concat_reference(&["a", "b"]), concat_reference(&["b", "a"]));
    }
}
