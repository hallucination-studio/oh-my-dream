use projects::project::application::{
    ProjectApplicationError, ProjectMutationCommandHash, ProjectMutationOperation,
    ProjectMutationOutcome, ProjectMutationReceipt, ProjectMutationRequestId,
    ProjectMutationResultFingerprint,
};
use projects::project::domain::{
    ProjectAggregate, ProjectCreatedAt, ProjectId, ProjectName, ProjectRevision, ProjectUpdatedAt,
};
use rusqlite::{Connection, Row, params};
use uuid::Uuid;

use super::persistence;

pub(super) struct SqliteProjectRow {
    id: Vec<u8>,
    name: String,
    revision: i64,
    created_at: i64,
    updated_at: i64,
}

impl SqliteProjectRow {
    pub(super) fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            revision: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    }

    pub(super) fn from_domain(project: &ProjectAggregate) -> Result<Self, ProjectApplicationError> {
        Ok(Self {
            id: project.id().as_uuid().as_bytes().to_vec(),
            name: project.name().as_str().to_owned(),
            revision: i64::try_from(project.revision().get()).map_err(|_| persistence())?,
            created_at: project.created_at().get(),
            updated_at: project.updated_at().get(),
        })
    }

    pub(super) fn try_into_domain(self) -> Result<ProjectAggregate, ProjectApplicationError> {
        ProjectAggregate::restore(
            project_id(&self.id)?,
            ProjectName::new(self.name).map_err(|_| persistence())?,
            revision(self.revision)?,
            ProjectCreatedAt::new(self.created_at).map_err(|_| persistence())?,
            ProjectUpdatedAt::new(self.updated_at).map_err(|_| persistence())?,
        )
        .map_err(|_| persistence())
    }

    pub(super) fn insert(&self, connection: &Connection) -> Result<(), ProjectApplicationError> {
        connection
            .execute(
                "INSERT INTO projects(project_id, name, revision, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![self.id, self.name, self.revision, self.created_at, self.updated_at],
            )
            .map(|_| ())
            .map_err(|_| persistence())
    }

    pub(super) fn update(
        &self,
        connection: &Connection,
        expected_revision: ProjectRevision,
    ) -> Result<usize, ProjectApplicationError> {
        let expected = i64::try_from(expected_revision.get()).map_err(|_| persistence())?;
        connection
            .execute(
                "UPDATE projects SET name = ?2, revision = ?3, updated_at = ?4
                 WHERE project_id = ?1 AND revision = ?5",
                params![self.id, self.name, self.revision, self.updated_at, expected],
            )
            .map_err(|_| persistence())
    }
}

pub(super) struct SqliteProjectMutationReceiptRow {
    request_id: Vec<u8>,
    command_hash: Vec<u8>,
    operation: i64,
    project: SqliteProjectRow,
    fingerprint: Vec<u8>,
}

impl SqliteProjectMutationReceiptRow {
    pub(super) fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            request_id: row.get(0)?,
            command_hash: row.get(1)?,
            operation: row.get(2)?,
            project: SqliteProjectRow {
                id: row.get(3)?,
                name: row.get(4)?,
                revision: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            },
            fingerprint: row.get(8)?,
        })
    }

    pub(super) fn from_domain(
        receipt: &ProjectMutationReceipt,
    ) -> Result<Self, ProjectApplicationError> {
        Ok(Self {
            request_id: receipt.request_id().as_uuid().as_bytes().to_vec(),
            command_hash: receipt.command_hash().as_bytes().to_vec(),
            operation: operation_to_i64(receipt.operation()),
            project: SqliteProjectRow::from_domain(receipt.outcome().project())?,
            fingerprint: receipt.result_fingerprint().as_bytes().to_vec(),
        })
    }

    pub(super) fn try_into_domain(self) -> Result<ProjectMutationReceipt, ProjectApplicationError> {
        ProjectMutationReceipt::restore(
            request_id(&self.request_id)?,
            ProjectMutationCommandHash::from_bytes(array(self.command_hash)?),
            operation_from_i64(self.operation)?,
            ProjectMutationOutcome::from_project(self.project.try_into_domain()?),
            ProjectMutationResultFingerprint::from_bytes(array(self.fingerprint)?),
        )
    }

    pub(super) fn insert(&self, connection: &Connection) -> Result<(), ProjectApplicationError> {
        connection
            .execute(
                "INSERT INTO project_mutation_receipts(
                    request_id, command_hash, operation, project_id, name, revision,
                    created_at, updated_at, result_fingerprint
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    self.request_id,
                    self.command_hash,
                    self.operation,
                    self.project.id,
                    self.project.name,
                    self.project.revision,
                    self.project.created_at,
                    self.project.updated_at,
                    self.fingerprint,
                ],
            )
            .map(|_| ())
            .map_err(|_| persistence())
    }
}

fn project_id(bytes: &[u8]) -> Result<ProjectId, ProjectApplicationError> {
    let uuid = Uuid::from_slice(bytes).map_err(|_| persistence())?;
    ProjectId::from_uuid(uuid).ok_or_else(persistence)
}

fn request_id(bytes: &[u8]) -> Result<ProjectMutationRequestId, ProjectApplicationError> {
    let uuid = Uuid::from_slice(bytes).map_err(|_| persistence())?;
    ProjectMutationRequestId::from_uuid(uuid).ok_or_else(persistence)
}

fn revision(value: i64) -> Result<ProjectRevision, ProjectApplicationError> {
    u64::try_from(value).ok().and_then(ProjectRevision::from_non_zero).ok_or_else(persistence)
}

fn array(bytes: Vec<u8>) -> Result<[u8; 32], ProjectApplicationError> {
    bytes.try_into().map_err(|_| persistence())
}

fn operation_to_i64(operation: ProjectMutationOperation) -> i64 {
    match operation {
        ProjectMutationOperation::Create => 1,
        ProjectMutationOperation::Rename => 2,
    }
}

fn operation_from_i64(value: i64) -> Result<ProjectMutationOperation, ProjectApplicationError> {
    match value {
        1 => Ok(ProjectMutationOperation::Create),
        2 => Ok(ProjectMutationOperation::Rename),
        _ => Err(persistence()),
    }
}
