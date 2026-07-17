use async_trait::async_trait;

use super::{DesktopBackendConfig, DesktopBackendConfigRepositoryError};

/// Singleton validated backend configuration persistence consumed at startup.
#[async_trait]
pub trait DesktopBackendConfigRepositoryInterface: Send + Sync {
    async fn load_or_initialize_desktop_backend_config(
        &self,
    ) -> Result<DesktopBackendConfig, DesktopBackendConfigRepositoryError>;

    async fn save_desktop_backend_config(
        &self,
        config: DesktopBackendConfig,
    ) -> Result<(), DesktopBackendConfigRepositoryError>;
}
