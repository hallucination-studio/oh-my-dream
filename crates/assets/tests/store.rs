use assets::{AssetError, AssetKind, AssetStore, NewAsset};
use image::{ImageBuffer, Rgba};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn inserts_image_and_reads_it_back() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    let image_path = write_test_image(temp.path().join("source.png"));
    let snapshot = json!({"workflow": "image", "seed": 42});

    let asset = store
        .insert(NewAsset {
            kind: AssetKind::Image,
            file_path: image_path.to_string_lossy().into_owned(),
            workflow_snapshot: snapshot.clone(),
            source_node_id: Some("node-image".to_owned()),
            tags: vec!["cover".to_owned()],
        })
        .expect("asset should be inserted");

    let stored = store.get(&asset.id).expect("asset should be found");

    assert_eq!(stored.kind, AssetKind::Image);
    assert_eq!(stored.workflow_snapshot, snapshot);
    assert_eq!(stored.source_node_id, Some("node-image".to_owned()));
    assert_eq!(stored.tags, vec!["cover".to_owned()]);
    assert!(Path::new(&stored.file_path).exists());
    assert!(stored.thumbnail_path.as_deref().is_some_and(|path| Path::new(path).exists()));
    assert_eq!(
        fs::read(&stored.file_path).expect("stored file should be readable"),
        fs::read(&image_path).expect("source image should be readable")
    );
}

#[test]
fn workflow_snapshot_round_trips() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    let image_path = write_test_image(temp.path().join("snapshot.png"));
    let snapshot = json!({
        "version": "1.0",
        "nodes": [{"id": "prompt", "type": "TextPrompt"}]
    });

    let asset = store
        .insert(NewAsset {
            kind: AssetKind::Image,
            file_path: image_path.to_string_lossy().into_owned(),
            workflow_snapshot: snapshot.clone(),
            source_node_id: None,
            tags: Vec::new(),
        })
        .expect("asset should be inserted");

    assert_eq!(store.workflow_snapshot(&asset.id).expect("snapshot should be found"), snapshot);
}

#[test]
fn lists_by_kind_with_newest_assets_first() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    let image_path = write_test_image(temp.path().join("list.png"));
    let video_path = write_test_video(temp.path().join("list.mp4"));

    let image = store
        .insert(NewAsset {
            kind: AssetKind::Image,
            file_path: image_path.to_string_lossy().into_owned(),
            workflow_snapshot: json!({"kind": "image"}),
            source_node_id: Some("image-node".to_owned()),
            tags: Vec::new(),
        })
        .expect("image should be inserted");
    let video = store
        .insert(NewAsset {
            kind: AssetKind::Video,
            file_path: video_path.to_string_lossy().into_owned(),
            workflow_snapshot: json!({"kind": "video"}),
            source_node_id: Some("video-node".to_owned()),
            tags: Vec::new(),
        })
        .expect("video should be inserted");

    let all = store.list(None).expect("all assets should list");
    let images = store.list(Some(AssetKind::Image)).expect("image assets should list");
    let videos = store.list(Some(AssetKind::Video)).expect("video assets should list");

    assert_eq!(
        all.iter().map(|asset| asset.id.as_str()).collect::<Vec<_>>(),
        vec![video.id.as_str(), image.id.as_str(),]
    );
    assert_eq!(
        images.iter().map(|asset| asset.id.as_str()).collect::<Vec<_>>(),
        vec![image.id.as_str(),]
    );
    assert_eq!(
        videos.iter().map(|asset| asset.id.as_str()).collect::<Vec<_>>(),
        vec![video.id.as_str(),]
    );
}

#[test]
fn missing_asset_id_returns_not_found() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");

    let error = store.get("missing").expect_err("missing id should fail");

    assert!(matches!(error, AssetError::NotFound { id } if id == "missing"));
}

fn write_test_image(path: PathBuf) -> PathBuf {
    let image = ImageBuffer::from_pixel(4, 4, Rgba([200_u8, 40, 80, 255]));
    image.save(&path).expect("test image should be written");
    path
}

fn write_test_video(path: PathBuf) -> PathBuf {
    fs::write(&path, b"not a real video").expect("test video should be written");
    path
}
