use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use assets::asset::{
    application::{
        AssetFinalizeContentUseCase, AssetGetUseCase, AssetIssuePreviewUseCase,
        AssetRecordNodeOutputUseCase, AssetResolveContentUseCase,
    },
    interfaces::{
        AssetClockInterface, AssetIdentityGeneratorInterface, AssetIngestTransactionInterface,
        AssetManagedContentStoreInterface, AssetMediaInspectorInterface, AssetRepositoryInterface,
    },
};
use backends::provider_routing::{
    ImageToVideoProviderRouteInterface, ImageToVideoProviderRouterImpl,
    ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl,
    TextToImageProviderRouteInterface, TextToImageProviderRouterImpl,
    TextToSpeechProviderRouteInterface, TextToSpeechProviderRouterImpl,
};
use engine::node_capability::{WorkflowNodeCapabilityInterface, WorkflowNodeCapabilityRegistry};
use nodes::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileCatalog, GenerationProfileId,
    GenerationProfileRef, GenerationProfileVersion, ImageToVideoCapabilityImpl,
    ProvideLiteralTextCapabilityImpl, ReadAudioAssetCapabilityImpl, ReadImageAssetCapabilityImpl,
    ReadVideoAssetCapabilityImpl, TextToImageCapabilityImpl, TextToSpeechCapabilityImpl,
};
use rusqlite::Connection;

use super::{DesktopCompositionError, SqliteDesktopBackendSettingsAdapterImpl};
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
    credential_repository::{
        GenerationProviderCredentialId, GenerationProviderCredentialRepositoryInterface,
    },
    desktop_backend_config::{DesktopBackendConfig, GenerationProviderRouteConfig},
    desktop_bridges::DesktopWorkflowMediaPreviewAdapterImpl,
    desktop_node_capability_asset_bridge::DesktopNodeCapabilityAssetBridgeAdapterImpl,
    provider_adapters::{
        ElevenLabsTextToSpeechProviderRouteImpl, ElevenLabsVoiceId,
        FalImageToVideoProviderRouteImpl, FalTextToImageProviderRouteImpl,
    },
};

pub(super) struct DesktopNodeCapabilityComposition {
    pub(super) registry: Arc<WorkflowNodeCapabilityRegistry>,
    pub(super) catalog: Arc<GenerationProfileCatalog>,
    pub(super) availability_reader: Arc<dyn GenerationProfileAvailabilityReaderInterface>,
    pub(super) preview_issuer: Arc<DesktopWorkflowMediaPreviewAdapterImpl>,
}

pub(super) fn compose_node_capabilities(
    connection: Arc<Mutex<Connection>>,
    managed_content_root: PathBuf,
    media_inspector_executable: PathBuf,
    settings: Arc<SqliteDesktopBackendSettingsAdapterImpl>,
    config: &DesktopBackendConfig,
) -> Result<DesktopNodeCapabilityComposition, DesktopCompositionError> {
    let assets =
        compose_asset_bridge(connection, managed_content_root, media_inspector_executable)?;
    let (text_to_image, image_to_video, text_to_speech) =
        compose_provider_routers(settings, &config.generation_provider_routes)?;
    let availability = Arc::new(ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl::new(
        text_to_image.clone(),
        image_to_video.clone(),
        text_to_speech.clone(),
    ));
    let catalog = Arc::new(
        GenerationProfileCatalog::frozen_mvp().map_err(|_| DesktopCompositionError::Business)?,
    );
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
                text_to_image,
                assets.bridge.clone(),
            )
            .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            ImageToVideoCapabilityImpl::try_new(
                catalog.clone(),
                availability.clone(),
                assets.bridge.clone(),
                image_to_video,
                assets.bridge.clone(),
            )
            .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            TextToSpeechCapabilityImpl::try_new(
                catalog.clone(),
                availability.clone(),
                text_to_speech,
                assets.bridge,
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
    })
}

struct DesktopAssetComposition {
    bridge: Arc<DesktopNodeCapabilityAssetBridgeAdapterImpl>,
    preview_issuer: Arc<DesktopWorkflowMediaPreviewAdapterImpl>,
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
    let resolve = Arc::new(AssetResolveContentUseCase::new(repository.clone(), managed.clone()));
    let get = Arc::new(AssetGetUseCase::new(repository.clone()));
    let issue_preview = Arc::new(AssetIssuePreviewUseCase::new(
        repository.clone(),
        clock.clone(),
        identities.clone(),
    ));
    let preview_protocol = Arc::new(
        DesktopAssetPreviewProtocolAdapterImpl::try_new(get, resolve.clone(), clock.clone())
            .map_err(|_| DesktopCompositionError::Business)?,
    );
    let record = Arc::new(AssetRecordNodeOutputUseCase::new(
        repository, ingest, managed, inspector, clock, identities, finalizer,
    ));
    Ok(DesktopAssetComposition {
        bridge: Arc::new(DesktopNodeCapabilityAssetBridgeAdapterImpl::new(resolve, record)),
        preview_issuer: Arc::new(DesktopWorkflowMediaPreviewAdapterImpl::new(
            issue_preview,
            preview_protocol,
        )),
    })
}

type ProviderRouters = (
    Arc<TextToImageProviderRouterImpl>,
    Arc<ImageToVideoProviderRouterImpl>,
    Arc<TextToSpeechProviderRouterImpl>,
);

fn compose_provider_routers(
    settings: Arc<SqliteDesktopBackendSettingsAdapterImpl>,
    routes: &[GenerationProviderRouteConfig],
) -> Result<ProviderRouters, DesktopCompositionError> {
    let credentials: Arc<dyn GenerationProviderCredentialRepositoryInterface> = settings;
    let mut image_routes = Vec::new();
    let mut video_routes = Vec::new();
    let mut speech_routes = Vec::new();
    for route in routes {
        let profile = profile_ref(&route.profile_ref)?;
        let credential = GenerationProviderCredentialId::new(route.credential_id.clone())
            .map_err(|_| DesktopCompositionError::Config)?;
        match route.route_id.as_str() {
            "fal.text_to_image" => image_routes.push((
                profile,
                Arc::new(
                    FalTextToImageProviderRouteImpl::try_new(credential, credentials.clone())
                        .map_err(|_| DesktopCompositionError::Business)?,
                ) as Arc<dyn TextToImageProviderRouteInterface>,
            )),
            "fal.image_to_video" => video_routes.push((
                profile,
                Arc::new(
                    FalImageToVideoProviderRouteImpl::try_new(credential, credentials.clone())
                        .map_err(|_| DesktopCompositionError::Business)?,
                ) as Arc<dyn ImageToVideoProviderRouteInterface>,
            )),
            "elevenlabs.text_to_speech" => speech_routes.push((
                profile,
                Arc::new(
                    ElevenLabsTextToSpeechProviderRouteImpl::try_new(
                        credential,
                        credentials.clone(),
                        voice_id(route)?,
                    )
                    .map_err(|_| DesktopCompositionError::Business)?,
                ) as Arc<dyn TextToSpeechProviderRouteInterface>,
            )),
            _ => return Err(DesktopCompositionError::Config),
        }
    }
    Ok((
        Arc::new(
            TextToImageProviderRouterImpl::try_new(image_routes)
                .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            ImageToVideoProviderRouterImpl::try_new(video_routes)
                .map_err(|_| DesktopCompositionError::Business)?,
        ),
        Arc::new(
            TextToSpeechProviderRouterImpl::try_new(speech_routes)
                .map_err(|_| DesktopCompositionError::Business)?,
        ),
    ))
}

fn profile_ref(value: &str) -> Result<GenerationProfileRef, DesktopCompositionError> {
    let (id, version) = value.split_once('@').ok_or(DesktopCompositionError::Config)?;
    Ok(GenerationProfileRef::new(
        GenerationProfileId::try_new(id).map_err(|_| DesktopCompositionError::Config)?,
        GenerationProfileVersion::try_new(
            version.parse().map_err(|_| DesktopCompositionError::Config)?,
        )
        .map_err(|_| DesktopCompositionError::Config)?,
    ))
}

fn voice_id(
    route: &GenerationProviderRouteConfig,
) -> Result<ElevenLabsVoiceId, DesktopCompositionError> {
    let value = route.endpoint.rsplit('/').next().ok_or(DesktopCompositionError::Config)?;
    ElevenLabsVoiceId::try_new(value.to_owned()).map_err(|_| DesktopCompositionError::Config)
}
