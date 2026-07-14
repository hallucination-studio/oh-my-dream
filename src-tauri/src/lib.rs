#![forbid(unsafe_code)]

pub mod assistant;
pub mod assistant_commands;
pub mod assistant_operations;
pub mod assistant_runtime;
pub mod assistant_sidecar;
pub mod assistant_transport;
pub mod capability_catalog;
pub mod capability_discovery;
mod command_error;
pub mod commands;
pub mod dto;
mod mock_generation;
pub mod state;
pub mod workflow_authority;
pub mod workflow_patch_operation;
mod workflow_repository;
pub mod workflow_run_commands;
pub mod workflow_run_dto;
pub mod workflow_runs;
pub mod workspace_snapshot;

use commands::{
    assets_root, assistant_send, cancel_workflow_run, create_project, get_asset,
    get_assistant_config, get_capability_bundles, get_capability_catalog, get_providers,
    list_assets, list_projects, open_project, run_workflow, search_capabilities,
    set_active_provider, set_assistant_config, set_provider_key, start_workflow_run,
    workflow_apply_patch,
};
use tauri::Manager;

/// Runs the Tauri application.
pub fn run() -> tauri::Result<()> {
    init_logging();
    tauri::Builder::default()
        .setup(|app| {
            let state = state::AppState::from_app_handle(app.handle())
                .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_workflow,
            assistant_send,
            start_workflow_run,
            cancel_workflow_run,
            list_assets,
            get_asset,
            assets_root,
            list_projects,
            create_project,
            open_project,
            workflow_apply_patch,
            get_providers,
            set_active_provider,
            set_provider_key,
            get_capability_catalog,
            search_capabilities,
            get_capability_bundles,
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
