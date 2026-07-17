use std::path::Path;

use assets::asset::application::AssetApplicationError;

use super::storage;

pub(super) fn reject_symlink(path: &Path) -> Result<(), AssetApplicationError> {
    if path.symlink_metadata().is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        Err(storage())
    } else {
        Ok(())
    }
}

pub(super) fn remove_stale_temporary_files(root: &Path) -> Result<(), AssetApplicationError> {
    for entry in std::fs::read_dir(root).map_err(|_| storage())? {
        let entry = entry.map_err(|_| storage())?;
        if entry.file_name().to_string_lossy().ends_with(".tmp") {
            std::fs::remove_file(entry.path()).map_err(|_| storage())?;
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn restrict_directory(path: &Path) -> Result<(), AssetApplicationError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(|_| storage())
}

#[cfg(not(unix))]
pub(super) fn restrict_directory(_path: &Path) -> Result<(), AssetApplicationError> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn restrict_file(path: &Path) -> Result<(), AssetApplicationError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o400)).map_err(|_| storage())
}

#[cfg(not(unix))]
pub(super) fn restrict_file(_path: &Path) -> Result<(), AssetApplicationError> {
    Ok(())
}
