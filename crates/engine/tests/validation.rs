use engine::{
    EngineError, Executor, InputPort, Node, NodeParams, NodeRegistry, NodeRunContext, NodeRunError,
    NodeRunResult, OutputPort, OutputRef, PortType, ResultCache, Value, ValueMap, Workflow,
    WorkflowNode,
};
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn rejects_unsupported_workflow_version() {
    let runs = Arc::new(AtomicUsize::new(0));
    let registry = registry([valid_definition("Source", Arc::clone(&runs))]);
    let workflow = Workflow {
        version: "2.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![workflow_node("source", "Source")],
    };

    let error = Executor::new(&registry)
        .execute(&workflow, &mut ResultCache::new())
        .expect_err("unsupported workflow version should fail");

    assert!(matches!(
        error,
        EngineError::UnsupportedWorkflowVersion { version } if version == "2.0"
    ));
    assert_eq!(runs.load(Ordering::SeqCst), 0);
}

#[test]
fn rejects_duplicate_node_ids() {
    let runs = Arc::new(AtomicUsize::new(0));
    let registry = registry([valid_definition("Source", Arc::clone(&runs))]);
    let workflow = Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![workflow_node("duplicate", "Source"), workflow_node("duplicate", "Source")],
    };

    let error = Executor::new(&registry)
        .execute(&workflow, &mut ResultCache::new())
        .expect_err("duplicate node ids should fail");

    assert!(matches!(
        error,
        EngineError::DuplicateNodeId { node_id } if node_id == "duplicate"
    ));
    assert_eq!(runs.load(Ordering::SeqCst), 0);
}

#[test]
fn rejects_wiring_to_undeclared_target_input() {
    let source_runs = Arc::new(AtomicUsize::new(0));
    let target_runs = Arc::new(AtomicUsize::new(0));
    let registry = registry([
        valid_definition("Source", Arc::clone(&source_runs)),
        valid_definition("Target", Arc::clone(&target_runs)),
    ]);
    let mut target = workflow_node("target", "Target");
    target.inputs.insert("typo".to_owned(), OutputRef("source".to_owned(), "text".to_owned()));
    let workflow = Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![workflow_node("source", "Source"), target],
    };

    let error = Executor::new(&registry)
        .execute(&workflow, &mut ResultCache::new())
        .expect_err("undeclared target input should fail");

    assert!(matches!(
        error,
        EngineError::UnknownTargetInput { node_id, input }
            if node_id == "target" && input == "typo"
    ));
    assert_eq!(source_runs.load(Ordering::SeqCst), 0);
    assert_eq!(target_runs.load(Ordering::SeqCst), 0);
}

#[test]
fn rejects_default_value_with_wrong_port_type() {
    let runs = Arc::new(AtomicUsize::new(0));
    let definition = NodeDefinition {
        type_id: "DefaultMismatch",
        inputs: vec![InputPort {
            name: "text".to_owned(),
            port_type: PortType::String,
            required: true,
            default: Some(Value::Image("asset://image".to_owned())),
        }],
        outputs: vec![],
        result: ValueMap::new(),
        runs: Arc::clone(&runs),
    };
    let registry = registry([definition]);
    let workflow = single_node_workflow("DefaultMismatch");

    let error = Executor::new(&registry)
        .execute(&workflow, &mut ResultCache::new())
        .expect_err("wrongly typed default should fail");

    assert!(matches!(
        error,
        EngineError::DefaultTypeMismatch {
            node_id,
            input,
            input_type: PortType::String,
            default_type: PortType::Image,
        } if node_id == "node" && input == "text"
    ));
    assert_eq!(runs.load(Ordering::SeqCst), 0);
}

#[test]
fn rejects_missing_declared_output_before_caching() {
    let definition = output_definition(vec![output("text", PortType::String)], ValueMap::new());

    assert_invalid_output_is_not_cached(definition, |error| {
        matches!(
            error,
            EngineError::MissingNodeOutput { node_id, output }
                if node_id == "node" && output == "text"
        )
    });
}

#[test]
fn rejects_undeclared_extra_output_before_caching() {
    let definition = output_definition(
        vec![],
        BTreeMap::from([("extra".to_owned(), Value::String("value".to_owned()))]),
    );

    assert_invalid_output_is_not_cached(definition, |error| {
        matches!(
            error,
            EngineError::UnexpectedNodeOutput { node_id, output }
                if node_id == "node" && output == "extra"
        )
    });
}

#[test]
fn rejects_wrongly_typed_output_before_caching() {
    let definition = output_definition(
        vec![output("text", PortType::String)],
        BTreeMap::from([("text".to_owned(), Value::Image("asset://image".to_owned()))]),
    );

    assert_invalid_output_is_not_cached(definition, |error| {
        matches!(
            error,
            EngineError::OutputTypeMismatch {
                node_id,
                output,
                output_type: PortType::String,
                actual_type: PortType::Image,
            } if node_id == "node" && output == "text"
        )
    });
}

fn assert_invalid_output_is_not_cached(
    definition: NodeDefinition,
    matches_expected_error: impl Fn(&EngineError) -> bool,
) {
    let runs = Arc::clone(&definition.runs);
    let registry = registry([definition]);
    let workflow = single_node_workflow("BrokenOutput");
    let executor = Executor::new(&registry);
    let mut cache = ResultCache::new();

    for _ in 0..2 {
        let error =
            executor.execute(&workflow, &mut cache).expect_err("invalid node output should fail");
        assert!(matches_expected_error(&error), "unexpected error: {error}");
    }
    assert_eq!(runs.load(Ordering::SeqCst), 2);
}

fn registry<const N: usize>(definitions: [NodeDefinition; N]) -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    for definition in definitions {
        registry.register(
            definition.type_id,
            Box::new(move |_| Ok(Box::new(TestNode::from_definition(&definition)))),
        );
    }
    registry
}

fn valid_definition(type_id: &'static str, runs: Arc<AtomicUsize>) -> NodeDefinition {
    NodeDefinition {
        type_id,
        inputs: vec![],
        outputs: vec![output("text", PortType::String)],
        result: BTreeMap::from([("text".to_owned(), Value::String("value".to_owned()))]),
        runs,
    }
}

fn output_definition(outputs: Vec<OutputPort>, result: ValueMap) -> NodeDefinition {
    NodeDefinition {
        type_id: "BrokenOutput",
        inputs: vec![],
        outputs,
        result,
        runs: Arc::new(AtomicUsize::new(0)),
    }
}

fn single_node_workflow(type_id: &str) -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![workflow_node("node", type_id)],
    }
}

fn workflow_node(id: &str, type_id: &str) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        type_id: type_id.to_owned(),
        params: NodeParams::new(),
        inputs: BTreeMap::new(),
        position: None,
    }
}

fn output(name: &str, port_type: PortType) -> OutputPort {
    OutputPort { name: name.to_owned(), port_type }
}

struct NodeDefinition {
    type_id: &'static str,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    result: ValueMap,
    runs: Arc<AtomicUsize>,
}

struct TestNode {
    type_id: &'static str,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    result: ValueMap,
    runs: Arc<AtomicUsize>,
}

impl TestNode {
    fn from_definition(definition: &NodeDefinition) -> Self {
        Self {
            type_id: definition.type_id,
            inputs: definition.inputs.clone(),
            outputs: definition.outputs.clone(),
            result: definition.result.clone(),
            runs: Arc::clone(&definition.runs),
        }
    }
}

impl Node for TestNode {
    fn type_id(&self) -> &str {
        self.type_id
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn run(
        &self,
        _inputs: &ValueMap,
        _context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, NodeRunError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(NodeRunResult::new(self.result.clone()))
    }
}
