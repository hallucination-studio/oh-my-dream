mod cancellation;

pub(crate) use cancellation::{
    TestCancellation, commit_then_cancel_registry, fail_then_cancel_registry, single_node_workflow,
};

use engine::{
    InputPort, Node, NodeExecutionState, NodeParams, NodeProgressEvent, NodeRegistry,
    NodeRunContext, NodeRunResult, OutputPort, OutputRef, PortType, Value, ValueMap, Workflow,
    WorkflowNode,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Default)]
pub(crate) struct RunCounters {
    pub(crate) text_prompt: Arc<AtomicUsize>,
    pub(crate) upper_case: Arc<AtomicUsize>,
    pub(crate) collect: Arc<AtomicUsize>,
}

pub(crate) fn registry(counters: RunCounters) -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    let text_prompt_runs = Arc::clone(&counters.text_prompt);
    registry.register(
        "TextPrompt",
        Box::new(move |params| {
            let text = params
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            Ok(Box::new(TextPromptNode {
                text,
                outputs: vec![output("text", PortType::String)],
                runs: Arc::clone(&text_prompt_runs),
            }))
        }),
    );

    let upper_case_runs = Arc::clone(&counters.upper_case);
    registry.register(
        "UpperCase",
        Box::new(move |_| {
            Ok(Box::new(UpperCaseNode {
                inputs: vec![required_input("text", PortType::String)],
                outputs: vec![output("text", PortType::String)],
                runs: Arc::clone(&upper_case_runs),
            }))
        }),
    );

    let collect_runs = Arc::clone(&counters.collect);
    registry.register(
        "Collect",
        Box::new(move |_| {
            Ok(Box::new(CollectNode {
                inputs: vec![required_input("text", PortType::String)],
                outputs: vec![output("text", PortType::String)],
                runs: Arc::clone(&collect_runs),
            }))
        }),
    );

    registry.register(
        "ImageSource",
        Box::new(|_| {
            Ok(Box::new(ImageSourceNode { outputs: vec![output("image", PortType::Image)] }))
        }),
    );
    registry
}

fn required_input(name: &str, port_type: PortType) -> InputPort {
    InputPort { name: name.to_owned(), port_type, required: true, default: None }
}

fn output(name: &str, port_type: PortType) -> OutputPort {
    OutputPort { name: name.to_owned(), port_type }
}

pub(crate) fn linear_workflow(text: &str) -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![
            WorkflowNode {
                id: "prompt".to_owned(),
                type_id: "TextPrompt".to_owned(),
                params: BTreeMap::from([(
                    "text".to_owned(),
                    serde_json::Value::String(text.to_owned()),
                )])
                .into_iter()
                .collect(),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "upper".to_owned(),
                type_id: "UpperCase".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::from([(
                    "text".to_owned(),
                    OutputRef("prompt".to_owned(), "text".to_owned()),
                )]),
                position: None,
            },
            WorkflowNode {
                id: "collect".to_owned(),
                type_id: "Collect".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::from([(
                    "text".to_owned(),
                    OutputRef("upper".to_owned(), "text".to_owned()),
                )]),
                position: None,
            },
        ],
    }
}

struct TextPromptNode {
    text: String,
    outputs: Vec<OutputPort>,
    runs: Arc<AtomicUsize>,
}

impl Node for TextPromptNode {
    fn type_id(&self) -> &str {
        "TextPrompt"
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
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(NodeRunResult {
            outputs: BTreeMap::from([("text".to_owned(), Value::String(self.text.clone()))]),
            cost: Some(7),
        })
    }
}

struct UpperCaseNode {
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    runs: Arc<AtomicUsize>,
}

impl Node for UpperCaseNode {
    fn type_id(&self) -> &str {
        "UpperCase"
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
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let Value::String(text) =
            inputs.get("text").ok_or_else(|| TestNodeError("missing text input".to_owned()))?
        else {
            return Err(Box::new(TestNodeError("text input was not a string".to_owned())));
        };
        Ok(NodeRunResult {
            outputs: BTreeMap::from([("text".to_owned(), Value::String(text.to_uppercase()))]),
            cost: Some(7),
        })
    }
}

struct CollectNode {
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    runs: Arc<AtomicUsize>,
}

impl Node for CollectNode {
    fn type_id(&self) -> &str {
        "Collect"
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
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let text = inputs
            .get("text")
            .ok_or_else(|| TestNodeError("missing text input".to_owned()))?
            .clone();
        Ok(NodeRunResult { outputs: BTreeMap::from([("text".to_owned(), text)]), cost: Some(7) })
    }
}

struct ImageSourceNode {
    outputs: Vec<OutputPort>,
}

impl Node for ImageSourceNode {
    fn type_id(&self) -> &str {
        "ImageSource"
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
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        Ok(NodeRunResult {
            outputs: BTreeMap::from([(
                "image".to_owned(),
                Value::Image("asset://image".to_owned()),
            )]),
            cost: None,
        })
    }
}

pub(crate) struct FailingNode;

impl Node for FailingNode {
    fn type_id(&self) -> &str {
        "Failing"
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        &[]
    }

    fn run(
        &self,
        _inputs: &ValueMap,
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        Err(Box::new(TestNodeError("boom".to_owned())))
    }
}

pub(crate) fn event_summary(
    events: &[NodeProgressEvent],
) -> Vec<(String, NodeExecutionState, Option<f32>, Option<i64>)> {
    events
        .iter()
        .map(|event| (event.node_id.clone(), event.state, event.progress, event.cost))
        .collect()
}

#[derive(Debug)]
struct TestNodeError(String);

impl fmt::Display for TestNodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for TestNodeError {}
