//! Sole Desktop construction root and typed application host.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use engine::{
    node_capability::WorkflowNodeCapabilityRegistry, workflow::WorkflowRunEventPublisherInterface,
};
use rusqlite::Connection;
use uuid::Uuid;

use crate::{
    backend_settings_adapter::SqliteDesktopBackendSettingsAdapterImpl,
    credential_repository::{
        AssistantModelCredentialId, AssistantModelCredentialRepositoryError,
        AssistantModelCredentialRepositoryInterface,
    },
    desktop_backend_config::{DesktopBackendConfig, DesktopBackendConfigRepositoryInterface},
    metadata_sqlite::{metadata_sqlite_path, open_metadata_sqlite},
    post_commit_effect::{
        DesktopApplicationInstanceId, SqliteDesktopPostCommitEffectOutboxAdapterImpl,
    },
    post_commit_worker::{
        DesktopCommittedWorkflowEventDeliveryAdapterImpl, DesktopPostCommitEffectExecutorInterface,
        DesktopPostCommitEffectWorker, DesktopStartupRecovery,
        DesktopWorkflowRestartRecoveryAdapterImpl, DesktopWorkflowRunRestartInterrupterInterface,
        SystemDesktopPostCommitWorkerClockAdapterImpl,
    },
    workflow_run_event_publisher::{
        DesktopEventEmitterInterface, TauriWorkflowRunEventPublisherAdapterImpl,
    },
    workflow_storage_adapters::SqliteWorkflowRunRepositoryAdapterImpl,
};

mod node_capabilities;

/// Filesystem locations derived from the operating-system application-data root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopApplicationPaths {
    /// Root containing the single private metadata database.
    pub config_root: PathBuf,
    /// Root for managed Asset content.
    pub managed_content_root: PathBuf,
    /// Private ffprobe executable selected by the application bundle.
    pub media_inspector_executable: PathBuf,
}

impl DesktopApplicationPaths {
    /// Derives both private roots without reading paths from backend configuration.
    #[must_use]
    pub fn from_application_data_root(root: &Path) -> Self {
        Self {
            config_root: root.join("config"),
            managed_content_root: root.join("assets"),
            media_inspector_executable: root.join("tools").join("ffprobe"),
        }
    }
}

/// Business graph constructed inside the root after repositories exist.
pub struct DesktopBusinessComposition {
    /// Closed three-effect dispatcher.
    pub post_commit_effect_executor: Arc<dyn DesktopPostCommitEffectExecutorInterface>,
    /// Workflow-owned restart transition entry point.
    pub workflow_restart_interrupter: Arc<dyn DesktopWorkflowRunRestartInterrupterInterface>,
}

/// Fully constructed process host passed only to Tauri entry points.
pub struct DesktopApplicationHost {
    /// Validated immutable startup configuration.
    pub config: DesktopBackendConfig,
    /// Shared config and isolated plaintext credential repository adapter.
    pub backend_settings: Arc<SqliteDesktopBackendSettingsAdapterImpl>,
    /// Exact-seven capability registry.
    pub node_capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    /// Workflow persistence used by typed command dependencies.
    pub workflow_repository: Arc<SqliteWorkflowRunRepositoryAdapterImpl>,
    /// Closed worker started after recovery succeeds.
    pub post_commit_worker: DesktopPostCommitEffectWorker,
    startup_recovery: DesktopStartupRecovery,
    assistant_commands_enabled: bool,
    _metadata_connection: Arc<Mutex<Connection>>,
}

impl DesktopApplicationHost {
    /// Completes ordered recovery before any command registration becomes reachable.
    pub async fn recover_before_accepting_commands(&self) -> Result<(), DesktopCompositionError> {
        self.startup_recovery
            .recover_before_accepting_commands()
            .await
            .map_err(|_| DesktopCompositionError::StartupRecovery)
    }

    /// Reports isolated Assistant command availability without retaining its secret.
    #[must_use]
    pub const fn assistant_commands_enabled(&self) -> bool {
        self.assistant_commands_enabled
    }
}

/// Construction or pre-command recovery failed.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum DesktopCompositionError {
    /// A private root or metadata database could not be initialized.
    #[error("Desktop metadata initialization failed")]
    Metadata,
    /// Backend configuration could not be loaded and validated.
    #[error("Desktop backend configuration failed")]
    Config,
    /// The supplied business graph could not be constructed.
    #[error("Desktop business composition failed")]
    Business,
    /// A process identity or worker bound was invalid.
    #[error("Desktop worker composition failed")]
    Worker,
    /// Startup recovery failed before command admission.
    #[error("Desktop startup recovery failed")]
    StartupRecovery,
}

/// The only module allowed to construct concrete Desktop adapters.
pub struct DesktopCompositionRoot;

struct DesktopInfrastructureComposition {
    connection: Arc<Mutex<Connection>>,
    settings: Arc<SqliteDesktopBackendSettingsAdapterImpl>,
    config: DesktopBackendConfig,
    workflow_repository: Arc<SqliteWorkflowRunRepositoryAdapterImpl>,
    outbox: Arc<SqliteDesktopPostCommitEffectOutboxAdapterImpl>,
    publisher: Arc<dyn WorkflowRunEventPublisherInterface>,
    node_capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

const EXACT_NODE_CAPABILITY_REFS: [&str; 7] = [
    "audio.read_asset@1.0",
    "audio.synthesize_speech_from_text@1.0",
    "image.generate_from_text@1.0",
    "image.read_asset@1.0",
    "text.provide_literal@1.0",
    "video.generate_from_image@1.0",
    "video.read_asset@1.0",
];

impl DesktopCompositionRoot {
    /// Builds the host while allowing tests to substitute a deterministic business graph.
    pub async fn compose_with_business(
        paths: DesktopApplicationPaths,
        emitter: Arc<dyn DesktopEventEmitterInterface>,
        build_business: impl FnOnce(
            Arc<SqliteWorkflowRunRepositoryAdapterImpl>,
            Arc<dyn WorkflowRunEventPublisherInterface>,
            &DesktopBackendConfig,
        )
            -> Result<DesktopBusinessComposition, DesktopCompositionError>,
    ) -> Result<DesktopApplicationHost, DesktopCompositionError> {
        std::fs::create_dir_all(&paths.config_root)
            .map_err(|_| DesktopCompositionError::Metadata)?;
        std::fs::create_dir_all(&paths.managed_content_root)
            .map_err(|_| DesktopCompositionError::Metadata)?;
        let connection = open_metadata_sqlite(&metadata_sqlite_path(&paths.config_root))
            .map_err(|_| DesktopCompositionError::Metadata)?;
        let connection = Arc::new(Mutex::new(connection));
        let settings = Arc::new(
            SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::clone(&connection))
                .map_err(|_| DesktopCompositionError::Config)?,
        );
        let config = settings
            .load_or_initialize_desktop_backend_config()
            .await
            .map_err(|_| DesktopCompositionError::Config)?;
        let outbox = Arc::new(
            SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(Arc::clone(&connection))
                .map_err(|_| DesktopCompositionError::Metadata)?,
        );
        let capabilities = node_capabilities::compose_node_capabilities(
            Arc::clone(&connection),
            paths.managed_content_root,
            paths.media_inspector_executable,
            Arc::clone(&settings),
            &config,
        )?;
        if !has_exact_node_capabilities(&capabilities) {
            return Err(DesktopCompositionError::Business);
        }
        let workflow_repository = Arc::new(
            SqliteWorkflowRunRepositoryAdapterImpl::try_new(
                Arc::clone(&connection),
                Arc::clone(&capabilities),
            )
            .map_err(|_| DesktopCompositionError::Metadata)?,
        );
        let publisher: Arc<dyn WorkflowRunEventPublisherInterface> =
            Arc::new(TauriWorkflowRunEventPublisherAdapterImpl::new(emitter));
        let business =
            build_business(Arc::clone(&workflow_repository), Arc::clone(&publisher), &config)?;
        Self::finish_host(
            DesktopInfrastructureComposition {
                connection,
                settings,
                config,
                workflow_repository,
                outbox,
                publisher,
                node_capabilities: capabilities,
            },
            business,
        )
        .await
    }

    async fn finish_host(
        infrastructure: DesktopInfrastructureComposition,
        business: DesktopBusinessComposition,
    ) -> Result<DesktopApplicationHost, DesktopCompositionError> {
        let DesktopInfrastructureComposition {
            connection,
            settings,
            config,
            workflow_repository,
            outbox,
            publisher,
            node_capabilities,
        } = infrastructure;
        let instance_id = DesktopApplicationInstanceId::from_uuid(Uuid::new_v4())
            .map_err(|_| DesktopCompositionError::Worker)?;
        let clock = Arc::new(SystemDesktopPostCommitWorkerClockAdapterImpl);
        let delivery = Arc::new(DesktopCommittedWorkflowEventDeliveryAdapterImpl::new(
            workflow_repository.clone(),
            publisher,
        ));
        let worker = DesktopPostCommitEffectWorker::try_new(
            instance_id,
            outbox.clone(),
            business.post_commit_effect_executor,
            delivery,
            clock.clone(),
            usize::from(config.post_commit_effect_concurrency),
        )
        .map_err(|_| DesktopCompositionError::Worker)?;
        let workflow_recovery = Arc::new(DesktopWorkflowRestartRecoveryAdapterImpl::new(
            workflow_repository.clone(),
            business.workflow_restart_interrupter,
        ));
        let startup_recovery =
            DesktopStartupRecovery::new(instance_id, outbox, workflow_recovery, clock);
        let assistant_commands_enabled =
            assistant_commands_enabled(&config, settings.as_ref()).await?;
        let host = DesktopApplicationHost {
            config,
            backend_settings: settings,
            node_capabilities,
            workflow_repository,
            post_commit_worker: worker,
            startup_recovery,
            assistant_commands_enabled,
            _metadata_connection: connection,
        };
        host.recover_before_accepting_commands().await?;
        Ok(host)
    }
}

fn has_exact_node_capabilities(registry: &WorkflowNodeCapabilityRegistry) -> bool {
    let actual = registry
        .list_node_capability_contracts()
        .into_iter()
        .map(|contract| contract.contract_ref().to_string())
        .collect::<BTreeSet<_>>();
    let expected = EXACT_NODE_CAPABILITY_REFS.into_iter().map(str::to_owned).collect();
    actual == expected
}

async fn assistant_commands_enabled(
    config: &DesktopBackendConfig,
    credentials: &dyn AssistantModelCredentialRepositoryInterface,
) -> Result<bool, DesktopCompositionError> {
    if !config.assistant_model.enabled {
        return Ok(false);
    }
    let Ok(id) = AssistantModelCredentialId::new(config.assistant_model.credential_id.clone())
    else {
        return Err(DesktopCompositionError::Config);
    };
    match credentials.load_assistant_model_credential(&id).await {
        Ok(secret) => {
            drop(secret);
            Ok(true)
        }
        Err(AssistantModelCredentialRepositoryError::NotFound) => Ok(false),
        Err(_) => Err(DesktopCompositionError::Config),
    }
}

#[cfg(test)]
mod tests;
