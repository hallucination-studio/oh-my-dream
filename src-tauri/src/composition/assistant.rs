use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use assistant::application::{
    AssistantActiveInvocationRegistry, AssistantApplyWorkflowChangeEffectUseCase,
    AssistantDecideWorkflowChangeUseCase, AssistantGetPendingWorkflowChangeUseCase,
    AssistantReviewEvidenceRegistry, AssistantReviewWorkflowChangeUseCase,
    AssistantSendMessageUseCase, AssistantToolDispatcherImpl,
};
use engine::workflow::{
    WorkflowEvaluateMutationUseCase, WorkflowGetCurrentUseCase, WorkflowGetRunUseCase,
    WorkflowListActiveRunsUseCase, WorkflowListRunEventsUseCase,
};
use rusqlite::Connection;

use super::{DesktopCompositionError, node_capabilities::DesktopNodeCapabilityComposition};
use crate::{
    assistant_adapters::{
        LocalFilesystemAssistantModelContinuationStoreAdapterImpl,
        SqliteAssistantProductionPlanRepositoryAdapterImpl,
        SqliteAssistantWorkflowChangeRepositoryAdapterImpl, SystemAssistantClockAdapterImpl,
    },
    assistant_commands_v5::{DesktopAssistantCommandAdapterImpl, DesktopAssistantCommandInterface},
    assistant_model_runner::{
        DynamicAssistantSidecarProcessLauncherAdapterImpl,
        PythonAgentsAssistantModelRunnerAdapterImpl,
    },
    assistant_presentation::TauriAssistantPresentationEventPublisherAdapterImpl,
    assistant_process_command::configured_assistant_command,
    assistant_reviewer_protocol::DesktopAssistantReviewerProtocolAdapterImpl,
    assistant_tool_runtime::{
        DesktopAssistantProtocolToolExecutorAdapterImpl,
        DesktopAssistantToolExecutionContextFactoryAdapterImpl,
    },
    assistant_workflow_bridge::{
        DesktopAssistantWorkflowBridgeAdapterImpl, DesktopAssistantWorkspaceBridgeAdapterImpl,
    },
    backend_settings_adapter::SqliteDesktopBackendSettingsAdapterImpl,
    desktop_backend_config::DesktopBackendConfig,
    post_commit_worker::DesktopAssistantEffectExecutorInterface,
    workflow_adapters::{
        SystemWorkflowClockAdapterImpl, UuidV4WorkflowIdentityGeneratorAdapterImpl,
    },
    workflow_run_event_publisher::DesktopEventEmitterInterface,
    workflow_storage_adapters::SqliteWorkflowRunRepositoryAdapterImpl,
};

pub(super) struct DesktopAssistantComposition {
    pub commands: Arc<dyn DesktopAssistantCommandInterface>,
    pub effect_executor: Arc<dyn DesktopAssistantEffectExecutorInterface>,
}

type WorkflowRepository = SqliteWorkflowRunRepositoryAdapterImpl;
type WorkflowClock = SystemWorkflowClockAdapterImpl;
type WorkflowIdentities = UuidV4WorkflowIdentityGeneratorAdapterImpl;
type AssistantWorkflowBridge = DesktopAssistantWorkflowBridgeAdapterImpl<
    WorkflowRepository,
    WorkflowRepository,
    WorkflowClock,
    WorkflowIdentities,
>;
type AssistantWorkspaceBridge =
    DesktopAssistantWorkspaceBridgeAdapterImpl<WorkflowRepository, WorkflowRepository>;

pub(super) fn compose(
    connection: Arc<Mutex<Connection>>,
    config_root: &Path,
    config: &DesktopBackendConfig,
    settings: Arc<SqliteDesktopBackendSettingsAdapterImpl>,
    workflow_repository: Arc<SqliteWorkflowRunRepositoryAdapterImpl>,
    nodes: &DesktopNodeCapabilityComposition,
    emitter: Arc<dyn DesktopEventEmitterInterface>,
) -> Result<DesktopAssistantComposition, DesktopCompositionError> {
    let (workflow, workspace) = compose_bridges(workflow_repository, nodes);
    compose_application(connection, config_root, config, settings, emitter, workflow, workspace)
}

fn compose_bridges(
    workflow_repository: Arc<WorkflowRepository>,
    nodes: &DesktopNodeCapabilityComposition,
) -> (AssistantWorkflowBridge, AssistantWorkspaceBridge) {
    let workflow_clock = Arc::new(SystemWorkflowClockAdapterImpl);
    let workflow_identities = Arc::new(UuidV4WorkflowIdentityGeneratorAdapterImpl);
    let get_current = Arc::new(WorkflowGetCurrentUseCase::new(workflow_repository.clone()));
    let get_run = Arc::new(WorkflowGetRunUseCase::new(workflow_repository.clone()));
    let list_events = Arc::new(WorkflowListRunEventsUseCase::new(workflow_repository.clone()));
    let start_run = Arc::new(engine::workflow::WorkflowStartRunUseCase::new(
        workflow_repository.clone(),
        workflow_repository.clone(),
        workflow_clock.clone(),
        workflow_identities.clone(),
        nodes.registry.clone(),
    ));
    let workflow = DesktopAssistantWorkflowBridgeAdapterImpl::new(
        Arc::new(WorkflowEvaluateMutationUseCase::new(
            workflow_repository.clone(),
            workflow_clock,
            nodes.registry.clone(),
        )),
        Arc::new(engine::workflow::WorkflowApplyMutationUseCase::new(
            workflow_repository.clone(),
            Arc::new(SystemWorkflowClockAdapterImpl),
            nodes.registry.clone(),
        )),
        get_current.clone(),
        start_run,
        get_run,
        list_events,
    );
    let workspace = DesktopAssistantWorkspaceBridgeAdapterImpl::new(
        get_current,
        Arc::new(WorkflowListActiveRunsUseCase::new(workflow_repository)),
        nodes.asset_get.clone(),
        nodes.asset_list.clone(),
        Arc::new(nodes::NodeCapabilityListUseCase::new(nodes.registry.clone())),
        Arc::new(nodes::GenerationProfileListForCapabilityUseCase::new(
            nodes.registry.clone(),
            nodes.catalog.clone(),
            nodes.availability_reader.clone(),
        )),
    );
    (workflow, workspace)
}

fn compose_application(
    connection: Arc<Mutex<Connection>>,
    config_root: &Path,
    config: &DesktopBackendConfig,
    settings: Arc<SqliteDesktopBackendSettingsAdapterImpl>,
    emitter: Arc<dyn DesktopEventEmitterInterface>,
    workflow: AssistantWorkflowBridge,
    workspace: AssistantWorkspaceBridge,
) -> Result<DesktopAssistantComposition, DesktopCompositionError> {
    let changes = SqliteAssistantWorkflowChangeRepositoryAdapterImpl::try_new(connection.clone())
        .map_err(|_| DesktopCompositionError::Business)?;
    let plans = SqliteAssistantProductionPlanRepositoryAdapterImpl::try_new(connection)
        .map_err(|_| DesktopCompositionError::Business)?;
    let continuations = LocalFilesystemAssistantModelContinuationStoreAdapterImpl::try_new(
        config_root.join("assistant-continuations"),
    )
    .map_err(|_| DesktopCompositionError::Business)?;
    let dispatcher = AssistantToolDispatcherImpl::try_new(
        workspace.clone(),
        workspace.clone(),
        plans,
        workflow.clone(),
        changes.clone(),
    )
    .map_err(|_| DesktopCompositionError::Business)?;
    let reviewer = DesktopAssistantReviewerProtocolAdapterImpl::new(
        AssistantReviewWorkflowChangeUseCase::new(
            changes.clone(),
            SystemAssistantClockAdapterImpl,
            AssistantReviewEvidenceRegistry::default(),
        ),
        continuations.clone(),
    );
    let runner = PythonAgentsAssistantModelRunnerAdapterImpl::new(
        compose_launcher(settings)?,
        DesktopAssistantProtocolToolExecutorAdapterImpl::new(dispatcher),
        DesktopAssistantToolExecutionContextFactoryAdapterImpl::new(
            Arc::new(SystemAssistantClockAdapterImpl),
            config.assistant_protocol_budgets.approval_expiry_ms,
        ),
        TauriAssistantPresentationEventPublisherAdapterImpl::new(emitter),
        reviewer,
    );
    let active = AssistantActiveInvocationRegistry::default();
    let commands = Arc::new(DesktopAssistantCommandAdapterImpl::new(
        AssistantSendMessageUseCase::new(runner.clone(), workspace, active),
        AssistantGetPendingWorkflowChangeUseCase::new(changes.clone()),
        AssistantDecideWorkflowChangeUseCase::new(changes.clone(), continuations.clone()),
    ));
    let effect_executor = Arc::new(AssistantApplyWorkflowChangeEffectUseCase::new(
        changes,
        workflow.clone(),
        continuations,
        runner,
        workflow,
    ));
    Ok(DesktopAssistantComposition { commands, effect_executor })
}

fn compose_launcher(
    settings: Arc<SqliteDesktopBackendSettingsAdapterImpl>,
) -> Result<DynamicAssistantSidecarProcessLauncherAdapterImpl, DesktopCompositionError> {
    let command = configured_assistant_command().map_err(|_| DesktopCompositionError::Business)?;
    Ok(DynamicAssistantSidecarProcessLauncherAdapterImpl::new(command, settings))
}
