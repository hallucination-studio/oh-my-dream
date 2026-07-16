use std::path::PathBuf;

/// Concrete media kind required by a managed Asset consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetMediaKind {
    Image,
    Video,
    Audio,
}

/// Trusted request for one managed local Asset reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetReferenceRequest<'a> {
    pub project_id: &'a str,
    pub asset_id: &'a str,
    pub expected_kind: AssetMediaKind,
}

/// Boundary result containing only data required by node execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAssetReference {
    pub asset_id: String,
    pub local_path: PathBuf,
    pub prompt: Option<String>,
}

/// Safe structured failures from managed Asset resolution.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssetReferenceError {
    #[error("managed asset was not found")]
    NotFound,
    #[error("managed asset is outside the current project scope")]
    OutOfScope,
    #[error("managed asset has the wrong media kind")]
    WrongKind,
    #[error("managed asset content is unavailable")]
    MissingContent,
    #[error("managed asset storage is unavailable")]
    StorageUnavailable,
}

/// Consumer-owned boundary for resolving stable Asset identities.
pub trait AssetReferenceResolverInterface: Send + Sync {
    fn resolve(
        &self,
        request: AssetReferenceRequest<'_>,
    ) -> Result<ResolvedAssetReference, AssetReferenceError>;
}
