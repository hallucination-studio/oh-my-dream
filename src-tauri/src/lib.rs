#![forbid(unsafe_code)]

pub mod commands;
pub mod dto;
pub mod state;

use commands::{
    assets_root, create_project, get_asset, get_providers, list_assets, list_projects,
    load_workflow, open_project, run_workflow, save_workflow, set_active_provider,
    set_provider_key,
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
            set_provider_key
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
