use std::sync::{Arc, Mutex};

use engine::workflow::WorkflowGetCurrentUseCase;
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
    backend_settings_adapter::SqliteDesktopBackendSettingsAdapterImpl,
    desktop_backend_config::DesktopBackendConfigRepositoryInterface,
    desktop_bridges::DesktopProjectWorkflowBridgeAdapterImpl,
    metadata_sqlite::{metadata_sqlite_path, open_metadata_sqlite},
    post_commit_effect::SqliteDesktopPostCommitEffectOutboxAdapterImpl,
    project_adapters::{
        SqliteProjectRepositoryAdapterImpl, SystemProjectClockAdapterImpl,
        UuidProjectIdentityGeneratorAdapterImpl,
    },
    workflow_storage_adapters::SqliteWorkflowRunRepositoryAdapterImpl,
};

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
    /// Compatible Generation Profile list and availability boundary.
    pub generation_profile_list: Arc<GenerationProfileListForCapabilityUseCase>,
    _metadata_connection: Arc<Mutex<Connection>>,
}

pub(super) async fn compose(
    paths: DesktopApplicationPaths,
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
    SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(connection.clone())
        .map_err(|_| DesktopCompositionError::Metadata)?;
    let node_composition = node_capabilities::compose_node_capabilities(
        connection.clone(),
        paths.managed_content_root,
        paths.media_inspector_executable,
        settings,
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
    Ok(dependencies(connection, project_repository, workflow_repository, node_composition))
}

fn dependencies(
    connection: Arc<Mutex<Connection>>,
    repository: Arc<SqliteProjectRepositoryAdapterImpl>,
    workflow_repository: Arc<SqliteWorkflowRunRepositoryAdapterImpl>,
    node_composition: node_capabilities::DesktopNodeCapabilityComposition,
) -> DesktopActivatedCommandDependencies {
    let repository_interface: Arc<dyn ProjectRepositoryInterface> = repository;
    let clock = Arc::new(SystemProjectClockAdapterImpl);
    let get_current = Arc::new(WorkflowGetCurrentUseCase::new(workflow_repository));
    let summary: Arc<dyn ProjectWorkflowSummaryReaderInterface> =
        Arc::new(DesktopProjectWorkflowBridgeAdapterImpl::new(
            get_current,
            node_composition.registry.clone(),
        ));
    DesktopActivatedCommandDependencies {
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
        generation_profile_list: Arc::new(GenerationProfileListForCapabilityUseCase::new(
            node_composition.registry,
            node_composition.catalog,
            node_composition.availability_reader,
        )),
        _metadata_connection: connection,
    }
}
