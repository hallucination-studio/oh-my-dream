use std::sync::{Arc, Mutex};

use assets::asset::application::{
    AssetFinalizeContentUseCase, AssetRecordNodeOutputUseCase, AssetRecoverNodeOutputUseCase,
};
use assets::asset::domain::{AssetContentDigest, AssetId, AssetNodeOutputProduction};
use assets::asset::interfaces::{
    AssetClockInterface, AssetIdentityGeneratorInterface, AssetIngestTransactionInterface,
    AssetManagedContentStoreInterface, AssetMediaInspectorInterface, AssetRepositoryInterface,
};
use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
    WorkflowNodeExecutionId, WorkflowRunId,
};
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use image::{DynamicImage, ImageFormat, RgbaImage};
use nodes::{GenerationProfileId, GenerationProfileRef, GenerationProfileVersion};
use projects::project::domain::ProjectId;
use rusqlite::Connection;
use tasks::generation_task::application::{
    GenerationTaskAssetKey, GenerationTaskAssetRecovery, GenerationTaskProviderResult,
    GenerationTaskStoreAssetCommand,
};
use tasks::generation_task::domain::{
    AssetSnapshotRef, GenerationProviderId, GenerationProviderRouteId, GenerationTaskAggregate,
    GenerationTaskId, GenerationTaskIdempotencyKey, GenerationTaskOrigin, GenerationTaskRequest,
    GenerationTaskTarget, GenerationTaskText, GenerationTaskTimestamp, ImageAspectRatio,
    ImageGenerationSpec, VideoDurationSeconds, VideoGenerationSpec, VoiceGenerationSpec,
};
use tasks::generation_task::interfaces::{
    GenerationTaskAssetSinkInterface, ImageGenerationProviderResult, VideoGenerationProviderResult,
    VoiceGenerationProviderResult,
};
use tempfile::TempDir;
use uuid::Uuid;

use super::{DesktopGenerationTaskAssetSinkAdapterImpl, record_command};
use crate::asset_adapters::{
    ImageAndFfprobeAssetMediaInspectorAdapterImpl, SystemAssetClockAdapterImpl,
    UuidV4AssetIdentityGeneratorAdapterImpl,
};
use crate::asset_storage_adapters::{
    LocalFilesystemAssetManagedContentStoreAdapterImpl, SqliteAssetIngestTransactionAdapterImpl,
    SqliteAssetRepositoryAdapterImpl,
};
use crate::post_commit_effect::SqliteDesktopPostCommitEffectOutboxAdapterImpl;

#[tokio::test]
async fn key_first_recovery_and_store_replay_create_one_asset_and_one_managed_object() {
    let fixture = Fixture::new();
    let task = image_task();
    let key = GenerationTaskAssetKey::from_task(&task);

    assert_eq!(
        fixture.sink.recover_generation_task_asset(key).await.unwrap(),
        GenerationTaskAssetRecovery::SourceRequired
    );
    let first = fixture.sink.store_generation_task_asset(store_command(&task)).await.unwrap();
    assert!(matches!(
        fixture.sink.recover_generation_task_asset(key).await.unwrap(),
        GenerationTaskAssetRecovery::Available(ref value) if value == &first
    ));
    let replay = fixture.sink.store_generation_task_asset(store_command(&task)).await.unwrap();

    assert_eq!(replay, first);
    assert_eq!(fixture.row_count("assets"), 1);
    assert_eq!(fixture.row_count("asset_node_output_bindings"), 1);
    assert_eq!(fixture.row_count("asset_content_finalizations"), 1);
    assert_eq!(regular_file_count(&fixture.content_root.join("managed")), 1);
    assert_eq!(regular_file_count(&fixture.content_root.join("staging")), 0);
}

#[test]
fn video_and_voice_commands_use_frozen_output_and_provenance_mappings() {
    let source_id = AssetId::from_uuid(uuid(20)).unwrap();
    let video = task_with_request(
        GenerationTaskRequest::Video(
            VideoGenerationSpec::try_new(
                AssetSnapshotRef::new(
                    source_id,
                    assets::asset::domain::AssetMediaKind::Image,
                    AssetContentDigest::from_bytes([2; 32]),
                ),
                VideoDurationSeconds::Five,
                None,
            )
            .unwrap(),
        ),
        "video.generate_from_image",
        "video.standard",
    );
    let video_store = GenerationTaskStoreAssetCommand::from_task(
        &video,
        GenerationTaskProviderResult::Video(
            VideoGenerationProviderResult::try_new(vec![1]).unwrap(),
        ),
        time(110),
    );
    let video_record = record_command(&video_store).unwrap();
    assert_eq!(video_record.output_key().output_key().as_str(), "video");
    assert_eq!(video_record.display_name().as_str(), "Generated Video");
    assert!(matches!(
        video_record.production(),
        AssetNodeOutputProduction::ProviderDerived { source_asset_ids, .. }
            if source_asset_ids.as_slice()[0].asset_id() == source_id
    ));

    let voice = task_with_request(
        GenerationTaskRequest::Voice(VoiceGenerationSpec::new(
            GenerationTaskText::try_new("hello").unwrap(),
        )),
        "audio.synthesize_speech",
        "voice.standard",
    );
    let voice_store = GenerationTaskStoreAssetCommand::from_task(
        &voice,
        GenerationTaskProviderResult::Voice(
            VoiceGenerationProviderResult::try_new(vec![1]).unwrap(),
        ),
        time(110),
    );
    let voice_record = record_command(&voice_store).unwrap();
    assert_eq!(voice_record.output_key().output_key().as_str(), "audio");
    assert_eq!(voice_record.display_name().as_str(), "Synthesized Speech");
    assert!(matches!(
        voice_record.production(),
        AssetNodeOutputProduction::ProviderGenerated { .. }
    ));
}

#[test]
fn store_translation_rejects_provider_media_that_does_not_match_the_task_request() {
    let image = image_task();
    let mismatched = GenerationTaskStoreAssetCommand::from_task(
        &image,
        GenerationTaskProviderResult::Voice(
            VoiceGenerationProviderResult::try_new(vec![1]).unwrap(),
        ),
        time(110),
    );

    assert_eq!(
        record_command(&mismatched).err(),
        Some(tasks::generation_task::application::GenerationTaskBoundaryError::Permanent)
    );
}

struct Fixture {
    sink: DesktopGenerationTaskAssetSinkAdapterImpl,
    connection: Arc<Mutex<Connection>>,
    content_root: std::path::PathBuf,
    _temp: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let content_root = temp.path().join("content");
        let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(connection.clone()).unwrap();
        let repository: Arc<dyn AssetRepositoryInterface> =
            Arc::new(SqliteAssetRepositoryAdapterImpl::try_new(connection.clone()).unwrap());
        let ingest: Arc<dyn AssetIngestTransactionInterface> =
            Arc::new(SqliteAssetIngestTransactionAdapterImpl::try_new(connection.clone()).unwrap());
        let managed: Arc<dyn AssetManagedContentStoreInterface> = Arc::new(
            LocalFilesystemAssetManagedContentStoreAdapterImpl::try_new(content_root.clone())
                .unwrap(),
        );
        let inspector: Arc<dyn AssetMediaInspectorInterface> = Arc::new(
            ImageAndFfprobeAssetMediaInspectorAdapterImpl::new(temp.path().join("unused-ffprobe")),
        );
        let clock: Arc<dyn AssetClockInterface> = Arc::new(SystemAssetClockAdapterImpl);
        let identities: Arc<dyn AssetIdentityGeneratorInterface> =
            Arc::new(UuidV4AssetIdentityGeneratorAdapterImpl);
        let finalizer = Arc::new(AssetFinalizeContentUseCase::new(
            repository.clone(),
            ingest.clone(),
            managed.clone(),
        ));
        let record = Arc::new(AssetRecordNodeOutputUseCase::new(
            repository.clone(),
            ingest,
            managed,
            inspector,
            clock,
            identities,
            finalizer,
        ));
        let recover = Arc::new(AssetRecoverNodeOutputUseCase::new(repository));
        Self {
            sink: DesktopGenerationTaskAssetSinkAdapterImpl::new(recover, record),
            connection,
            content_root,
            _temp: temp,
        }
    }

    fn row_count(&self, table: &str) -> i64 {
        self.connection
            .lock()
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .unwrap()
    }
}

fn store_command(task: &GenerationTaskAggregate) -> GenerationTaskStoreAssetCommand {
    GenerationTaskStoreAssetCommand::from_task(
        task,
        GenerationTaskProviderResult::Image(
            ImageGenerationProviderResult::try_new(png_bytes()).unwrap(),
        ),
        time(110),
    )
}

fn image_task() -> GenerationTaskAggregate {
    task_with_request(
        GenerationTaskRequest::Image(ImageGenerationSpec::new(
            GenerationTaskText::try_new("lake at dawn").unwrap(),
            ImageAspectRatio::Square,
        )),
        "image.generate_from_text",
        "image.high_quality_general",
    )
}

fn task_with_request(
    request: GenerationTaskRequest,
    capability_id: &str,
    profile_id: &str,
) -> GenerationTaskAggregate {
    GenerationTaskAggregate::create(
        GenerationTaskId::from_uuid(uuid(1)).unwrap(),
        GenerationTaskOrigin::new(
            ProjectId::from_uuid(uuid(2)).unwrap(),
            WorkflowId::from_uuid(uuid(3)).unwrap(),
            WorkflowRevision::new(4).unwrap(),
            WorkflowRunId::from_uuid(uuid(5)).unwrap(),
            WorkflowNodeId::from_uuid(uuid(6)).unwrap(),
            WorkflowNodeExecutionId::from_uuid(uuid(7)).unwrap(),
            NodeCapabilityContractRef::new(
                NodeCapabilityContractId::new(capability_id).unwrap(),
                NodeCapabilityContractVersion::new(1, 0).unwrap(),
            ),
        ),
        GenerationTaskIdempotencyKey::try_new("node-execution-7").unwrap(),
        GenerationTaskTarget::new(
            GenerationProfileRef::new(
                GenerationProfileId::try_new(profile_id).unwrap(),
                GenerationProfileVersion::try_new(1).unwrap(),
            ),
            GenerationProviderId::try_new("mock").unwrap(),
            GenerationProviderRouteId::try_new("mock.image.high-quality-general.v1").unwrap(),
        ),
        request,
        time(100),
        time(30_100),
    )
    .unwrap()
}

fn png_bytes() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255])))
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn regular_file_count(root: &std::path::Path) -> usize {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .map(
            |path| {
                if path.is_dir() { regular_file_count(&path) } else { usize::from(path.is_file()) }
            },
        )
        .sum()
}

fn time(value: i64) -> GenerationTaskTimestamp {
    GenerationTaskTimestamp::from_utc_milliseconds(value).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
