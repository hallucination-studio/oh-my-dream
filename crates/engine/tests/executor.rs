use engine::{
    EngineError, Executor, InputPort, Node, NodeExecutionState, NodeParams, NodeProgressEvent,
    NodeRegistry, NodeRunContext, NodeRunResult, OutputPort, OutputRef, PortType, ResultCache,
    Value, ValueMap, Workflow, WorkflowNode,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn executes_text_prompt_uppercase_collect_workflow() {
    let counters = RunCounters::default();
    let registry = registry(counters);
    let workflow = linear_workflow("hello");
    let mut cache = ResultCache::new();

    let outputs =
        Executor::new(&registry).execute(&workflow, &mut cache).expect("workflow should execute");

    assert_eq!(
        outputs.get("collect").and_then(|values| values.get("text")),
        Some(&Value::String("HELLO".to_owned()))
    );
}

#[test]
fn reuses_cached_node_outputs_on_second_run() {
    let counters = RunCounters::default();
    let registry = registry(counters.clone());
    let workflow = linear_workflow("hello");
    let mut cache = ResultCache::new();
    let executor = Executor::new(&registry);

    executor.execute(&workflow, &mut cache).expect("first run should execute");
    executor.execute(&workflow, &mut cache).expect("second run should execute");

    assert_eq!(counters.text_prompt.load(Ordering::SeqCst), 1);
    assert_eq!(counters.upper_case.load(Ordering::SeqCst), 1);
    assert_eq!(counters.collect.load(Ordering::SeqCst), 1);
}

#[test]
fn emits_running_and_done_events_with_node_costs() {
    let counters = RunCounters::default();
    let registry = registry(counters);
    let workflow = linear_workflow("hello");
    let mut cache = ResultCache::new();
    let mut events = Vec::new();

    Executor::new(&registry)
        .execute_with_observer(&workflow, &mut cache, &mut |event| events.push(event.clone()))
        .expect("workflow should execute");

    assert_eq!(
        event_summary(&events),
        vec![
            ("prompt".to_owned(), NodeExecutionState::Running, Some(0.0), None),
            ("prompt".to_owned(), NodeExecutionState::Done, Some(1.0), Some(7)),
            ("upper".to_owned(), NodeExecutionState::Running, Some(0.0), None),
            ("upper".to_owned(), NodeExecutionState::Done, Some(1.0), Some(7)),
            ("collect".to_owned(), NodeExecutionState::Running, Some(0.0), None),
            ("collect".to_owned(), NodeExecutionState::Done, Some(1.0), Some(7)),
        ]
    );
}

#[test]
fn emits_cached_events_with_cached_cost_without_rerunning_nodes() {
    let counters = RunCounters::default();
    let registry = registry(counters.clone());
    let workflow = linear_workflow("hello");
    let mut cache = ResultCache::new();
    let executor = Executor::new(&registry);

    executor.execute(&workflow, &mut cache).expect("first run should execute");
    let mut events = Vec::new();
    executor
        .execute_with_observer(&workflow, &mut cache, &mut |event| events.push(event.clone()))
        .expect("second run should use cache");

    assert_eq!(counters.text_prompt.load(Ordering::SeqCst), 1);
    assert_eq!(counters.upper_case.load(Ordering::SeqCst), 1);
    assert_eq!(counters.collect.load(Ordering::SeqCst), 1);
    assert_eq!(
        event_summary(&events),
        vec![
            ("prompt".to_owned(), NodeExecutionState::Cached, Some(1.0), Some(7)),
            ("upper".to_owned(), NodeExecutionState::Cached, Some(1.0), Some(7)),
            ("collect".to_owned(), NodeExecutionState::Cached, Some(1.0), Some(7)),
        ]
    );
}

#[test]
fn does_not_reuse_cached_outputs_across_projects() {
    let counters = RunCounters::default();
    let registry = registry(counters.clone());
    let mut first = linear_workflow("hello");
    first.project_id = "project-a".to_owned();
    let mut second = linear_workflow("hello");
    second.project_id = "project-b".to_owned();
    let mut cache = ResultCache::new();
    let executor = Executor::new(&registry);

    executor.execute(&first, &mut cache).expect("first project should execute");
    executor.execute(&second, &mut cache).expect("second project should execute");

    assert_eq!(counters.text_prompt.load(Ordering::SeqCst), 2);
    assert_eq!(counters.upper_case.load(Ordering::SeqCst), 2);
    assert_eq!(counters.collect.load(Ordering::SeqCst), 2);
}

#[test]
fn emits_error_event_before_returning_node_failure() {
    let mut registry = NodeRegistry::new();
    registry.register("Failing", Box::new(|_| Ok(Box::new(FailingNode))));
    let workflow = Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![WorkflowNode {
            id: "fail".to_owned(),
            type_id: "Failing".to_owned(),
            params: NodeParams::new(),
            inputs: BTreeMap::new(),
            position: None,
        }],
    };
    let mut cache = ResultCache::new();
    let mut events = Vec::new();

    let error = Executor::new(&registry)
        .execute_with_observer(&workflow, &mut cache, &mut |event| events.push(event.clone()))
        .expect_err("node should fail");

    assert!(matches!(error, EngineError::NodeExecution { .. }));
    assert_eq!(
        event_summary(&events),
        vec![
            ("fail".to_owned(), NodeExecutionState::Running, Some(0.0), None),
            ("fail".to_owned(), NodeExecutionState::Error, None, None),
        ]
    );
}

#[test]
fn rejects_cycles_before_execution() {
    let counters = RunCounters::default();
    let registry = registry(counters);
    let mut nodes = linear_workflow("hello").nodes;
    nodes[0]
        .inputs
        .insert("ignored".to_owned(), OutputRef("collect".to_owned(), "text".to_owned()));
    let workflow = Workflow { version: "1.0".to_owned(), project_id: "default".to_owned(), nodes };

    let error = Executor::new(&registry)
        .execute(&workflow, &mut ResultCache::new())
        .expect_err("cycle should fail");

    assert!(matches!(error, EngineError::Cycle { .. }));
}

#[test]
fn rejects_type_mismatches_while_building_plan() {
    let counters = RunCounters::default();
    let registry = registry(counters);
    let workflow = Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![
            WorkflowNode {
                id: "image".to_owned(),
                type_id: "ImageSource".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::new(),
                position: None,
            },
            WorkflowNode {
                id: "upper".to_owned(),
                type_id: "UpperCase".to_owned(),
                params: NodeParams::new(),
                inputs: BTreeMap::from([(
                    "text".to_owned(),
                    OutputRef("image".to_owned(), "image".to_owned()),
                )]),
                position: None,
            },
        ],
    };

    let error = Executor::new(&registry)
        .execute(&workflow, &mut ResultCache::new())
        .expect_err("type mismatch should fail");

    assert!(matches!(
        error,
        EngineError::TypeMismatch {
            input_type: PortType::String,
            source_type: PortType::Image,
            ..
        }
    ));
}

#[derive(Clone, Default)]
struct RunCounters {
    text_prompt: Arc<AtomicUsize>,
    upper_case: Arc<AtomicUsize>,
    collect: Arc<AtomicUsize>,
}

fn registry(counters: RunCounters) -> NodeRegistry {
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

fn linear_workflow(text: &str) -> Workflow {
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

struct FailingNode;

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

fn event_summary(
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
