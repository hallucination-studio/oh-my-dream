//! Trusted Desktop file selection for one Asset import.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use assets::asset::{
    application::AssetImportSourceLease,
    domain::{AssetMediaKind, AssetOriginalFileName},
};
use async_trait::async_trait;
use tauri_plugin_dialog::{DialogExt, FilePath};

const IMPORT_DEADLINE: Duration = Duration::from_secs(30);

/// One already-open file selected by the trusted native dialog.
pub struct DesktopPickedAssetImportSource {
    /// Final source file name without a reusable path.
    pub original_file_name: AssetOriginalFileName,
    /// One-shot source stream bounded by the import deadline.
    pub source: AssetImportSourceLease,
}

/// Native file selection boundary consumed by the Asset command.
#[async_trait]
pub trait DesktopAssetImportSourcePickerInterface: Send + Sync {
    /// Selects and opens one local file, or returns `None` when the user cancels.
    async fn pick_asset_import_source(
        &self,
        expected_media_kind: AssetMediaKind,
    ) -> Result<Option<DesktopPickedAssetImportSource>, DesktopAssetImportSourcePickerError>;
}

/// Native selection or file-open failure.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("Desktop Asset import source selection failed")]
pub struct DesktopAssetImportSourcePickerError;

/// Tauri dialog and Tokio file implementation.
pub struct TauriDesktopAssetImportSourcePickerAdapterImpl {
    app: tauri::AppHandle,
}

impl TauriDesktopAssetImportSourcePickerAdapterImpl {
    /// Wires the process application handle used only for native file selection.
    #[must_use]
    pub const fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait]
impl DesktopAssetImportSourcePickerInterface for TauriDesktopAssetImportSourcePickerAdapterImpl {
    async fn pick_asset_import_source(
        &self,
        expected_media_kind: AssetMediaKind,
    ) -> Result<Option<DesktopPickedAssetImportSource>, DesktopAssetImportSourcePickerError> {
        let selected = self
            .app
            .dialog()
            .file()
            .add_filter(filter_name(expected_media_kind), extensions(expected_media_kind))
            .blocking_pick_file();
        let Some(path) = selected else {
            return Ok(None);
        };
        open_selected_path(path).await.map(Some)
    }
}

async fn open_selected_path(
    selected: FilePath,
) -> Result<DesktopPickedAssetImportSource, DesktopAssetImportSourcePickerError> {
    let path = selected.into_path().map_err(|_| DesktopAssetImportSourcePickerError)?;
    let original_file_name = file_name(&path)?;
    let file =
        tokio::fs::File::open(path).await.map_err(|_| DesktopAssetImportSourcePickerError)?;
    let deadline = Instant::now() + IMPORT_DEADLINE;
    Ok(DesktopPickedAssetImportSource {
        original_file_name,
        source: AssetImportSourceLease::new(deadline, Box::pin(file)),
    })
}

fn file_name(path: &Path) -> Result<AssetOriginalFileName, DesktopAssetImportSourcePickerError> {
    let value = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or(DesktopAssetImportSourcePickerError)?;
    AssetOriginalFileName::try_new(value).map_err(|_| DesktopAssetImportSourcePickerError)
}

const fn filter_name(kind: AssetMediaKind) -> &'static str {
    match kind {
        AssetMediaKind::Image => "Images",
        AssetMediaKind::Video => "Videos",
        AssetMediaKind::Audio => "Audio",
    }
}

const fn extensions(kind: AssetMediaKind) -> &'static [&'static str] {
    match kind {
        AssetMediaKind::Image => &["png", "jpg", "jpeg", "webp"],
        AssetMediaKind::Video => &["mp4", "webm"],
        AssetMediaKind::Audio => &["mp3", "wav", "ogg"],
    }
}
