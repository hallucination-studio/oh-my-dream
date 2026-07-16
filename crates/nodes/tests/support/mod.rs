use nodes::{
    AssetMediaKind, AssetReferenceError, AssetReferenceRequest, AssetReferenceResolverInterface,
    ResolvedAssetReference,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[allow(dead_code)]
pub(crate) struct MissingResolverImpl;

impl AssetReferenceResolverInterface for MissingResolverImpl {
    fn resolve(
        &self,
        _request: AssetReferenceRequest<'_>,
    ) -> Result<ResolvedAssetReference, AssetReferenceError> {
        Err(AssetReferenceError::NotFound)
    }
}

#[allow(dead_code)]
pub(crate) struct StoreResolverImpl(pub(crate) Arc<Mutex<assets::AssetStore>>);

impl AssetReferenceResolverInterface for StoreResolverImpl {
    fn resolve(
        &self,
        request: AssetReferenceRequest<'_>,
    ) -> Result<ResolvedAssetReference, AssetReferenceError> {
        let asset = self
            .0
            .lock()
            .map_err(|_| AssetReferenceError::StorageUnavailable)?
            .get(request.asset_id)
            .map_err(|_| AssetReferenceError::NotFound)?;
        let expected = match request.expected_kind {
            AssetMediaKind::Image => assets::AssetKind::Image,
            AssetMediaKind::Video => assets::AssetKind::Video,
            AssetMediaKind::Audio => assets::AssetKind::Audio,
        };
        if asset.kind != expected {
            return Err(AssetReferenceError::WrongKind);
        }
        Ok(ResolvedAssetReference {
            asset_id: asset.id,
            local_path: PathBuf::from(asset.file_path),
            prompt: asset.prompt,
        })
    }
}
