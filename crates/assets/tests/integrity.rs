use assets::{AssetError, AssetKind, AssetStore, NewAsset};
use image::{ImageBuffer, Rgba};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn rejects_unknown_project_before_copying_asset() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    let source = write_test_image(temp.path().join("source.png"));

    let error = store
        .insert(new_image(&source, Some("missing-project"), None))
        .expect_err("unknown project should be rejected");

    assert!(matches!(error, AssetError::NotFound { id } if id == "missing-project"));
    assert_directory_empty(&temp.path().join("files"));
    assert_directory_empty(&temp.path().join("thumbnails"));
}

#[test]
fn derives_project_name_from_the_authoritative_project() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    store.create_project_with_id("project-1", "Canonical Name").expect("project should be created");
    let source = write_test_image(temp.path().join("source.png"));

    let stored = store
        .insert(new_image(&source, Some("project-1"), Some("Caller Guess")))
        .expect("asset should be inserted");

    assert_eq!(stored.project_name.as_deref(), Some("Canonical Name"));
}

#[test]
fn thumbnail_failure_does_not_leave_copied_files() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    let source = temp.path().join("invalid.png");
    fs::write(&source, b"not an image").expect("invalid image should be written");

    let error = store
        .insert(new_image(&source, None, None))
        .expect_err("invalid image should fail thumbnail generation");

    assert!(matches!(error, AssetError::Thumbnail { .. }));
    assert_directory_empty(&temp.path().join("files"));
    assert_directory_empty(&temp.path().join("thumbnails"));
}

#[test]
fn asset_ids_are_opaque_random_identifiers() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    let source = write_test_image(temp.path().join("source.png"));

    let stored = store.insert(new_image(&source, None, None)).expect("asset should be inserted");
    let suffix = stored.id.strip_prefix("asset-").expect("asset id should have a prefix");

    assert_eq!(suffix.len(), 32);
    assert!(suffix.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
}

fn new_image(source: &Path, project_id: Option<&str>, project_name: Option<&str>) -> NewAsset {
    NewAsset {
        kind: AssetKind::Image,
        file_path: source.to_string_lossy().into_owned(),
        workflow_snapshot: json!({"version": "1.0", "nodes": []}),
        prompt: None,
        project_id: project_id.map(str::to_owned),
        project_name: project_name.map(str::to_owned),
        source_node_id: None,
        source_node_type: None,
        model: None,
        seed: None,
        cost: None,
        tags: Vec::new(),
    }
}

fn write_test_image(path: PathBuf) -> PathBuf {
    ImageBuffer::from_pixel(4, 4, Rgba([20_u8, 80, 160, 255]))
        .save(&path)
        .expect("test image should be written");
    path
}

fn assert_directory_empty(path: &Path) {
    let mut entries = fs::read_dir(path).expect("managed directory should exist");
    assert!(entries.next().is_none(), "{} should be empty", path.display());
}
