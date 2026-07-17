use std::{collections::BTreeMap, sync::Arc, time::Duration};

use engine::{
    node_capability::{
        NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
        NodeCapabilityGenerationProfileRefParameterValue, NodeCapabilityInputKey,
        NodeCapabilityOutputKey, NodeCapabilityParameterKey, NodeCapabilityParameterSet,
        NodeCapabilityParameterValue, WorkflowInputItemId, WorkflowRuntimeValue,
    },
    workflow::{
        WorkflowCreateCommand, WorkflowCreateRequestId, WorkflowRunEventPayload,
        WorkflowRunRequestId, WorkflowRunScope, WorkflowRunState, WorkflowStartRunCommand,
    },
    workflow_graph::{
        WorkflowAddNodeAction, WorkflowApplyMutationCommand, WorkflowBindSingleInputAction,
        WorkflowCanvasPosition, WorkflowInputItemEntity, WorkflowInputTarget,
        WorkflowMutationAction, WorkflowMutationRequestId, WorkflowNodeId,
    },
};
use oh_my_dream_tauri::{
    asset_import_source_picker::{
        DesktopAssetImportSourcePickerError, DesktopAssetImportSourcePickerInterface,
        DesktopPickedAssetImportSource,
    },
    composition::{
        DesktopActivatedCommandDependencies, DesktopApplicationPaths, DesktopCompositionRoot,
    },
    generation_task_effect_worker::GenerationTaskEffectWorkerStep,
    post_commit_worker::DesktopPostCommitWorkerStep,
    workflow_run_event_publisher::{DesktopEventEmissionError, DesktopEventEmitterInterface},
};
use projects::project::{
    application::{ProjectCreateRequest, ProjectMutationRequestId},
    domain::ProjectName,
};
use rusqlite::Connection;
use serde_json::Value;
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn chained_mock_workflow_restarts_without_duplicate_tasks_assets_or_completion() {
    tauri::async_runtime::block_on(async {
        let directory = tempdir().expect("directory");
        let mut paths = DesktopApplicationPaths::from_application_data_root(directory.path());
        paths.media_inspector_executable = find_executable("ffprobe");
        let dependencies = compose(paths.clone()).await;
        let (project_id, run_id, image_node_id, video_node_id) =
            admit_chained_workflow(&dependencies).await;

        progress_post_commit(&dependencies).await;
        progress_tasks(&dependencies).await;
        drop(dependencies);
        assert_eq!(running_task_count(directory.path()), 1);
        let dependencies = compose(paths).await;
        drive_until_terminal(&dependencies, project_id, run_id).await;

        let run = dependencies
            .workflow_get_run
            .get_workflow_run(project_id, run_id)
            .await
            .expect("completed run");
        assert_eq!(run.state(), WorkflowRunState::Succeeded);
        let image = node_output(&run, image_node_id, "image");
        let video = node_output(&run, video_node_id, "video");
        assert!(matches!(video, WorkflowRuntimeValue::Video(_)));
        let WorkflowRuntimeValue::Image(image) = image else { panic!("typed image output") };
        assert_eq!(
            run.events()
                .iter()
                .filter(|event| matches!(
                    event.payload(),
                    WorkflowRunEventPayload::WorkflowNodeSucceededEvent { .. }
                ))
                .count(),
            3
        );
        assert_eq!(
            run.events()
                .iter()
                .filter(|event| matches!(
                    event.payload(),
                    WorkflowRunEventPayload::WorkflowRunSucceededEvent
                ))
                .count(),
            1
        );

        drop(dependencies);
        let facts = task_facts(directory.path());
        assert_eq!(facts.image_tasks, 1);
        assert_eq!(facts.video_tasks, 1);
        assert_eq!(facts.image_assets, 1);
        assert_eq!(facts.video_assets, 1);
        assert_eq!(facts.completed_submit_effects, 2);
        assert_eq!(facts.video_input_asset_id, image.asset_id().as_bytes());
        assert_eq!(facts.video_input_digest, image.content_fingerprint().as_bytes());
    });
}

async fn admit_chained_workflow(
    dependencies: &DesktopActivatedCommandDependencies,
) -> (
    projects::project::domain::ProjectId,
    engine::node_capability::WorkflowRunId,
    WorkflowNodeId,
    WorkflowNodeId,
) {
    let project = dependencies
        .create
        .create_project(ProjectCreateRequest {
            request_id: ProjectMutationRequestId::from_uuid(id(1)).unwrap(),
            name: ProjectName::new("Chained Mock Workflow").unwrap(),
        })
        .await
        .expect("project");
    let workflow = dependencies
        .workflow_create
        .create_workflow(WorkflowCreateCommand::new(
            WorkflowCreateRequestId::from_uuid(id(2)).unwrap(),
            project.id(),
        ))
        .await
        .expect("workflow");
    let prompt = WorkflowNodeId::from_uuid(id(3)).unwrap();
    let image = WorkflowNodeId::from_uuid(id(4)).unwrap();
    let video = WorkflowNodeId::from_uuid(id(5)).unwrap();
    let actions = vec![
        add_node(prompt, "text.provide_literal", literal_parameters()),
        add_node(
            image,
            "image.generate_from_text",
            profile_parameters("image.high_quality_general"),
        ),
        add_node(
            video,
            "video.generate_from_image",
            profile_parameters("video.cinematic_image_animation"),
        ),
        bind(id(6), prompt, "text", image, "prompt"),
        bind(id(7), image, "image", video, "image"),
    ];
    let changed = dependencies
        .workflow_apply_mutation
        .apply_project_workflow_mutation(
            project.id(),
            WorkflowApplyMutationCommand::try_new(
                WorkflowMutationRequestId::from_uuid(id(8)).unwrap(),
                workflow.id,
                workflow.revision,
                actions,
            )
            .unwrap(),
        )
        .await
        .expect("chained workflow");
    let run = dependencies
        .workflow_start_run
        .start_project_workflow_run(
            project.id(),
            WorkflowStartRunCommand::new(
                WorkflowRunRequestId::from_uuid(id(9)).unwrap(),
                changed.workflow.id,
                changed.workflow.revision,
                WorkflowRunScope::WholeWorkflow,
            ),
        )
        .await
        .expect("run admission");
    (project.id(), run.run_id(), image, video)
}

async fn drive_until_terminal(
    dependencies: &DesktopActivatedCommandDependencies,
    project_id: projects::project::domain::ProjectId,
    run_id: engine::node_capability::WorkflowRunId,
) {
    for _ in 0..20 {
        progress_post_commit(dependencies).await;
        progress_tasks(dependencies).await;
        let run =
            dependencies.workflow_get_run.get_workflow_run(project_id, run_id).await.expect("run");
        if matches!(run.state(), WorkflowRunState::Succeeded | WorkflowRunState::Failed) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(550)).await;
    }
    panic!("chained Workflow did not become terminal");
}

async fn progress_post_commit(dependencies: &DesktopActivatedCommandDependencies) {
    let steps =
        dependencies.post_commit_worker.run_effect_batch().await.expect("post-commit batch");
    assert!(steps.iter().all(|step| matches!(
        step,
        DesktopPostCommitWorkerStep::Idle | DesktopPostCommitWorkerStep::Progressed
    )));
}

async fn progress_tasks(dependencies: &DesktopActivatedCommandDependencies) {
    let steps = dependencies
        .generation_task_effect_worker
        .run_effect_batch()
        .await
        .expect("Generation Task batch");
    assert!(steps.iter().all(|step| matches!(
        step,
        GenerationTaskEffectWorkerStep::Idle | GenerationTaskEffectWorkerStep::Progressed
    )));
}

fn add_node(
    node_id: WorkflowNodeId,
    capability_id: &str,
    parameter_set: NodeCapabilityParameterSet,
) -> WorkflowMutationAction {
    WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
        new_node_id: node_id,
        capability_contract: NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new(capability_id).unwrap(),
            NodeCapabilityContractVersion::new(1, 0).unwrap(),
        ),
        parameter_set,
        canvas_position: WorkflowCanvasPosition::try_new(0.0, 0.0).unwrap(),
    })
}

fn bind(
    item_id: Uuid,
    source_node_id: WorkflowNodeId,
    source_output: &str,
    target_node_id: WorkflowNodeId,
    target_input: &str,
) -> WorkflowMutationAction {
    WorkflowMutationAction::BindSingleInput(WorkflowBindSingleInputAction {
        target: WorkflowInputTarget {
            node_id: target_node_id,
            input_key: NodeCapabilityInputKey::new(target_input).unwrap(),
        },
        new_item: WorkflowInputItemEntity {
            id: WorkflowInputItemId::from_uuid(item_id).unwrap(),
            source_node_id,
            source_output_key: NodeCapabilityOutputKey::new(source_output).unwrap(),
            input_role_key: None,
        },
    })
}

fn literal_parameters() -> NodeCapabilityParameterSet {
    parameter_set([(
        "text",
        NodeCapabilityParameterValue::Text("A lighthouse above a stormy sea".to_owned()),
    )])
}

fn profile_parameters(profile_id: &str) -> NodeCapabilityParameterSet {
    parameter_set([(
        "generation_profile_ref",
        NodeCapabilityParameterValue::GenerationProfile(
            NodeCapabilityGenerationProfileRefParameterValue::new(profile_id, 1).unwrap(),
        ),
    )])
}

fn parameter_set<const N: usize>(
    values: [(&str, NodeCapabilityParameterValue); N],
) -> NodeCapabilityParameterSet {
    NodeCapabilityParameterSet::try_from_map(
        values
            .into_iter()
            .map(|(key, value)| (NodeCapabilityParameterKey::new(key).unwrap(), value))
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap()
}

fn node_output(
    run: &engine::workflow::WorkflowRunAggregate,
    node_id: WorkflowNodeId,
    output_key: &str,
) -> WorkflowRuntimeValue {
    run.node_executions()
        .iter()
        .find(|execution| execution.node_id() == node_id)
        .and_then(|execution| execution.outputs())
        .and_then(|outputs| outputs.get(&NodeCapabilityOutputKey::new(output_key).unwrap()))
        .cloned()
        .expect("node output")
}

#[derive(Debug)]
struct TaskFacts {
    image_tasks: i64,
    video_tasks: i64,
    image_assets: i64,
    video_assets: i64,
    completed_submit_effects: i64,
    video_input_asset_id: [u8; 16],
    video_input_digest: [u8; 32],
}

fn task_facts(root: &std::path::Path) -> TaskFacts {
    let connection = Connection::open(root.join("metadata.sqlite")).expect("metadata query");
    let scalar = |sql: &str| connection.query_row(sql, [], |row| row.get(0)).unwrap();
    let request: String = connection
        .query_row(
            "SELECT request_json FROM generation_tasks WHERE request_kind = 'Video'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "{}".to_owned());
    let request: Value = serde_json::from_str(&request).unwrap();
    TaskFacts {
        image_tasks: scalar("SELECT COUNT(*) FROM generation_tasks WHERE request_kind = 'Image'"),
        video_tasks: scalar("SELECT COUNT(*) FROM generation_tasks WHERE request_kind = 'Video'"),
        image_assets: scalar("SELECT COUNT(*) FROM assets WHERE media_kind = 0"),
        video_assets: scalar("SELECT COUNT(*) FROM assets WHERE media_kind = 1"),
        completed_submit_effects: scalar(
            "SELECT COUNT(*) FROM generation_task_outbox WHERE kind = 'SubmitTask' AND state = 'Completed'",
        ),
        video_input_asset_id: Uuid::parse_str(
            request["input_asset_id"].as_str().expect("input Asset ID"),
        )
        .unwrap()
        .into_bytes(),
        video_input_digest: json_bytes::<32>(&request["input_content_hash"]),
    }
}

fn running_task_count(root: &std::path::Path) -> i64 {
    Connection::open(root.join("metadata.sqlite"))
        .expect("metadata query")
        .query_row("SELECT COUNT(*) FROM generation_tasks WHERE status = 'Running'", [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn json_bytes<const N: usize>(value: &Value) -> [u8; N] {
    value
        .as_array()
        .expect("byte array")
        .iter()
        .map(|value| u8::try_from(value.as_u64().unwrap()).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

async fn compose(paths: DesktopApplicationPaths) -> DesktopActivatedCommandDependencies {
    DesktopCompositionRoot::compose_activated_commands_with_emitter(
        paths,
        Arc::new(TestEmitterImpl),
        Arc::new(CancelledPickerImpl),
    )
    .await
    .expect("composition")
}

fn id(seed: u128) -> Uuid {
    Uuid::from_u128(0x123e_4567_e89b_42d3_a456_0000_0000_0000 | seed)
}

fn find_executable(name: &str) -> std::path::PathBuf {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|path| path.join(name))
        .find(|path| path.is_file())
        .expect("ffprobe executable")
}

struct TestEmitterImpl;
struct CancelledPickerImpl;

#[async_trait::async_trait]
impl DesktopAssetImportSourcePickerInterface for CancelledPickerImpl {
    async fn pick_asset_import_source(
        &self,
        _expected_media_kind: assets::asset::domain::AssetMediaKind,
    ) -> Result<Option<DesktopPickedAssetImportSource>, DesktopAssetImportSourcePickerError> {
        Ok(None)
    }
}

impl DesktopEventEmitterInterface for TestEmitterImpl {
    fn emit_desktop_event(
        &self,
        _event_name: &str,
        _payload: serde_json::Value,
    ) -> Result<(), DesktopEventEmissionError> {
        Ok(())
    }
}
