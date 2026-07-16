use std::collections::BTreeMap;
use std::sync::{
    Mutex,
    atomic::{AtomicU8, Ordering},
};

use async_trait::async_trait;
use engine::node_capability::{
    WorkflowNodeCapabilityRegistry, WorkflowNodeExecutionId, WorkflowRunId,
};
use engine::workflow::{
    WorkflowAggregateRepositoryInterface, WorkflowApplicationError, WorkflowClockInterface,
    WorkflowCreateCommandHash, WorkflowCreateReceipt, WorkflowCreationCommit,
    WorkflowExecuteRunEffect, WorkflowIdentityGeneratorInterface, WorkflowLoadKey,
    WorkflowManagedMediaPreviewSource, WorkflowMediaPreview, WorkflowMediaPreviewIssuerInterface,
    WorkflowMutationCommit, WorkflowRunAdmissionCommit, WorkflowRunAdmissionReceipt,
    WorkflowRunAggregate, WorkflowRunEvent, WorkflowRunEventPublisherInterface, WorkflowRunLoadKey,
    WorkflowRunRepositoryInterface, WorkflowRunRequestId, WorkflowRunScope, WorkflowRunTime,
    WorkflowStartRunCommand,
};
use engine::workflow_graph::{
    WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowCreatedAt, WorkflowId,
    WorkflowMutationReceipt, WorkflowMutationRequestId, WorkflowRevision, WorkflowSchemaVersion,
    WorkflowUpdatedAt,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

pub(crate) struct WorkflowContractFakeImpl {
    state: Mutex<WorkflowContractFakeState>,
    workflow_identity_seed: AtomicU8,
    run_identity_seed: AtomicU8,
    node_execution_identity_seed: AtomicU8,
}

impl Default for WorkflowContractFakeImpl {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            workflow_identity_seed: AtomicU8::new(90),
            run_identity_seed: AtomicU8::new(91),
            node_execution_identity_seed: AtomicU8::new(92),
        }
    }
}

impl WorkflowContractFakeImpl {
    pub(crate) fn seed_workflow(&self, workflow: WorkflowAggregate) {
        self.state.lock().unwrap().workflows.insert(workflow.project_id, workflow);
    }
    pub(crate) fn has_execute_effect(&self, run_id: WorkflowRunId) -> bool {
        self.state.lock().unwrap().execute_effects.contains_key(&run_id)
    }
}

#[derive(Default)]
struct WorkflowContractFakeState {
    workflows: BTreeMap<ProjectId, WorkflowAggregate>,
    creation_receipts: BTreeMap<engine::workflow::WorkflowCreateRequestId, WorkflowCreateReceipt>,
    mutation_receipts: BTreeMap<WorkflowMutationRequestId, WorkflowMutationReceipt>,
    runs: BTreeMap<(ProjectId, WorkflowRunId), WorkflowRunAggregate>,
    run_receipts: BTreeMap<WorkflowRunRequestId, WorkflowRunAdmissionReceipt>,
    execute_effects: BTreeMap<WorkflowRunId, WorkflowExecuteRunEffect>,
}

#[async_trait]
impl WorkflowAggregateRepositoryInterface for WorkflowContractFakeImpl {
    async fn load_workflow(
        &self,
        key: WorkflowLoadKey,
    ) -> Result<Option<WorkflowAggregate>, WorkflowApplicationError> {
        let state = self.state.lock().map_err(|_| persistence())?;
        Ok(match key {
            WorkflowLoadKey::Project(project_id) => state.workflows.get(&project_id).cloned(),
            WorkflowLoadKey::Workflow(workflow_id) => {
                state.workflows.values().find(|workflow| workflow.id == workflow_id).cloned()
            }
        })
    }

    async fn load_workflow_creation_receipt(
        &self,
        request_id: engine::workflow::WorkflowCreateRequestId,
    ) -> Result<Option<WorkflowCreateReceipt>, WorkflowApplicationError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| persistence())?
            .creation_receipts
            .get(&request_id)
            .cloned())
    }

    async fn load_workflow_mutation_receipt(
        &self,
        request_id: WorkflowMutationRequestId,
    ) -> Result<Option<WorkflowMutationReceipt>, WorkflowApplicationError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| persistence())?
            .mutation_receipts
            .get(&request_id)
            .cloned())
    }

    async fn commit_workflow_creation(
        &self,
        commit: WorkflowCreationCommit,
    ) -> Result<WorkflowAggregate, WorkflowApplicationError> {
        let (workflow, receipt) = commit.into_parts();
        let mut state = self.state.lock().map_err(|_| persistence())?;
        if let Some(existing) = state.creation_receipts.get(&receipt.request_id()) {
            return if existing.command_hash() == receipt.command_hash() {
                Ok(existing.created_workflow().clone())
            } else {
                Err(WorkflowApplicationError::WorkflowCreationIdempotencyConflict)
            };
        }
        if state.workflows.contains_key(&workflow.project_id) {
            return Err(WorkflowApplicationError::WorkflowAlreadyExistsForProject);
        }
        state.workflows.insert(workflow.project_id, workflow.clone());
        state.creation_receipts.insert(receipt.request_id(), receipt);
        Ok(workflow)
    }

    async fn commit_workflow_mutation(
        &self,
        commit: WorkflowMutationCommit,
    ) -> Result<WorkflowMutationReceipt, WorkflowApplicationError> {
        let (workflow, expected_revision, receipt) = commit.into_parts();
        let mut state = self.state.lock().map_err(|_| persistence())?;
        if let Some(existing) = state.mutation_receipts.get(&receipt.request_id()) {
            return if existing.command_hash() == receipt.command_hash() {
                Ok(existing.clone())
            } else {
                Err(WorkflowApplicationError::WorkflowMutationIdempotencyConflict)
            };
        }
        let current = state.workflows.get(&workflow.project_id).ok_or(
            WorkflowApplicationError::WorkflowNotFound {
                key: WorkflowLoadKey::Project(workflow.project_id),
            },
        )?;
        if current.revision != expected_revision {
            return Err(WorkflowApplicationError::WorkflowRevisionConflict);
        }
        state.workflows.insert(workflow.project_id, workflow);
        state.mutation_receipts.insert(receipt.request_id(), receipt.clone());
        Ok(receipt)
    }
}

#[async_trait]
impl WorkflowRunRepositoryInterface for WorkflowContractFakeImpl {
    async fn load_workflow_run(
        &self,
        key: WorkflowRunLoadKey,
    ) -> Result<Option<WorkflowRunAggregate>, WorkflowApplicationError> {
        let state = self.state.lock().map_err(|_| persistence())?;
        Ok(match key {
            WorkflowRunLoadKey::Run(run_id) => {
                state.runs.values().find(|run| run.run_id() == run_id).cloned()
            }
            WorkflowRunLoadKey::ProjectScoped { project_id, workflow_run_id } => {
                state.runs.get(&(project_id, workflow_run_id)).cloned()
            }
        })
    }

    async fn load_workflow_run_admission_receipt(
        &self,
        request_id: WorkflowRunRequestId,
    ) -> Result<Option<WorkflowRunAdmissionReceipt>, WorkflowApplicationError> {
        Ok(self.state.lock().map_err(|_| persistence())?.run_receipts.get(&request_id).cloned())
    }

    async fn admit_workflow_run(
        &self,
        commit: WorkflowRunAdmissionCommit,
    ) -> Result<WorkflowRunAdmissionReceipt, WorkflowApplicationError> {
        let (run, receipt, effect) = commit.into_parts();
        let key = (run.project_id(), run.run_id());
        let mut state = self.state.lock().map_err(|_| persistence())?;
        if let Some(existing) = state.run_receipts.get(&receipt.request_id()) {
            return if existing.command_hash() == receipt.command_hash() {
                Ok(existing.clone())
            } else {
                Err(WorkflowApplicationError::WorkflowRunIdempotencyConflict)
            };
        }
        state.runs.insert(key, run);
        state.run_receipts.insert(receipt.request_id(), receipt.clone());
        state.execute_effects.insert(effect.workflow_run_id, effect);
        Ok(receipt)
    }

    async fn commit_workflow_run_transition(
        &self,
        run: WorkflowRunAggregate,
        expected_last_event_count: usize,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
        let key = (run.project_id(), run.run_id());
        let mut state = self.state.lock().map_err(|_| persistence())?;
        let current = state.runs.get(&key).ok_or(WorkflowApplicationError::WorkflowRunNotFound)?;
        if current.events().len() != expected_last_event_count {
            return Err(WorkflowApplicationError::WorkflowRevisionConflict);
        }
        state.runs.insert(key, run.clone());
        Ok(run)
    }
}

impl WorkflowClockInterface for WorkflowContractFakeImpl {
    fn current_workflow_time(&self) -> Result<WorkflowRunTime, WorkflowApplicationError> {
        WorkflowRunTime::from_utc_milliseconds(123).map_err(|_| persistence())
    }
}

impl WorkflowIdentityGeneratorInterface for WorkflowContractFakeImpl {
    fn generate_workflow_id(&self) -> WorkflowId {
        workflow_id(self.workflow_identity_seed.fetch_add(1, Ordering::Relaxed))
    }
    fn generate_workflow_run_id(&self) -> WorkflowRunId {
        WorkflowRunId::from_uuid(uuid(self.run_identity_seed.fetch_add(1, Ordering::Relaxed)))
            .unwrap()
    }
    fn generate_workflow_node_execution_id(&self) -> WorkflowNodeExecutionId {
        WorkflowNodeExecutionId::from_uuid(uuid(
            self.node_execution_identity_seed.fetch_add(1, Ordering::Relaxed),
        ))
        .unwrap()
    }
}

#[async_trait]
impl WorkflowMediaPreviewIssuerInterface for WorkflowContractFakeImpl {
    async fn issue_workflow_media_preview(
        &self,
        _project_id: ProjectId,
        _source: WorkflowManagedMediaPreviewSource,
    ) -> Result<WorkflowMediaPreview, WorkflowApplicationError> {
        WorkflowMediaPreview::try_new("preview-token")
    }
}

#[async_trait]
impl WorkflowRunEventPublisherInterface for WorkflowContractFakeImpl {
    async fn publish_committed_workflow_run_event(
        &self,
        _event: WorkflowRunEvent,
    ) -> Result<(), WorkflowApplicationError> {
        Ok(())
    }
}

#[tokio::test]
async fn repository_fake_atomically_rejects_a_second_workflow_for_one_project() {
    let fake = WorkflowContractFakeImpl::default();
    let workflow = empty_workflow(1, 2);
    let commit = creation_commit(workflow.clone(), 4);
    assert_eq!(fake.commit_workflow_creation(commit).await.unwrap(), workflow);
    let error =
        fake.commit_workflow_creation(creation_commit(empty_workflow(1, 3), 5)).await.unwrap_err();
    assert_eq!(error, WorkflowApplicationError::WorkflowAlreadyExistsForProject);
    assert_eq!(
        fake.load_workflow(WorkflowLoadKey::Project(project_id(1))).await.unwrap().unwrap().id,
        workflow.id
    );
}

#[tokio::test]
async fn repository_fake_replays_matching_creation_without_a_second_write() {
    let fake = WorkflowContractFakeImpl::default();
    let workflow = empty_workflow(13, 14);
    let first = creation_commit(workflow.clone(), 6);
    let replay = creation_commit(workflow.clone(), 6);

    assert_eq!(fake.commit_workflow_creation(first).await.unwrap(), workflow);
    assert_eq!(fake.commit_workflow_creation(replay).await.unwrap(), workflow);
    assert_eq!(fake.state.lock().unwrap().workflows.len(), 1);
}

#[tokio::test]
async fn run_repository_fake_admits_run_receipt_and_effect_as_one_unit() {
    let fake = WorkflowContractFakeImpl::default();
    let run_id = WorkflowRunId::from_uuid(uuid(20)).unwrap();
    let project_id = project_id(21);
    let run = super::workflow_run_domain::queued_run(
        run_id,
        project_id,
        workflow_id(22),
        WorkflowRunScope::WholeWorkflow,
        vec![(
            engine::workflow_graph::WorkflowNodeId::from_uuid(uuid(23)).unwrap(),
            WorkflowNodeExecutionId::from_uuid(uuid(24)).unwrap(),
        )],
        WorkflowRunTime::from_utc_milliseconds(1).unwrap(),
    )
    .unwrap();
    let request_id = WorkflowRunRequestId::from_uuid(uuid(25)).unwrap();
    let receipt = WorkflowRunAdmissionReceipt::new(
        WorkflowStartRunCommand::new(
            request_id,
            workflow_id(22),
            WorkflowRevision::new(1).unwrap(),
            WorkflowRunScope::WholeWorkflow,
        ),
        run_id,
    );
    let commit = WorkflowRunAdmissionCommit::try_new(
        run,
        receipt.clone(),
        WorkflowExecuteRunEffect { workflow_run_id: run_id },
    )
    .unwrap();

    assert_eq!(fake.admit_workflow_run(commit).await.unwrap(), receipt);
    assert!(
        fake.load_workflow_run(WorkflowRunLoadKey::ProjectScoped {
            project_id,
            workflow_run_id: run_id,
        })
        .await
        .unwrap()
        .is_some()
    );
    assert_eq!(fake.load_workflow_run_admission_receipt(request_id).await.unwrap(), Some(receipt));
}

#[test]
fn deterministic_clock_and_identity_fake_satisfy_exact_interfaces() {
    let fake = WorkflowContractFakeImpl::default();
    assert_eq!(fake.current_workflow_time().unwrap().as_utc_milliseconds(), 123);
    assert_eq!(fake.generate_workflow_id(), workflow_id(90));
}

#[test]
fn atomic_creation_unit_rejects_a_receipt_for_another_snapshot() {
    let workflow = empty_workflow(7, 8);
    let receipt = WorkflowCreateReceipt::new(
        engine::workflow::WorkflowCreateRequestId::from_uuid(uuid(9)).unwrap(),
        WorkflowCreateCommandHash::from_bytes([10; 32]),
        empty_workflow(7, 11),
    )
    .unwrap();
    assert_eq!(
        WorkflowCreationCommit::try_new(workflow, receipt).unwrap_err(),
        WorkflowApplicationError::WorkflowPersistenceFailure
    );
}

fn creation_commit(workflow: WorkflowAggregate, request_seed: u8) -> WorkflowCreationCommit {
    WorkflowCreationCommit::try_new(
        workflow.clone(),
        WorkflowCreateReceipt::new(
            engine::workflow::WorkflowCreateRequestId::from_uuid(uuid(request_seed)).unwrap(),
            WorkflowCreateCommandHash::from_bytes([5; 32]),
            workflow,
        )
        .unwrap(),
    )
    .unwrap()
}

fn empty_workflow(project_seed: u8, workflow_seed: u8) -> WorkflowAggregate {
    WorkflowAggregate::try_restore(
        WorkflowAggregateRestoreData {
            schema_version: WorkflowSchemaVersion::CURRENT,
            id: workflow_id(workflow_seed),
            project_id: project_id(project_seed),
            revision: WorkflowRevision::new(1).unwrap(),
            created_at: WorkflowCreatedAt::from_utc_milliseconds(1).unwrap(),
            updated_at: WorkflowUpdatedAt::from_utc_milliseconds(1).unwrap(),
            nodes: Vec::new(),
            input_bindings: Vec::new(),
        },
        &WorkflowNodeCapabilityRegistry::try_new([]).unwrap(),
    )
    .unwrap()
}

fn persistence() -> WorkflowApplicationError {
    WorkflowApplicationError::WorkflowPersistenceFailure
}

fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}
fn workflow_id(seed: u8) -> WorkflowId {
    WorkflowId::from_uuid(uuid(seed)).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
