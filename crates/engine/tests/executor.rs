use engine::{
    EngineError, Executor, InputPort, Node, NodeParams, NodeRegistry, OutputPort, OutputRef,
    PortType, ResultCache, Value, ValueMap, Workflow, WorkflowNode,
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
fn rejects_cycles_before_execution() {
    let counters = RunCounters::default();
    let registry = registry(counters);
    let mut nodes = linear_workflow("hello").nodes;
    nodes[0]
        .inputs
        .insert("ignored".to_owned(), OutputRef("collect".to_owned(), "text".to_owned()));
    let workflow = Workflow { version: "1.0".to_owned(), nodes };

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

    fn run(&self, _inputs: &ValueMap) -> Result<ValueMap, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(BTreeMap::from([("text".to_owned(), Value::String(self.text.clone()))]))
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

    fn run(&self, inputs: &ValueMap) -> Result<ValueMap, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let Value::String(text) =
            inputs.get("text").ok_or_else(|| TestNodeError("missing text input".to_owned()))?
        else {
            return Err(Box::new(TestNodeError("text input was not a string".to_owned())));
        };
        Ok(BTreeMap::from([("text".to_owned(), Value::String(text.to_uppercase()))]))
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

    fn run(&self, inputs: &ValueMap) -> Result<ValueMap, Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let text = inputs
            .get("text")
            .ok_or_else(|| TestNodeError("missing text input".to_owned()))?
            .clone();
        Ok(BTreeMap::from([("text".to_owned(), text)]))
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

    fn run(&self, _inputs: &ValueMap) -> Result<ValueMap, Box<dyn Error + Send + Sync>> {
        Ok(BTreeMap::from([("image".to_owned(), Value::Image("asset://image".to_owned()))]))
    }
}

#[derive(Debug)]
struct TestNodeError(String);

impl fmt::Display for TestNodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for TestNodeError {}
