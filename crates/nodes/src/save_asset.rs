use crate::SharedAssetStore;
use crate::error::{NodesError, boxed};
use crate::params::{json_param, optional_param, video_input};
use crate::ports::required_input;
use assets::{AssetKind, NewAsset};
use engine::{
    InputPort, Node, NodeParams, NodeRegistry, NodeRunError, OutputPort, PortType, ValueMap,
};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tracing::info;

const TYPE_ID: &str = "SaveAsset";

pub(crate) fn register(registry: &mut NodeRegistry, store: SharedAssetStore) {
    registry.register(
        TYPE_ID,
        Box::new(move |params| {
            SaveAssetNode::from_params(params, store.clone()).map(boxed_node).map_err(boxed)
        }),
    );
}

struct SaveAssetNode {
    store: SharedAssetStore,
    workflow_snapshot: serde_json::Value,
    source_node_id: Option<String>,
    inputs: Vec<InputPort>,
}

impl SaveAssetNode {
    fn from_params(params: &NodeParams, store: SharedAssetStore) -> Result<Self, NodesError> {
        Ok(Self {
            store,
            workflow_snapshot: json_param(params, "workflow_snapshot", serde_json::Value::Null),
            source_node_id: optional_param(params, &["source_node_id"])?,
            inputs: vec![required_input("media", PortType::Video)],
        })
    }

    fn save(&self, media: &str) -> Result<(), NodesError> {
        let file_path = materialize_media_ref(media)?;
        let asset = self
            .store
            .lock()
            .map_err(|_| NodesError::AssetStoreLock)?
            .insert(NewAsset {
                kind: AssetKind::Video,
                file_path,
                workflow_snapshot: self.workflow_snapshot.clone(),
                source_node_id: self.source_node_id.clone(),
                tags: Vec::new(),
            })
            .map_err(|source| NodesError::Asset { source })?;
        info!(type_id = TYPE_ID, asset_id = %asset.id, "saved generated asset");
        Ok(())
    }
}

impl Node for SaveAssetNode {
    fn type_id(&self) -> &str {
        TYPE_ID
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &[]
    }

    fn run(&self, inputs: &ValueMap) -> Result<ValueMap, NodeRunError> {
        let media = video_input(inputs, "media").map_err(boxed)?;
        self.save(media).map_err(boxed)?;
        Ok(BTreeMap::new())
    }
}

fn materialize_media_ref(reference: &str) -> Result<String, NodesError> {
    if let Some(path) = local_file_path(reference)
        && path.exists()
    {
        return Ok(path.to_string_lossy().into_owned());
    }

    let path = placeholder_path(reference)?;
    fs::write(&path, placeholder_bytes(reference)).map_err(|source| {
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

fn placeholder_path(reference: &str) -> Result<PathBuf, NodesError> {
    let directory = std::env::temp_dir().join("oh-my-dream-node-assets");
    fs::create_dir_all(&directory).map_err(|source| NodesError::MaterializeMedia {
        reference: reference.to_owned(),
        message: source.to_string(),
    })?;
    Ok(directory.join(format!("{}.mp4", stable_hash(reference))))
}

fn placeholder_bytes(reference: &str) -> Vec<u8> {
    format!("oh-my-dream placeholder media\nreference={reference}\n").into_bytes()
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn boxed_node(node: SaveAssetNode) -> Box<dyn Node> {
    Box::new(node)
}
