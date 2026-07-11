#![forbid(unsafe_code)]

pub mod assistant;
pub mod assistant_capabilities;
pub mod assistant_sidecar;
pub mod commands;
pub mod dto;
mod mock_generation;
pub mod state;
pub mod workflow_runs;

use commands::{
    assets_root, create_project, get_asset, get_assistant_config, get_assistant_session,
    get_capability_manifest, get_providers, install_skill, list_assets, list_projects, list_skills,
    load_workflow, open_project, run_workflow, save_workflow, set_active_provider,
    set_assistant_config, set_provider_key, set_skill_enabled, uninstall_skill,
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
            list_assets,
            get_asset,
            assets_root,
            list_projects,
            create_project,
            open_project,
            save_workflow,
            load_workflow,
            get_providers,
            set_active_provider,
            set_provider_key,
            get_capability_manifest,
            get_assistant_config,
            set_assistant_config,
            get_assistant_session,
            list_skills,
            install_skill,
            set_skill_enabled,
            uninstall_skill
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
