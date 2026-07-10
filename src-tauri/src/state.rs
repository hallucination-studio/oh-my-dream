use crate::dto::AssistantSessionDto;
use crate::mock_generation::MockGenerationAdapter;
use anyhow::{Context, Result};
use assets::AssetStore;
use backends::MockBackend;
use engine::NodeRegistry;
use nodes::SharedAssetStore;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Managed application state shared by Tauri commands.
pub struct AppState {
    /// Root directory for stored asset files and metadata.
    pub root: PathBuf,
    /// Root directory for local non-asset configuration.
    pub config_root: PathBuf,
    /// Deterministic backend used for the first local integration.
    pub backend: Arc<MockBackend>,
    /// Local asset store.
    pub store: SharedAssetStore,
    /// Registry populated with all concrete workflow nodes.
    pub registry: NodeRegistry,
    /// Local assistant sidecar session for this app lifetime.
    pub assistant_session: Mutex<Option<AssistantSessionDto>>,
    /// Running assistant sidecar process in app builds.
    pub assistant_process: Mutex<Option<Child>>,
    /// Whether this state should spawn the Python assistant.
    pub assistant_sidecar_enabled: bool,
}

impl AppState {
    /// Builds app state from a Tauri app handle.
    pub fn from_app_handle(handle: &tauri::AppHandle) -> Result<Self> {
        let app_data_dir =
            handle.path().app_data_dir().context("resolve application data directory")?;
        Self::from_roots_with_sidecar(
            app_data_dir.join("assets"),
            app_data_dir.join("config"),
            true,
        )
    }

    /// Builds app state from explicit asset and config roots.
    pub fn from_roots(root: impl AsRef<Path>, config_root: impl AsRef<Path>) -> Result<Self> {
        let backend = Arc::new(MockBackend::new());
        Self::from_roots_with_backend_and_sidecar(root, config_root, backend, false)
    }

    /// Builds app state using an explicit asset root.
    pub fn from_asset_root(root: impl AsRef<Path>) -> Result<Self> {
        let backend = Arc::new(MockBackend::new());
        Self::from_asset_root_with_backend(root, backend)
    }

    /// Builds app state using an explicit asset root and mock backend.
    pub fn from_asset_root_with_backend(
        root: impl AsRef<Path>,
        backend: Arc<MockBackend>,
    ) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let config_root = sibling_config_root(&root);
        Self::from_roots_with_backend_and_sidecar(root, config_root, backend, false)
    }

    /// Builds app state using explicit asset/config roots and mock backend.
    pub fn from_roots_with_backend(
        root: impl AsRef<Path>,
        config_root: impl AsRef<Path>,
        backend: Arc<MockBackend>,
    ) -> Result<Self> {
        Self::from_roots_with_backend_and_sidecar(root, config_root, backend, false)
    }

    /// Builds app state using explicit roots and sidecar mode.
    pub fn from_roots_with_sidecar(
        root: impl AsRef<Path>,
        config_root: impl AsRef<Path>,
        assistant_sidecar_enabled: bool,
    ) -> Result<Self> {
        let backend = Arc::new(MockBackend::new());
        Self::from_roots_with_backend_and_sidecar(
            root,
            config_root,
            backend,
            assistant_sidecar_enabled,
        )
    }

    /// Builds app state using explicit roots, mock backend, and sidecar mode.
    pub fn from_roots_with_backend_and_sidecar(
        root: impl AsRef<Path>,
        config_root: impl AsRef<Path>,
        backend: Arc<MockBackend>,
        assistant_sidecar_enabled: bool,
    ) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let config_root = config_root.as_ref().to_path_buf();
        std::fs::create_dir_all(&config_root).context("create config root")?;
        let store =
            Arc::new(Mutex::new(AssetStore::open(root.as_path()).context("open asset store")?));
        seed_default_project(&store)?;
        let mut registry = NodeRegistry::new();
        let adapter = Arc::new(MockGenerationAdapter::new(Arc::clone(&backend)));
        let image: Arc<dyn nodes::TextToImageGenerator> = adapter.clone();
        let video: Arc<dyn nodes::ImageToVideoGenerator> = adapter.clone();
        let audio: Arc<dyn nodes::TextToAudioGenerator> = adapter;
        nodes::register_all(&mut registry, image, video, audio, Arc::clone(&store));
        Ok(Self {
            root,
            config_root,
            backend,
            store,
            registry,
            assistant_session: Mutex::new(None),
            assistant_process: Mutex::new(None),
            assistant_sidecar_enabled,
        })
    }
}

fn sibling_config_root(root: &Path) -> PathBuf {
    root.with_file_name(format!(
        "{}-config",
        root.file_name().and_then(std::ffi::OsStr::to_str).unwrap_or("assets")
    ))
}

fn seed_default_project(store: &SharedAssetStore) -> Result<()> {
    let store = store.lock().map_err(|_| anyhow::anyhow!("asset store lock was poisoned"))?;
    if store.get_project("default").is_ok() {
        return Ok(());
    }
    store.create_project_with_id("default", "Default")?;
    Ok(())
}
