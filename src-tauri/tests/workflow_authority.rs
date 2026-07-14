use engine::{Workflow, WorkflowNode};
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_authority::{WorkflowAuthorityError, WorkflowCommitRequest};
use rusqlite::Connection;
use serde_json::{Map, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::tempdir;

fn workflow(project_id: &str, node_ids: &[&str]) -> Workflow {
    Workflow {
        version: "1.0".to_owned(),
        project_id: project_id.to_owned(),
        nodes: node_ids
            .iter()
            .map(|id| WorkflowNode {
                id: (*id).to_owned(),
                type_id: "TextPrompt".to_owned(),
                contract_version: "1.0".to_owned(),
                params: Map::from_iter([(String::from("text"), json!(id))]),
                inputs: BTreeMap::new(),
                position: None,
            })
            .collect(),
    }
}

fn request(
    project_id: &str,
    expected_revision: Option<u64>,
    request_id: &str,
    request_hash: &str,
    node_ids: &[&str],
) -> WorkflowCommitRequest {
    WorkflowCommitRequest::new(
        project_id,
        expected_revision,
        request_id,
        request_hash,
        workflow(project_id, node_ids),
    )
}

fn row_count(config_root: &std::path::Path, table: &str) -> i64 {
    let connection =
        Connection::open(config_root.join("workflow.sqlite")).expect("open authority database");
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
        .expect("count authority rows")
}

#[test]
fn new_project_has_no_head_and_empty_first_mutation_is_not_a_creation() {
    let assets_root = tempdir().expect("create assets root");
    let config_root = tempdir().expect("create config root");
    let state = AppState::from_roots(assets_root.path(), config_root.path()).expect("build state");
    let project =
        state.store.lock().expect("lock store").create_project("Empty").expect("create project");

    assert!(state.workflow_authority.load_head(&project.id).expect("read absent head").is_none());
    let result = state
        .workflow_authority
        .apply(request(&project.id, None, "empty-request", "empty-hash", &[]))
        .expect("empty mutation is a no-op");

    assert!(!result.changed);
    assert!(result.head.is_none());
    assert!(state.workflow_authority.load_head(&project.id).expect("read absent head").is_none());
    assert_eq!(row_count(config_root.path(), "workflow_heads"), 0);
}

#[test]
fn first_non_empty_commit_creates_revision_one_and_one_undo_unit() {
    let assets_root = tempdir().expect("create assets root");
    let config_root = tempdir().expect("create config root");
    let state = AppState::from_roots(assets_root.path(), config_root.path()).expect("build state");
    let result = state
        .workflow_authority
        .apply(request("default", None, "request-1", "hash-1", &["prompt"]))
        .expect("first patch");
    let head = result.head.expect("first patch returns head");

    assert!(result.changed);
    assert!(!result.deduplicated);
    assert_eq!(head.project_id, "default");
    assert_eq!(head.revision, 1);
    assert_eq!(head.workflow.nodes.len(), 1);
    assert!(result.undo_id.is_some());
    assert_eq!(row_count(config_root.path(), "workflow_heads"), 1);
    assert_eq!(row_count(config_root.path(), "workflow_undo"), 1);
    assert_eq!(row_count(config_root.path(), "workflow_receipts"), 1);
}

#[test]
fn duplicate_requests_survive_reopen_and_hash_reuse_is_rejected() {
    let assets_root = tempdir().expect("create assets root");
    let config_root = tempdir().expect("create config root");
    let first = AppState::from_roots(assets_root.path(), config_root.path()).expect("build state");
    let original = request("default", None, "request-1", "hash-1", &["prompt"]);
    first.workflow_authority.apply(original.clone()).expect("first patch");
    drop(first);

    let reopened =
        AppState::from_roots(assets_root.path(), config_root.path()).expect("reopen state");
    let duplicate =
        reopened.workflow_authority.apply(original).expect("duplicate request returns its receipt");
    assert!(duplicate.deduplicated);
    assert!(duplicate.changed);
    assert!(duplicate.undo_id.is_some());
    assert_eq!(duplicate.head.expect("receipt head").revision, 1);

    let error = reopened
        .workflow_authority
        .apply(request("default", None, "request-1", "different-hash", &["other"]))
        .expect_err("request id cannot be reused with another hash");
    assert!(matches!(error, WorkflowAuthorityError::RequestHashMismatch { .. }));
    assert_eq!(row_count(config_root.path(), "workflow_heads"), 1);
    assert_eq!(row_count(config_root.path(), "workflow_undo"), 1);
}

#[test]
fn normalized_no_op_keeps_revision_without_another_undo_unit() {
    let assets_root = tempdir().expect("create assets root");
    let config_root = tempdir().expect("create config root");
    let state = AppState::from_roots(assets_root.path(), config_root.path()).expect("build state");
    state
        .workflow_authority
        .apply(request("default", None, "request-1", "hash-1", &["prompt"]))
        .expect("first patch");
    let result = state
        .workflow_authority
        .apply(request("default", Some(1), "request-2", "hash-2", &["prompt"]))
        .expect("normalized no-op");

    assert!(!result.changed);
    assert_eq!(result.head.expect("existing head").revision, 1);
    assert!(result.undo_id.is_none());
    assert_eq!(row_count(config_root.path(), "workflow_undo"), 1);
    assert_eq!(row_count(config_root.path(), "workflow_receipts"), 2);
}

#[test]
fn clearing_an_existing_workflow_keeps_an_empty_head_present() {
    let assets_root = tempdir().expect("create assets root");
    let config_root = tempdir().expect("create config root");
    let state = AppState::from_roots(assets_root.path(), config_root.path()).expect("build state");
    state
        .workflow_authority
        .apply(request("default", None, "request-1", "hash-1", &["prompt"]))
        .expect("first patch");

    let result = state
        .workflow_authority
        .apply(request("default", Some(1), "request-2", "hash-2", &[]))
        .expect("clear Workflow");
    let head = result.head.expect("empty Workflow remains present");

    assert!(result.changed);
    assert_eq!(head.revision, 2);
    assert!(head.workflow.nodes.is_empty());
    assert_eq!(row_count(config_root.path(), "workflow_heads"), 1);
    assert_eq!(row_count(config_root.path(), "workflow_undo"), 2);
}

#[test]
fn stale_expected_revisions_fail_without_writing() {
    let assets_root = tempdir().expect("create assets root");
    let config_root = tempdir().expect("create config root");
    let state = AppState::from_roots(assets_root.path(), config_root.path()).expect("build state");
    state
        .workflow_authority
        .apply(request("default", None, "request-1", "hash-1", &["prompt"]))
        .expect("first patch");

    let error = state
        .workflow_authority
        .apply(request("default", None, "request-2", "hash-2", &["other"]))
        .expect_err("create-only request must conflict with an existing head");
    assert!(matches!(error, WorkflowAuthorityError::RevisionConflict { .. }));
    assert_eq!(row_count(config_root.path(), "workflow_heads"), 1);
    assert_eq!(row_count(config_root.path(), "workflow_undo"), 1);
    assert_eq!(row_count(config_root.path(), "workflow_receipts"), 1);
}

#[test]
fn concurrent_first_commits_accept_one_and_preserve_one_revision() {
    let assets_root = tempdir().expect("create assets root");
    let config_root = tempdir().expect("create config root");
    let state = AppState::from_roots(assets_root.path(), config_root.path()).expect("build state");
    let authority = Arc::clone(&state.workflow_authority);

    std::thread::scope(|scope| {
        let left = Arc::clone(&authority);
        let right = Arc::clone(&authority);
        let left = scope
            .spawn(move || left.apply(request("default", None, "left", "left-hash", &["left"])));
        let right = scope.spawn(move || {
            right.apply(request("default", None, "right", "right-hash", &["right"]))
        });
        let outcomes = [left.join().expect("left thread"), right.join().expect("right thread")];
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    Err(WorkflowAuthorityError::RevisionConflict { .. })
                ))
                .count(),
            1
        );
    });

    let head = state
        .workflow_authority
        .load_head("default")
        .expect("read concurrent head")
        .expect("one commit accepted");
    assert_eq!(head.revision, 1);
    assert_eq!(row_count(config_root.path(), "workflow_heads"), 1);
    assert_eq!(row_count(config_root.path(), "workflow_undo"), 1);
    assert_eq!(row_count(config_root.path(), "workflow_receipts"), 1);
}
