use anyhow::{Context, Result};
use assets::AssetStore;
use backends::{InferenceBackend, MockBackend};
use engine::NodeRegistry;
use nodes::SharedAssetStore;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Managed application state shared by Tauri commands.
pub struct AppState {
    /// Root directory for stored asset files and metadata.
    pub root: PathBuf,
    /// Deterministic backend used for the first local integration.
    pub backend: Arc<MockBackend>,
    /// Local asset store.
    pub store: SharedAssetStore,
    /// Registry populated with all concrete workflow nodes.
    pub registry: NodeRegistry,
}

impl AppState {
    /// Builds app state from a Tauri app handle.
    pub fn from_app_handle(handle: &tauri::AppHandle) -> Result<Self> {
        let app_data_dir =
            handle.path().app_data_dir().context("resolve application data directory")?;
        Self::from_asset_root(app_data_dir.join("assets"))
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
        let store =
            Arc::new(Mutex::new(AssetStore::open(root.as_path()).context("open asset store")?));
        let mut registry = NodeRegistry::new();
        let registry_backend: Arc<dyn InferenceBackend> = backend.clone();
        nodes::register_all(&mut registry, registry_backend, Arc::clone(&store));
        Ok(Self { root, backend, store, registry })
    }
}
