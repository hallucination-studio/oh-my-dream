//! SQLite metadata plus managed on-disk asset files.

use crate::error::{AssetError, Result};
use crate::model::{Asset, AssetKind, AssetQuery, AssetSort, NewAsset, Project};
use crate::thumbnail::generate_thumbnail;
use rusqlite::types::{Type, ValueRef};
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

const DATABASE_FILE: &str = "assets.sqlite";
const ASSET_COLUMNS: &str = "id, kind, file_path, thumbnail_path, workflow_snapshot, \
    prompt, project_id, project_name, source_node_id, source_node_type, model, seed, cost, \
    tags, created_at";

/// Local asset store backed by SQLite metadata and copied media files.
pub struct AssetStore {
    connection: Connection,
    files_dir: PathBuf,
    thumbnails_dir: PathBuf,
}

impl AssetStore {
    /// Opens or creates an asset store rooted at `root_dir`.
    pub fn open(root_dir: impl AsRef<Path>) -> Result<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();
        let files_dir = root_dir.join("files");
        let thumbnails_dir = root_dir.join("thumbnails");
        create_dir(&root_dir, "create asset store root")?;
        create_dir(&files_dir, "create asset files directory")?;
        create_dir(&thumbnails_dir, "create thumbnail directory")?;

        let database_path = root_dir.join(DATABASE_FILE);
        let connection = Connection::open(&database_path).map_err(|source| {
            storage_error(format!("open SQLite database `{}`: {source}", database_path.display()))
        })?;
        let store = Self { connection, files_dir, thumbnails_dir };
        store.migrate()?;
        Ok(store)
    }

    /// Inserts a new asset, copying the source file into the store.
    pub fn insert(&self, asset: NewAsset) -> Result<Asset> {
        let project_name = self.project_name(asset.project_id.as_deref())?;
        let id = self.next_asset_id()?;
        let created_at = unix_timestamp(&id)?;
        let stored_path = self.copy_asset_file(&id, &asset.file_path)?;
        let thumbnail_path =
            generate_thumbnail(&id, asset.kind, &stored_path, &self.thumbnails_dir).map_err(
                |source| cleanup_after_failure("generate thumbnail", &stored_path, source),
            )?;
        let stored = Asset {
            id,
            kind: asset.kind,
            file_path: path_to_string(&stored_path),
            thumbnail_path: Some(path_to_string(&thumbnail_path)),
            workflow_snapshot: asset.workflow_snapshot,
            prompt: asset.prompt,
            project_id: asset.project_id,
            project_name,
            source_node_id: asset.source_node_id,
            source_node_type: asset.source_node_type,
            model: asset.model,
            seed: asset.seed,
            cost: asset.cost,
            tags: asset.tags,
            created_at,
        };
        if let Err(source) = self.insert_metadata(&stored) {
            let source = cleanup_after_failure("insert asset metadata", &stored_path, source);
            return Err(cleanup_after_failure("insert asset metadata", &thumbnail_path, source));
        }
        info!(asset_id = %stored.id, kind = kind_as_str(stored.kind), "asset stored");
        Ok(stored)
    }

    /// Returns one asset by id.
    pub fn get(&self, id: &str) -> Result<Asset> {
        self.query_one(id)?.ok_or_else(|| AssetError::NotFound { id: id.to_owned() })
    }

    /// Lists assets, optionally filtering by kind, newest first.
    pub fn list(&self, kind: Option<AssetKind>) -> Result<Vec<Asset>> {
        self.list_with_query(&AssetQuery { kind, ..AssetQuery::default() })
    }

    /// Lists assets using the professional library filters.
    pub fn list_with_query(&self, query: &AssetQuery) -> Result<Vec<Asset>> {
        let mut sql = format!("SELECT {ASSET_COLUMNS} FROM assets");
        let mut clauses = Vec::new();
        let mut params = Vec::new();

        if let Some(kind) = query.kind {
            clauses.push("kind = ?".to_owned());
            params.push(kind_as_str(kind).to_owned());
        }
        if let Some(project_id) = non_empty(query.project_id.as_deref()) {
            clauses.push("project_id = ?".to_owned());
            params.push(project_id.to_owned());
        }
        if let Some(model) = non_empty(query.model.as_deref()) {
            clauses.push("model = ?".to_owned());
            params.push(model.to_owned());
        }
        if let Some(prompt) = non_empty(query.prompt.as_deref()) {
            clauses.push("prompt LIKE ?".to_owned());
            params.push(format!("%{prompt}%"));
        }

        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(order_clause(query.sort));
        self.query_many(&sql, &params)
    }

    /// Returns the workflow snapshot stored for an asset.
    pub fn workflow_snapshot(&self, id: &str) -> Result<serde_json::Value> {
        Ok(self.get(id)?.workflow_snapshot)
    }

    /// Creates a project.
    pub fn create_project(&self, name: &str) -> Result<Project> {
        let id = self.next_project_id()?;
        self.create_project_with_id(&id, name)
    }

    /// Creates a project with a caller-provided id.
    pub fn create_project_with_id(&self, id: &str, name: &str) -> Result<Project> {
        let created_at = unix_timestamp(id)?;
        let project = Project { id: id.to_owned(), name: name.to_owned(), created_at };
        self.connection
            .execute(
                "INSERT INTO projects (id, name, created_at) VALUES (?1, ?2, ?3)",
                params![project.id, project.name, project.created_at],
            )
            .map_err(|source| storage_error(format!("insert project `{}`: {source}", name)))?;
        Ok(project)
    }

    /// Lists all projects oldest first.
    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, created_at FROM projects ORDER BY created_at ASC, id ASC")
            .map_err(|source| storage_error(format!("prepare project list query: {source}")))?;
        let rows = statement
            .query_map([], project_row)
            .map_err(|source| storage_error(format!("query project list: {source}")))?;
        collect_projects(rows)
    }

    /// Returns one project by id.
    pub fn get_project(&self, id: &str) -> Result<Project> {
        self.connection
            .query_row("SELECT id, name, created_at FROM projects WHERE id = ?1", [id], project_row)
            .optional()
            .map_err(|source| storage_error(format!("query project `{id}`: {source}")))?
            .ok_or_else(|| AssetError::NotFound { id: id.to_owned() })
    }

    /// Stores a workflow JSON snapshot for a project.
    pub fn save_workflow(&self, project_id: &str, workflow: serde_json::Value) -> Result<()> {
        let workflow_json = json_string(&workflow, "serialize workflow")?;
        let updated_at = unix_timestamp(project_id)?;
        self.connection
            .execute(
                "INSERT INTO workflows (project_id, workflow_json, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(project_id) DO UPDATE SET
                    workflow_json = excluded.workflow_json,
                    updated_at = excluded.updated_at",
                params![project_id, workflow_json, updated_at],
            )
            .map_err(|source| {
                storage_error(format!("save workflow for `{project_id}`: {source}"))
            })?;
        Ok(())
    }

    /// Loads a workflow JSON snapshot for a project.
    pub fn load_workflow(&self, project_id: &str) -> Result<serde_json::Value> {
        let workflow_json: String = self
            .connection
            .query_row(
                "SELECT workflow_json FROM workflows WHERE project_id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| {
                storage_error(format!("query workflow for project `{project_id}`: {source}"))
            })?
            .ok_or_else(|| AssetError::NotFound { id: project_id.to_owned() })?;
        parse_json(project_id, &workflow_json, "workflow")
    }

    fn migrate(&self) -> Result<()> {
        self.connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS assets (
                    id TEXT PRIMARY KEY NOT NULL,
                    kind TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    thumbnail_path TEXT,
                    workflow_snapshot TEXT NOT NULL,
                    prompt TEXT,
                    project_id TEXT,
                    project_name TEXT,
                    source_node_id TEXT,
                    source_node_type TEXT,
                    model TEXT,
                    seed TEXT,
                    cost INTEGER,
                    tags TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS projects (
                    id TEXT PRIMARY KEY NOT NULL,
                    name TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS workflows (
                    project_id TEXT PRIMARY KEY NOT NULL,
                    workflow_json TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                );",
            )
            .map_err(|source| storage_error(format!("migrate asset database: {source}")))?;
        self.ensure_asset_columns()
    }

    fn next_asset_id(&self) -> Result<String> {
        let mut bytes = [0_u8; 16];
        getrandom::getrandom(&mut bytes)
            .map_err(|source| storage_error(format!("generate asset id: {source}")))?;
        Ok(format!("asset-{}", encode_hex(&bytes)))
    }

    fn project_name(&self, project_id: Option<&str>) -> Result<Option<String>> {
        project_id.map(|id| self.get_project(id).map(|project| project.name)).transpose()
    }

    fn next_project_id(&self) -> Result<String> {
        let next: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(CAST(SUBSTR(id, 9) AS INTEGER)), 0) + 1 \
                 FROM projects WHERE id LIKE 'project-%'",
                [],
                |row| row.get(0),
            )
            .map_err(|source| storage_error(format!("allocate next project id: {source}")))?;
        Ok(format!("project-{next:016}"))
    }

    fn copy_asset_file(&self, id: &str, source_path: &str) -> Result<PathBuf> {
        let source_path = Path::new(source_path);
        let stored_path = self.files_dir.join(file_name_for_id(id, source_path));
        fs::copy(source_path, &stored_path).map_err(|source| {
            storage_error(format!(
                "copy asset file `{}` to `{}`: {source}",
                source_path.display(),
                stored_path.display()
            ))
        })?;
        Ok(stored_path)
    }

    fn insert_metadata(&self, asset: &Asset) -> Result<()> {
        let snapshot = json_string(&asset.workflow_snapshot, "serialize workflow snapshot")?;
        let tags = json_string(&asset.tags, "serialize asset tags")?;
        self.connection
            .execute(
                "INSERT INTO assets (
                    id, kind, file_path, thumbnail_path, workflow_snapshot,
                    prompt, project_id, project_name, source_node_id, source_node_type,
                    model, seed, cost, tags, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    asset.id,
                    kind_as_str(asset.kind),
                    asset.file_path,
                    asset.thumbnail_path,
                    snapshot,
                    asset.prompt,
                    asset.project_id,
                    asset.project_name,
                    asset.source_node_id,
                    asset.source_node_type,
                    asset.model,
                    asset.seed.map(|value| format!("u64:{value}")),
                    asset.cost,
                    tags,
                    asset.created_at
                ],
            )
            .map_err(|source| {
                storage_error(format!("insert metadata for asset `{}`: {source}", asset.id))
            })?;
        Ok(())
    }

    fn query_one(&self, id: &str) -> Result<Option<Asset>> {
        let row = self
            .connection
            .query_row(
                &format!("SELECT {ASSET_COLUMNS} FROM assets WHERE id = ?1"),
                [id],
                asset_row,
            )
            .optional()
            .map_err(|source| storage_error(format!("query asset `{id}`: {source}")))?;
        row.map(AssetRow::into_asset).transpose()
    }

    fn query_many(&self, sql: &str, params: &[String]) -> Result<Vec<Asset>> {
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(|source| storage_error(format!("prepare asset list query: {source}")))?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(params.iter().map(String::as_str)), asset_row)
            .map_err(|source| storage_error(format!("query asset list: {source}")))?;
        collect_assets(rows)
    }

    fn ensure_asset_columns(&self) -> Result<()> {
        for (name, declaration) in [
            ("prompt", "prompt TEXT"),
            ("project_id", "project_id TEXT"),
            ("project_name", "project_name TEXT"),
            ("source_node_type", "source_node_type TEXT"),
            ("model", "model TEXT"),
            ("seed", "seed TEXT"),
            ("cost", "cost INTEGER"),
        ] {
            if !self.asset_column_exists(name)? {
                self.connection
                    .execute(&format!("ALTER TABLE assets ADD COLUMN {declaration}"), [])
                    .map_err(|source| {
                        storage_error(format!("add asset column `{name}`: {source}"))
                    })?;
            }
        }
        Ok(())
    }

    fn asset_column_exists(&self, name: &str) -> Result<bool> {
        let mut statement = self
            .connection
            .prepare("PRAGMA table_info(assets)")
            .map_err(|source| storage_error(format!("prepare table info query: {source}")))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|source| storage_error(format!("query asset table info: {source}")))?;
        for row in rows {
            let column =
                row.map_err(|source| storage_error(format!("read asset column: {source}")))?;
            if column == name {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

struct AssetRow {
    id: String,
    kind: String,
    file_path: String,
    thumbnail_path: Option<String>,
    workflow_snapshot: String,
    prompt: Option<String>,
    project_id: Option<String>,
    project_name: Option<String>,
    source_node_id: Option<String>,
    source_node_type: Option<String>,
    model: Option<String>,
    seed: Option<u64>,
    cost: Option<i64>,
    tags: String,
    created_at: i64,
}

impl AssetRow {
    fn into_asset(self) -> Result<Asset> {
        let kind = parse_kind(&self.id, &self.kind)?;
        let workflow_snapshot = parse_json(&self.id, &self.workflow_snapshot, "workflow snapshot")?;
        let tags = parse_json(&self.id, &self.tags, "tags")?;
        Ok(Asset {
            id: self.id,
            kind,
            file_path: self.file_path,
            thumbnail_path: self.thumbnail_path,
            workflow_snapshot,
            prompt: self.prompt,
            project_id: self.project_id,
            project_name: self.project_name,
            source_node_id: self.source_node_id,
            source_node_type: self.source_node_type,
            model: self.model,
            seed: self.seed,
            cost: self.cost,
            tags,
            created_at: self.created_at,
        })
    }
}

fn asset_row(row: &Row<'_>) -> rusqlite::Result<AssetRow> {
    Ok(AssetRow {
        id: row.get(0)?,
        kind: row.get(1)?,
        file_path: row.get(2)?,
        thumbnail_path: row.get(3)?,
        workflow_snapshot: row.get(4)?,
        prompt: row.get(5)?,
        project_id: row.get(6)?,
        project_name: row.get(7)?,
        source_node_id: row.get(8)?,
        source_node_type: row.get(9)?,
        model: row.get(10)?,
        seed: seed_from_row(row, 11)?,
        cost: row.get(12)?,
        tags: row.get(13)?,
        created_at: row.get(14)?,
    })
}

fn seed_from_row(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    match row.get_ref(index)? {
        ValueRef::Null => Ok(None),
        ValueRef::Integer(value) => Ok(Some(value as u64)),
        ValueRef::Text(value) => {
            let encoded = std::str::from_utf8(value).map_err(|source| {
                rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(source))
            })?;
            let parsed = match encoded.strip_prefix("u64:") {
                Some(digits) => digits.parse(),
                None => {
                    encoded.parse().or_else(|_| encoded.parse::<i64>().map(|value| value as u64))
                }
            };
            parsed.map(Some).map_err(|source| {
                rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(source))
            })
        }
        value => {
            Err(rusqlite::Error::InvalidColumnType(index, "seed".to_owned(), value.data_type()))
        }
    }
}

fn collect_assets<F>(rows: rusqlite::MappedRows<'_, F>) -> Result<Vec<Asset>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<AssetRow>,
{
    let mut assets = Vec::new();
    for row in rows {
        let row = row.map_err(|source| storage_error(format!("read asset row: {source}")))?;
        assets.push(row.into_asset()?);
    }
    Ok(assets)
}

fn project_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project { id: row.get(0)?, name: row.get(1)?, created_at: row.get(2)? })
}

fn collect_projects<F>(rows: rusqlite::MappedRows<'_, F>) -> Result<Vec<Project>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<Project>,
{
    let mut projects = Vec::new();
    for row in rows {
        projects.push(row.map_err(|source| storage_error(format!("read project row: {source}")))?);
    }
    Ok(projects)
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| (!value.is_empty()).then_some(value))
}

fn order_clause(sort: AssetSort) -> &'static str {
    match sort {
        AssetSort::Newest => " ORDER BY created_at DESC, rowid DESC",
        AssetSort::Oldest => " ORDER BY created_at ASC, rowid ASC",
        AssetSort::CostDesc => " ORDER BY cost DESC, created_at DESC, rowid DESC",
        AssetSort::CostAsc => " ORDER BY cost IS NULL ASC, cost ASC, created_at DESC, rowid DESC",
    }
}

fn kind_as_str(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Image => "image",
        AssetKind::Video => "video",
        AssetKind::Audio => "audio",
    }
}

fn parse_kind(asset_id: &str, value: &str) -> Result<AssetKind> {
    match value {
        "image" => Ok(AssetKind::Image),
        "video" => Ok(AssetKind::Video),
        "audio" => Ok(AssetKind::Audio),
        _ => Err(storage_error(format!("asset `{asset_id}` has unknown kind `{value}`"))),
    }
}

fn json_string<T: serde::Serialize>(value: &T, operation: &str) -> Result<String> {
    serde_json::to_string(value).map_err(|source| storage_error(format!("{operation}: {source}")))
}

fn parse_json<T: serde::de::DeserializeOwned>(
    asset_id: &str,
    value: &str,
    field: &str,
) -> Result<T> {
    serde_json::from_str(value)
        .map_err(|source| storage_error(format!("parse {field} for asset `{asset_id}`: {source}")))
}

fn create_dir(path: &Path, operation: &str) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|source| storage_error(format!("{operation} `{}`: {source}", path.display())))
}

fn file_name_for_id(id: &str, source_path: &Path) -> String {
    source_path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map_or_else(|| id.to_owned(), |extension| format!("{id}.{extension}"))
}

fn unix_timestamp(asset_id: &str) -> Result<i64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|source| {
        storage_error(format!("read timestamp for asset `{asset_id}`: {source}"))
    })?;
    Ok(duration.as_secs() as i64)
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn storage_error(message: String) -> AssetError {
    AssetError::Storage { message }
}

fn cleanup_after_failure(operation: &'static str, path: &Path, source: AssetError) -> AssetError {
    match fs::remove_file(path) {
        Ok(()) => source,
        Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => source,
        Err(cleanup) => AssetError::Cleanup {
            operation,
            source: Box::new(source),
            path: path.to_path_buf(),
            cleanup,
        },
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}
