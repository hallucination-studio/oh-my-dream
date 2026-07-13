use assets::{AssetError, AssetKind, AssetQuery, AssetSort, AssetStore, NewAsset};
use image::{ImageBuffer, Rgba};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn inserts_image_and_reads_it_back() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    store.create_project_with_id("project-1", "Launch").expect("project should be created");
    let image_path = write_test_image(temp.path().join("source.png"));
    let snapshot = json!({"workflow": "image", "seed": 42});

    let asset = store
        .insert(NewAsset {
            kind: AssetKind::Image,
            file_path: image_path.to_string_lossy().into_owned(),
            workflow_snapshot: snapshot.clone(),
            prompt: Some("cover prompt".to_owned()),
            project_id: Some("project-1".to_owned()),
            project_name: Some("Launch".to_owned()),
            source_node_id: Some("node-image".to_owned()),
            source_node_type: Some("TextToImage".to_owned()),
            model: Some("mock-image".to_owned()),
            seed: Some(42),
            cost: Some(250),
            tags: vec!["cover".to_owned()],
        })
        .expect("asset should be inserted");

    let stored = store.get(&asset.id).expect("asset should be found");

    assert_eq!(stored.kind, AssetKind::Image);
    assert_eq!(stored.workflow_snapshot, snapshot);
    assert_eq!(stored.prompt.as_deref(), Some("cover prompt"));
    assert_eq!(stored.project_id.as_deref(), Some("project-1"));
    assert_eq!(stored.project_name.as_deref(), Some("Launch"));
    assert_eq!(stored.source_node_id, Some("node-image".to_owned()));
    assert_eq!(stored.source_node_type.as_deref(), Some("TextToImage"));
    assert_eq!(stored.model.as_deref(), Some("mock-image"));
    assert_eq!(stored.seed, Some(42));
    assert_eq!(stored.cost, Some(250));
    assert_eq!(stored.tags, vec!["cover".to_owned()]);
    assert!(Path::new(&stored.file_path).exists());
    assert!(stored.thumbnail_path.as_deref().is_some_and(|path| Path::new(path).exists()));
    assert_eq!(
        fs::read(&stored.file_path).expect("stored file should be readable"),
        fs::read(&image_path).expect("source image should be readable")
    );
}

#[test]
fn seed_round_trips_the_full_u64_range() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    store.create_project_with_id("project-1", "Launch").expect("project should be created");
    let asset = store
        .insert(NewAsset {
            kind: AssetKind::Image,
            file_path: write_test_image(temp.path().join("max-seed.png"))
                .to_string_lossy()
                .into_owned(),
            workflow_snapshot: json!({}),
            prompt: None,
            project_id: Some("project-1".to_owned()),
            project_name: None,
            source_node_id: None,
            source_node_type: None,
            model: None,
            seed: Some(u64::MAX),
            cost: None,
            tags: Vec::new(),
        })
        .expect("asset should be inserted");

    assert_eq!(store.get(&asset.id).expect("asset should load").seed, Some(u64::MAX));
}

#[test]
fn reads_legacy_signed_integer_seed_encoding() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    store.create_project_with_id("project-1", "Launch").expect("project should be created");
    let asset = store
        .insert(NewAsset {
            kind: AssetKind::Image,
            file_path: write_test_image(temp.path().join("legacy-seed.png"))
                .to_string_lossy()
                .into_owned(),
            workflow_snapshot: json!({}),
            prompt: None,
            project_id: Some("project-1".to_owned()),
            project_name: None,
            source_node_id: None,
            source_node_type: None,
            model: None,
            seed: Some(1),
            cost: None,
            tags: Vec::new(),
        })
        .expect("asset should be inserted");
    drop(store);
    let connection = rusqlite::Connection::open(temp.path().join("assets.sqlite"))
        .expect("legacy database should open");
    connection
        .execute("UPDATE assets SET seed = -1 WHERE id = ?1", [&asset.id])
        .expect("legacy seed should update");
    drop(connection);

    let reopened = AssetStore::open(temp.path()).expect("store should reopen");

    assert_eq!(reopened.get(&asset.id).expect("asset should load").seed, Some(u64::MAX));
}

#[test]
fn queries_assets_by_kind_project_model_prompt_and_sort() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    store.create_project_with_id("project-a", "A").expect("project A should be created");
    store.create_project_with_id("project-b", "B").expect("project B should be created");
    let cheap = store
        .insert(NewAsset {
            kind: AssetKind::Image,
            file_path: write_test_image(temp.path().join("cheap.png"))
                .to_string_lossy()
                .into_owned(),
            workflow_snapshot: json!({}),
            prompt: Some("quiet ocean".to_owned()),
            project_id: Some("project-a".to_owned()),
            project_name: Some("A".to_owned()),
            source_node_id: Some("image-a".to_owned()),
            source_node_type: Some("TextToImage".to_owned()),
            model: Some("mock-image".to_owned()),
            seed: Some(1),
            cost: Some(100),
            tags: Vec::new(),
        })
        .expect("cheap asset should insert");
    let expensive = store
        .insert(NewAsset {
            kind: AssetKind::Video,
            file_path: write_test_video(temp.path().join("expensive.mp4"))
                .to_string_lossy()
                .into_owned(),
            workflow_snapshot: json!({}),
            prompt: Some("loud ocean".to_owned()),
            project_id: Some("project-a".to_owned()),
            project_name: Some("A".to_owned()),
            source_node_id: Some("video-a".to_owned()),
            source_node_type: Some("ImageToVideo".to_owned()),
            model: Some("mock-video".to_owned()),
            seed: Some(2),
            cost: Some(900),
            tags: Vec::new(),
        })
        .expect("expensive asset should insert");
    store
        .insert(NewAsset {
            kind: AssetKind::Audio,
            file_path: write_test_audio(temp.path().join("other.wav"))
                .to_string_lossy()
                .into_owned(),
            workflow_snapshot: json!({}),
            prompt: Some("city rain".to_owned()),
            project_id: Some("project-b".to_owned()),
            project_name: Some("B".to_owned()),
            source_node_id: Some("audio-b".to_owned()),
            source_node_type: Some("TextToAudio".to_owned()),
            model: Some("mock-audio".to_owned()),
            seed: Some(3),
            cost: Some(300),
            tags: Vec::new(),
        })
        .expect("audio asset should insert");

    let query = AssetQuery {
        kind: None,
        project_id: Some("project-a".to_owned()),
        model: None,
        prompt: Some("ocean".to_owned()),
        sort: AssetSort::CostDesc,
    };
    let assets = store.list_with_query(&query).expect("query should run");

    assert_eq!(
        assets.iter().map(|asset| asset.id.as_str()).collect::<Vec<_>>(),
        vec![expensive.id.as_str(), cheap.id.as_str()]
    );

    let image_query = AssetQuery {
        kind: Some(AssetKind::Image),
        project_id: Some("project-a".to_owned()),
        model: Some("mock-image".to_owned()),
        prompt: Some("quiet".to_owned()),
        sort: AssetSort::Newest,
    };
    let images = store.list_with_query(&image_query).expect("image query should run");
    assert_eq!(images.iter().map(|asset| asset.id.as_str()).collect::<Vec<_>>(), vec![cheap.id]);
}

#[test]
fn cost_ascending_sorts_known_costs_before_unknown_costs() {
    let temp = TempDir::new().expect("temp dir should be created");
    let store = AssetStore::open(temp.path()).expect("store should open");
    let unknown = store
        .insert(NewAsset {
            kind: AssetKind::Video,
            file_path: write_test_video(temp.path().join("unknown.mp4"))
                .to_string_lossy()
                .into_owned(),
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
        .expect("unknown cost asset should insert");
    let known = store
        .insert(NewAsset {
            kind: AssetKind::Video,
            file_path: write_test_video(temp.path().join("known.mp4"))
                .to_string_lossy()
                .into_owned(),
            workflow_snapshot: json!({}),
            prompt: None,
            project_id: None,
            project_name: None,
            source_node_id: None,
            source_node_type: None,
            model: None,
            seed: None,
            cost: Some(100),
            tags: Vec::new(),
        })
        .expect("known cost asset should insert");

    let assets = store
        .list_with_query(&AssetQuery { sort: AssetSort::CostAsc, ..AssetQuery::default() })
        .expect("query should run");

    assert_eq!(
        assets.iter().map(|asset| asset.id.as_str()).collect::<Vec<_>>(),
        vec![known.id.as_str(), unknown.id.as_str()]
    );
}

#[test]
fn persists_projects_and_workflows_across_reopen() {
    let temp = TempDir::new().expect("temp dir should be created");
    let project_id = {
        let store = AssetStore::open(temp.path()).expect("store should open");
        let project = store.create_project("Launch").expect("project should create");
        assert_eq!(project.name, "Launch");
        store
            .save_workflow(
                &project.id,
                serde_json::json!({
                    "version": "1.0",
                    "project_id": project.id,
                    "nodes": []
                }),
            )
            .expect("workflow should save");
        project.id
    };

    let reopened = AssetStore::open(temp.path()).expect("store should reopen");
    let projects = reopened.list_projects().expect("projects should list");
    let workflow = reopened.load_workflow(&project_id).expect("workflow should load");

    assert_eq!(
        projects.iter().map(|project| project.name.as_str()).collect::<Vec<_>>(),
        vec!["Launch"]
    );
    assert_eq!(workflow["project_id"], project_id);
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
            prompt: None,
            project_id: None,
            project_name: None,
            source_node_id: Some("image-node".to_owned()),
            source_node_type: Some("TextToImage".to_owned()),
            model: None,
            seed: None,
            cost: None,
            tags: Vec::new(),
        })
        .expect("image should be inserted");
    let video = store
        .insert(NewAsset {
            kind: AssetKind::Video,
            file_path: video_path.to_string_lossy().into_owned(),
            workflow_snapshot: json!({"kind": "video"}),
            prompt: None,
            project_id: None,
            project_name: None,
            source_node_id: Some("video-node".to_owned()),
            source_node_type: Some("ImageToVideo".to_owned()),
            model: None,
            seed: None,
            cost: None,
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

fn write_test_audio(path: PathBuf) -> PathBuf {
    fs::write(&path, b"not a real audio file").expect("test audio should be written");
    path
}
