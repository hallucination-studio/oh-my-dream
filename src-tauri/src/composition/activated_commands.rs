use std::sync::{Arc, Mutex};

use assets::asset::application::{
    AssetGetUseCase, AssetImportUseCase, AssetIssuePreviewUseCase, AssetListUseCase,
};
use engine::node_capability::WorkflowNodeCapabilityRegistry;
use engine::workflow::{
    WorkflowApplyMutationUseCase, WorkflowCancelRunUseCase, WorkflowCheckReadinessUseCase,
    WorkflowCreateUseCase, WorkflowExecutionCancellationRegistry, WorkflowGetCurrentUseCase,
    WorkflowGetNodePresentationUseCase, WorkflowGetRunUseCase, WorkflowListRunEventsUseCase,
    WorkflowStartRunUseCase,
};
use nodes::{GenerationProfileListForCapabilityUseCase, NodeCapabilityListUseCase};
use projects::project::{
    application::{
        ProjectCreateUseCase, ProjectGetUseCase, ProjectListUseCase, ProjectOpenUseCase,
        ProjectRenameUseCase,
    },
    interfaces::{ProjectRepositoryInterface, ProjectWorkflowSummaryReaderInterface},
};
use rusqlite::Connection;

use super::{DesktopApplicationPaths, DesktopCompositionError, node_capabilities};
use crate::{
    asset_import_source_picker::DesktopAssetImportSourcePickerInterface,
    asset_preview_protocol::DesktopAssetPreviewProtocolAdapterImpl,
    backend_settings_adapter::SqliteDesktopBackendSettingsAdapterImpl,
    desktop_backend_config::DesktopBackendConfigRepositoryInterface,
    desktop_bridges::{
        DesktopProjectWorkflowBridgeAdapterImpl, DesktopWorkflowMediaPreviewAdapterImpl,
    },
    metadata_sqlite::{metadata_sqlite_path, open_metadata_sqlite},
    post_commit_effect::{
        DesktopApplicationInstanceId, SqliteDesktopPostCommitEffectOutboxAdapterImpl,
    },
    post_commit_worker::{
        DesktopCommittedWorkflowEventDeliveryAdapterImpl,
        DesktopPostCommitEffectExecutorAdapterImpl, DesktopPostCommitEffectWorker,
        DesktopStartupRecovery, DesktopWorkflowRestartRecoveryAdapterImpl,
        SystemDesktopPostCommitWorkerClockAdapterImpl,
    },
    project_adapters::{
        SqliteProjectRepositoryAdapterImpl, SystemProjectClockAdapterImpl,
        UuidProjectIdentityGeneratorAdapterImpl,
    },
    workflow_adapters::{
        SystemWorkflowClockAdapterImpl, UuidV4WorkflowIdentityGeneratorAdapterImpl,
    },
    workflow_run_event_publisher::{
        DesktopEventEmitterInterface, TauriWorkflowRunEventPublisherAdapterImpl,
    },
    workflow_storage_adapters::SqliteWorkflowRunRepositoryAdapterImpl,
};

type WorkflowRepository = SqliteWorkflowRunRepositoryAdapterImpl;
type WorkflowClock = SystemWorkflowClockAdapterImpl;
type WorkflowIdentities = UuidV4WorkflowIdentityGeneratorAdapterImpl;
type WorkflowPublisher = TauriWorkflowRunEventPublisherAdapterImpl;

/// Typed dependencies for the currently activated command entry points.
pub struct DesktopActivatedCommandDependencies {
    /// Project creation application boundary.
    pub create: Arc<ProjectCreateUseCase>,
    /// Project rename application boundary.
    pub rename: Arc<ProjectRenameUseCase>,
    /// Exact Project query boundary.
    pub get: Arc<ProjectGetUseCase>,
    /// Stable Project list boundary.
    pub list: Arc<ProjectListUseCase>,
    /// Project plus same-snapshot current-Workflow summary boundary.
    pub open: Arc<ProjectOpenUseCase>,
    /// Exact-seven Node Capability list boundary.
    pub node_capability_list: Arc<NodeCapabilityListUseCase>,
    /// Same immutable registry used for same-snapshot Workflow readiness projections.
    pub node_capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    /// Compatible Generation Profile list and availability boundary.
    pub generation_profile_list: Arc<GenerationProfileListForCapabilityUseCase>,
    /// Idempotent current Workflow creation boundary.
    pub workflow_create:
        Arc<WorkflowCreateUseCase<WorkflowRepository, WorkflowClock, WorkflowIdentities>>,
    /// Current Workflow query boundary.
    pub workflow_get_current: Arc<WorkflowGetCurrentUseCase<WorkflowRepository>>,
    /// Canonical ten-action mutation boundary.
    pub workflow_apply_mutation:
        Arc<WorkflowApplyMutationUseCase<WorkflowRepository, WorkflowClock>>,
    /// Authoritative readiness boundary.
    pub workflow_check_readiness: Arc<WorkflowCheckReadinessUseCase<WorkflowRepository>>,
    /// Durable Run admission boundary.
    pub workflow_start_run: Arc<
        WorkflowStartRunUseCase<
            WorkflowRepository,
            WorkflowRepository,
            WorkflowClock,
            WorkflowIdentities,
        >,
    >,
    /// Durable Run cancellation boundary.
    pub workflow_cancel_run:
        Arc<WorkflowCancelRunUseCase<WorkflowRepository, WorkflowClock, WorkflowPublisher>>,
    /// Project-scoped Run query boundary.
    pub workflow_get_run: Arc<WorkflowGetRunUseCase<WorkflowRepository>>,
    /// Bounded durable Run event query boundary.
    pub workflow_list_run_events: Arc<WorkflowListRunEventsUseCase<WorkflowRepository>>,
    /// Current four-shell node presentation boundary.
    pub workflow_get_node_presentation: Arc<
        WorkflowGetNodePresentationUseCase<
            WorkflowRepository,
            WorkflowRepository,
            DesktopWorkflowMediaPreviewAdapterImpl,
        >,
    >,
    /// Trusted local Asset import boundary.
    pub asset_import: Arc<AssetImportUseCase>,
    /// Project-scoped Asset query boundary.
    pub asset_get: Arc<AssetGetUseCase>,
    /// Stable bounded Project Asset page boundary.
    pub asset_list: Arc<AssetListUseCase>,
    /// Five-minute Asset preview permission boundary.
    pub asset_issue_preview: Arc<AssetIssuePreviewUseCase>,
    /// Process-keyed signed preview URI adapter.
    pub asset_preview_protocol: Arc<DesktopAssetPreviewProtocolAdapterImpl>,
    /// Native file selection and already-open source boundary.
    pub asset_import_source_picker: Arc<dyn DesktopAssetImportSourcePickerInterface>,
    /// Canonical Assistant command boundary.
    pub assistant: Arc<dyn crate::assistant_commands_v5::DesktopAssistantCommandInterface>,
    /// Closed durable effect worker shared by Workflow, Asset, and Assistant.
    pub post_commit_worker: DesktopPostCommitEffectWorker,
    _metadata_connection: Arc<Mutex<Connection>>,
}

pub(super) async fn compose(
    paths: DesktopApplicationPaths,
    emitter: Arc<dyn DesktopEventEmitterInterface>,
    asset_import_source_picker: Arc<dyn DesktopAssetImportSourcePickerInterface>,
) -> Result<DesktopActivatedCommandDependencies, DesktopCompositionError> {
    std::fs::create_dir_all(&paths.config_root).map_err(|_| DesktopCompositionError::Metadata)?;
    let connection = Arc::new(Mutex::new(
        open_metadata_sqlite(&metadata_sqlite_path(&paths.config_root))
            .map_err(|_| DesktopCompositionError::Metadata)?,
    ));
    let settings = Arc::new(
        SqliteDesktopBackendSettingsAdapterImpl::try_new(connection.clone())
            .map_err(|_| DesktopCompositionError::Config)?,
    );
    let config = settings
        .load_or_initialize_desktop_backend_config()
        .await
        .map_err(|_| DesktopCompositionError::Config)?;
    let outbox = Arc::new(
        SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(connection.clone())
            .map_err(|_| DesktopCompositionError::Metadata)?,
    );
    let node_composition = node_capabilities::compose_node_capabilities(
        connection.clone(),
        paths.managed_content_root,
        paths.media_inspector_executable,
        settings.clone(),
        &config,
    )?;
    let workflow_repository = Arc::new(
        SqliteWorkflowRunRepositoryAdapterImpl::try_new(
            connection.clone(),
            node_composition.registry.clone(),
        )
        .map_err(|_| DesktopCompositionError::Metadata)?,
    );
    let project_repository = Arc::new(
        SqliteProjectRepositoryAdapterImpl::try_new(connection.clone())
            .map_err(|_| DesktopCompositionError::Metadata)?,
    );
    let assistant = super::assistant::compose(
        connection.clone(),
        &paths.config_root,
        &config,
        settings,
        workflow_repository.clone(),
        &node_composition,
        emitter.clone(),
    )?;
    dependencies(DependencyInputs {
        connection,
        repository: project_repository,
        workflow_repository,
        nodes: node_composition,
        emitter,
        asset_import_source_picker,
        assistant,
        outbox,
        config,
    })
    .await
}

struct DependencyInputs {
    connection: Arc<Mutex<Connection>>,
    repository: Arc<SqliteProjectRepositoryAdapterImpl>,
    workflow_repository: Arc<SqliteWorkflowRunRepositoryAdapterImpl>,
    nodes: node_capabilities::DesktopNodeCapabilityComposition,
    emitter: Arc<dyn DesktopEventEmitterInterface>,
    asset_import_source_picker: Arc<dyn DesktopAssetImportSourcePickerInterface>,
    assistant: super::assistant::DesktopAssistantComposition,
    outbox: Arc<SqliteDesktopPostCommitEffectOutboxAdapterImpl>,
    config: crate::desktop_backend_config::DesktopBackendConfig,
}

async fn dependencies(
    input: DependencyInputs,
) -> Result<DesktopActivatedCommandDependencies, DesktopCompositionError> {
    let DependencyInputs {
        connection,
        repository,
        workflow_repository,
        nodes: node_composition,
        emitter,
        asset_import_source_picker,
        assistant,
        outbox,
        config,
    } = input;
    let repository_interface: Arc<dyn ProjectRepositoryInterface> = repository;
    let clock = Arc::new(SystemProjectClockAdapterImpl);
    let get_current = Arc::new(WorkflowGetCurrentUseCase::new(workflow_repository.clone()));
    let summary: Arc<dyn ProjectWorkflowSummaryReaderInterface> =
        Arc::new(DesktopProjectWorkflowBridgeAdapterImpl::new(
            get_current,
            node_composition.registry.clone(),
        ));
    let workflow_clock = Arc::new(SystemWorkflowClockAdapterImpl);
    let workflow_identities = Arc::new(UuidV4WorkflowIdentityGeneratorAdapterImpl);
    let workflow_publisher = Arc::new(TauriWorkflowRunEventPublisherAdapterImpl::new(emitter));
    let cancellations = Arc::new(WorkflowExecutionCancellationRegistry::default());
    let workflow_executor = Arc::new(
        engine::workflow::WorkflowExecuteRunUseCase::try_new(
            workflow_repository.clone(),
            workflow_clock.clone(),
            workflow_publisher.clone(),
            node_composition.registry.clone(),
            cancellations.clone(),
            usize::from(config.workflow_node_concurrency),
        )
        .map_err(|_| DesktopCompositionError::Business)?,
    );
    let executor = Arc::new(DesktopPostCommitEffectExecutorAdapterImpl::new(
        workflow_executor.clone(),
        node_composition.asset_finalizer.clone(),
        assistant.effect_executor,
    ));
    let worker_clock = Arc::new(SystemDesktopPostCommitWorkerClockAdapterImpl);
    let delivery = Arc::new(DesktopCommittedWorkflowEventDeliveryAdapterImpl::new(
        workflow_repository.clone(),
        workflow_publisher.clone(),
    ));
    let instance_id = DesktopApplicationInstanceId::from_uuid(uuid::Uuid::new_v4())
        .map_err(|_| DesktopCompositionError::Worker)?;
    DesktopStartupRecovery::new(
        instance_id,
        outbox.clone(),
        Arc::new(DesktopWorkflowRestartRecoveryAdapterImpl::new(
            workflow_repository.clone(),
            workflow_executor,
        )),
        worker_clock.clone(),
    )
    .recover_before_accepting_commands()
    .await
    .map_err(|_| DesktopCompositionError::StartupRecovery)?;
    let worker = DesktopPostCommitEffectWorker::try_new(
        instance_id,
        outbox,
        executor,
        delivery,
        worker_clock,
        usize::from(config.post_commit_effect_concurrency),
    )
    .map_err(|_| DesktopCompositionError::Worker)?;
    Ok(DesktopActivatedCommandDependencies {
        create: Arc::new(ProjectCreateUseCase::new(
            repository_interface.clone(),
            clock.clone(),
            Arc::new(UuidProjectIdentityGeneratorAdapterImpl),
        )),
        rename: Arc::new(ProjectRenameUseCase::new(repository_interface.clone(), clock)),
        get: Arc::new(ProjectGetUseCase::new(repository_interface.clone())),
        list: Arc::new(ProjectListUseCase::new(repository_interface.clone())),
        open: Arc::new(ProjectOpenUseCase::new(repository_interface, summary)),
        node_capability_list: Arc::new(NodeCapabilityListUseCase::new(
            node_composition.registry.clone(),
        )),
        node_capabilities: node_composition.registry.clone(),
        generation_profile_list: Arc::new(GenerationProfileListForCapabilityUseCase::new(
            node_composition.registry.clone(),
            node_composition.catalog,
            node_composition.availability_reader,
        )),
        workflow_create: Arc::new(WorkflowCreateUseCase::new(
            workflow_repository.clone(),
            workflow_clock.clone(),
            workflow_identities.clone(),
            node_composition.registry.clone(),
        )),
        workflow_get_current: Arc::new(WorkflowGetCurrentUseCase::new(workflow_repository.clone())),
        workflow_apply_mutation: Arc::new(WorkflowApplyMutationUseCase::new(
            workflow_repository.clone(),
            workflow_clock.clone(),
            node_composition.registry.clone(),
        )),
        workflow_check_readiness: Arc::new(WorkflowCheckReadinessUseCase::new(
            workflow_repository.clone(),
            node_composition.registry.clone(),
        )),
        workflow_start_run: Arc::new(WorkflowStartRunUseCase::new(
            workflow_repository.clone(),
            workflow_repository.clone(),
            workflow_clock.clone(),
            workflow_identities,
            node_composition.registry.clone(),
        )),
        workflow_cancel_run: Arc::new(WorkflowCancelRunUseCase::new(
            workflow_repository.clone(),
            workflow_clock,
            workflow_publisher,
            cancellations,
        )),
        workflow_get_run: Arc::new(WorkflowGetRunUseCase::new(workflow_repository.clone())),
        workflow_list_run_events: Arc::new(WorkflowListRunEventsUseCase::new(
            workflow_repository.clone(),
        )),
        workflow_get_node_presentation: Arc::new(WorkflowGetNodePresentationUseCase::new(
            workflow_repository.clone(),
            workflow_repository,
            node_composition.preview_issuer,
            node_composition.registry,
        )),
        asset_import: node_composition.asset_import,
        asset_get: node_composition.asset_get,
        asset_list: node_composition.asset_list,
        asset_issue_preview: node_composition.asset_issue_preview,
        asset_preview_protocol: node_composition.asset_preview_protocol,
        asset_import_source_picker,
        assistant: assistant.commands,
        post_commit_worker: worker,
        _metadata_connection: connection,
    })
}
