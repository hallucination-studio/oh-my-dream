mod executor_support;

use engine::{
    EngineError, Executor, NodeExecutionState, NodeParams, NodeRegistry, OutputRef, PortType,
    ResultCache, Value, Workflow, WorkflowNode,
};
use executor_support::{FailingNode, RunCounters, event_summary, linear_workflow, registry};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

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
    nodes[1].inputs.insert("text".to_owned(), OutputRef("collect".to_owned(), "text".to_owned()));
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
