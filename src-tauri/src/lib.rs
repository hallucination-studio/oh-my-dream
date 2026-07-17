#![forbid(unsafe_code)]

pub mod asset_adapters;
pub mod asset_command_dto;
pub mod asset_commands;
pub mod asset_import_source_picker;
pub mod asset_preview_protocol;
pub mod asset_storage_adapters;
pub mod assistant_adapters;
pub mod assistant_command_dto;
pub mod assistant_commands_v5;
pub mod assistant_model_runner;
pub mod assistant_presentation;
pub mod assistant_process_command;
pub mod assistant_reviewer_protocol;
pub mod assistant_tool_runtime;
pub mod assistant_workflow_bridge;
pub mod backend_settings_adapter;
pub mod composition;
pub mod credential_repository;
pub mod desktop_backend_config;
pub mod desktop_bridges;
pub mod desktop_node_capability_asset_bridge;
pub mod generation_provider_settings_commands;
pub mod generation_task_asset_sink_adapter;
pub mod generation_task_command_dto;
pub mod generation_task_commands;
pub mod generation_task_effect_worker;
pub mod generation_task_origin_state_adapter;
pub mod generation_task_start_adapter;
pub mod generation_task_storage_adapter;
pub mod generation_task_workflow_completion_adapter;
pub(crate) mod metadata_sqlite;
pub mod node_capability_commands;
pub mod post_commit_effect;
pub mod post_commit_worker;
pub mod project_adapters;
pub mod project_commands;
pub mod workflow_adapters;
pub mod workflow_command_dto;
pub mod workflow_commands;
pub mod workflow_mutation_commands;
pub mod workflow_presentation_dto;
pub mod workflow_readiness_dto;
pub mod workflow_run_event_publisher;
pub mod workflow_storage_adapters;

use asset_commands::{asset_get, asset_import, asset_issue_preview, asset_list};
use assistant_commands_v5::{
    assistant_decide_workflow_change, assistant_get_pending_workflow_change, assistant_send_message,
};
use generation_provider_settings_commands::{
    generation_provider_settings_apply, generation_provider_settings_get,
};
use generation_task_commands::{generation_task_get, generation_task_list};
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
    let runtime = std::sync::Arc::new(std::sync::Mutex::new(None));
    let setup_runtime = runtime.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol(
            "desktop-asset",
            |context, request, responder| {
                let protocol = context
                    .app_handle()
                    .state::<composition::DesktopActivatedCommandDependencies>()
                    .asset_preview_protocol
                    .clone();
                let request = asset_preview_protocol::tauri_request(&request);
                tauri::async_runtime::spawn(async move {
                    responder.respond(asset_preview_protocol::tauri_response(
                        protocol.handle(request).await,
                    ));
                });
            },
        )
        .setup(move |app| {
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
                    std::sync::Arc::new(
                        asset_import_source_picker::TauriDesktopAssetImportSourcePickerAdapterImpl::new(
                            app.handle().clone(),
                        ),
                    ),
                ),
            )
            .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
            let worker = project_commands.post_commit_worker.clone();
            let task_worker = project_commands.generation_task_effect_worker.clone();
            app.manage(project_commands);
            let post_join = tauri::async_runtime::spawn(run_post_commit_worker(worker.clone()));
            let task_join =
                tauri::async_runtime::spawn(run_generation_task_effect_worker(task_worker.clone()));
            *setup_runtime.lock().map_err(|_| "worker runtime lock failed")? = Some(
                DesktopWorkerRuntime { post_worker: worker, task_worker, post_join, task_join },
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            assistant_send_message,
            assistant_get_pending_workflow_change,
            assistant_decide_workflow_change,
            asset_import,
            asset_get,
            asset_list,
            asset_issue_preview,
            project_create,
            project_rename,
            project_get,
            project_list,
            project_open,
            node_capability_list,
            generation_profile_list_for_capability,
            generation_provider_settings_get,
            generation_provider_settings_apply,
            generation_task_get,
            generation_task_list,
            workflow_create,
            workflow_get_current,
            workflow_apply_mutation,
            workflow_check_readiness,
            workflow_start_run,
            workflow_cancel_run,
            workflow_get_run,
            workflow_list_run_events,
            workflow_get_node_presentation,
        ])
        .build(tauri::generate_context!())?;
    let exit_runtime = runtime.clone();
    app.run(move |_, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. })
            && let Ok(locked) = exit_runtime.lock()
            && let Some(runtime) = locked.as_ref()
        {
            runtime.post_worker.cancel();
            runtime.task_worker.cancel();
        }
    });
    if let Some(runtime) = runtime.lock().ok().and_then(|mut value| value.take()) {
        tauri::async_runtime::block_on(async {
            if let Err(error) = runtime.post_join.await {
                tracing::error!(?error, "Desktop post-commit worker join failed");
            }
            if let Err(error) = runtime.task_join.await {
                tracing::error!(?error, "Generation Task effect worker join failed");
            }
        });
    }
    Ok(())
}

struct DesktopWorkerRuntime {
    post_worker: post_commit_worker::DesktopPostCommitEffectWorker,
    task_worker: composition::DesktopTaskWorker,
    post_join: tauri::async_runtime::JoinHandle<()>,
    task_join: tauri::async_runtime::JoinHandle<()>,
}

async fn run_generation_task_effect_worker(worker: composition::DesktopTaskWorker) {
    use generation_task_effect_worker::GenerationTaskEffectWorkerStep;
    while !worker.is_cancelled() {
        match worker.run_effect_batch().await {
            Ok(steps) if steps.iter().all(|step| *step == GenerationTaskEffectWorkerStep::Idle) => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Ok(_) => {}
            Err(error) => {
                tracing::error!(?error, "Generation Task effect worker iteration failed");
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        }
    }
}

async fn run_post_commit_worker(worker: post_commit_worker::DesktopPostCommitEffectWorker) {
    use post_commit_worker::DesktopPostCommitWorkerStep;
    while !worker.is_cancelled() {
        match worker.run_effect_batch().await {
            Ok(steps) if steps.iter().all(|step| *step == DesktopPostCommitWorkerStep::Idle) => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Ok(_) => {}
            Err(error) => {
                tracing::error!(?error, "Desktop post-commit worker iteration failed");
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        }
    }
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
