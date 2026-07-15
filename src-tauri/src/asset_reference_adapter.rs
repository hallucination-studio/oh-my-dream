use crate::managed_asset_access::get_visible;
use assets::{AssetKind, AssetStore};
use nodes::{
    AssetMediaKind, AssetReferenceError, AssetReferenceRequest, AssetReferenceResolver,
    ResolvedAssetReference,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) struct AssetStoreReferenceResolver {
    store: Arc<Mutex<AssetStore>>,
}

impl AssetStoreReferenceResolver {
    pub(crate) fn new(store: Arc<Mutex<AssetStore>>) -> Self {
        Self { store }
    }
}

impl AssetReferenceResolver for AssetStoreReferenceResolver {
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
