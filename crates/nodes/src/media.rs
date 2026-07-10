use crate::error::NodesError;
use crate::{GeneratedArtifact, GeneratedOutput, InlineMedia, MediaKind, SharedAssetStore};
use assets::{Asset, AssetKind, NewAsset};
use engine::NodeRunContext;
use std::io::Write;
use std::path::Path;
use tempfile::{Builder, NamedTempFile};

pub(crate) struct AssetMetadata {
    pub(crate) prompt: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) seed: Option<u64>,
}

pub(crate) fn store_generated_asset(
    store: &SharedAssetStore,
    kind: AssetKind,
    output: &GeneratedOutput,
    source_node_type: &str,
    context: &NodeRunContext<'_>,
    metadata: AssetMetadata,
) -> Result<Asset, NodesError> {
    let media = inline_media(&output.artifact, kind)?;
    let project = project_info(store, context.project_id())?;
    let source = materialize_inline_media(media)?;
    let file_path = source.path().to_string_lossy().into_owned();
    let asset = store
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
            cost: output.cost,
            tags: Vec::new(),
        })
        .map_err(|source| NodesError::Asset { source })?;
    drop(source);
    Ok(asset)
}

#[derive(Debug)]
pub(crate) struct ResolvedImageInput {
    pub(crate) file_path: String,
    pub(crate) prompt: Option<String>,
}

pub(crate) fn resolve_image_input(
    store: &SharedAssetStore,
    reference: &str,
) -> Result<ResolvedImageInput, NodesError> {
    let asset = store.lock().map_err(|_| NodesError::AssetStoreLock)?.get(reference);
    match asset {
        Ok(asset) => Ok(ResolvedImageInput {
            file_path: existing_local_file(&asset.file_path)?,
            prompt: asset.prompt,
        }),
        Err(assets::AssetError::NotFound { .. }) => {
            Ok(ResolvedImageInput { file_path: existing_local_file(reference)?, prompt: None })
        }
        Err(source) => Err(NodesError::ImageAssetLookup { source }),
    }
}

pub(crate) fn materialize_inline_media(media: &InlineMedia) -> Result<NamedTempFile, NodesError> {
    let mut file = Builder::new()
        .prefix("oh-my-dream-inline-")
        .suffix(media.format().file_suffix())
        .tempfile()
        .map_err(|source| NodesError::MaterializeMedia { operation: "create", source })?;
    file.write_all(media.bytes())
        .map_err(|source| NodesError::MaterializeMedia { operation: "write", source })?;
    file.flush().map_err(|source| NodesError::MaterializeMedia { operation: "flush", source })?;
    Ok(file)
}

fn inline_media(
    artifact: &GeneratedArtifact,
    expected: AssetKind,
) -> Result<&InlineMedia, NodesError> {
    let GeneratedArtifact::InlineMedia(media) = artifact else {
        return Err(NodesError::RemoteMediaOutput);
    };
    let expected_kind = media_kind(expected);
    if media.kind() != expected_kind {
        return Err(NodesError::InlineMediaKindMismatch {
            actual: media.kind().label(),
            expected: expected_kind.label(),
        });
    }
    Ok(media)
}

fn existing_local_file(reference: &str) -> Result<String, NodesError> {
    Path::new(reference)
        .is_file()
        .then(|| reference.to_owned())
        .ok_or(NodesError::InvalidImageInput)
}

fn media_kind(kind: AssetKind) -> MediaKind {
    match kind {
        AssetKind::Image => MediaKind::Image,
        AssetKind::Video => MediaKind::Video,
        AssetKind::Audio => MediaKind::Audio,
    }
}

fn project_info(store: &SharedAssetStore, project_id: &str) -> Result<assets::Project, NodesError> {
    store
        .lock()
        .map_err(|_| NodesError::AssetStoreLock)?
        .get_project(project_id)
        .map_err(|source| NodesError::Asset { source })
}

#[cfg(test)]
mod tests {
    use super::{materialize_inline_media, resolve_image_input};
    use crate::{InlineMedia, SharedAssetStore};
    use assets::{Asset, AssetKind, AssetStore, NewAsset};
    use serde_json::json;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    #[test]
    fn rejects_remote_image_inputs_without_echoing_credentials() {
        let directory = TempDir::new().expect("temp directory");
        let store = shared_store(&directory);
        let reference = "https://media.example/image.png?token=secret";

        let error = resolve_image_input(&store, reference)
            .expect_err("remote image cannot cross a local-path boundary");

        assert!(error.to_string().contains("local file or stored asset"));
        assert!(!error.to_string().contains(reference));
        assert!(!error.to_string().contains("secret"));
    }

    #[test]
    fn accepts_an_existing_local_image_input() {
        let directory = TempDir::new().expect("temp directory");
        let store = shared_store(&directory);
        let path = directory.path().join("source.png");
        fs::write(&path, b"local image bytes").expect("local image");
        let reference = path.to_str().expect("UTF-8 test path");

        let resolved = resolve_image_input(&store, reference).expect("local image path");

        assert_eq!(resolved.file_path, reference);
        assert_eq!(resolved.prompt, None);
    }

    #[test]
    fn rejects_a_stored_asset_when_its_managed_file_is_missing() {
        let directory = TempDir::new().expect("temp directory");
        let store = shared_store(&directory);
        let asset = insert_test_image(&store);
        fs::remove_file(&asset.file_path).expect("remove managed asset file");

        let error = resolve_image_input(&store, &asset.id)
            .expect_err("missing managed file cannot cross a local-path boundary");

        assert!(error.to_string().contains("local file or stored asset"));
        assert!(!error.to_string().contains(&asset.file_path));
    }

    #[test]
    fn accepts_an_asset_prefixed_local_file_when_no_asset_exists() {
        let directory = TempDir::new().expect("temp directory");
        let store = shared_store(&directory);
        let local = tempfile::Builder::new()
            .prefix("asset-preview-")
            .suffix(".png")
            .tempfile_in(".")
            .expect("asset-prefixed local file");
        let reference = local.path().file_name().and_then(|name| name.to_str()).expect("file name");

        let resolved = resolve_image_input(&store, reference).expect("local image path");

        assert_eq!(resolved.file_path, reference);
        assert_eq!(resolved.prompt, None);
    }

    #[test]
    fn inline_media_uses_unique_private_files_that_are_deleted_on_drop() {
        let first = materialize_inline_media(&InlineMedia::png(b"first".to_vec()))
            .expect("first inline file");
        let second = materialize_inline_media(&InlineMedia::png(b"second".to_vec()))
            .expect("second inline file");
        let first_path = first.path().to_path_buf();
        let second_path = second.path().to_path_buf();

        assert_ne!(first_path, second_path);
        assert_eq!(fs::read(&first_path).expect("first bytes"), b"first");
        assert_eq!(fs::read(&second_path).expect("second bytes"), b"second");
        assert_private(&first_path);
        assert_private(&second_path);

        drop(first);
        assert!(!first_path.exists());
        assert!(second_path.exists());
        drop(second);
        assert!(!second_path.exists());
    }

    fn shared_store(directory: &TempDir) -> SharedAssetStore {
        Arc::new(Mutex::new(AssetStore::open(directory.path()).expect("asset store")))
    }

    fn insert_test_image(store: &SharedAssetStore) -> Asset {
        let source = materialize_inline_media(&InlineMedia::png(MOCK_IMAGE_PNG.to_vec()))
            .expect("source image");
        store
            .lock()
            .expect("store lock")
            .insert(NewAsset {
                kind: AssetKind::Image,
                file_path: source.path().to_string_lossy().into_owned(),
                workflow_snapshot: json!({}),
                prompt: None,
                project_id: None,
                project_name: None,
                source_node_id: None,
                source_node_type: None,
                model: None,
                seed: None,
                cost: None,
                tags: Vec::new(),
            })
            .expect("stored image")
    }

    const MOCK_IMAGE_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    #[cfg(unix)]
    fn assert_private(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(path).expect("inline file metadata").permissions().mode();
        assert_eq!(mode & 0o077, 0, "inline temp file must not be group/world accessible");
    }

    #[cfg(not(unix))]
    fn assert_private(_path: &std::path::Path) {}
}
