//! Workflow executor and deterministic result cache.

use crate::cache::{ResultCache, cache_fingerprint};
use crate::error::{EngineError, Result};
use crate::graph::Workflow;
use crate::node::{NodeRunContext, NodeRunResult};
use crate::registry::NodeRegistry;
use crate::validation::{ExecutionPlan, PlanNode, build_plan, validate_node_outputs};
use crate::value::ValueMap;
use std::collections::{BTreeMap, BTreeSet};
use tracing::{debug, info};

/// Outputs produced by a single run, keyed by node id.
pub type RunOutputs = BTreeMap<String, ValueMap>;

/// Execution state for one workflow node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeExecutionState {
    /// Node is not currently running.
    Idle,
    /// Node is running.
    Running,
    /// Node finished successfully.
    Done,
    /// Node outputs were reused from cache.
    Cached,
    /// Node failed.
    Error,
}

/// Synchronous node execution event emitted by the executor.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeProgressEvent {
    /// Workflow node id.
    pub node_id: String,
    /// Current execution state.
    pub state: NodeExecutionState,
    /// Best-effort progress in `[0.0, 1.0]`.
    pub progress: Option<f32>,
    /// Estimated cost in micro-USD.
    pub cost: Option<i64>,
}

/// Caller-owned signal consulted while a workflow is executing.
pub trait CancellationSignal: Send + Sync {
    /// Returns whether the current workflow run should stop.
    fn is_cancelled(&self) -> bool;
}

/// Builds an execution plan from a [`Workflow`], validating wiring and ordering
/// nodes topologically, then runs nodes in order consulting the [`ResultCache`].
pub struct Executor<'r> {
    registry: &'r NodeRegistry,
}

impl<'r> Executor<'r> {
    /// Creates an executor bound to a node registry.
    #[must_use]
    pub fn new(registry: &'r NodeRegistry) -> Self {
        Self { registry }
    }

    /// Validates and executes `workflow`, returning each node's outputs.
    pub fn execute(&self, workflow: &Workflow, cache: &mut ResultCache) -> Result<RunOutputs> {
        let mut observer = |_event: &NodeProgressEvent| {};
        self.execute_interruptible(workflow, cache, &NeverCancelled, &mut observer)
    }

    /// Validates and executes `workflow`, synchronously reporting node events.
    pub fn execute_with_observer(
        &self,
        workflow: &Workflow,
        cache: &mut ResultCache,
        observer: &mut impl FnMut(&NodeProgressEvent),
    ) -> Result<RunOutputs> {
        self.execute_interruptible(workflow, cache, &NeverCancelled, observer)
    }

    /// Executes `workflow` while consulting a caller-owned cancellation signal.
    pub fn execute_interruptible(
        &self,
        workflow: &Workflow,
        cache: &mut ResultCache,
        cancellation: &dyn CancellationSignal,
        observer: &mut impl FnMut(&NodeProgressEvent),
    ) -> Result<RunOutputs> {
        ensure_not_cancelled(cancellation)?;
        let plan = build_plan(self.registry, workflow)?;
        let order = topological_order(&plan)?;
        let mut outputs = RunOutputs::new();
        let workflow_snapshot =
            serde_json::to_value(workflow).map_err(|source| EngineError::InvalidWorkflow {
                message: format!("serialize workflow snapshot: {source}"),
            })?;

        for index in order {
            ensure_not_cancelled(cancellation)?;
            execute_plan_node(
                &plan.nodes[index],
                &workflow.project_id,
                &workflow_snapshot,
                cache,
                &mut outputs,
                cancellation,
                observer,
            )?;
            ensure_not_cancelled(cancellation)?;
        }

        Ok(outputs)
    }
}

fn execute_plan_node(
    node: &PlanNode,
    project_id: &str,
    workflow_snapshot: &serde_json::Value,
    cache: &mut ResultCache,
    outputs: &mut RunOutputs,
    cancellation: &dyn CancellationSignal,
    observer: &mut dyn FnMut(&NodeProgressEvent),
) -> Result<()> {
    let inputs = resolve_inputs(node, outputs)?;
    let fingerprint = cache_fingerprint(project_id, &node.type_id, &node.params, &inputs);
    if reuse_cached_output(node, project_id, fingerprint, cache, outputs, observer) {
        return Ok(());
    }

    emit_node_event(observer, node, NodeExecutionState::Running, Some(0.0), None);
    info!(node_id = %node.id, type_id = %node.type_id, "executing node");
    let result =
        run_plan_node(node, project_id, workflow_snapshot, &inputs, cancellation, observer)?;
    ensure_not_cancelled(cancellation)?;
    info!(node_id = %node.id, type_id = %node.type_id, "node execution completed");
    cache.insert(project_id, &node.id, fingerprint, result.clone());
    emit_node_event(observer, node, NodeExecutionState::Done, Some(1.0), result.cost);
    outputs.insert(node.id.clone(), result.outputs);
    Ok(())
}

fn reuse_cached_output(
    node: &PlanNode,
    project_id: &str,
    fingerprint: u64,
    cache: &ResultCache,
    outputs: &mut RunOutputs,
    observer: &mut dyn FnMut(&NodeProgressEvent),
) -> bool {
    let Some(cached) = cache.get(project_id, &node.id, fingerprint) else {
        return false;
    };
    info!(node_id = %node.id, type_id = %node.type_id, "reusing cached node outputs");
    emit_node_event(observer, node, NodeExecutionState::Cached, Some(1.0), cached.cost);
    outputs.insert(node.id.clone(), cached.outputs);
    true
}

fn run_plan_node(
    node: &PlanNode,
    project_id: &str,
    workflow_snapshot: &serde_json::Value,
    inputs: &ValueMap,
    cancellation: &dyn CancellationSignal,
    observer: &mut dyn FnMut(&NodeProgressEvent),
) -> Result<NodeRunResult> {
    let run_result = {
        let mut context =
            NodeRunContext::new(&node.id, project_id, workflow_snapshot, cancellation, observer);
        node.node.run(inputs, &mut context)
    };
    let result = run_result
        .map_err(|source| EngineError::from((node.id.as_str(), node.type_id.as_str(), source)))
        .and_then(|result| {
            validate_node_outputs(node, &result.outputs)?;
            Ok(result)
        });
    if result.is_err() {
        emit_node_event(observer, node, NodeExecutionState::Error, None, None);
    }
    result
}

fn emit_node_event(
    observer: &mut dyn FnMut(&NodeProgressEvent),
    node: &PlanNode,
    state: NodeExecutionState,
    progress: Option<f32>,
    cost: Option<i64>,
) {
    observer(&NodeProgressEvent { node_id: node.id.clone(), state, progress, cost });
}

struct NeverCancelled;

impl CancellationSignal for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

fn ensure_not_cancelled(cancellation: &dyn CancellationSignal) -> Result<()> {
    if cancellation.is_cancelled() { Err(EngineError::Cancelled) } else { Ok(()) }
}

fn topological_order(plan: &ExecutionPlan) -> Result<Vec<usize>> {
    let mut emitted = BTreeSet::new();
    let mut order = Vec::with_capacity(plan.nodes.len());

    while order.len() < plan.nodes.len() {
        let mut progressed = false;
        for (index, node) in plan.nodes.iter().enumerate() {
            if emitted.contains(&index) || !dependencies_emitted(node, plan, &emitted) {
                continue;
            }
            emitted.insert(index);
            order.push(index);
            progressed = true;
        }

        if !progressed {
            let node_id = first_unemitted_node_id(plan, &emitted);
            return Err(EngineError::Cycle { node_id });
        }
    }

    debug!(node_count = order.len(), "workflow graph ordered topologically");
    Ok(order)
}

fn dependencies_emitted(node: &PlanNode, plan: &ExecutionPlan, emitted: &BTreeSet<usize>) -> bool {
    node.inputs.values().all(|source| {
        plan.nodes
            .iter()
            .position(|candidate| candidate.id == source.0)
            .is_some_and(|index| emitted.contains(&index))
    })
}

fn first_unemitted_node_id(plan: &ExecutionPlan, emitted: &BTreeSet<usize>) -> String {
    plan.nodes
        .iter()
        .enumerate()
        .find(|(index, _)| !emitted.contains(index))
        .map(|(_, node)| node.id.clone())
        .unwrap_or_default()
}

fn resolve_inputs(node: &PlanNode, outputs: &RunOutputs) -> Result<ValueMap> {
    let mut inputs = ValueMap::new();
    for port in node.node.inputs() {
        if let Some(default) = &port.default {
            inputs.insert(port.name.clone(), default.clone());
        }
    }

    for (input_name, source) in &node.inputs {
        let source_outputs =
            outputs.get(source.node_id()).ok_or_else(|| EngineError::UnknownSourceNode {
                node_id: node.id.clone(),
                input: input_name.clone(),
                source_node: source.node_id().to_owned(),
            })?;
        let value = source_outputs.get(source.output_name()).ok_or_else(|| {
            EngineError::UnknownSourceOutput {
                node_id: node.id.clone(),
                input: input_name.clone(),
                source_node: source.node_id().to_owned(),
                output: source.output_name().to_owned(),
            }
        })?;
        inputs.insert(input_name.clone(), value.clone());
    }

    Ok(inputs)
}
