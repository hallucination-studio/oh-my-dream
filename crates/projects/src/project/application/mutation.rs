//! Project mutation idempotency values shared with persistence.

use crate::project::domain::ProjectAggregate;
use sha2::{Digest, Sha256};
use uuid::{Uuid, Variant, Version};

/// Stable idempotency identity for one create or rename request.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ProjectMutationRequestId(Uuid);

impl ProjectMutationRequestId {
    /// Restores an identity only when the UUID is RFC-compatible version four.
    #[must_use]
    pub fn from_uuid(value: Uuid) -> Option<Self> {
        (value.get_version() == Some(Version::Random) && value.get_variant() == Variant::RFC4122)
            .then_some(Self(value))
    }

    /// Returns the UUID without choosing a boundary encoding.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// Canonical SHA-256 identity of one normalized Project mutation command.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ProjectMutationCommandHash([u8; 32]);

impl ProjectMutationCommandHash {
    /// Restores the exact digest bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Closed Project mutation kind stored in an idempotency receipt.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ProjectMutationOperation {
    /// Project creation.
    Create,
    /// Project rename.
    Rename,
}

/// Exact committed Project snapshot returned by a mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectMutationOutcome(ProjectAggregate);

impl ProjectMutationOutcome {
    /// Captures the exact committed Project snapshot.
    #[must_use]
    pub fn from_project(project: ProjectAggregate) -> Self {
        Self(project)
    }

    /// Returns the exact committed Project snapshot.
    #[must_use]
    pub fn project(&self) -> &ProjectAggregate {
        &self.0
    }
}

/// Integrity fingerprint of one exact mutation outcome.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ProjectMutationResultFingerprint([u8; 32]);

impl ProjectMutationResultFingerprint {
    /// Restores the exact digest bytes for later P3 validation.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Durable evidence and exact result for one Project mutation request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectMutationReceipt {
    /// Stable request identity.
    request_id: ProjectMutationRequestId,
    /// Canonical command identity.
    command_hash: ProjectMutationCommandHash,
    /// Closed create or rename operation.
    operation: ProjectMutationOperation,
    /// Exact committed Project snapshot.
    outcome: ProjectMutationOutcome,
    /// Integrity fingerprint of the outcome.
    result_fingerprint: ProjectMutationResultFingerprint,
}

impl ProjectMutationReceipt {
    /// Creates a receipt and computes the frozen outcome fingerprint.
    #[must_use]
    pub fn new(
        request_id: ProjectMutationRequestId,
        command_hash: ProjectMutationCommandHash,
        operation: ProjectMutationOperation,
        outcome: ProjectMutationOutcome,
    ) -> Self {
        let result_fingerprint = calculate_project_mutation_result_fingerprint(outcome.project());
        Self { request_id, command_hash, operation, outcome, result_fingerprint }
    }

    /// Restores a receipt only when its stored outcome fingerprint is valid.
    pub fn restore(
        request_id: ProjectMutationRequestId,
        command_hash: ProjectMutationCommandHash,
        operation: ProjectMutationOperation,
        outcome: ProjectMutationOutcome,
        result_fingerprint: ProjectMutationResultFingerprint,
    ) -> Result<Self, super::ProjectApplicationError> {
        if calculate_project_mutation_result_fingerprint(outcome.project()) != result_fingerprint {
            return Err(super::ProjectApplicationError::ProjectPersistenceFailure);
        }
        Ok(Self { request_id, command_hash, operation, outcome, result_fingerprint })
    }

    /// Returns the stable request identity.
    #[must_use]
    pub const fn request_id(&self) -> ProjectMutationRequestId {
        self.request_id
    }

    /// Returns the canonical command identity.
    #[must_use]
    pub const fn command_hash(&self) -> ProjectMutationCommandHash {
        self.command_hash
    }

    /// Returns the closed mutation operation.
    #[must_use]
    pub const fn operation(&self) -> ProjectMutationOperation {
        self.operation
    }

    /// Returns the exact committed Project outcome.
    #[must_use]
    pub const fn outcome(&self) -> &ProjectMutationOutcome {
        &self.outcome
    }

    /// Returns the stored outcome integrity fingerprint.
    #[must_use]
    pub const fn result_fingerprint(&self) -> ProjectMutationResultFingerprint {
        self.result_fingerprint
    }
}

fn calculate_project_mutation_result_fingerprint(
    project: &ProjectAggregate,
) -> ProjectMutationResultFingerprint {
    let mut hasher = Sha256::new();
    update_length_prefixed(&mut hasher, b"oh-my-dream/project-result/v1");
    hasher.update(project.id().as_uuid().as_bytes());
    update_length_prefixed(&mut hasher, project.name().as_str().as_bytes());
    hasher.update(project.revision().get().to_be_bytes());
    hasher.update(project.created_at().get().to_be_bytes());
    hasher.update(project.updated_at().get().to_be_bytes());
    ProjectMutationResultFingerprint(hasher.finalize().into())
}

fn update_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    // Callers pass only the fixed domain or a ProjectName bounded to 120 Unicode scalars.
    let length = bytes.len() as u32;
    hasher.update(length.to_be_bytes());
    hasher.update(bytes);
}
