use super::{
    PlanItemStatus, ProductionPlan, ProductionPlanError, ProductionPlanRepository, RestoredPlanItem,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const DATABASE_FILE: &str = "production_plan.sqlite";

pub struct ProductionPlanSqliteRepository {
    connection: Mutex<Connection>,
}

impl ProductionPlanSqliteRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ProductionPlanError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(storage)?;
        }
        let connection = Connection::open(path).map_err(storage)?;
        connection.busy_timeout(Duration::from_secs(5)).map_err(storage)?;
        let repository = Self { connection: Mutex::new(connection) };
        repository.migrate()?;
        Ok(repository)
    }

    #[must_use]
    pub fn path(config_root: impl AsRef<Path>) -> PathBuf {
        config_root.as_ref().join(DATABASE_FILE)
    }

    fn migrate(&self) -> Result<(), ProductionPlanError> {
        let connection = self.connection.lock().map_err(|_| lock_error())?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS production_plans (
                    project_id TEXT PRIMARY KEY NOT NULL,
                    revision INTEGER NOT NULL,
                    plan_json TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                );",
            )
            .map_err(storage)
    }
}

impl ProductionPlanRepository for ProductionPlanSqliteRepository {
    fn load(&self, project_id: &str) -> Result<Option<ProductionPlan>, ProductionPlanError> {
        let connection = self.connection.lock().map_err(|_| lock_error())?;
        let row = connection
            .query_row(
                "SELECT project_id, revision, plan_json FROM production_plans WHERE project_id = ?1",
                [project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(storage)?;
        row.map(decode).transpose()
    }

    fn insert(&self, plan: &ProductionPlan) -> Result<(), ProductionPlanError> {
        let connection = self.connection.lock().map_err(|_| lock_error())?;
        let changed = connection
            .execute(
                "INSERT OR IGNORE INTO production_plans
                 (project_id, revision, plan_json, updated_at)
                 VALUES (?1, ?2, ?3, unixepoch())",
                params![plan.project_id(), sqlite_revision(plan.revision())?, encode(plan)?],
            )
            .map_err(storage)?;
        if changed == 0 {
            return Err(ProductionPlanError::AlreadyExists {
                project_id: plan.project_id().to_owned(),
            });
        }
        Ok(())
    }

    fn update(
        &self,
        expected_revision: u64,
        plan: &ProductionPlan,
    ) -> Result<(), ProductionPlanError> {
        let connection = self.connection.lock().map_err(|_| lock_error())?;
        let changed = connection
            .execute(
                "UPDATE production_plans
                 SET revision = ?1, plan_json = ?2, updated_at = unixepoch()
                 WHERE project_id = ?3 AND revision = ?4",
                params![
                    sqlite_revision(plan.revision())?,
                    encode(plan)?,
                    plan.project_id(),
                    sqlite_revision(expected_revision)?,
                ],
            )
            .map_err(storage)?;
        if changed == 0 {
            drop(connection);
            let actual = self.load(plan.project_id())?.map(|stored| stored.revision()).unwrap_or(0);
            return Err(ProductionPlanError::RevisionConflict {
                expected: expected_revision,
                actual,
            });
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
struct PlanBody {
    title: String,
    items: Vec<PlanItemRow>,
}

#[derive(Deserialize, Serialize)]
struct PlanItemRow {
    id: String,
    summary: String,
    status: PlanItemStatus,
    note: Option<String>,
}

fn encode(plan: &ProductionPlan) -> Result<String, ProductionPlanError> {
    let body = PlanBody {
        title: plan.title().to_owned(),
        items: plan
            .items()
            .iter()
            .map(|item| PlanItemRow {
                id: item.id().to_owned(),
                summary: item.summary().to_owned(),
                status: item.status(),
                note: item.note().map(str::to_owned),
            })
            .collect(),
    };
    serde_json::to_string(&body).map_err(corrupt)
}

fn decode(row: (String, i64, String)) -> Result<ProductionPlan, ProductionPlanError> {
    let (project_id, revision, json) = row;
    let body: PlanBody = serde_json::from_str(&json).map_err(corrupt)?;
    let revision = u64::try_from(revision).map_err(corrupt)?;
    ProductionPlan::restore(
        project_id,
        revision,
        body.title,
        body.items
            .into_iter()
            .map(|item| RestoredPlanItem {
                id: item.id,
                summary: item.summary,
                status: item.status,
                note: item.note,
            })
            .collect(),
    )
}

fn sqlite_revision(revision: u64) -> Result<i64, ProductionPlanError> {
    i64::try_from(revision).map_err(corrupt)
}

fn storage(error: impl std::fmt::Display) -> ProductionPlanError {
    ProductionPlanError::Storage { message: error.to_string() }
}

fn corrupt(error: impl std::fmt::Display) -> ProductionPlanError {
    ProductionPlanError::CorruptData { message: error.to_string() }
}

fn lock_error() -> ProductionPlanError {
    ProductionPlanError::Storage { message: "production plan lock was poisoned".to_owned() }
}
