use super::*;

type RawFinalizationRow = (Vec<u8>, Vec<u8>, i64, i64, Vec<u8>);

pub(super) fn load_asset(
    connection: &Connection,
    asset_id: AssetId,
) -> Result<Option<AssetAggregate>, AssetApplicationError> {
    connection
        .query_row(
            "SELECT asset_id, project_id, media_kind, content_state, created_at, body_json
             FROM assets WHERE asset_id = ?1",
            [asset_id.as_uuid().as_bytes().as_slice()],
            read_asset_row,
        )
        .optional()
        .map_err(|_| storage())?
        .map(decode_asset)
        .transpose()
}

pub(super) fn load_by_output_key(
    connection: &Connection,
    key: &AssetNodeOutputKey,
) -> Result<Option<AssetAggregate>, AssetApplicationError> {
    let asset_id = connection
        .query_row(
            "SELECT asset_id FROM asset_node_output_bindings
             WHERE workflow_run_id = ?1 AND node_execution_id = ?2
               AND output_key = ?3 AND ordinal = ?4",
            params![
                key.workflow_run_id().as_uuid().as_bytes(),
                key.node_execution_id().as_uuid().as_bytes(),
                key.output_key().as_str(),
                i64::from(key.ordinal()),
            ],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(|_| storage())?;
    asset_id
        .map(|value| {
            let id = AssetId::from_uuid(uuid(&value)?).map_err(|_| storage())?;
            load_asset(connection, id)?.ok_or_else(storage)
        })
        .transpose()
}

pub(super) fn list_assets(
    connection: &Connection,
    query: AssetListQuery,
) -> Result<AssetListPage, AssetApplicationError> {
    let limit = i64::from(query.limit().get()) + 1;
    let kind = query.media_kind().map(row::encode_media_kind);
    let cursor_time = query.cursor().map(|value| value.created_at().as_utc_milliseconds());
    let cursor_id = query.cursor().map(|value| value.asset_id().as_uuid());
    let mut statement = connection
        .prepare(
            "SELECT asset_id, project_id, media_kind, content_state, created_at, body_json
             FROM assets
             WHERE project_id = ?1
               AND (?2 IS NULL OR media_kind = ?2)
               AND (?3 IS NULL OR created_at < ?3 OR (created_at = ?3 AND asset_id < ?4))
             ORDER BY created_at DESC, asset_id DESC LIMIT ?5",
        )
        .map_err(|_| storage())?;
    let rows = statement
        .query_map(
            params![
                query.project_id().as_uuid().as_bytes(),
                kind,
                cursor_time,
                cursor_id.as_ref().map(Uuid::as_bytes),
                limit,
            ],
            read_asset_row,
        )
        .map_err(|_| storage())?;
    let mut assets = rows
        .map(|row| row.map_err(|_| storage()).and_then(decode_asset))
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = assets.len() > usize::from(query.limit().get());
    assets.truncate(usize::from(query.limit().get()));
    let next = has_more
        .then(|| assets.last())
        .flatten()
        .map(|asset| AssetListCursor::new(asset.created_at(), asset.id()));
    Ok(AssetListPage::new(assets, next))
}

pub(super) fn load_finalization(
    connection: &Connection,
    id: AssetContentFinalizationId,
) -> Result<Option<FinalizationRow>, AssetApplicationError> {
    connection
        .query_row(
            "SELECT finalization_id, asset_id, created_at, completed, body_json
             FROM asset_content_finalizations WHERE finalization_id = ?1",
            [id.as_uuid().as_bytes().as_slice()],
            read_finalization_row,
        )
        .optional()
        .map_err(|_| storage())?
        .map(decode_raw_finalization)
        .transpose()
}

pub(super) fn list_finalizations(
    connection: &Connection,
    cursor: Option<AssetFinalizationRecoveryCursor>,
    limit: AssetPageLimit,
) -> Result<AssetContentFinalizationRecoveryPage, AssetApplicationError> {
    let mut statement = connection
        .prepare(
            "SELECT finalization_id, asset_id, created_at, completed, body_json
             FROM asset_content_finalizations
             WHERE completed = 0
               AND (?1 IS NULL OR created_at > ?1 OR (created_at = ?1 AND finalization_id > ?2))
             ORDER BY created_at, finalization_id LIMIT ?3",
        )
        .map_err(|_| storage())?;
    let cursor_time = cursor.map(|value| value.created_at().as_utc_milliseconds());
    let cursor_id = cursor.map(|value| value.finalization_id().as_uuid());
    let rows = statement
        .query_map(
            params![
                cursor_time,
                cursor_id.as_ref().map(Uuid::as_bytes),
                i64::from(limit.get()) + 1,
            ],
            read_finalization_row,
        )
        .map_err(|_| storage())?;
    let mut values = rows
        .map(|row| row.map_err(|_| storage()).and_then(decode_raw_finalization))
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = values.len() > usize::from(limit.get());
    values.truncate(usize::from(limit.get()));
    let finalizations = values.into_iter().map(|row| row.value).collect::<Vec<_>>();
    let next = has_more.then(|| finalizations.last()).flatten().map(|value| {
        AssetFinalizationRecoveryCursor::new(value.created_at(), value.finalization_id())
    });
    Ok(AssetContentFinalizationRecoveryPage::new(finalizations, next))
}

pub(super) fn list_available(
    connection: &Connection,
    cursor: Option<AssetAvailableContentRecoveryCursor>,
    limit: AssetPageLimit,
) -> Result<AssetAvailableContentRecoveryPage, AssetApplicationError> {
    let mut statement = connection
        .prepare(
            "SELECT asset_id, project_id, media_kind, content_state, created_at, body_json
             FROM assets WHERE content_state = 1
               AND (?1 IS NULL OR created_at > ?1 OR (created_at = ?1 AND asset_id > ?2))
             ORDER BY created_at, asset_id LIMIT ?3",
        )
        .map_err(|_| storage())?;
    let cursor_time = cursor.map(|value| value.created_at().as_utc_milliseconds());
    let cursor_id = cursor.map(|value| value.asset_id().as_uuid());
    let rows = statement
        .query_map(
            params![
                cursor_time,
                cursor_id.as_ref().map(Uuid::as_bytes),
                i64::from(limit.get()) + 1,
            ],
            read_asset_row,
        )
        .map_err(|_| storage())?;
    let mut assets = rows
        .map(|row| row.map_err(|_| storage()).and_then(decode_asset))
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = assets.len() > usize::from(limit.get());
    assets.truncate(usize::from(limit.get()));
    let next = has_more
        .then(|| assets.last())
        .flatten()
        .map(|asset| AssetAvailableContentRecoveryCursor::new(asset.created_at(), asset.id()));
    Ok(AssetAvailableContentRecoveryPage::new(assets, next))
}

pub(super) fn commit_pending(
    connection: &mut Connection,
    command: AssetCommitPendingContentCommand,
    output_key: Option<AssetNodeOutputKey>,
) -> Result<AssetCommitWorkflowNodeOutputPendingResult, AssetApplicationError> {
    let transaction = connection.transaction().map_err(|_| storage())?;
    if let Some(key) = output_key.as_ref()
        && let Some(existing) = load_by_output_key(&transaction, key)?
    {
        transaction.commit().map_err(|_| storage())?;
        return Ok(AssetCommitWorkflowNodeOutputPendingResult::OutputKeyAlreadyBound {
            asset: Box::new(existing),
        });
    }
    insert_asset(&transaction, command.asset())?;
    insert_finalization(&transaction, command.finalization())?;
    if let Some(key) = output_key {
        insert_output_key(&transaction, &key, command.asset().id())?;
    }
    insert_ready_post_commit_effect(
        &transaction,
        DesktopPostCommitEffectId::from_uuid(command.effect().finalization_id().as_uuid())
            .map_err(|_| storage())?,
        DesktopPostCommitEffect::Asset(command.effect()),
        DesktopPostCommitTimestamp::from_epoch_millis(
            command.asset().created_at().as_utc_milliseconds(),
        )
        .map_err(|_| storage())?,
    )
    .map_err(|_| storage())?;
    transaction.commit().map_err(|_| storage())?;
    Ok(AssetCommitWorkflowNodeOutputPendingResult::Committed)
}

pub(super) fn commit_available(
    connection: &mut Connection,
    command: AssetCommitFinalizedContentAvailableCommand,
) -> Result<(), AssetApplicationError> {
    let transaction = connection.transaction().map_err(|_| storage())?;
    let current = load_asset(&transaction, command.asset().id())?.ok_or_else(identity)?;
    let finalization =
        load_finalization(&transaction, command.finalization_id())?.ok_or_else(identity)?;
    if current == *command.asset() && finalization.completed {
        transaction.commit().map_err(|_| storage())?;
        return Ok(());
    }
    let AssetManagedContentState::Pending { descriptor, finalization_id } = current.content_state()
    else {
        return Err(identity());
    };
    if !same_immutable(&current, command.asset())
        || *finalization_id != command.finalization_id()
        || finalization.completed
        || finalization.value.asset_id() != current.id()
        || finalization.value.descriptor() != descriptor
        || descriptor != command.asset().content_state().descriptor()
    {
        return Err(identity());
    }
    update_asset(&transaction, command.asset())?;
    complete_finalization(&transaction, command.finalization_id())?;
    transaction.commit().map_err(|_| storage())
}

pub(super) fn commit_missing(
    connection: &mut Connection,
    command: AssetCommitContentMissingCommand,
) -> Result<(), AssetApplicationError> {
    let transaction = connection.transaction().map_err(|_| storage())?;
    let current = load_asset(&transaction, command.asset().id())?.ok_or_else(identity)?;
    if current == *command.asset() {
        transaction.commit().map_err(|_| storage())?;
        return Ok(());
    }
    if !same_immutable(&current, command.asset()) {
        return Err(identity());
    }
    match (current.content_state(), command.finalization_id()) {
        (AssetManagedContentState::Pending { descriptor, finalization_id }, Some(expected))
            if *finalization_id == expected
                && descriptor == command.asset().content_state().descriptor() =>
        {
            let row = load_finalization(&transaction, expected)?.ok_or_else(identity)?;
            if row.completed
                || row.value.asset_id() != current.id()
                || row.value.descriptor() != descriptor
            {
                return Err(identity());
            }
            complete_finalization(&transaction, expected)?;
        }
        (AssetManagedContentState::Available { descriptor }, None)
            if descriptor == command.asset().content_state().descriptor() => {}
        _ => return Err(identity()),
    }
    update_asset(&transaction, command.asset())?;
    transaction.commit().map_err(|_| storage())
}

fn insert_asset(
    connection: &Connection,
    asset: &AssetAggregate,
) -> Result<(), AssetApplicationError> {
    connection
        .execute(
            "INSERT INTO assets
             (asset_id, project_id, media_kind, content_state, created_at, body_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                asset.id().as_uuid().as_bytes(),
                asset.project_id().as_uuid().as_bytes(),
                row::encode_media_kind(asset.media_kind()),
                row::encode_content_state(asset.content_state()),
                asset.created_at().as_utc_milliseconds(),
                encode_asset(asset)?,
            ],
        )
        .map(|_| ())
        .map_err(|_| identity())
}

fn update_asset(
    connection: &Connection,
    asset: &AssetAggregate,
) -> Result<(), AssetApplicationError> {
    let changed = connection
        .execute(
            "UPDATE assets SET content_state = ?1, body_json = ?2 WHERE asset_id = ?3",
            params![
                row::encode_content_state(asset.content_state()),
                encode_asset(asset)?,
                asset.id().as_uuid().as_bytes(),
            ],
        )
        .map_err(|_| storage())?;
    if changed == 1 { Ok(()) } else { Err(identity()) }
}

fn insert_finalization(
    connection: &Connection,
    value: &AssetContentFinalization,
) -> Result<(), AssetApplicationError> {
    connection
        .execute(
            "INSERT INTO asset_content_finalizations
             (finalization_id, asset_id, created_at, completed, body_json)
             VALUES (?1, ?2, ?3, 0, ?4)",
            params![
                value.finalization_id().as_uuid().as_bytes(),
                value.asset_id().as_uuid().as_bytes(),
                value.created_at().as_utc_milliseconds(),
                row::encode_finalization(value)?,
            ],
        )
        .map(|_| ())
        .map_err(|_| identity())
}

fn insert_output_key(
    connection: &Connection,
    key: &AssetNodeOutputKey,
    asset_id: AssetId,
) -> Result<(), AssetApplicationError> {
    connection
        .execute(
            "INSERT INTO asset_node_output_bindings
             (workflow_run_id, node_execution_id, output_key, ordinal, asset_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                key.workflow_run_id().as_uuid().as_bytes(),
                key.node_execution_id().as_uuid().as_bytes(),
                key.output_key().as_str(),
                i64::from(key.ordinal()),
                asset_id.as_uuid().as_bytes(),
            ],
        )
        .map(|_| ())
        .map_err(|_| identity())
}

fn complete_finalization(
    connection: &Connection,
    id: AssetContentFinalizationId,
) -> Result<(), AssetApplicationError> {
    let changed = connection
        .execute(
            "UPDATE asset_content_finalizations SET completed = 1
             WHERE finalization_id = ?1 AND completed = 0",
            [id.as_uuid().as_bytes().as_slice()],
        )
        .map_err(|_| storage())?;
    if changed == 1 { Ok(()) } else { Err(identity()) }
}

fn read_asset_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AssetRow> {
    Ok(AssetRow {
        asset_id: row.get(0)?,
        project_id: row.get(1)?,
        media_kind: row.get(2)?,
        content_state: row.get(3)?,
        created_at: row.get(4)?,
        body_json: row.get(5)?,
    })
}

fn read_finalization_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawFinalizationRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
}

fn decode_raw_finalization(
    row: RawFinalizationRow,
) -> Result<FinalizationRow, AssetApplicationError> {
    decode_finalization(row.0, row.1, row.2, row.3, row.4)
}
