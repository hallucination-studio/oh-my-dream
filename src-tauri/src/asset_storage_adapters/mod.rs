//! Concrete persistence and managed-byte adapters for Asset interfaces.

mod managed_content;
mod sqlite;

pub use managed_content::LocalFilesystemAssetManagedContentStoreAdapterImpl;
pub use sqlite::{SqliteAssetIngestTransactionAdapterImpl, SqliteAssetRepositoryAdapterImpl};
