mod executor_support;

use engine::{
    CapabilityEffect, EngineError, InputBinding, NodeExecutionState, NodeParams, NodeRegistry,
    OutputRef, PortType, ResultCache, Workflow, WorkflowGraphExecutor, WorkflowNode,
    WorkflowNodeValue,
};
use executor_support::{
    FailingNodeImpl, RunCounters, TestCancellationImpl, capability_effect_registry,
    commit_then_cancel_registry, event_summary, fail_then_cancel_registry, linear_workflow,
    local_read_workflow, ordered_video_workflow, registry, single_node_workflow,
};
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[test]
fn executes_text_prompt_uppercase_collect_workflow() {
    let counters = RunCounters::default();
    let registry = registry(counters);
    let workflow = linear_workflow("hello");
    let mut cache = ResultCache::new();

    let outputs = WorkflowGraphExecutor::new(&registry)
        .execute(&workflow, &mut cache)
        .expect("workflow should execute");

    assert_eq!(
        outputs.get("collect").and_then(|values| values.get("text")),
        Some(&WorkflowNodeValue::String("HELLO".to_owned()))
    );
}

#[test]
fn reuses_cached_node_outputs_on_second_run() {
    let counters = RunCounters::default();
    let registry = registry(counters.clone());
    let workflow = linear_workflow("hello");
    let mut cache = ResultCache::new();
    let executor = WorkflowGraphExecutor::new(&registry);

    executor.execute(&workflow, &mut cache).expect("first run should execute");
    executor.execute(&workflow, &mut cache).expect("second run should execute");

    assert_eq!(counters.text_prompt.load(Ordering::SeqCst), 1);
    assert_eq!(counters.upper_case.load(Ordering::SeqCst), 1);
    assert_eq!(counters.collect.load(Ordering::SeqCst), 1);
}

#[test]
fn local_read_reruns_while_unchanged_downstream_output_is_cached() {
    let counters = RunCounters::default();
    let registry = capability_effect_registry(
        counters.clone(),
        Arc::new(AtomicBool::new(true)),
        CapabilityEffect::LocalRead,
    );
    let workflow = local_read_workflow();
    let mut cache = ResultCache::new();
    let executor = WorkflowGraphExecutor::new(&registry);

    executor.execute(&workflow, &mut cache).expect("first run should execute");
    executor.execute(&workflow, &mut cache).expect("second run should resolve local state");

    assert_eq!(counters.text_prompt.load(Ordering::SeqCst), 2);
    assert_eq!(counters.upper_case.load(Ordering::SeqCst), 1);
}

#[test]
fn local_read_observes_unavailability_after_a_successful_run() {
    let counters = RunCounters::default();
    let available = Arc::new(AtomicBool::new(true));
    let registry =
        capability_effect_registry(counters, Arc::clone(&available), CapabilityEffect::LocalRead);
    let workflow = local_read_workflow();
    let mut cache = ResultCache::new();
    let executor = WorkflowGraphExecutor::new(&registry);

    executor.execute(&workflow, &mut cache).expect("available source should execute");
    available.store(false, Ordering::SeqCst);
    let error = executor
        .execute(&workflow, &mut cache)
        .expect_err("unavailable source must not be hidden by cache");

    assert!(matches!(error, EngineError::NodeExecution { node_id, .. } if node_id == "source"));
}

#[test]
fn pure_and_external_capabilities_retain_cache_reuse() {
    for effect in [CapabilityEffect::Pure, CapabilityEffect::External] {
        let counters = RunCounters::default();
        let registry =
            capability_effect_registry(counters.clone(), Arc::new(AtomicBool::new(true)), effect);
        let mut cache = ResultCache::new();
        let executor = WorkflowGraphExecutor::new(&registry);

        executor.execute(&local_read_workflow(), &mut cache).expect("first run should execute");
        executor.execute(&local_read_workflow(), &mut cache).expect("second run should use cache");

        assert_eq!(counters.text_prompt.load(Ordering::SeqCst), 1, "effect {effect:?}");
    }
}

#[test]
fn reordered_many_input_reruns_only_the_affected_downstream_node() {
    let counters = RunCounters::default();
    let registry = registry(counters.clone());
    let mut cache = ResultCache::new();
    let executor = WorkflowGraphExecutor::new(&registry);

    executor
        .execute(&ordered_video_workflow(false), &mut cache)
        .expect("first order should execute");
    executor
        .execute(&ordered_video_workflow(false), &mut cache)
        .expect("unchanged order should use cache");
    executor
        .execute(&ordered_video_workflow(true), &mut cache)
        .expect("reversed order should execute");

    assert_eq!(counters.video_source.load(Ordering::SeqCst), 2);
    assert_eq!(counters.video_concat.load(Ordering::SeqCst), 2);
}

#[test]
fn emits_running_and_done_events_with_node_costs() {
    let counters = RunCounters::default();
    let registry = registry(counters);
    let workflow = linear_workflow("hello");
    let mut cache = ResultCache::new();
    let mut events = Vec::new();

    WorkflowGraphExecutor::new(&registry)
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
    let executor = WorkflowGraphExecutor::new(&registry);

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
    let executor = WorkflowGraphExecutor::new(&registry);

    executor.execute(&first, &mut cache).expect("first project should execute");
    executor.execute(&second, &mut cache).expect("second project should execute");

    assert_eq!(counters.text_prompt.load(Ordering::SeqCst), 2);
    assert_eq!(counters.upper_case.load(Ordering::SeqCst), 2);
    assert_eq!(counters.collect.load(Ordering::SeqCst), 2);
}

#[test]
fn preserves_each_project_cache_namespace() {
    let counters = RunCounters::default();
    let registry = registry(counters.clone());
    let mut first = linear_workflow("hello");
    first.project_id = "project-a".to_owned();
    let mut second = linear_workflow("hello");
    second.project_id = "project-b".to_owned();
    let mut cache = ResultCache::new();
    let executor = WorkflowGraphExecutor::new(&registry);

    executor.execute(&first, &mut cache).expect("first project should execute");
    executor.execute(&second, &mut cache).expect("second project should execute");
    executor.execute(&first, &mut cache).expect("first project should remain cached");

    assert_eq!(counters.text_prompt.load(Ordering::SeqCst), 2);
    assert_eq!(counters.upper_case.load(Ordering::SeqCst), 2);
    assert_eq!(counters.collect.load(Ordering::SeqCst), 2);
}

#[test]
fn cancellation_stops_before_the_next_node() {
    let counters = RunCounters::default();
    let registry = registry(counters.clone());
    let workflow = linear_workflow("hello");
    let mut cache = ResultCache::new();
    let cancellation = TestCancellationImpl::default();

    let error = WorkflowGraphExecutor::new(&registry)
        .execute_interruptible(&workflow, &mut cache, &cancellation, &mut |event| {
            if event.node_id == "prompt" && event.state == NodeExecutionState::Done {
                cancellation.cancel();
            }
        })
        .expect_err("execution should observe cancellation");

    assert!(matches!(error, EngineError::Cancelled));
    assert_eq!(counters.text_prompt.load(Ordering::SeqCst), 1);
    assert_eq!(counters.upper_case.load(Ordering::SeqCst), 0);
    assert_eq!(counters.collect.load(Ordering::SeqCst), 0);
}

#[test]
fn successful_final_node_commit_wins_over_late_cancellation() {
    let cancellation = Arc::new(TestCancellationImpl::default());
    let runs = Arc::new(AtomicUsize::new(0));
    let registry = commit_then_cancel_registry(Arc::clone(&cancellation), Arc::clone(&runs));
    let workflow = single_node_workflow("CommitThenCancel");
    let mut cache = ResultCache::new();
    let mut events = Vec::new();

    let outputs = WorkflowGraphExecutor::new(&registry)
        .execute_interruptible(&workflow, &mut cache, cancellation.as_ref(), &mut |event| {
            events.push(event.clone());
        })
        .expect("successful node return should commit the final result");
    WorkflowGraphExecutor::new(&registry)
        .execute(&workflow, &mut cache)
        .expect("cache should be reusable");

    assert_eq!(outputs["commit"]["text"], WorkflowNodeValue::String("committed".to_owned()));
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(
        event_summary(&events),
        vec![
            ("commit".to_owned(), NodeExecutionState::Running, Some(0.0), None),
            ("commit".to_owned(), NodeExecutionState::Done, Some(1.0), None),
        ]
    );
}

#[test]
fn node_failure_wins_over_concurrent_cancellation() {
    let cancellation = Arc::new(TestCancellationImpl::default());
    let registry = fail_then_cancel_registry(Arc::clone(&cancellation));
    let mut events = Vec::new();

    let error = WorkflowGraphExecutor::new(&registry)
        .execute_interruptible(
            &single_node_workflow("FailThenCancel"),
            &mut ResultCache::new(),
            cancellation.as_ref(),
            &mut |event| events.push(event.clone()),
        )
        .expect_err("node failure should remain actionable");

    assert!(matches!(error, EngineError::NodeExecution { .. }));
    assert_eq!(
        event_summary(&events),
        vec![
            ("commit".to_owned(), NodeExecutionState::Running, Some(0.0), None),
            ("commit".to_owned(), NodeExecutionState::Error, None, None),
        ]
    );
}

#[test]
fn emits_error_event_before_returning_node_failure() {
    let mut registry = NodeRegistry::new();
    registry.register("Failing", Box::new(|_| Ok(Box::new(FailingNodeImpl))));
    let workflow = Workflow {
        version: "1.0".to_owned(),
        project_id: "default".to_owned(),
        nodes: vec![WorkflowNode {
            id: "fail".to_owned(),
            type_id: "Failing".to_owned(),
            contract_version: "1.0".to_owned(),
            params: NodeParams::new(),
            inputs: BTreeMap::new(),
            position: None,
        }],
    };
    let mut cache = ResultCache::new();
    let mut events = Vec::new();

    let error = WorkflowGraphExecutor::new(&registry)
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
    nodes[1].inputs.insert(
        "text".to_owned(),
        InputBinding::single(OutputRef("collect".to_owned(), "text".to_owned())),
    );
    let workflow = Workflow { version: "1.0".to_owned(), project_id: "default".to_owned(), nodes };

    let error = WorkflowGraphExecutor::new(&registry)
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
                    InputBinding::single(OutputRef("image".to_owned(), "image".to_owned())),
                )]),
                position: None,
            },
        ],
    };

    let error = WorkflowGraphExecutor::new(&registry)
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
