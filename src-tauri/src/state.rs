use anyhow::{Context, Result};
use assets::AssetStore;
use backends::{InferenceBackend, MockBackend};
use engine::NodeRegistry;
use nodes::SharedAssetStore;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Managed application state shared by Tauri commands.
pub struct AppState {
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
        let store =
            Arc::new(Mutex::new(AssetStore::open(root.as_ref()).context("open asset store")?));
        let mut registry = NodeRegistry::new();
        let registry_backend: Arc<dyn InferenceBackend> = backend.clone();
        nodes::register_all(&mut registry, registry_backend, Arc::clone(&store));
        Ok(Self { backend, store, registry })
    }
}
