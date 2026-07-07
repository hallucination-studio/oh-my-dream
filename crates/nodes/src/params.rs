use crate::error::NodesError;
use engine::{NodeParams, Value, ValueMap};
use serde::de::DeserializeOwned;

pub(crate) fn string_param(
    params: &NodeParams,
    names: &[&str],
    default: &str,
) -> Result<String, NodesError> {
    optional_param::<String>(params, names).map(|value| value.unwrap_or_else(|| default.to_owned()))
}

pub(crate) fn optional_param<T>(
    params: &NodeParams,
    names: &[&str],
) -> Result<Option<T>, NodesError>
where
    T: DeserializeOwned,
{
    let Some((name, value)) = find_param(params, names) else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|source| invalid_param(name, source.to_string()))
}

pub(crate) fn json_param(
    params: &NodeParams,
    name: &str,
    default: serde_json::Value,
) -> serde_json::Value {
    params.get(name).cloned().unwrap_or(default)
}

pub(crate) fn text_input<'a>(inputs: &'a ValueMap, name: &str) -> Result<&'a str, NodesError> {
    match inputs.get(name) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => wrong_input(name, "string"),
        None => Err(NodesError::MissingInput { name: name.to_owned() }),
    }
}

pub(crate) fn image_input<'a>(inputs: &'a ValueMap, name: &str) -> Result<&'a str, NodesError> {
    match inputs.get(name) {
        Some(Value::Image(value)) => Ok(value),
        Some(_) => wrong_input(name, "image"),
        None => Err(NodesError::MissingInput { name: name.to_owned() }),
    }
}

pub(crate) fn video_input<'a>(inputs: &'a ValueMap, name: &str) -> Result<&'a str, NodesError> {
    match inputs.get(name) {
        Some(Value::Video(value)) => Ok(value),
        Some(_) => wrong_input(name, "video"),
        None => Err(NodesError::MissingInput { name: name.to_owned() }),
    }
}

fn find_param<'a>(
    params: &'a NodeParams,
    names: &[&'a str],
) -> Option<(&'a str, &'a serde_json::Value)> {
    names.iter().find_map(|name| params.get(*name).map(|value| (*name, value)))
}

fn invalid_param(name: &str, reason: String) -> NodesError {
    NodesError::InvalidParam { name: name.to_owned(), reason }
}

fn wrong_input<T>(name: &str, expected: &'static str) -> Result<T, NodesError> {
    Err(NodesError::WrongInputType { name: name.to_owned(), expected })
}
