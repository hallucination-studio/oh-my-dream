use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use projects::project::domain::ProjectId;
use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::{
    domain::{
        AssistantApprovalScopeId, AssistantModelInvocationId, AssistantProductionPlanAggregate,
        AssistantProductionPlanId, AssistantSessionId, AssistantUserIntent,
        AssistantWorkflowChangeAggregate, AssistantWorkflowChangeCandidate,
        AssistantWorkflowChangeExpiry, AssistantWorkflowChangeId, AssistantWorkflowChangeLineage,
        AssistantWorkflowFingerprint, AssistantWorkflowMutationDigest,
        AssistantWorkflowStableAliasSet,
    },
    interfaces::{
        AssistantApplicationError, AssistantNodeCapabilityCatalogReaderInterface,
        AssistantNodeCapabilityCatalogRequest, AssistantNodeCapabilityCatalogSnapshot,
        AssistantProductionPlanRepositoryInterface, AssistantWorkflowChangeRepositoryInterface,
        AssistantWorkflowEvaluationRequest, AssistantWorkflowEvaluationResult,
        AssistantWorkflowMutationEvaluatorInterface, AssistantWorkspaceSnapshot,
        AssistantWorkspaceSnapshotReaderInterface,
    },
};

#[test]
fn catalog_contains_exactly_the_eleven_frozen_tools() {
    let catalog = AssistantToolCatalog::try_new().unwrap();
    let actual =
        catalog.contracts().iter().map(|contract| contract.id().as_str()).collect::<BTreeSet<_>>();
    let expected = [
        "assistant.workspace.get_snapshot@1",
        "assistant.node_capability.list@1",
        "assistant.node_capability.describe@1",
        "assistant.production_plan.get@1",
        "assistant.production_plan.create@1",
        "assistant.production_plan.replace@1",
        "assistant.production_plan.update_item@1",
        "assistant.workflow.evaluate_mutation@1",
        "assistant.workflow.propose_change@1",
        "assistant.workflow.get_change@1",
        "assistant.workflow.request_apply@1",
    ]
    .into_iter()
    .collect();

    assert_eq!(actual, expected);
    assert!(catalog.contract("assistant.workflow.apply@1").is_none());
    assert!(catalog.contract("assistant.workflow.start_run@1").is_none());
    assert_eq!(catalog.contracts().iter().filter(|tool| tool.requires_human_approval()).count(), 1);
    assert_eq!(
        catalog.contract("assistant.workflow.request_apply@1").unwrap().effect(),
        AssistantToolEffect::HumanApprovalRequest
    );
}

#[test]
fn strict_input_validation_rejects_unknown_fields_before_deserialization() {
    let catalog = AssistantToolCatalog::try_new().unwrap();
    let contract = catalog.contract("assistant.workspace.get_snapshot@1").unwrap();

    assert!(contract.validate_input(&json!({})).is_ok());
    assert_eq!(
        contract.validate_input(&json!({"project_id": "model-chosen"})),
        Err(AssistantApplicationError::ProtocolViolation)
    );
}

#[test]
fn output_contract_rejects_shape_drift() {
    let catalog = AssistantToolCatalog::try_new().unwrap();
    let contract = catalog.contract("assistant.workspace.get_snapshot@1").unwrap();

    assert!(contract.validate_output(&json!({"snapshot": {}})).is_ok());
    assert_eq!(
        contract.validate_output(&json!({"snapshot": {}, "path": "/private"})),
        Err(AssistantApplicationError::ProtocolViolation)
    );
}

#[test]
fn tool_ids_reject_unversioned_and_unlisted_values() {
    assert!(AssistantToolId::try_new("assistant.workspace.get_snapshot@1").is_ok());
    assert!(AssistantToolId::try_new("assistant.workspace.get_snapshot").is_err());
    assert!(AssistantToolId::try_new("assistant.workflow.apply@1").is_err());
}

#[test]
fn capability_describe_request_is_exact_bounded_and_unique() {
    assert!(AssistantNodeCapabilityCatalogRequest::describe(vec!["image@1".into()]).is_ok());
    assert!(AssistantNodeCapabilityCatalogRequest::describe(Vec::new()).is_err());
    assert!(AssistantNodeCapabilityCatalogRequest::describe(vec!["image@1".into(); 2]).is_err());
    assert!(
        AssistantNodeCapabilityCatalogRequest::describe(vec![
            "a@1".into(),
            "b@1".into(),
            "c@1".into(),
            "d@1".into(),
        ])
        .is_err()
    );
}

#[tokio::test]
async fn dispatcher_executes_plan_tools_and_rejects_direct_authority() {
    let (dispatcher, _) = dispatcher();
    let context = context();

    let created = dispatcher
        .execute(
            context,
            "assistant.production_plan.create@1",
            json!({"title": "Plan", "items": [{"id": "first", "goal": "Draft"}]}),
        )
        .await
        .unwrap();
    assert_eq!(created["revision"], 1);

    let updated = dispatcher
        .execute(
            context,
            "assistant.production_plan.update_item@1",
            json!({
                "expected_revision": 1,
                "item_id": "first",
                "transition": {"type": "start"}
            }),
        )
        .await
        .unwrap();
    assert_eq!(updated["revision"], 2);
    assert_eq!(updated["items"][0]["state"], "in_progress");

    assert_eq!(
        dispatcher.execute(context, "assistant.workflow.apply@1", json!({})).await,
        Err(AssistantApplicationError::ProtocolViolation)
    );
}

#[tokio::test]
async fn dispatcher_executes_bounded_authoritative_read_tools() {
    let (dispatcher, _) = dispatcher();
    let context = context();

    let workspace =
        dispatcher.execute(context, "assistant.workspace.get_snapshot@1", json!({})).await.unwrap();
    assert_eq!(workspace["snapshot"], json!({}));

    let described = dispatcher
        .execute(
            context,
            "assistant.node_capability.describe@1",
            json!({"contract_refs": ["image.generate_from_text@1.0"]}),
        )
        .await
        .unwrap();
    assert_eq!(described["requested_contract_refs"], json!(["image.generate_from_text@1.0"]));
}

#[tokio::test]
async fn dispatcher_evaluates_without_persistence_and_proposes_with_persistence() {
    let (dispatcher, changes) = dispatcher();
    let context = context();
    let input = json!({
        "base_workflow_revision": 1,
        "ordered_mutations": [{"type": "add_node"}]
    });

    let evaluated = dispatcher
        .execute(context, "assistant.workflow.evaluate_mutation@1", input.clone())
        .await
        .unwrap();
    assert_eq!(evaluated["state"], "proposed");
    assert!(changes.values.lock().unwrap().is_empty());

    let proposed =
        dispatcher.execute(context, "assistant.workflow.propose_change@1", input).await.unwrap();
    assert_eq!(proposed["change_id"], change_id().as_uuid().to_string());
    assert_eq!(changes.values.lock().unwrap().len(), 1);
}

type TestDispatcher = AssistantToolDispatcherImpl<
    WorkspaceFake,
    CapabilityFake,
    PlanRepositoryFake,
    EvaluatorFake,
    Arc<ChangeRepositoryFake>,
>;

fn dispatcher() -> (TestDispatcher, Arc<ChangeRepositoryFake>) {
    let changes = Arc::new(ChangeRepositoryFake::default());
    let dispatcher = AssistantToolDispatcherImpl::try_new(
        WorkspaceFake,
        CapabilityFake,
        PlanRepositoryFake::default(),
        EvaluatorFake,
        Arc::clone(&changes),
    )
    .unwrap();
    (dispatcher, changes)
}

fn context() -> AssistantToolExecutionContext {
    AssistantToolExecutionContext {
        project_id: project_id(),
        session_id: session_id(),
        production_plan_id: plan_id(),
    }
}

struct WorkspaceFake;

#[async_trait]
impl AssistantWorkspaceSnapshotReaderInterface for WorkspaceFake {
    async fn read_assistant_workspace_snapshot(
        &self,
        _project_id: ProjectId,
        _session_id: AssistantSessionId,
    ) -> Result<AssistantWorkspaceSnapshot, AssistantApplicationError> {
        AssistantWorkspaceSnapshot::new(b"{}".to_vec())
    }
}

struct CapabilityFake;

#[async_trait]
impl AssistantNodeCapabilityCatalogReaderInterface for CapabilityFake {
    async fn read_assistant_node_capability_catalog(
        &self,
        _request: AssistantNodeCapabilityCatalogRequest,
    ) -> Result<AssistantNodeCapabilityCatalogSnapshot, AssistantApplicationError> {
        AssistantNodeCapabilityCatalogSnapshot::new(br#"{"contracts":[]}"#.to_vec())
    }
}

#[derive(Default)]
struct PlanRepositoryFake {
    value: Mutex<Option<AssistantProductionPlanAggregate>>,
}

#[async_trait]
impl AssistantProductionPlanRepositoryInterface for PlanRepositoryFake {
    async fn load_assistant_production_plan(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
    ) -> Result<Option<AssistantProductionPlanAggregate>, AssistantApplicationError> {
        Ok(self
            .value
            .lock()
            .unwrap()
            .as_ref()
            .filter(|plan| plan.project_id() == project_id && plan.session_id() == session_id)
            .cloned())
    }

    async fn compare_and_swap_assistant_production_plan(
        &self,
        expected_revision: Option<crate::domain::AssistantProductionPlanRevision>,
        plan: AssistantProductionPlanAggregate,
    ) -> Result<(), AssistantApplicationError> {
        let mut stored = self.value.lock().unwrap();
        if stored.as_ref().map(AssistantProductionPlanAggregate::revision) != expected_revision {
            return Err(AssistantApplicationError::RevisionConflict);
        }
        *stored = Some(plan);
        Ok(())
    }
}

struct EvaluatorFake;

#[async_trait]
impl AssistantWorkflowMutationEvaluatorInterface for EvaluatorFake {
    async fn evaluate_assistant_workflow_mutations(
        &self,
        request: AssistantWorkflowEvaluationRequest,
    ) -> Result<AssistantWorkflowEvaluationResult, AssistantApplicationError> {
        Ok(AssistantWorkflowEvaluationResult {
            candidate: AssistantWorkflowChangeCandidate {
                id: change_id(),
                project_id: request.project_id,
                session_id: request.session_id,
                base_workflow_revision: request.base_workflow_revision,
                ordered_mutations: request.ordered_mutations,
                stable_aliases: AssistantWorkflowStableAliasSet::default(),
                readiness_issues: Vec::new(),
                mutation_digest: AssistantWorkflowMutationDigest::new([7; 32]),
                resulting_workflow_fingerprint: AssistantWorkflowFingerprint::new([8; 32]),
                lineage: AssistantWorkflowChangeLineage::UserMessage {
                    invocation_id: AssistantModelInvocationId::from_uuid(uuid(5)).unwrap(),
                    intent: AssistantUserIntent::new("Create workflow").unwrap(),
                },
                approval_scope_id: AssistantApprovalScopeId::from_uuid(uuid(6)).unwrap(),
                expires_at: AssistantWorkflowChangeExpiry::new(60_000).unwrap(),
            },
        })
    }
}

#[derive(Default)]
struct ChangeRepositoryFake {
    values: Mutex<BTreeMap<AssistantWorkflowChangeId, AssistantWorkflowChangeAggregate>>,
}

#[async_trait]
impl AssistantWorkflowChangeRepositoryInterface for Arc<ChangeRepositoryFake> {
    async fn load_assistant_workflow_change(
        &self,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        Ok(self.values.lock().unwrap().get(&change_id).cloned())
    }

    async fn load_pending_assistant_workflow_change(
        &self,
        _project_id: ProjectId,
        _session_id: AssistantSessionId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        Ok(None)
    }

    async fn insert_assistant_workflow_change(
        &self,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        self.values.lock().unwrap().insert(change.id(), change);
        Ok(())
    }

    async fn commit_assistant_workflow_change_transition(
        &self,
        _expected_state: crate::domain::AssistantWorkflowChangeState,
        _change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        Err(AssistantApplicationError::InvalidTransition)
    }

    async fn commit_assistant_workflow_change_apply_decision(
        &self,
        _expected_state: crate::domain::AssistantWorkflowChangeState,
        _change: AssistantWorkflowChangeAggregate,
        _effect: crate::application::AssistantApplyWorkflowChangeEffect,
    ) -> Result<(), AssistantApplicationError> {
        Err(AssistantApplicationError::InvalidTransition)
    }
}

fn project_id() -> ProjectId {
    ProjectId::from_uuid(uuid(1)).unwrap()
}

fn session_id() -> AssistantSessionId {
    AssistantSessionId::from_uuid(uuid(2)).unwrap()
}

fn plan_id() -> AssistantProductionPlanId {
    AssistantProductionPlanId::from_uuid(uuid(3)).unwrap()
}

fn change_id() -> AssistantWorkflowChangeId {
    AssistantWorkflowChangeId::from_uuid(uuid(4)).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
