use assets::{Asset, AssetError, AssetQuery, AssetSort, AssetStore};

pub(crate) fn is_visible(asset: &Asset, project_id: &str) -> bool {
    asset.project_id.as_deref().is_none_or(|owner| owner == project_id)
}

pub(crate) fn get_visible(
    store: &AssetStore,
    project_id: &str,
    asset_id: &str,
) -> Result<Option<Asset>, AssetError> {
    match store.get(asset_id) {
        Ok(asset) if is_visible(&asset, project_id) => Ok(Some(asset)),
        Ok(_) | Err(AssetError::NotFound { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn list_visible(
    store: &AssetStore,
    project_id: &str,
    limit: usize,
) -> Result<Vec<Asset>, AssetError> {
    let assets =
        store.list_with_query(&AssetQuery { sort: AssetSort::Newest, ..AssetQuery::default() })?;
    Ok(assets.into_iter().filter(|asset| is_visible(asset, project_id)).take(limit).collect())
}
