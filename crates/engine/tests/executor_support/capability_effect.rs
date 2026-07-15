use super::{RunCounters, TestNodeError, output, registry};
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilitySelector, InputBinding, InputPort, Node, NodeInputs,
    NodeParams, NodeRegistry, NodeRunContext, NodeRunResult, OutputPort, OutputRef, PortType,
    Value, Workflow, WorkflowNode,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub(crate) fn capability_effect_registry(
    counters: RunCounters,
    available: Arc<AtomicBool>,
    effect: CapabilityEffect,
) -> NodeRegistry {
    let mut registry = registry(counters.clone());
    let runs = Arc::clone(&counters.text_prompt);
    let registration = CapabilityRegistration::new(
        CapabilityContract::new(
            CapabilityRef::new("ManagedTextSource", "1.0"),
            Vec::new(),
            vec![CapabilityPort::output("text", PortType::String)],
            serde_json::json!({ "type": "object", "additionalProperties": false }),
            NodeParams::new(),
            vec![effect],
        ),
        CapabilityPresentation::new("Managed Text", "Managed local text", "test", Vec::new()),
        Box::new(|params| Ok(params.clone())),
        Box::new(move |_| {
            Ok(Box::new(ManagedTextNode {
                available: Arc::clone(&available),
                outputs: vec![output("text", PortType::String)],
                runs: Arc::clone(&runs),
            }))
        }),
    )
    .with_selector(CapabilitySelector::new("Text", "managed"));
    registry
        .register_selector_capability(registration)
        .expect("register capability-effect test node");
    registry
}

pub(crate) fn local_read_workflow() -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![
            WorkflowNode {
                id: "source".to_owned(),
                type_id: "ManagedTextSource".to_owned(),
                contract_version: "1.0".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "upper".to_owned(),
                type_id: "UpperCase".to_owned(),
                contract_version: "1.0".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::from([(
                    "text".to_owned(),
                    InputBinding::single(OutputRef("source".to_owned(), "text".to_owned())),
                )]),
                position: None,
            },
        ],
    }
}

struct ManagedTextNode {
    available: Arc<AtomicBool>,
    outputs: Vec<OutputPort>,
    runs: Arc<AtomicUsize>,
}

impl Node for ManagedTextNode {
    fn type_id(&self) -> &str {
        "ManagedTextSource"
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
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        if !self.available.load(Ordering::SeqCst) {
            return Err(Box::new(TestNodeError("managed source is unavailable".to_owned())));
        }
        Ok(NodeRunResult::new(BTreeMap::from([(
            "text".to_owned(),
            Value::String("stable".to_owned()),
        )])))
    }
}
