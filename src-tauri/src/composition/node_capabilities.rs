use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use assets::asset::{
    application::{
        AssetFinalizeContentUseCase, AssetGetUseCase, AssetImportUseCase, AssetIssuePreviewUseCase,
        AssetListUseCase, AssetRecordNodeOutputUseCase, AssetRecoverNodeOutputUseCase,
        AssetResolveContentUseCase,
    },
    interfaces::{
        AssetClockInterface, AssetIdentityGeneratorInterface, AssetIngestTransactionInterface,
        AssetManagedContentStoreInterface, AssetMediaInspectorInterface, AssetRepositoryInterface,
    },
};
use backends::mock_generation_provider::{
    GenerationProviderRegistryProfileAvailabilityReaderAdapterImpl,
    MockGenerationProviderRegistryImpl,
};
use engine::node_capability::{WorkflowNodeCapabilityInterface, WorkflowNodeCapabilityRegistry};
use nodes::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileCatalog,
    ImageToVideoCapabilityImpl, NodeCapabilityGenerationTaskStarterInterface,
    ProvideLiteralTextCapabilityImpl, ReadAudioAssetCapabilityImpl, ReadImageAssetCapabilityImpl,
    ReadVideoAssetCapabilityImpl, TextToImageCapabilityImpl, TextToSpeechCapabilityImpl,
};
use rusqlite::Connection;

use super::DesktopCompositionError;
use crate::{
    asset_adapters::{
        ImageAndFfprobeAssetMediaInspectorAdapterImpl, SystemAssetClockAdapterImpl,
        UuidV4AssetIdentityGeneratorAdapterImpl,
    },
    asset_preview_protocol::DesktopAssetPreviewProtocolAdapterImpl,
    asset_storage_adapters::{
        LocalFilesystemAssetManagedContentStoreAdapterImpl,
        SqliteAssetIngestTransactionAdapterImpl, SqliteAssetRepositoryAdapterImpl,
    },
    desktop_bridges::DesktopWorkflowMediaPreviewAdapterImpl,
    desktop_node_capability_asset_bridge::DesktopNodeCapabilityAssetBridgeAdapterImpl,
    generation_task_start_adapter::{
        DesktopNodeCapabilityGenerationTaskStartAdapterImpl, SystemGenerationTaskClockAdapterImpl,
    },
    generation_task_storage_adapter::SqliteGenerationTaskRepositoryAdapterImpl,
};

pub(super) struct DesktopNodeCapabilityComposition {
    pub(super) registry: Arc<WorkflowNodeCapabilityRegistry>,
    pub(super) catalog: Arc<GenerationProfileCatalog>,
    pub(super) availability_reader: Arc<dyn GenerationProfileAvailabilityReaderInterface>,
    pub(super) preview_issuer: Arc<DesktopWorkflowMediaPreviewAdapterImpl>,
    pub(super) asset_import: Arc<AssetImportUseCase>,
    pub(super) asset_get: Arc<AssetGetUseCase>,
    pub(super) asset_list: Arc<AssetListUseCase>,
    pub(super) asset_issue_preview: Arc<AssetIssuePreviewUseCase>,
    pub(super) asset_preview_protocol: Arc<DesktopAssetPreviewProtocolAdapterImpl>,
    pub(super) asset_finalizer: Arc<AssetFinalizeContentUseCase>,
    pub(super) task_repository: SqliteGenerationTaskRepositoryAdapterImpl,
    pub(super) task_provider_registry: Arc<MockGenerationProviderRegistryImpl>,
    pub(super) task_provider_contracts:
        Arc<Vec<tasks::generation_task::GenerationProviderContract>>,
    pub(super) asset_recover_node_output: Arc<AssetRecoverNodeOutputUseCase>,
    pub(super) asset_record_node_output: Arc<AssetRecordNodeOutputUseCase>,
}

pub(super) fn compose_node_capabilities(
    connection: Arc<Mutex<Connection>>,
    managed_content_root: PathBuf,
    media_inspector_executable: PathBuf,
) -> Result<DesktopNodeCapabilityComposition, DesktopCompositionError> {
    let assets =
        compose_asset_bridge(connection.clone(), managed_content_root, media_inspector_executable)?;
    let catalog = Arc::new(
        GenerationProfileCatalog::frozen_mvp().map_err(|_| DesktopCompositionError::Business)?,
    );
    let task_repository = SqliteGenerationTaskRepositoryAdapterImpl::try_new(connection.clone())
        .map_err(|_| DesktopCompositionError::Business)?;
    let task_registry = Arc::new(
        MockGenerationProviderRegistryImpl::try_new()
            .map_err(|_| DesktopCompositionError::Business)?,
    );
    let task_provider_contracts =
        Arc::new(vec![task_registry.generation_provider_contract().clone()]);
    let availability = Arc::new(
        GenerationProviderRegistryProfileAvailabilityReaderAdapterImpl::new(task_registry.clone()),
    );
    let task_starter: Arc<dyn NodeCapabilityGenerationTaskStarterInterface> =
        Arc::new(DesktopNodeCapabilityGenerationTaskStartAdapterImpl::new(
            task_repository.clone(),
            task_registry.clone(),
            SystemGenerationTaskClockAdapterImpl::default(),
        ));
    let implementations: Vec<Arc<dyn WorkflowNodeCapabilityInterface>> = vec![
        Arc::new(
            ProvideLiteralTextCapabilityImpl::try_new()
                .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            ReadImageAssetCapabilityImpl::try_new(assets.bridge.clone())
                .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            ReadVideoAssetCapabilityImpl::try_new(assets.bridge.clone())
                .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            ReadAudioAssetCapabilityImpl::try_new(assets.bridge.clone())
                .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            TextToImageCapabilityImpl::try_new(
                catalog.clone(),
                availability.clone(),
                task_starter.clone(),
            )
            .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            ImageToVideoCapabilityImpl::try_new(
                catalog.clone(),
                availability.clone(),
                assets.bridge.clone(),
                task_starter.clone(),
            )
            .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            TextToSpeechCapabilityImpl::try_new(
                catalog.clone(),
                availability.clone(),
                task_starter,
            )
            .map_err(|_| DesktopCompositionError::Business)?,
        ),
    ];
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new(implementations)
            .map_err(|_| DesktopCompositionError::Business)?,
    );
    Ok(DesktopNodeCapabilityComposition {
        registry,
        catalog,
        availability_reader: availability,
        preview_issuer: assets.preview_issuer,
        asset_import: assets.import,
        asset_get: assets.get,
        asset_list: assets.list,
        asset_issue_preview: assets.issue_preview,
        asset_preview_protocol: assets.preview_protocol,
        asset_finalizer: assets.finalizer,
        task_repository,
        task_provider_registry: task_registry,
        task_provider_contracts,
        asset_recover_node_output: assets.recover_node_output,
        asset_record_node_output: assets.record_node_output,
    })
}

struct DesktopAssetComposition {
    bridge: Arc<DesktopNodeCapabilityAssetBridgeAdapterImpl>,
    preview_issuer: Arc<DesktopWorkflowMediaPreviewAdapterImpl>,
    import: Arc<AssetImportUseCase>,
    get: Arc<AssetGetUseCase>,
    list: Arc<AssetListUseCase>,
    issue_preview: Arc<AssetIssuePreviewUseCase>,
    preview_protocol: Arc<DesktopAssetPreviewProtocolAdapterImpl>,
    finalizer: Arc<AssetFinalizeContentUseCase>,
    recover_node_output: Arc<AssetRecoverNodeOutputUseCase>,
    record_node_output: Arc<AssetRecordNodeOutputUseCase>,
}

fn compose_asset_bridge(
    connection: Arc<Mutex<Connection>>,
    managed_content_root: PathBuf,
    media_inspector_executable: PathBuf,
) -> Result<DesktopAssetComposition, DesktopCompositionError> {
    let repository: Arc<dyn AssetRepositoryInterface> = Arc::new(
        SqliteAssetRepositoryAdapterImpl::try_new(connection.clone())
            .map_err(|_| DesktopCompositionError::Business)?,
    );
    let ingest: Arc<dyn AssetIngestTransactionInterface> = Arc::new(
        SqliteAssetIngestTransactionAdapterImpl::try_new(connection)
            .map_err(|_| DesktopCompositionError::Business)?,
    );
    let managed: Arc<dyn AssetManagedContentStoreInterface> = Arc::new(
        LocalFilesystemAssetManagedContentStoreAdapterImpl::try_new(managed_content_root)
            .map_err(|_| DesktopCompositionError::Business)?,
    );
    let clock: Arc<dyn AssetClockInterface> = Arc::new(SystemAssetClockAdapterImpl);
    let identities: Arc<dyn AssetIdentityGeneratorInterface> =
        Arc::new(UuidV4AssetIdentityGeneratorAdapterImpl);
    let inspector: Arc<dyn AssetMediaInspectorInterface> =
        Arc::new(ImageAndFfprobeAssetMediaInspectorAdapterImpl::new(media_inspector_executable));
    let finalizer = Arc::new(AssetFinalizeContentUseCase::new(
        repository.clone(),
        ingest.clone(),
        managed.clone(),
    ));
    let import = Arc::new(AssetImportUseCase::new(
        ingest.clone(),
        managed.clone(),
        inspector.clone(),
        clock.clone(),
        identities.clone(),
        finalizer.clone(),
    ));
    let resolve = Arc::new(AssetResolveContentUseCase::new(repository.clone(), managed.clone()));
    let get = Arc::new(AssetGetUseCase::new(repository.clone()));
    let list = Arc::new(AssetListUseCase::new(repository.clone()));
    let issue_preview = Arc::new(AssetIssuePreviewUseCase::new(
        repository.clone(),
        clock.clone(),
        identities.clone(),
    ));
    let preview_protocol = Arc::new(
        DesktopAssetPreviewProtocolAdapterImpl::try_new(
            get.clone(),
            resolve.clone(),
            clock.clone(),
        )
        .map_err(|_| DesktopCompositionError::Business)?,
    );
    let record = Arc::new(AssetRecordNodeOutputUseCase::new(
        repository.clone(),
        ingest,
        managed,
        inspector,
        clock,
        identities,
        finalizer.clone(),
    ));
    let recover_node_output = Arc::new(AssetRecoverNodeOutputUseCase::new(repository.clone()));
    Ok(DesktopAssetComposition {
        bridge: Arc::new(DesktopNodeCapabilityAssetBridgeAdapterImpl::new(resolve, record.clone())),
        preview_issuer: Arc::new(DesktopWorkflowMediaPreviewAdapterImpl::new(
            issue_preview.clone(),
            preview_protocol.clone(),
        )),
        import,
        get,
        list,
        issue_preview,
        preview_protocol,
        finalizer,
        recover_node_output,
        record_node_output: record,
    })
}
