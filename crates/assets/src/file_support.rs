use crate::error::{AssetError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn create_dir(path: &Path, operation: &str) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|source| storage_error(format!("{operation} `{}`: {source}", path.display())))
}

pub(crate) fn file_name_for_id(id: &str, source_path: &Path) -> String {
    source_path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map_or_else(|| id.to_owned(), |extension| format!("{id}.{extension}"))
}

pub(crate) fn unix_timestamp(asset_id: &str) -> Result<i64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|source| {
        storage_error(format!("read timestamp for asset `{asset_id}`: {source}"))
    })?;
    Ok(duration.as_secs() as i64)
}

pub(crate) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn cleanup_after_failure(
    operation: &'static str,
    path: &Path,
    source: AssetError,
) -> AssetError {
    match fs::remove_file(path) {
        Ok(()) => source,
        Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => source,
        Err(cleanup) => AssetError::Cleanup {
            operation,
            source: Box::new(source),
            path: PathBuf::from(path),
            cleanup,
        },
    }
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn storage_error(message: String) -> AssetError {
    AssetError::Storage { message }
}
