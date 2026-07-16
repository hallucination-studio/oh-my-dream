use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use assistant::{
    domain::{AssistantModelContinuationRef, AssistantModelInvocationId, AssistantSessionId},
    interfaces::{
        AssistantApplicationError, AssistantModelContinuationEnvelope,
        AssistantModelContinuationStoreInterface, AssistantStoredContinuation,
    },
};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024 + 4096;

/// Private local-filesystem implementation of single-use continuation storage.
#[derive(Clone)]
pub struct LocalFilesystemAssistantModelContinuationStoreAdapterImpl {
    root: Arc<PathBuf>,
    lock: Arc<Mutex<()>>,
}

impl LocalFilesystemAssistantModelContinuationStoreAdapterImpl {
    pub fn try_new(root: impl Into<PathBuf>) -> Result<Self, AssistantApplicationError> {
        let root = root.into();
        if root.symlink_metadata().is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(storage());
        }
        std::fs::create_dir_all(&root).map_err(|_| storage())?;
        restrict_directory(&root)?;
        Ok(Self { root: Arc::new(root), lock: Arc::new(Mutex::new(())) })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&Path) -> Result<T, AssistantApplicationError> + Send + 'static,
    ) -> Result<T, AssistantApplicationError> {
        let root = Arc::clone(&self.root);
        let lock = Arc::clone(&self.lock);
        tokio::task::spawn_blocking(move || {
            let _guard = lock.lock().map_err(|_| storage())?;
            operation(root.as_path())
        })
        .await
        .map_err(|_| storage())?
    }
}

#[async_trait]
impl AssistantModelContinuationStoreInterface
    for LocalFilesystemAssistantModelContinuationStoreAdapterImpl
{
    async fn store_assistant_model_continuation(
        &self,
        continuation: AssistantStoredContinuation,
    ) -> Result<(), AssistantApplicationError> {
        self.blocking(move |root| store(root, continuation)).await
    }

    async fn load_assistant_model_continuation(
        &self,
        continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError> {
        let continuation_ref = continuation_ref.clone();
        self.blocking(move |root| load(root, &continuation_ref)).await
    }

    async fn consume_assistant_model_continuation(
        &self,
        continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError> {
        let continuation_ref = continuation_ref.clone();
        self.blocking(move |root| {
            let loaded = load(root, &continuation_ref)?;
            if loaded.is_some() {
                std::fs::remove_file(path(root, &continuation_ref)).map_err(|_| storage())?;
            }
            Ok(loaded)
        })
        .await
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ContinuationRow {
    continuation_ref: String,
    project_id: [u8; 16],
    session_id: [u8; 16],
    invocation_id: [u8; 16],
    envelope: Vec<u8>,
}

fn store(
    root: &Path,
    continuation: AssistantStoredContinuation,
) -> Result<(), AssistantApplicationError> {
    let destination = path(root, &continuation.continuation_ref);
    if let Some(existing) = load(root, &continuation.continuation_ref)? {
        return if existing == continuation { Ok(()) } else { Err(incompatible()) };
    }
    let bytes =
        serde_json::to_vec(&ContinuationRow::from_domain(&continuation)).map_err(|_| storage())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(storage());
    }
    let temporary = destination.with_extension("tmp");
    let mut file = private_create(&temporary)?;
    file.write_all(&bytes).and_then(|()| file.sync_all()).map_err(|_| storage())?;
    std::fs::rename(&temporary, &destination).map_err(|_| storage())
}

fn load(
    root: &Path,
    continuation_ref: &AssistantModelContinuationRef,
) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError> {
    let path = path(root, continuation_ref);
    if path.symlink_metadata().is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(incompatible());
    }
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage()),
    };
    if file.metadata().map_err(|_| storage())?.len() > MAX_FILE_BYTES {
        return Err(AssistantApplicationError::ContinuationIncompatible);
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|_| storage())?;
    let row: ContinuationRow = serde_json::from_slice(&bytes)
        .map_err(|_| AssistantApplicationError::ContinuationIncompatible)?;
    let stored = row.into_domain()?;
    if &stored.continuation_ref != continuation_ref {
        return Err(AssistantApplicationError::ContinuationIncompatible);
    }
    Ok(Some(stored))
}

impl ContinuationRow {
    fn from_domain(value: &AssistantStoredContinuation) -> Self {
        Self {
            continuation_ref: value.continuation_ref.as_str().to_owned(),
            project_id: *value.project_id.as_uuid().as_bytes(),
            session_id: *value.session_id.as_uuid().as_bytes(),
            invocation_id: *value.invocation_id.as_uuid().as_bytes(),
            envelope: value.envelope.as_bytes().to_vec(),
        }
    }

    fn into_domain(self) -> Result<AssistantStoredContinuation, AssistantApplicationError> {
        Ok(AssistantStoredContinuation {
            continuation_ref: AssistantModelContinuationRef::new(self.continuation_ref)
                .map_err(|_| incompatible())?,
            project_id: ProjectId::from_uuid(Uuid::from_bytes(self.project_id))
                .ok_or_else(incompatible)?,
            session_id: AssistantSessionId::from_uuid(Uuid::from_bytes(self.session_id))
                .map_err(|_| incompatible())?,
            invocation_id: AssistantModelInvocationId::from_uuid(Uuid::from_bytes(
                self.invocation_id,
            ))
            .map_err(|_| incompatible())?,
            envelope: AssistantModelContinuationEnvelope::new(self.envelope)?,
        })
    }
}

fn path(root: &Path, continuation_ref: &AssistantModelContinuationRef) -> PathBuf {
    let digest = Sha256::digest(continuation_ref.as_str().as_bytes());
    let name = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    root.join(name)
}

fn private_create(path: &Path) -> Result<File, AssistantApplicationError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|_| storage())
}

#[cfg(unix)]
fn restrict_directory(path: &Path) -> Result<(), AssistantApplicationError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(|_| storage())
}

#[cfg(not(unix))]
fn restrict_directory(_path: &Path) -> Result<(), AssistantApplicationError> {
    Ok(())
}

fn storage() -> AssistantApplicationError {
    AssistantApplicationError::ExternalBoundaryFailed
}

fn incompatible() -> AssistantApplicationError {
    AssistantApplicationError::ContinuationIncompatible
}
