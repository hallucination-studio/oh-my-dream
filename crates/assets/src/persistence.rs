use crate::error::{AssetError, AssetResult};
use crate::model::{Asset, AssetKind, Project};
use rusqlite::Row;
use rusqlite::types::{Type, ValueRef};

pub(crate) const ASSET_COLUMNS: &str = "id, kind, file_path, thumbnail_path, workflow_snapshot, \
    prompt, project_id, project_name, source_node_id, source_node_type, model, seed, cost, \
    tags, created_at";

pub(crate) struct AssetRow {
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
    pub(crate) fn into_asset(self) -> AssetResult<Asset> {
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

pub(crate) fn asset_row(row: &Row<'_>) -> rusqlite::Result<AssetRow> {
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

pub(crate) fn collect_assets<F>(rows: rusqlite::MappedRows<'_, F>) -> AssetResult<Vec<Asset>>
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

pub(crate) fn project_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project { id: row.get(0)?, name: row.get(1)?, created_at: row.get(2)? })
}

pub(crate) fn collect_projects<F>(rows: rusqlite::MappedRows<'_, F>) -> AssetResult<Vec<Project>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<Project>,
{
    let mut projects = Vec::new();
    for row in rows {
        projects.push(row.map_err(|source| storage_error(format!("read project row: {source}")))?);
    }
    Ok(projects)
}

pub(crate) fn kind_as_str(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Image => "image",
        AssetKind::Video => "video",
        AssetKind::Audio => "audio",
    }
}

fn seed_from_row(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    match row.get_ref(index)? {
        ValueRef::Null => Ok(None),
        ValueRef::Integer(value) => Ok(Some(value as u64)),
        ValueRef::Text(value) => parse_text_seed(value, index).map(Some),
        value => {
            Err(rusqlite::Error::InvalidColumnType(index, "seed".to_owned(), value.data_type()))
        }
    }
}

fn parse_text_seed(value: &[u8], index: usize) -> rusqlite::Result<u64> {
    let encoded = std::str::from_utf8(value).map_err(|source| conversion_error(index, source))?;
    let parsed = match encoded.strip_prefix("u64:") {
        Some(digits) => digits.parse(),
        None => encoded.parse().or_else(|_| encoded.parse::<i64>().map(|value| value as u64)),
    };
    parsed.map_err(|source| conversion_error(index, source))
}

fn conversion_error(
    index: usize,
    source: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(source))
}

fn parse_kind(asset_id: &str, value: &str) -> AssetResult<AssetKind> {
    match value {
        "image" => Ok(AssetKind::Image),
        "video" => Ok(AssetKind::Video),
        "audio" => Ok(AssetKind::Audio),
        _ => Err(storage_error(format!("asset `{asset_id}` has unknown kind `{value}`"))),
    }
}

fn parse_json<T: serde::de::DeserializeOwned>(
    asset_id: &str,
    value: &str,
    field: &str,
) -> AssetResult<T> {
    serde_json::from_str(value)
        .map_err(|source| storage_error(format!("parse {field} for asset `{asset_id}`: {source}")))
}

fn storage_error(message: String) -> AssetError {
    AssetError::Storage { message }
}
