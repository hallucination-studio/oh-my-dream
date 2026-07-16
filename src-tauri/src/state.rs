use crate::assistant_approval::{PendingApprovalService, PendingApprovalSqliteRepository};
use crate::assistant_runtime::AssistantSidecarCommand;
use crate::assistant_sidecar::configured_assistant_command;
use crate::mock_generation::MockGenerationAdapterImpl;
use crate::production_plan::{ProductionPlanService, ProductionPlanSqliteRepository};
use crate::reviewed_change::{ReviewedChangeService, ReviewedChangeSqliteRepository};
use crate::workflow_authority::WorkflowAuthority;
use crate::workflow_repository::WorkflowSqliteRepository;
use crate::workflow_runs::WorkflowRuns;
use anyhow::{Context, Result};
use assets::AssetStore;
use backends::MockBackendImpl;
use engine::NodeRegistry;
use nodes::SharedAssetStore;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Manager;

const ASSISTANT_CONTRACT_EPOCH: &str = "explicit-outputs-v2";

pub(crate) fn assistant_epoch_root(config_root: &Path) -> PathBuf {
    config_root.join("assistant_epochs").join(ASSISTANT_CONTRACT_EPOCH)
}

/// Managed application state shared by Tauri commands.
pub struct AppState {
    /// Root directory for stored asset files and metadata.
    pub root: PathBuf,
    /// Root directory for local non-asset configuration.
    pub config_root: PathBuf,
    /// Process-owned connection to the single private metadata database.
    _metadata_connection: Arc<Mutex<rusqlite::Connection>>,
    /// Deterministic backend used for the first local integration.
    pub backend: Arc<MockBackendImpl>,
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
    /// Durable SDK state paused for one human decision.
    pub pending_approval: Arc<PendingApprovalService>,
    /// In-process guard preventing concurrent Runner invocations per Session.
    pub active_assistant_sessions: Arc<std::sync::Mutex<HashSet<String>>>,
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
        let backend = Arc::new(MockBackendImpl::new());
        Self::from_roots_with_backend(root, config_root, backend)
    }

    /// Builds app state using an explicit asset root.
    pub fn from_asset_root(root: impl AsRef<Path>) -> Result<Self> {
        let backend = Arc::new(MockBackendImpl::new());
        Self::from_asset_root_with_backend(root, backend)
    }

    /// Builds app state using an explicit asset root and mock backend.
    pub fn from_asset_root_with_backend(
        root: impl AsRef<Path>,
        backend: Arc<MockBackendImpl>,
    ) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let config_root = sibling_config_root(&root);
        Self::from_roots_with_backend(root, config_root, backend)
    }

    /// Builds app state using explicit roots and mock backend.
    pub fn from_roots_with_backend(
        root: impl AsRef<Path>,
        config_root: impl AsRef<Path>,
        backend: Arc<MockBackendImpl>,
    ) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let config_root = config_root.as_ref().to_path_buf();
        std::fs::create_dir_all(&config_root).context("create config root")?;
        let metadata_connection = crate::metadata_sqlite::open_metadata_sqlite(
            &crate::metadata_sqlite::metadata_sqlite_path(&config_root),
        )
        .map_err(anyhow::Error::new)
        .context("open metadata storage")?;
        let assistant_sidecar_command = configured_assistant_command()
            .map_err(anyhow::Error::msg)
            .context("resolve assistant stdio command")?;
        let store =
            Arc::new(Mutex::new(AssetStore::open(root.as_path()).context("open asset store")?));
        let mut registry = NodeRegistry::new();
        let adapter = Arc::new(MockGenerationAdapterImpl::new(Arc::clone(&backend)));
        let image: Arc<dyn nodes::TextToImageGeneratorInterface> = adapter.clone();
        let reference_image: Arc<dyn nodes::ReferenceImageGeneratorInterface> = adapter.clone();
        let reference_video: Arc<dyn nodes::ReferenceVideoGeneratorInterface> = adapter.clone();
        let video: Arc<dyn nodes::ImageToVideoGeneratorInterface> = adapter.clone();
        let audio: Arc<dyn nodes::TextToAudioGeneratorInterface> = adapter;
        let asset_resolver: Arc<dyn nodes::AssetReferenceResolverInterface> =
            Arc::new(crate::asset_reference_adapter::AssetStoreReferenceResolverImpl::new(
                Arc::clone(&store),
            ));
        nodes::register_all(
            &mut registry,
            nodes::GenerationAdapters::new(image, reference_image, reference_video, video, audio),
            Arc::clone(&store),
            asset_resolver,
        )
        .context("register workflow capabilities")?;
        let registry = Arc::new(registry);
        let workflow_runs = Arc::new(WorkflowRuns::new(Arc::clone(&registry)));
        let workflow_repository =
            WorkflowSqliteRepository::open(WorkflowSqliteRepository::path(&config_root))
                .map_err(|error| anyhow::anyhow!(error.to_string()))
                .context("open Workflow authority")?;
        let workflow_authority = Arc::new(WorkflowAuthority::from_repository(workflow_repository));
        let assistant_state_root = assistant_epoch_root(&config_root);
        let production_plan_repository = ProductionPlanSqliteRepository::open(
            ProductionPlanSqliteRepository::path(&assistant_state_root),
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))
        .context("open production plan memory")?;
        let production_plan =
            Arc::new(ProductionPlanService::new(Arc::new(production_plan_repository)));
        let reviewed_change_repository = ReviewedChangeSqliteRepository::open(
            ReviewedChangeSqliteRepository::path(&assistant_state_root),
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))
        .context("open reviewed-change candidates")?;
        let reviewed_change = Arc::new(ReviewedChangeService::new(
            Arc::clone(&registry),
            Arc::clone(&workflow_authority)
                as Arc<dyn crate::reviewed_change::CandidateWorkflowSource>,
            Arc::new(reviewed_change_repository),
        ));
        let pending_approval_repository = PendingApprovalSqliteRepository::open(
            PendingApprovalSqliteRepository::path(&assistant_state_root),
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))
        .context("open pending Assistant approvals")?;
        let pending_approval =
            Arc::new(PendingApprovalService::new(Arc::new(pending_approval_repository)));
        Ok(Self {
            root,
            config_root,
            _metadata_connection: Arc::new(Mutex::new(metadata_connection)),
            backend,
            store,
            registry,
            workflow_runs,
            workflow_authority,
            production_plan,
            reviewed_change,
            pending_approval,
            active_assistant_sessions: Arc::new(std::sync::Mutex::new(HashSet::new())),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn new_assistant_epoch_never_opens_legacy_orchestration_files() {
        let directory = tempdir().expect("root");
        let asset_root = directory.path().join("assets");
        let config_root = directory.path().join("config");
        std::fs::create_dir_all(config_root.join("assistant_sessions")).expect("legacy root");
        let legacy_paths = [
            config_root.join("production_plan.sqlite"),
            config_root.join("reviewed_change.sqlite"),
            config_root.join("assistant_approval.sqlite"),
            config_root.join("assistant_sessions/project.sqlite3"),
        ];
        for path in &legacy_paths {
            std::fs::write(path, b"not a sqlite database").expect("legacy file");
        }

        let _state = AppState::from_roots(&asset_root, &config_root)
            .expect("legacy orchestration files must be unreachable");

        for path in legacy_paths {
            assert_eq!(std::fs::read(path).expect("legacy bytes"), b"not a sqlite database");
        }
        let epoch_root = assistant_epoch_root(&config_root);
        assert!(epoch_root.join("production_plan.sqlite").exists());
        assert!(epoch_root.join("reviewed_change.sqlite").exists());
        assert!(epoch_root.join("assistant_approval.sqlite").exists());
    }
}
