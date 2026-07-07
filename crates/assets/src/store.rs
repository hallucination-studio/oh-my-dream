//! SQLite metadata plus managed on-disk asset files.

use crate::error::{AssetError, Result};
use crate::model::{Asset, AssetKind, NewAsset};
use crate::thumbnail::generate_thumbnail;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

const DATABASE_FILE: &str = "assets.sqlite";

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
        let id = self.next_asset_id()?;
        let created_at = unix_timestamp(&id)?;
        let stored_path = self.copy_asset_file(&id, &asset.file_path)?;
        let thumbnail_path =
            generate_thumbnail(&id, asset.kind, &stored_path, &self.thumbnails_dir)?;
        let stored = Asset {
            id,
            kind: asset.kind,
            file_path: path_to_string(&stored_path),
            thumbnail_path: Some(path_to_string(&thumbnail_path)),
            workflow_snapshot: asset.workflow_snapshot,
            source_node_id: asset.source_node_id,
            tags: asset.tags,
            created_at,
        };
        self.insert_metadata(&stored)?;
        info!(asset_id = %stored.id, kind = kind_as_str(stored.kind), "asset stored");
        Ok(stored)
    }

    /// Returns one asset by id.
    pub fn get(&self, id: &str) -> Result<Asset> {
        self.query_one(id)?.ok_or_else(|| AssetError::NotFound { id: id.to_owned() })
    }

    /// Lists assets, optionally filtering by kind, newest first.
    pub fn list(&self, kind: Option<AssetKind>) -> Result<Vec<Asset>> {
        match kind {
            Some(kind) => self.query_many(
                "SELECT id, kind, file_path, thumbnail_path, workflow_snapshot, \
                 source_node_id, tags, created_at FROM assets \
                 WHERE kind = ?1 ORDER BY created_at DESC, id DESC",
                &[kind_as_str(kind)],
            ),
            None => self.query_many(
                "SELECT id, kind, file_path, thumbnail_path, workflow_snapshot, \
                 source_node_id, tags, created_at FROM assets \
                 ORDER BY created_at DESC, id DESC",
                &[],
            ),
        }
    }

    /// Returns the workflow snapshot stored for an asset.
    pub fn workflow_snapshot(&self, id: &str) -> Result<serde_json::Value> {
        Ok(self.get(id)?.workflow_snapshot)
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
                    source_node_id TEXT,
                    tags TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );",
            )
            .map_err(|source| storage_error(format!("migrate asset database: {source}")))
    }

    fn next_asset_id(&self) -> Result<String> {
        let next: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(CAST(SUBSTR(id, 7) AS INTEGER)), 0) + 1 \
                 FROM assets WHERE id LIKE 'asset-%'",
                [],
                |row| row.get(0),
            )
            .map_err(|source| storage_error(format!("allocate next asset id: {source}")))?;
        Ok(format!("asset-{next:016}"))
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
                    source_node_id, tags, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    asset.id,
                    kind_as_str(asset.kind),
                    asset.file_path,
                    asset.thumbnail_path,
                    snapshot,
                    asset.source_node_id,
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
                "SELECT id, kind, file_path, thumbnail_path, workflow_snapshot, \
                 source_node_id, tags, created_at FROM assets WHERE id = ?1",
                [id],
                asset_row,
            )
            .optional()
            .map_err(|source| storage_error(format!("query asset `{id}`: {source}")))?;
        row.map(AssetRow::into_asset).transpose()
    }

    fn query_many(&self, sql: &str, params: &[&str]) -> Result<Vec<Asset>> {
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(|source| storage_error(format!("prepare asset list query: {source}")))?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(params.iter().copied()), asset_row)
            .map_err(|source| storage_error(format!("query asset list: {source}")))?;
        collect_assets(rows)
    }
}

struct AssetRow {
    id: String,
    kind: String,
    file_path: String,
    thumbnail_path: Option<String>,
    workflow_snapshot: String,
    source_node_id: Option<String>,
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
            source_node_id: self.source_node_id,
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
        source_node_id: row.get(5)?,
        tags: row.get(6)?,
        created_at: row.get(7)?,
    })
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

fn kind_as_str(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Image => "image",
        AssetKind::Video => "video",
    }
}

fn parse_kind(asset_id: &str, value: &str) -> Result<AssetKind> {
    match value {
        "image" => Ok(AssetKind::Image),
        "video" => Ok(AssetKind::Video),
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
