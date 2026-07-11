use tracing::error;

pub(crate) fn command_error(operation: &str, source: impl std::fmt::Display) -> String {
    error!(operation, error = %source, "tauri command failed");
    format!("{operation}: {source}")
}
