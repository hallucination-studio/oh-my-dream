use super::*;

pub(super) fn build_execution_request(
    run: &WorkflowRunAggregate,
    execution_id: WorkflowNodeExecutionId,
    cancellation: NodeCapabilityExecutionCancellation,
    capabilities: &WorkflowNodeCapabilityRegistry,
) -> Result<
    (
        Arc<dyn crate::node_capability::WorkflowNodeCapabilityInterface>,
        NodeCapabilityExecutionRequest,
    ),
    WorkflowApplicationError,
> {
    let planned = run
        .plan()
        .nodes()
        .iter()
        .find(|node| node.node_execution_id == execution_id)
        .ok_or_else(persistence)?;
    let capability = capabilities
        .resolve_node_capability(&planned.capability_contract)
        .map_err(|_| WorkflowApplicationError::WorkflowCapabilityExecutionFailure)?;
    let inputs = resolve_inputs(run, planned, capability.node_capability_contract())?;
    let deadline = crate::node_capability::NodeCapabilityExecutionDeadline::at(
        Instant::now() + Duration::from_secs(5),
    );
    Ok((
        capability,
        NodeCapabilityExecutionRequest {
            context: WorkflowNodeExecutionContext {
                project_id: run.project_id(),
                workflow_run_id: run.run_id(),
                node_execution_id: execution_id,
                deadline,
                cancellation,
            },
            origin: WorkflowNodeExecutionOrigin::new(
                run.workflow_id(),
                run.workflow_revision(),
                planned.node_id,
                planned.capability_contract.clone(),
            ),
            normalized_parameters: planned.normalized_parameters.clone(),
            inputs,
        },
    ))
}

fn resolve_inputs(
    run: &WorkflowRunAggregate,
    planned: &crate::workflow::WorkflowPlannedNode,
    contract: &crate::node_capability::NodeCapabilityContract,
) -> Result<WorkflowNodeInputSet, WorkflowApplicationError> {
    let mut values = BTreeMap::new();
    for planned_binding in &planned.input_bindings {
        let items = planned_binding
            .binding
            .items()
            .map(|item| {
                let source = run
                    .node_executions()
                    .iter()
                    .find(|execution| execution.node_id() == item.source_node_id)
                    .and_then(|execution| execution.outputs())
                    .and_then(|outputs| outputs.get(&item.source_output_key))
                    .cloned()
                    .ok_or_else(persistence)?;
                Ok(WorkflowRuntimeInputItem {
                    input_item_id: item.id,
                    input_role_key: item.input_role_key.clone(),
                    value: source,
                })
            })
            .collect::<Result<Vec<_>, WorkflowApplicationError>>()?;
        let value = match &planned_binding.binding {
            crate::workflow_graph::WorkflowInputBinding::Single { .. } => {
                WorkflowNodeInputValue::Single(items.into_iter().next().ok_or_else(persistence)?)
            }
            crate::workflow_graph::WorkflowInputBinding::OrderedReferences { .. } => {
                WorkflowNodeInputValue::OrderedReferences(items)
            }
        };
        values.insert(planned_binding.input_key.clone(), value);
    }
    WorkflowNodeInputSet::try_new(contract, values).map_err(|_| persistence())
}

pub(super) fn ready_node_execution_ids(run: &WorkflowRunAggregate) -> Vec<WorkflowNodeExecutionId> {
    run.node_executions()
        .iter()
        .filter(|execution| {
            execution.state() == WorkflowNodeExecutionState::Pending
                && dependencies(run, execution.node_id()).iter().all(|source| {
                    run.node_executions().iter().any(|candidate| {
                        candidate.node_id() == *source
                            && candidate.state() == WorkflowNodeExecutionState::Succeeded
                    })
                })
        })
        .map(|execution| execution.execution_id())
        .collect()
}

pub(super) fn failed_ancestors(
    run: &WorkflowRunAggregate,
    node_id: WorkflowNodeId,
) -> Vec<WorkflowNodeId> {
    let mut failed = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut frontier = dependencies(run, node_id);
    while let Some(source) = frontier.pop() {
        if !visited.insert(source) {
            continue;
        }
        if run.node_executions().iter().any(|execution| {
            execution.node_id() == source && execution.state() == WorkflowNodeExecutionState::Failed
        }) {
            failed.insert(source);
        }
        frontier.extend(dependencies(run, source));
    }
    failed.into_iter().collect()
}

fn dependencies(run: &WorkflowRunAggregate, node_id: WorkflowNodeId) -> Vec<WorkflowNodeId> {
    run.plan()
        .nodes()
        .iter()
        .find(|node| node.node_id == node_id)
        .into_iter()
        .flat_map(|node| &node.input_bindings)
        .flat_map(|binding| binding.binding.items().map(|item| item.source_node_id))
        .collect()
}

pub(super) fn readiness_execution_error(
    contract_ref: crate::node_capability::NodeCapabilityContractRef,
    execution_id: WorkflowNodeExecutionId,
    issue: crate::node_capability::NodeCapabilityReadinessIssue,
) -> NodeCapabilityExecutionError {
    let target = match issue.target() {
        NodeCapabilityReadinessTarget::Capability => NodeCapabilityExecutionTarget::Capability,
        NodeCapabilityReadinessTarget::ManagedAsset { parameter_key, .. }
        | NodeCapabilityReadinessTarget::GenerationProfile { parameter_key, .. } => {
            NodeCapabilityExecutionTarget::Parameter(parameter_key.clone())
        }
    };
    NodeCapabilityExecutionError::try_new(
        contract_ref.clone(),
        execution_id,
        NodeCapabilityExecutionStage::ResolveInputs,
        NodeCapabilityExecutionFailure::Readiness(issue),
        target,
    )
    .unwrap_or_else(|_| {
        NodeCapabilityExecutionError::invalid_capability_invocation(contract_ref, execution_id)
    })
}
