#![forbid(unsafe_code)]

pub mod asset_adapters;
pub mod asset_preview_protocol;
mod asset_reference_adapter;
pub mod asset_storage_adapters;
pub mod assistant;
pub mod assistant_adapters;
pub mod assistant_approval;
pub mod assistant_commands;
pub mod assistant_model_runner;
pub mod assistant_operations;
pub mod assistant_repair;
mod assistant_review_bridge;
pub mod assistant_runtime;
pub mod assistant_sidecar;
pub mod assistant_transport;
pub mod assistant_workflow_bridge;
pub mod backend_settings_adapter;
pub mod capability_catalog;
pub mod capability_discovery;
mod command_error;
pub mod commands;
pub mod composition;
pub mod credential_repository;
pub mod desktop_backend_config;
pub mod desktop_bridges;
pub mod desktop_node_capability_asset_bridge;
pub mod dto;
mod managed_asset_access;
pub(crate) mod metadata_sqlite;
mod mock_generation;
pub mod node_capability_commands;
pub mod post_commit_effect;
pub mod post_commit_worker;
pub mod production_plan;
pub mod project_adapters;
pub mod project_commands;
pub mod provider_adapters;
pub mod reviewed_change;
pub mod state;
pub mod workflow_adapters;
pub mod workflow_authority;
pub mod workflow_command_dto;
pub mod workflow_commands;
pub mod workflow_mutation_commands;
pub mod workflow_patch_operation;
pub mod workflow_presentation_dto;
pub mod workflow_readiness_dto;
mod workflow_repository;
pub mod workflow_run_dto;
pub mod workflow_run_event_publisher;
pub mod workflow_runs;
pub mod workflow_storage_adapters;
pub mod workspace_snapshot;

use commands::{
    assets_root, assistant_decide_approval, assistant_get_pending_approval, assistant_send,
    get_asset, get_assistant_config, get_capability_catalog, get_providers, list_assets,
    set_active_provider, set_assistant_config, set_provider_key,
};
use node_capability_commands::{generation_profile_list_for_capability, node_capability_list};
use project_commands::{project_create, project_get, project_list, project_open, project_rename};
use tauri::Manager;
use workflow_commands::{
    workflow_cancel_run, workflow_check_readiness, workflow_create, workflow_get_current,
    workflow_get_node_presentation, workflow_get_run, workflow_list_run_events, workflow_start_run,
};
use workflow_mutation_commands::workflow_apply_mutation;

/// Runs the Tauri application.
pub fn run() -> tauri::Result<()> {
    init_logging();
    tauri::Builder::default()
        .setup(|app| {
            let app_data_root = app
                .handle()
                .path()
                .app_data_dir()
                .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
            let project_commands = tauri::async_runtime::block_on(
                composition::DesktopCompositionRoot::compose_activated_commands_with_emitter(
                    composition::DesktopApplicationPaths::from_application_data_root(
                        &app_data_root,
                    ),
                    std::sync::Arc::new(
                        workflow_run_event_publisher::TauriAppHandleEventEmitterAdapterImpl::new(
                            app.handle().clone(),
                        ),
                    ),
                ),
            )
            .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
            let state = state::AppState::from_app_handle(app.handle())
                .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
            app.manage(state);
            app.manage(project_commands);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            assistant_send,
            assistant_get_pending_approval,
            assistant_decide_approval,
            list_assets,
            get_asset,
            assets_root,
            project_create,
            project_rename,
            project_get,
            project_list,
            project_open,
            node_capability_list,
            generation_profile_list_for_capability,
            workflow_create,
            workflow_get_current,
            workflow_apply_mutation,
            workflow_check_readiness,
            workflow_start_run,
            workflow_cancel_run,
            workflow_get_run,
            workflow_list_run_events,
            workflow_get_node_presentation,
            get_providers,
            set_active_provider,
            set_provider_key,
            get_capability_catalog,
            get_assistant_config,
            set_assistant_config,
        ])
        .run(tauri::generate_context!())
}

fn init_logging() {
    match tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
    {
        Ok(()) => {}
        Err(error) => eprintln!("tracing subscriber initialization skipped: {error}"),
    }
}
