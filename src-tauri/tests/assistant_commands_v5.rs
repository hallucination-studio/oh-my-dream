use std::sync::Arc;

use async_trait::async_trait;
use oh_my_dream_tauri::{
    asset_import_source_picker::{
        DesktopAssetImportSourcePickerError, DesktopAssetImportSourcePickerInterface,
        DesktopPickedAssetImportSource,
    },
    assistant_command_dto::{
        AssistantDecideWorkflowChangeRequestDto, AssistantGetPendingWorkflowChangeRequestDto,
        AssistantSendMessageRequestDto, AssistantWorkflowChangeDecisionDto,
    },
    assistant_commands_v5::{
        assistant_decide_workflow_change_with_dependencies,
        assistant_get_pending_workflow_change_with_dependencies,
        assistant_send_message_with_dependencies,
    },
    composition::{DesktopApplicationPaths, DesktopCompositionRoot},
    workflow_run_event_publisher::{DesktopEventEmissionError, DesktopEventEmitterInterface},
};
use projects::project::{
    application::{ProjectCreateRequest, ProjectMutationRequestId},
    domain::ProjectName,
};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn canonical_assistant_commands_resolve_project_and_fail_closed_when_disabled() {
    tauri::async_runtime::block_on(async {
        let directory = tempdir().unwrap();
        let dependencies = DesktopCompositionRoot::compose_activated_commands_with_emitter(
            DesktopApplicationPaths::from_application_data_root(directory.path()),
            Arc::new(TestEmitterImpl),
            Arc::new(NoSelectionImpl),
        )
        .await
        .unwrap();
        let project = dependencies
            .create
            .create_project(ProjectCreateRequest {
                request_id: ProjectMutationRequestId::from_uuid(id(1)).unwrap(),
                name: ProjectName::new("Assistant").unwrap(),
            })
            .await
            .unwrap();
        let project_id = project.id().as_uuid().to_string();

        let pending = assistant_get_pending_workflow_change_with_dependencies(
            AssistantGetPendingWorkflowChangeRequestDto { project_id: project_id.clone() },
            &dependencies,
        )
        .await
        .unwrap();
        assert!(pending.is_none());

        let send = assistant_send_message_with_dependencies(
            AssistantSendMessageRequestDto {
                project_id: project_id.clone(),
                workflow_present: false,
                workflow_revision: None,
                selected_node_ids: Vec::new(),
                selected_asset_ids: Vec::new(),
                text: "Build a scene".to_owned(),
            },
            &dependencies,
        )
        .await
        .unwrap_err();
        assert_eq!(send.code, "provider.unavailable");

        let decision = assistant_decide_workflow_change_with_dependencies(
            AssistantDecideWorkflowChangeRequestDto {
                project_id,
                workflow_change_id: id(2).to_string(),
                approval_scope_id: id(3).to_string(),
                mutation_digest_hex: "00".repeat(32),
                decision: AssistantWorkflowChangeDecisionDto::Approve,
            },
            &dependencies,
        )
        .await
        .unwrap_err();
        assert_eq!(decision.code, "assistant.not_found");
    });
}

struct TestEmitterImpl;

impl DesktopEventEmitterInterface for TestEmitterImpl {
    fn emit_desktop_event(
        &self,
        _event_name: &str,
        _payload: serde_json::Value,
    ) -> Result<(), DesktopEventEmissionError> {
        Ok(())
    }
}

struct NoSelectionImpl;

#[async_trait]
impl DesktopAssetImportSourcePickerInterface for NoSelectionImpl {
    async fn pick_asset_import_source(
        &self,
        _expected_media_kind: assets::asset::domain::AssetMediaKind,
    ) -> Result<Option<DesktopPickedAssetImportSource>, DesktopAssetImportSourcePickerError> {
        Ok(None)
    }
}

fn id(seed: u128) -> Uuid {
    let mut bytes = seed.to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
