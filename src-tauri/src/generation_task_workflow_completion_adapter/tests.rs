use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use assets::asset::{application::*, domain::*, interfaces::*};
use async_trait::async_trait;
use engine::{
    node_capability::*,
    workflow::*,
    workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision},
};
use nodes::*;
use projects::project::domain::ProjectId;
use rusqlite::Connection;
use tasks::generation_task::*;
use uuid::Uuid;

use super::DesktopGenerationTaskWorkflowCompletionAdapterImpl;
use crate::{
    post_commit_effect::SqliteDesktopPostCommitEffectOutboxAdapterImpl,
    workflow_adapters::SystemWorkflowClockAdapterImpl,
    workflow_storage_adapters::SqliteWorkflowRunRepositoryAdapterImpl,
};

#[tokio::test]
async fn media_completion_commits_one_output_event_and_downstream_effect_across_replay() {
    let capability = Arc::new(
        TextToImageCapabilityImpl::try_new(
            Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap()),
            AvailableProfileFakeImpl,
            NodeCapabilityGenerationTaskStarterFakeImpl::default(),
        )
        .unwrap(),
    );
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new([
            capability.clone() as Arc<dyn WorkflowNodeCapabilityInterface>
        ])
        .unwrap(),
    );
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(connection.clone()).unwrap();
    let workflow_repository = Arc::new(
        SqliteWorkflowRunRepositoryAdapterImpl::try_new(connection.clone(), registry.clone())
            .unwrap(),
    );
    let (run, origin) = waiting_run(capability.as_ref());
    admit_run(workflow_repository.as_ref(), run.clone()).await;
    workflow_repository.commit_workflow_run_transition(run.clone(), 1).await.unwrap();

    let asset = available_image_asset(origin.project_id, 80, [9; 32]);
    let asset_get = Arc::new(AssetGetUseCase::new(Arc::new(OneAssetRepositoryFakeImpl {
        asset: asset.clone(),
    })));
    let workflow_completion = Arc::new(WorkflowCompleteGenerationTaskUseCase::new(
        workflow_repository.clone(),
        Arc::new(SystemWorkflowClockAdapterImpl),
        registry,
    ));
    let bridge =
        DesktopGenerationTaskWorkflowCompletionAdapterImpl::new(workflow_completion, asset_get);
    let task = succeeded_image_task(origin, asset.id());

    assert_eq!(
        bridge.complete_generation_task_workflow_origin(&task).await.unwrap(),
        GenerationTaskWorkflowCompletionOutcome::Applied
    );
    assert_eq!(
        bridge.complete_generation_task_workflow_origin(&task).await.unwrap(),
        GenerationTaskWorkflowCompletionOutcome::AlreadyApplied
    );

    let committed = workflow_repository
        .load_workflow_run(WorkflowRunLoadKey::Run(run.run_id()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(committed.events().len(), run.events().len() + 1);
    let outputs = committed.node_executions()[0].outputs().unwrap();
    let WorkflowRuntimeValue::Image(output) =
        outputs.get(&NodeCapabilityOutputKey::new("image").unwrap()).unwrap()
    else {
        panic!("expected Image output")
    };
    assert_eq!(output.asset_id().as_bytes(), asset.id().as_uuid().into_bytes());
    assert_eq!(output.content_fingerprint().as_bytes(), [9; 32]);
    let locked = connection.lock().unwrap();
    let completion_effects: u32 = locked
        .query_row(
            "SELECT COUNT(*) FROM desktop_post_commit_effects WHERE effect_id = ?1 AND state = 0",
            [task.id().as_uuid().as_bytes().as_slice()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(completion_effects, 1);
}

#[derive(Clone)]
struct AvailableProfileFakeImpl;

#[async_trait]
impl GenerationProfileAvailabilityReaderInterface for AvailableProfileFakeImpl {
    async fn read_generation_profile_availability(
        &self,
        request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError> {
        request
            .profile_refs()
            .iter()
            .cloned()
            .map(|profile| {
                GenerationProfileAvailabilityObservation::try_new(
                    profile,
                    GenerationProfileAvailabilityState::Available,
                    1,
                    2,
                )
            })
            .collect()
    }
}

struct OneAssetRepositoryFakeImpl {
    asset: AssetAggregate,
}

#[async_trait]
impl AssetRepositoryInterface for OneAssetRepositoryFakeImpl {
    async fn find_asset_by_id(
        &self,
        asset_id: AssetId,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError> {
        Ok((self.asset.id() == asset_id).then(|| self.asset.clone()))
    }
    async fn find_asset_by_node_output_key(
        &self,
        _: AssetNodeOutputKey,
    ) -> Result<Option<AssetAggregate>, AssetApplicationError> {
        Ok(None)
    }
    async fn list_project_assets(
        &self,
        _: AssetListQuery,
    ) -> Result<AssetListPage, AssetApplicationError> {
        Err(AssetApplicationError::NotFound)
    }
    async fn find_asset_content_finalization(
        &self,
        _: AssetContentFinalizationId,
    ) -> Result<Option<AssetContentFinalization>, AssetApplicationError> {
        Ok(None)
    }
    async fn list_unfinished_asset_content_finalizations(
        &self,
        _: Option<AssetFinalizationRecoveryCursor>,
        _: AssetPageLimit,
    ) -> Result<AssetContentFinalizationRecoveryPage, AssetApplicationError> {
        Err(AssetApplicationError::NotFound)
    }
    async fn list_available_assets_for_content_verification(
        &self,
        _: Option<AssetAvailableContentRecoveryCursor>,
        _: AssetPageLimit,
    ) -> Result<AssetAvailableContentRecoveryPage, AssetApplicationError> {
        Err(AssetApplicationError::NotFound)
    }
    async fn is_asset_staged_content_referenced(
        &self,
        _: AssetStagedContentRef,
    ) -> Result<bool, AssetApplicationError> {
        Ok(false)
    }
}

#[derive(Clone, Copy)]
struct TestOrigin {
    project_id: ProjectId,
    workflow_id: WorkflowId,
    revision: WorkflowRevision,
    run_id: WorkflowRunId,
    node_id: WorkflowNodeId,
    execution_id: WorkflowNodeExecutionId,
}

fn waiting_run(
    capability: &impl WorkflowNodeCapabilityInterface,
) -> (WorkflowRunAggregate, TestOrigin) {
    let origin = TestOrigin {
        project_id: project_id(1),
        workflow_id: WorkflowId::from_uuid(uuid(2)).unwrap(),
        revision: WorkflowRevision::new(1).unwrap(),
        run_id: WorkflowRunId::from_uuid(uuid(3)).unwrap(),
        node_id: WorkflowNodeId::from_uuid(uuid(4)).unwrap(),
        execution_id: WorkflowNodeExecutionId::from_uuid(uuid(5)).unwrap(),
    };
    let plan = WorkflowExecutionPlan::try_new(
        origin.workflow_id,
        origin.revision,
        WorkflowRunScope::WholeWorkflow,
        vec![WorkflowPlannedNode {
            node_id: origin.node_id,
            node_execution_id: origin.execution_id,
            capability_contract: capability.node_capability_contract().contract_ref().clone(),
            normalized_parameters: normalized_parameters(capability),
            input_bindings: Vec::new(),
        }],
    )
    .unwrap();
    let mut run = WorkflowRunAggregate::try_new_queued(
        origin.run_id,
        origin.project_id,
        plan,
        workflow_time(1),
    )
    .unwrap();
    run.start(workflow_time(2)).unwrap();
    run.start_node(origin.execution_id, workflow_time(3)).unwrap();
    run.wait_node_for_external_completion(origin.execution_id, workflow_time(4)).unwrap();
    (run, origin)
}

fn normalized_parameters(
    capability: &impl WorkflowNodeCapabilityInterface,
) -> NodeCapabilityNormalizedParameters {
    let profile = GenerationProfileCatalog::frozen_mvp()
        .unwrap()
        .list_active_generation_profiles_for_capability(
            capability.node_capability_contract().contract_ref(),
        )[0]
    .profile_ref()
    .clone();
    capability
        .normalize_node_parameters(
            &NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
                NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
                NodeCapabilityParameterValue::GenerationProfile(
                    profile.to_node_capability_parameter_value().unwrap(),
                ),
            )]))
            .unwrap(),
        )
        .unwrap()
}

async fn admit_run(repository: &SqliteWorkflowRunRepositoryAdapterImpl, run: WorkflowRunAggregate) {
    let request_id = WorkflowRunRequestId::from_uuid(uuid(6)).unwrap();
    let receipt = WorkflowRunAdmissionReceipt::new(
        WorkflowStartRunCommand::new(
            request_id,
            run.workflow_id(),
            run.workflow_revision(),
            run.scope(),
        ),
        run.run_id(),
    );
    // Restore the exact queued admission snapshot from the frozen plan.
    let queued = WorkflowRunAggregate::try_new_queued(
        run.run_id(),
        run.project_id(),
        run.plan().clone(),
        workflow_time(1),
    )
    .unwrap();
    repository
        .admit_workflow_run(
            WorkflowRunAdmissionCommit::try_new(
                queued,
                receipt,
                WorkflowExecuteRunEffect { workflow_run_id: run.run_id() },
            )
            .unwrap(),
        )
        .await
        .unwrap();
}

fn succeeded_image_task(origin: TestOrigin, asset_id: AssetId) -> GenerationTaskAggregate {
    let task_origin = GenerationTaskOrigin::new(
        origin.project_id,
        origin.workflow_id,
        origin.revision,
        origin.run_id,
        origin.node_id,
        origin.execution_id,
        contract_ref(),
    );
    let mut task = GenerationTaskAggregate::create(
        GenerationTaskId::from_uuid(uuid(70)).unwrap(),
        task_origin,
        GenerationTaskIdempotencyKey::try_new("completion-test").unwrap(),
        GenerationTaskTarget::new(
            profile(),
            GenerationProviderId::try_new("mock").unwrap(),
            GenerationProviderRouteId::try_new("mock.image.high-quality-general.v1").unwrap(),
        ),
        GenerationTaskRequest::Image(ImageGenerationSpec::new(
            GenerationTaskText::try_new("draw").unwrap(),
            tasks::generation_task::ImageAspectRatio::Square,
        )),
        task_time(10),
        task_time(100),
    )
    .unwrap();
    task.begin_submission(task_time(11)).unwrap();
    task.complete(
        GenerationTaskResult::Asset(GenerationTaskAssetResult::new(
            asset_id,
            assets::asset::domain::AssetMediaKind::Image,
        )),
        task_time(12),
    )
    .unwrap();
    task
}

fn available_image_asset(project_id: ProjectId, seed: u8, digest: [u8; 32]) -> AssetAggregate {
    let digest = AssetContentDigest::from_bytes(digest);
    let descriptor = AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(digest),
        digest,
        10,
        AssetMediaMimeType::ImagePng,
        assets::asset::domain::AssetMediaKind::Image,
    )
    .unwrap();
    AssetAggregate::try_restore(
        AssetId::from_uuid(uuid(seed)).unwrap(),
        project_id,
        assets::asset::domain::AssetMediaKind::Image,
        AssetManagedContentState::Available { descriptor },
        AssetMediaFacts::try_image(10, 10).unwrap(),
        AssetOrigin::imported(
            AssetImportId::from_uuid(uuid(seed + 1)).unwrap(),
            AssetOriginalFileName::try_new("generated.png").unwrap(),
        ),
        AssetDisplayName::try_new("Generated Image").unwrap(),
        AssetCreatedAt::from_utc_milliseconds(1).unwrap(),
    )
    .unwrap()
}

fn profile() -> GenerationProfileRef {
    GenerationProfileRef::new(
        GenerationProfileId::try_new("image.high_quality_general").unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    )
}
fn contract_ref() -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new("image.generate_from_text").unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
fn workflow_time(value: i64) -> WorkflowRunTime {
    WorkflowRunTime::from_utc_milliseconds(value).unwrap()
}
fn task_time(value: i64) -> GenerationTaskTimestamp {
    GenerationTaskTimestamp::from_utc_milliseconds(value).unwrap()
}
fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
