use crate::error::NodesError;
use crate::{GeneratedArtifact, GeneratedOutput, InlineMedia, MediaKind, SharedAssetStore};
use assets::{Asset, AssetKind, NewAsset};
use engine::NodeRunContext;
use std::io::Write;
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
    use super::materialize_inline_media;
    use crate::InlineMedia;
    use std::fs;

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

    #[cfg(unix)]
    fn assert_private(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(path).expect("inline file metadata").permissions().mode();
        assert_eq!(mode & 0o077, 0, "inline temp file must not be group/world accessible");
    }

    #[cfg(not(unix))]
    fn assert_private(_path: &std::path::Path) {}
}
