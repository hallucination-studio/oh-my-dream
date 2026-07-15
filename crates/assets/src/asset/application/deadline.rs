//! Shared process-monotonic deadline enforcement for Asset use cases.

use std::future::Future;
use std::time::Instant;

use super::AssetApplicationError;

pub(crate) async fn run_asset_operation_before_deadline<T, F>(
    deadline: Instant,
    operation: F,
) -> Result<T, AssetApplicationError>
where
    F: Future<Output = Result<T, AssetApplicationError>>,
{
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), operation)
        .await
        .map_err(|_| AssetApplicationError::DeadlineExceeded)?
}

pub(crate) fn reject_elapsed_asset_deadline(
    deadline: Instant,
) -> Result<(), AssetApplicationError> {
    if Instant::now() >= deadline { Err(AssetApplicationError::DeadlineExceeded) } else { Ok(()) }
}
