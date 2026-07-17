use super::*;

pub(super) fn is_stale(
    workflow: &WorkflowAggregate,
    run: &WorkflowRunAggregate,
    node_id: WorkflowNodeId,
    capabilities: &WorkflowNodeCapabilityRegistry,
) -> bool {
    let mut ancestors = BTreeSet::from([node_id]);
    let mut frontier = vec![node_id];
    while let Some(current) = frontier.pop() {
        let Some(plan_node) = run.plan().nodes().iter().find(|node| node.node_id == current) else {
            return true;
        };
        for source in plan_node
            .input_bindings
            .iter()
            .flat_map(|binding| binding.binding.items().map(|item| item.source_node_id))
        {
            if ancestors.insert(source) {
                frontier.push(source);
            }
        }
    }
    ancestors.into_iter().any(|current| node_differs(workflow, run, current, capabilities))
}

fn node_differs(
    workflow: &WorkflowAggregate,
    run: &WorkflowRunAggregate,
    node_id: WorkflowNodeId,
    capabilities: &WorkflowNodeCapabilityRegistry,
) -> bool {
    let Some(current) = workflow.nodes().get(&node_id) else { return true };
    let Some(planned) = run.plan().nodes().iter().find(|node| node.node_id == node_id) else {
        return true;
    };
    if current.capability_contract != planned.capability_contract {
        return true;
    }
    let Ok(capability) = capabilities.resolve_node_capability(&current.capability_contract) else {
        return true;
    };
    if capability.normalize_node_parameters(&current.parameter_set).ok().as_ref()
        != Some(&planned.normalized_parameters)
    {
        return true;
    }
    let current_bindings = workflow
        .input_bindings()
        .iter()
        .filter(|(target, _)| target.node_id == node_id)
        .map(|(target, binding)| crate::workflow::WorkflowPlannedInputBinding {
            input_key: target.input_key.clone(),
            binding: binding.clone(),
        })
        .collect::<Vec<_>>();
    current_bindings != planned.input_bindings
}
