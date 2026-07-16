mod cancellation;
mod capability_effect;

pub(crate) use cancellation::{
    TestCancellationImpl, commit_then_cancel_registry, fail_then_cancel_registry,
    single_node_workflow,
};
pub(crate) use capability_effect::{capability_effect_registry, local_read_workflow};

use engine::{
    InputBinding, InputPort, InputValue, NodeExecutionState, NodeInputs, NodeInterface, NodeParams,
    NodeProgressEvent, NodeRegistry, NodeRunContextImpl, NodeRunResult, OutputPort, OutputRef,
    PortCardinality, PortType, Value, Workflow, WorkflowNode,
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
    pub(crate) video_source: Arc<AtomicUsize>,
    pub(crate) video_concat: Arc<AtomicUsize>,
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
            Ok(Box::new(TextPromptNodeImpl {
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
            Ok(Box::new(UpperCaseNodeImpl {
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
            Ok(Box::new(CollectNodeImpl {
                inputs: vec![required_input("text", PortType::String)],
                outputs: vec![output("text", PortType::String)],
                runs: Arc::clone(&collect_runs),
            }))
        }),
    );

    registry.register(
        "ImageSource",
        Box::new(|_| {
            Ok(Box::new(ImageSourceNodeImpl { outputs: vec![output("image", PortType::Image)] }))
        }),
    );
    let video_source_runs = Arc::clone(&counters.video_source);
    registry.register(
        "VideoSource",
        Box::new(move |params| {
            let reference = params
                .get("reference")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            Ok(Box::new(VideoSourceNodeImpl {
                reference,
                runs: Arc::clone(&video_source_runs),
                outputs: vec![output("video", PortType::Video)],
            }))
        }),
    );
    let video_concat_runs = Arc::clone(&counters.video_concat);
    registry.register(
        "VideoConcat",
        Box::new(move |_| {
            Ok(Box::new(VideoConcatNodeImpl {
                runs: Arc::clone(&video_concat_runs),
                inputs: vec![required_many_input("clips", PortType::Video)],
                outputs: vec![output("video", PortType::Video)],
            }))
        }),
    );
    registry
}

fn required_input(name: &str, port_type: PortType) -> InputPort {
    InputPort {
        name: name.to_owned(),
        port_type,
        cardinality: PortCardinality::One,
        required: true,
        default: None,
    }
}

fn required_many_input(name: &str, port_type: PortType) -> InputPort {
    InputPort {
        name: name.to_owned(),
        port_type,
        cardinality: PortCardinality::Many { minimum: 2, maximum: None },
        required: true,
        default: None,
    }
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
                contract_version: "1.0".to_owned(),
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
                contract_version: "1.0".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::from([(
                    "text".to_owned(),
                    InputBinding::single(OutputRef("prompt".to_owned(), "text".to_owned())),
                )]),
                position: None,
            },
            WorkflowNode {
                id: "collect".to_owned(),
                type_id: "Collect".to_owned(),
                contract_version: "1.0".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::from([(
                    "text".to_owned(),
                    InputBinding::single(OutputRef("upper".to_owned(), "text".to_owned())),
                )]),
                position: None,
            },
        ],
    }
}

pub(crate) fn ordered_video_workflow(reversed: bool) -> Workflow {
    let sources = if reversed {
        vec![
            OutputRef("second".to_owned(), "video".to_owned()),
            OutputRef("first".to_owned(), "video".to_owned()),
        ]
    } else {
        vec![
            OutputRef("first".to_owned(), "video".to_owned()),
            OutputRef("second".to_owned(), "video".to_owned()),
        ]
    };
    Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![
            video_source_workflow_node("first", "asset-a"),
            video_source_workflow_node("second", "asset-b"),
            WorkflowNode {
                id: "concat".to_owned(),
                type_id: "VideoConcat".to_owned(),
                contract_version: "1.0".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::from([("clips".to_owned(), InputBinding::ordered_many(sources))]),
                position: None,
            },
        ],
    }
}

fn video_source_workflow_node(id: &str, reference: &str) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        type_id: "VideoSource".to_owned(),
        contract_version: "1.0".to_owned(),
        params: NodeParams::from_iter([(
            "reference".to_owned(),
            serde_json::Value::String(reference.to_owned()),
        )]),
        inputs: BTreeMap::new(),
        position: None,
    }
}

struct TextPromptNodeImpl {
    text: String,
    outputs: Vec<OutputPort>,
    runs: Arc<AtomicUsize>,
}

impl NodeInterface for TextPromptNodeImpl {
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
        _inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(NodeRunResult {
            outputs: BTreeMap::from([("text".to_owned(), Value::String(self.text.clone()))]),
            cost: Some(7),
        })
    }
}

struct UpperCaseNodeImpl {
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    runs: Arc<AtomicUsize>,
}

impl NodeInterface for UpperCaseNodeImpl {
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
        inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let InputValue::Single(Value::String(text)) =
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

struct CollectNodeImpl {
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    runs: Arc<AtomicUsize>,
}

impl NodeInterface for CollectNodeImpl {
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
        inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let text = inputs
            .get("text")
            .ok_or_else(|| TestNodeError("missing text input".to_owned()))?
            .clone();
        let InputValue::Single(text) = text else {
            return Err(Box::new(TestNodeError("text input was not scalar".to_owned())));
        };
        Ok(NodeRunResult { outputs: BTreeMap::from([("text".to_owned(), text)]), cost: Some(7) })
    }
}

struct ImageSourceNodeImpl {
    outputs: Vec<OutputPort>,
}

struct VideoSourceNodeImpl {
    reference: String,
    runs: Arc<AtomicUsize>,
    outputs: Vec<OutputPort>,
}

impl NodeInterface for VideoSourceNodeImpl {
    fn type_id(&self) -> &str {
        "VideoSource"
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
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(NodeRunResult::new(BTreeMap::from([(
            "video".to_owned(),
            Value::Video(self.reference.clone()),
        )])))
    }
}

struct VideoConcatNodeImpl {
    runs: Arc<AtomicUsize>,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl NodeInterface for VideoConcatNodeImpl {
    fn type_id(&self) -> &str {
        "VideoConcat"
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn run(
        &self,
        inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
    ) -> Result<NodeRunResult, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let InputValue::OrderedMany(clips) = &inputs["clips"] else {
            return Err(Box::new(TestNodeError("clips input was not ordered".to_owned())));
        };
        Ok(NodeRunResult::new(BTreeMap::from([(
            "video".to_owned(),
            Value::Video(format!("{clips:?}")),
        )])))
    }
}

impl NodeInterface for ImageSourceNodeImpl {
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
        _inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
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

pub(crate) struct FailingNodeImpl;

impl NodeInterface for FailingNodeImpl {
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
        _inputs: &NodeInputs,
        _context: &mut NodeRunContextImpl,
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
