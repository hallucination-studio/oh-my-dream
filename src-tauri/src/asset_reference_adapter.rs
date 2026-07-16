use crate::managed_asset_access::get_visible;
use assets::{AssetKind, AssetStore};
use nodes::{
    AssetMediaKind, AssetReferenceError, AssetReferenceRequest, AssetReferenceResolverInterface,
    ResolvedAssetReference,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) struct AssetStoreReferenceResolverImpl {
    store: Arc<Mutex<AssetStore>>,
}

impl AssetStoreReferenceResolverImpl {
    pub(crate) fn new(store: Arc<Mutex<AssetStore>>) -> Self {
        Self { store }
    }
}

impl AssetReferenceResolverInterface for AssetStoreReferenceResolverImpl {
    fn resolve(
        &self,
        request: AssetReferenceRequest<'_>,
    ) -> Result<ResolvedAssetReference, AssetReferenceError> {
        let store = self.store.lock().map_err(|_| AssetReferenceError::StorageUnavailable)?;
        let asset = get_visible(&store, request.project_id, request.asset_id)
            .map_err(|_| AssetReferenceError::StorageUnavailable)?
            .ok_or(AssetReferenceError::NotFound)?;
        if asset.kind != asset_kind(request.expected_kind) {
            return Err(AssetReferenceError::WrongKind);
        }
        let local_path = PathBuf::from(&asset.file_path);
        if !local_path.is_file() {
            return Err(AssetReferenceError::MissingContent);
        }
        Ok(ResolvedAssetReference { asset_id: asset.id, local_path, prompt: asset.prompt })
    }
}

fn asset_kind(kind: AssetMediaKind) -> AssetKind {
    match kind {
        AssetMediaKind::Image => AssetKind::Image,
        AssetMediaKind::Video => AssetKind::Video,
        AssetMediaKind::Audio => AssetKind::Audio,
    }
}

#[cfg(test)]
mod tests {
    use super::AssetStoreReferenceResolverImpl;
    use assets::{AssetKind, AssetStore, NewAsset};
    use nodes::{
        AssetMediaKind, AssetReferenceError, AssetReferenceRequest, AssetReferenceResolverInterface,
    };
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[test]
    fn asset_reference_adapter_enforces_scope_kind_and_content() {
        let root = tempdir().expect("asset root");
        let source = root.path().join("source.mp4");
        fs::write(&source, b"video").expect("source content");
        let store =
            Arc::new(Mutex::new(AssetStore::open(root.path().join("store")).expect("store")));
        let project_a = store.lock().expect("store").create_project("A").expect("project A");
        let project_b = store.lock().expect("store").create_project("B").expect("project B");
        let private = insert(&store, &source, Some(&project_a.id));
        let global = insert(&store, &source, None);
        let resolver = AssetStoreReferenceResolverImpl::new(Arc::clone(&store));

        assert!(
            resolver.resolve(request(&project_a.id, &private.id, AssetMediaKind::Video)).is_ok()
        );
        assert!(
            resolver.resolve(request(&project_b.id, &global.id, AssetMediaKind::Video)).is_ok()
        );
        assert_eq!(
            resolver.resolve(request(&project_b.id, &private.id, AssetMediaKind::Video)),
            Err(AssetReferenceError::NotFound)
        );
        assert_eq!(
            resolver.resolve(request(&project_a.id, &private.id, AssetMediaKind::Image)),
            Err(AssetReferenceError::WrongKind)
        );
        fs::remove_file(&private.file_path).expect("remove managed content");
        assert_eq!(
            resolver.resolve(request(&project_a.id, &private.id, AssetMediaKind::Video)),
            Err(AssetReferenceError::MissingContent)
        );
    }

    fn insert(
        store: &Arc<Mutex<AssetStore>>,
        source: &std::path::Path,
        project_id: Option<&str>,
    ) -> assets::Asset {
        store
            .lock()
            .expect("store")
            .insert(NewAsset {
                kind: AssetKind::Video,
                file_path: source.to_string_lossy().into_owned(),
                workflow_snapshot: serde_json::json!({}),
                prompt: None,
                project_id: project_id.map(str::to_owned),
                project_name: None,
                source_node_id: None,
                source_node_type: None,
                model: None,
                seed: None,
                cost: None,
                tags: Vec::new(),
            })
            .expect("insert Asset")
    }

    fn request<'a>(
        project_id: &'a str,
        asset_id: &'a str,
        expected_kind: AssetMediaKind,
    ) -> AssetReferenceRequest<'a> {
        AssetReferenceRequest { project_id, asset_id, expected_kind }
    }
}
