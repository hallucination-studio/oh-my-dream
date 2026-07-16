use super::*;

#[test]
fn workflow_boundary_rejects_noncanonical_ids_revisions_and_sequences() {
    assert!(uuid("123E4567-E89B-42D3-A456-426614174000").is_err());
    assert!(revision("01").is_err());
    assert!(sequence("0").is_err());
}

#[test]
fn workflow_boundary_accepts_both_closed_run_scopes() {
    let node_id = "123e4567-e89b-42d3-a456-426614174000";
    assert_eq!(scope(WorkflowRunScopeDto::WholeWorkflow).unwrap(), WorkflowRunScope::WholeWorkflow);
    assert_eq!(
        scope(WorkflowRunScopeDto::ThroughNode { node_id: node_id.to_owned() }).unwrap(),
        WorkflowRunScope::ThroughNode(workflow_node_id(node_id).unwrap())
    );
}
