use crate::assistant_runtime::AssistantSidecarCommand;
use crate::assistant_sidecar::configured_assistant_command;
use crate::mock_generation::MockGenerationAdapter;
use crate::production_plan::{ProductionPlanService, ProductionPlanSqliteRepository};
use crate::reviewed_change::{ReviewedChangeService, ReviewedChangeSqliteRepository};
use crate::workflow_authority::WorkflowAuthority;
use crate::workflow_repository::WorkflowSqliteRepository;
use crate::workflow_runs::WorkflowRuns;
use anyhow::{Context, Result};
use assets::AssetStore;
use backends::MockBackend;
use engine::NodeRegistry;
use nodes::SharedAssetStore;
use std::path::{Path, PathBuf};
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
    pub registry: Arc<NodeRegistry>,
    /// App-lifetime workflow run coordinator and project result caches.
    pub workflow_runs: Arc<WorkflowRuns>,
    /// Durable optional Workflow head authority.
    pub workflow_authority: Arc<WorkflowAuthority>,
    /// Durable Agent-owned production memory.
    pub production_plan: Arc<ProductionPlanService>,
    /// Immutable Workflow candidates awaiting review and approval.
    pub reviewed_change: Arc<ReviewedChangeService>,
    /// Command selected by the composition root for the framed stdio runtime.
    pub assistant_sidecar_command: AssistantSidecarCommand,
}

impl AppState {
    /// Builds app state from a Tauri app handle.
    pub fn from_app_handle(handle: &tauri::AppHandle) -> Result<Self> {
        let app_data_dir =
            handle.path().app_data_dir().context("resolve application data directory")?;
        Self::from_roots(app_data_dir.join("assets"), app_data_dir.join("config"))
    }

    /// Builds app state from explicit asset and config roots.
    pub fn from_roots(root: impl AsRef<Path>, config_root: impl AsRef<Path>) -> Result<Self> {
        let backend = Arc::new(MockBackend::new());
        Self::from_roots_with_backend(root, config_root, backend)
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
        Self::from_roots_with_backend(root, config_root, backend)
    }

    /// Builds app state using explicit roots and mock backend.
    pub fn from_roots_with_backend(
        root: impl AsRef<Path>,
        config_root: impl AsRef<Path>,
        backend: Arc<MockBackend>,
    ) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let config_root = config_root.as_ref().to_path_buf();
        std::fs::create_dir_all(&config_root).context("create config root")?;
        let assistant_sidecar_command = configured_assistant_command()
            .map_err(anyhow::Error::msg)
            .context("resolve assistant stdio command")?;
        let store =
            Arc::new(Mutex::new(AssetStore::open(root.as_path()).context("open asset store")?));
        let mut registry = NodeRegistry::new();
        let adapter = Arc::new(MockGenerationAdapter::new(Arc::clone(&backend)));
        let image: Arc<dyn nodes::TextToImageGenerator> = adapter.clone();
        let video: Arc<dyn nodes::ImageToVideoGenerator> = adapter.clone();
        let audio: Arc<dyn nodes::TextToAudioGenerator> = adapter;
        nodes::register_all(&mut registry, image, video, audio, Arc::clone(&store))
            .context("register workflow capabilities")?;
        let registry = Arc::new(registry);
        let workflow_runs = Arc::new(WorkflowRuns::new(Arc::clone(&registry)));
        let workflow_repository =
            WorkflowSqliteRepository::open(WorkflowSqliteRepository::path(&config_root))
                .map_err(|error| anyhow::anyhow!(error.to_string()))
                .context("open Workflow authority")?;
        let workflow_authority = Arc::new(WorkflowAuthority::from_repository(workflow_repository));
        let production_plan_repository = ProductionPlanSqliteRepository::open(
            ProductionPlanSqliteRepository::path(&config_root),
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))
        .context("open production plan memory")?;
        let production_plan =
            Arc::new(ProductionPlanService::new(Arc::new(production_plan_repository)));
        let reviewed_change_repository = ReviewedChangeSqliteRepository::open(
            ReviewedChangeSqliteRepository::path(&config_root),
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))
        .context("open reviewed-change candidates")?;
        let reviewed_change = Arc::new(ReviewedChangeService::new(
            Arc::clone(&registry),
            Arc::clone(&workflow_authority)
                as Arc<dyn crate::reviewed_change::CandidateWorkflowSource>,
            Arc::new(reviewed_change_repository),
        ));
        Ok(Self {
            root,
            config_root,
            backend,
            store,
            registry,
            workflow_runs,
            workflow_authority,
            production_plan,
            reviewed_change,
            assistant_sidecar_command,
        })
    }

    /// Returns the composition-root command for the framed stdio runtime.
    pub fn assistant_sidecar_command(&self) -> &AssistantSidecarCommand {
        &self.assistant_sidecar_command
    }
}

fn sibling_config_root(root: &Path) -> PathBuf {
    root.with_file_name(format!(
        "{}-config",
        root.file_name().and_then(std::ffi::OsStr::to_str).unwrap_or("assets")
    ))
}
