use crate::SharedAssetStore;
use crate::error::NodesError;
use assets::{Asset, AssetKind, NewAsset};
use engine::NodeRunContext;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

const PLACEHOLDER_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

pub(crate) struct AssetMetadata {
    pub(crate) prompt: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) seed: Option<u64>,
    pub(crate) cost: Option<i64>,
}

pub(crate) fn store_generated_asset(
    store: &SharedAssetStore,
    kind: AssetKind,
    reference: &str,
    source_node_type: &str,
    context: &NodeRunContext<'_>,
    metadata: AssetMetadata,
) -> Result<Asset, NodesError> {
    let file_path = materialize_media_ref(reference, kind)?;
    let project = project_info(store, context.project_id())?;
    store
        .lock()
        .map_err(|_| NodesError::AssetStoreLock)?
        .insert(NewAsset {
            kind,
            file_path,
            workflow_snapshot: context.workflow_snapshot().clone(),
            prompt: metadata.prompt,
            project_id: Some(context.project_id().to_owned()),
            project_name: Some(project.name),
            source_node_id: Some(context.node_id().to_owned()),
            source_node_type: Some(source_node_type.to_owned()),
            model: metadata.model,
            seed: metadata.seed,
            cost: metadata.cost,
            tags: Vec::new(),
        })
        .map_err(|source| NodesError::Asset { source })
}

pub(crate) fn resolve_media_reference(
    store: &SharedAssetStore,
    reference: &str,
) -> Result<String, NodesError> {
    if !reference.starts_with("asset-") {
        return Ok(reference.to_owned());
    }
    Ok(store
        .lock()
        .map_err(|_| NodesError::AssetStoreLock)?
        .get(reference)
        .map_err(|source| NodesError::Asset { source })?
        .file_path)
}

pub(crate) fn prompt_from_asset(
    store: &SharedAssetStore,
    reference: &str,
) -> Result<Option<String>, NodesError> {
    if !reference.starts_with("asset-") {
        return Ok(None);
    }
    Ok(store
        .lock()
        .map_err(|_| NodesError::AssetStoreLock)?
        .get(reference)
        .map_err(|source| NodesError::Asset { source })?
        .prompt)
}

fn project_info(store: &SharedAssetStore, project_id: &str) -> Result<assets::Project, NodesError> {
    store
        .lock()
        .map_err(|_| NodesError::AssetStoreLock)?
        .get_project(project_id)
        .map_err(|source| NodesError::Asset { source })
}

fn materialize_media_ref(reference: &str, kind: AssetKind) -> Result<String, NodesError> {
    if let Some(path) = local_file_path(reference)
        && path.exists()
    {
        return Ok(path.to_string_lossy().into_owned());
    }

    let path = placeholder_path(reference, kind)?;
    fs::write(&path, placeholder_bytes(reference, kind)).map_err(|source| {
        NodesError::MaterializeMedia {
            reference: reference.to_owned(),
            message: source.to_string(),
        }
    })?;
    Ok(path.to_string_lossy().into_owned())
}

fn local_file_path(reference: &str) -> Option<PathBuf> {
    reference
        .strip_prefix("file://")
        .map(PathBuf::from)
        .or_else(|| Path::new(reference).is_absolute().then(|| PathBuf::from(reference)))
}

fn placeholder_path(reference: &str, kind: AssetKind) -> Result<PathBuf, NodesError> {
    let directory = std::env::temp_dir().join("oh-my-dream-node-assets");
    fs::create_dir_all(&directory).map_err(|source| NodesError::MaterializeMedia {
        reference: reference.to_owned(),
        message: source.to_string(),
    })?;
    Ok(directory.join(format!("{}.{}", stable_hash(reference), extension(kind))))
}

fn placeholder_bytes(reference: &str, kind: AssetKind) -> Vec<u8> {
    match kind {
        AssetKind::Image => PLACEHOLDER_PNG.to_vec(),
        AssetKind::Video => {
            format!("oh-my-dream placeholder video\nreference={reference}\n").into_bytes()
        }
        AssetKind::Audio => {
            format!("oh-my-dream placeholder audio\nreference={reference}\n").into_bytes()
        }
    }
}

fn extension(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Image => "png",
        AssetKind::Video => "mp4",
        AssetKind::Audio => "wav",
    }
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
