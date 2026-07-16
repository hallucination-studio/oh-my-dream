use std::sync::{Arc, Mutex};

use assistant::{
    domain::{
        AssistantPlanItemEntity, AssistantPlanItemState, AssistantProductionPlanAggregate,
        AssistantProductionPlanId, AssistantProductionPlanRevision, AssistantSessionId,
    },
    interfaces::{AssistantApplicationError, AssistantProductionPlanRepositoryInterface},
};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS assistant_production_plans (
    plan_id BLOB PRIMARY KEY NOT NULL CHECK(length(plan_id) = 16),
    project_id BLOB NOT NULL CHECK(length(project_id) = 16),
    session_id BLOB NOT NULL CHECK(length(session_id) = 16),
    revision INTEGER NOT NULL CHECK(revision > 0),
    body_json BLOB NOT NULL,
    UNIQUE(project_id, session_id)
);
";

#[derive(Clone)]
pub struct SqliteAssistantProductionPlanRepositoryAdapterImpl {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteAssistantProductionPlanRepositoryAdapterImpl {
    pub fn try_new(connection: Arc<Mutex<Connection>>) -> Result<Self, AssistantApplicationError> {
        connection
            .lock()
            .map_err(|_| storage())?
            .execute_batch(CREATE_SCHEMA)
            .map_err(|_| storage())?;
        Ok(Self { connection })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, AssistantApplicationError> + Send + 'static,
    ) -> Result<T, AssistantApplicationError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection = connection.lock().map_err(|_| storage())?;
            operation(&mut connection)
        })
        .await
        .map_err(|_| storage())?
    }
}

#[async_trait]
impl AssistantProductionPlanRepositoryInterface
    for SqliteAssistantProductionPlanRepositoryAdapterImpl
{
    async fn load_assistant_production_plan(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
    ) -> Result<Option<AssistantProductionPlanAggregate>, AssistantApplicationError> {
        self.blocking(move |connection| load(connection, project_id, session_id)).await
    }

    async fn compare_and_swap_assistant_production_plan(
        &self,
        expected_revision: Option<AssistantProductionPlanRevision>,
        plan: AssistantProductionPlanAggregate,
    ) -> Result<(), AssistantApplicationError> {
        self.blocking(move |connection| save(connection, expected_revision, plan)).await
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlanBodyRow {
    title: String,
    items: Vec<PlanItemRow>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlanItemRow {
    id: String,
    goal: String,
    state: PlanItemStateRow,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
enum PlanItemStateRow {
    Pending,
    InProgress,
    Blocked { reason: String },
    Completed { acceptance_note: String },
}

fn load(
    connection: &Connection,
    project_id: ProjectId,
    session_id: AssistantSessionId,
) -> Result<Option<AssistantProductionPlanAggregate>, AssistantApplicationError> {
    let row = connection
        .query_row(
            "SELECT plan_id, revision, body_json FROM assistant_production_plans
             WHERE project_id = ?1 AND session_id = ?2",
            params![project_id.as_uuid().as_bytes(), session_id.as_uuid().as_bytes()],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?, row.get::<_, Vec<u8>>(2)?)),
        )
        .optional()
        .map_err(|_| storage())?;
    row.map(|(plan_id, revision, body)| decode(plan_id, project_id, session_id, revision, body))
        .transpose()
}

fn save(
    connection: &mut Connection,
    expected_revision: Option<AssistantProductionPlanRevision>,
    plan: AssistantProductionPlanAggregate,
) -> Result<(), AssistantApplicationError> {
    let body = encode(&plan)?;
    let revision = sqlite_revision(plan.revision().get())?;
    let changed = match expected_revision {
        None => connection.execute(
            "INSERT INTO assistant_production_plans
             (plan_id, project_id, session_id, revision, body_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                plan.id().as_uuid().as_bytes(),
                plan.project_id().as_uuid().as_bytes(),
                plan.session_id().as_uuid().as_bytes(),
                revision,
                body,
            ],
        ),
        Some(expected) => connection.execute(
            "UPDATE assistant_production_plans SET revision = ?1, body_json = ?2
             WHERE plan_id = ?3 AND project_id = ?4 AND session_id = ?5 AND revision = ?6",
            params![
                revision,
                body,
                plan.id().as_uuid().as_bytes(),
                plan.project_id().as_uuid().as_bytes(),
                plan.session_id().as_uuid().as_bytes(),
                sqlite_revision(expected.get())?,
            ],
        ),
    }
    .map_err(|error| {
        if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
            AssistantApplicationError::RevisionConflict
        } else {
            storage()
        }
    })?;
    if changed == 1 { Ok(()) } else { Err(AssistantApplicationError::RevisionConflict) }
}

fn encode(plan: &AssistantProductionPlanAggregate) -> Result<Vec<u8>, AssistantApplicationError> {
    let body = PlanBodyRow {
        title: plan.title().as_str().to_owned(),
        items: plan.items().iter().map(PlanItemRow::from_domain).collect(),
    };
    serde_json::to_vec(&body).map_err(|_| storage())
}

fn decode(
    plan_id: Vec<u8>,
    project_id: ProjectId,
    session_id: AssistantSessionId,
    revision: i64,
    body: Vec<u8>,
) -> Result<AssistantProductionPlanAggregate, AssistantApplicationError> {
    let body: PlanBodyRow = serde_json::from_slice(&body).map_err(|_| incompatible())?;
    AssistantProductionPlanAggregate::try_restore(
        AssistantProductionPlanId::from_uuid(uuid(plan_id)?).map_err(|_| incompatible())?,
        project_id,
        session_id,
        body.title,
        body.items.into_iter().map(PlanItemRow::into_domain).collect::<Result<_, _>>()?,
        u64::try_from(revision).map_err(|_| incompatible())?,
    )
    .map_err(|_| incompatible())
}

impl PlanItemRow {
    fn from_domain(item: &AssistantPlanItemEntity) -> Self {
        let state = match item.state() {
            AssistantPlanItemState::Pending => PlanItemStateRow::Pending,
            AssistantPlanItemState::InProgress => PlanItemStateRow::InProgress,
            AssistantPlanItemState::Blocked { reason } => {
                PlanItemStateRow::Blocked { reason: reason.as_str().to_owned() }
            }
            AssistantPlanItemState::Completed { acceptance_note } => {
                PlanItemStateRow::Completed { acceptance_note: acceptance_note.as_str().to_owned() }
            }
        };
        Self { id: item.id().as_str().to_owned(), goal: item.goal().as_str().to_owned(), state }
    }

    fn into_domain(self) -> Result<AssistantPlanItemEntity, AssistantApplicationError> {
        let state = match self.state {
            PlanItemStateRow::Pending => AssistantPlanItemState::Pending,
            PlanItemStateRow::InProgress => AssistantPlanItemState::InProgress,
            PlanItemStateRow::Blocked { reason } => AssistantPlanItemState::Blocked {
                reason: assistant::domain::AssistantPlanItemBlockedReason::new(reason)
                    .map_err(|_| incompatible())?,
            },
            PlanItemStateRow::Completed { acceptance_note } => AssistantPlanItemState::Completed {
                acceptance_note: assistant::domain::AssistantPlanItemAcceptanceNote::new(
                    acceptance_note,
                )
                .map_err(|_| incompatible())?,
            },
        };
        AssistantPlanItemEntity::try_restore(self.id, self.goal, state).map_err(|_| incompatible())
    }
}

fn uuid(bytes: Vec<u8>) -> Result<Uuid, AssistantApplicationError> {
    Uuid::from_slice(&bytes).map_err(|_| incompatible())
}

fn sqlite_revision(value: u64) -> Result<i64, AssistantApplicationError> {
    i64::try_from(value).map_err(|_| storage())
}

fn storage() -> AssistantApplicationError {
    AssistantApplicationError::ExternalBoundaryFailed
}

fn incompatible() -> AssistantApplicationError {
    AssistantApplicationError::ExternalBoundaryFailed
}
