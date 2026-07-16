use std::{
    io::Cursor,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use assets::asset::{
    application::{
        AssetGetQuery, AssetImportCommand, AssetImportSourceLease, AssetIssuePreviewCommand,
        AssetListQuery, AssetPageLimit,
    },
    domain::{AssetDisplayName, AssetMediaKind, AssetOriginalFileName},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use oh_my_dream_tauri::{
    asset_import_source_picker::{
        DesktopAssetImportSourcePickerError, DesktopAssetImportSourcePickerInterface,
        DesktopPickedAssetImportSource,
    },
    composition::{DesktopApplicationPaths, DesktopCompositionRoot},
    workflow_run_event_publisher::{DesktopEventEmissionError, DesktopEventEmitterInterface},
};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn canonical_asset_slice_imports_lists_gets_and_issues_signed_preview() {
    tauri::async_runtime::block_on(async {
        let directory = tempdir().unwrap();
        let dependencies = DesktopCompositionRoot::compose_activated_commands_with_emitter(
            DesktopApplicationPaths::from_application_data_root(directory.path()),
            Arc::new(TestEmitterImpl),
            Arc::new(PngPickerImpl::new()),
        )
        .await
        .unwrap();
        let project = dependencies
            .create
            .create_project(projects::project::application::ProjectCreateRequest {
                request_id: projects::project::application::ProjectMutationRequestId::from_uuid(
                    id(1),
                )
                .unwrap(),
                name: projects::project::domain::ProjectName::new("Assets").unwrap(),
            })
            .await
            .unwrap();
        let selected = dependencies
            .asset_import_source_picker
            .pick_asset_import_source(AssetMediaKind::Image)
            .await
            .unwrap()
            .unwrap();
        let imported = dependencies
            .asset_import
            .import_asset(AssetImportCommand::new(
                project.id(),
                AssetMediaKind::Image,
                AssetDisplayName::try_new("Cover").unwrap(),
                selected.original_file_name,
                selected.source,
            ))
            .await
            .unwrap();
        let page = dependencies
            .asset_list
            .list_assets(AssetListQuery::new(
                project.id(),
                Some(AssetMediaKind::Image),
                None,
                AssetPageLimit::from_u16(100).unwrap(),
            ))
            .await
            .unwrap();
        let loaded = dependencies
            .asset_get
            .get_asset(AssetGetQuery::new(project.id(), imported.id()))
            .await
            .unwrap();
        let lease = dependencies
            .asset_issue_preview
            .issue_asset_preview(AssetIssuePreviewCommand::new(project.id(), imported.id()))
            .await
            .unwrap();
        let uri = dependencies.asset_preview_protocol.issue_preview_uri(&lease);
        let other = dependencies
            .create
            .create_project(projects::project::application::ProjectCreateRequest {
                request_id: projects::project::application::ProjectMutationRequestId::from_uuid(
                    id(2),
                )
                .unwrap(),
                name: projects::project::domain::ProjectName::new("Other").unwrap(),
            })
            .await
            .unwrap();

        assert_eq!(page.assets(), std::slice::from_ref(&imported));
        assert_eq!(loaded, imported);
        assert!(uri.starts_with("desktop-asset://v1/"));
        assert!(!uri.contains("cover.png"));
        assert!(
            dependencies
                .asset_get
                .get_asset(AssetGetQuery::new(other.id(), imported.id()))
                .await
                .is_err()
        );
        assert!(
            dependencies
                .asset_issue_preview
                .issue_asset_preview(AssetIssuePreviewCommand::new(other.id(), imported.id()))
                .await
                .is_err()
        );
    });
}

struct PngPickerImpl {
    bytes: Mutex<Option<Vec<u8>>>,
}

impl PngPickerImpl {
    fn new() -> Self {
        let bytes = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
            .unwrap();
        Self { bytes: Mutex::new(Some(bytes)) }
    }
}

#[async_trait::async_trait]
impl DesktopAssetImportSourcePickerInterface for PngPickerImpl {
    async fn pick_asset_import_source(
        &self,
        expected_media_kind: AssetMediaKind,
    ) -> Result<Option<DesktopPickedAssetImportSource>, DesktopAssetImportSourcePickerError> {
        assert_eq!(expected_media_kind, AssetMediaKind::Image);
        let bytes = self.bytes.lock().unwrap().take();
        Ok(bytes.map(|bytes| DesktopPickedAssetImportSource {
            original_file_name: AssetOriginalFileName::try_new("cover.png").unwrap(),
            source: AssetImportSourceLease::new(
                Instant::now() + Duration::from_secs(30),
                Box::pin(Cursor::new(bytes)),
            ),
        }))
    }
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

fn id(seed: u128) -> Uuid {
    Uuid::from_u128(0x123e_4567_e89b_42d3_a456_0000_0000_0000 | seed)
}
