use crate::error::NodesError;
use engine::{InputValue, NodeInputs, NodeParams, Value};
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

pub(crate) fn canonicalize_mode(
    params: &NodeParams,
    normalized: &mut NodeParams,
    expected: &str,
) -> Result<(), NodesError> {
    if let Some(mode) = optional_param::<String>(params, &["mode"])?
        && mode != expected
    {
        return Err(invalid_param("mode", format!("must be `{expected}`")));
    }
    normalized.insert("mode".to_owned(), serde_json::Value::String(expected.to_owned()));
    Ok(())
}

pub(crate) fn reject_unknown_params(
    params: &NodeParams,
    allowed: &[&str],
) -> Result<(), NodesError> {
    let Some(name) = params.keys().find(|name| !allowed.contains(&name.as_str())) else {
        return Ok(());
    };
    Err(invalid_param(name, "unknown parameter".to_owned()))
}

pub(crate) fn text_input<'a>(inputs: &'a NodeInputs, name: &str) -> Result<&'a str, NodesError> {
    match inputs.get(name) {
        Some(InputValue::Single(Value::String(value))) => Ok(value),
        Some(InputValue::Single(_)) => wrong_input(name, "string"),
        Some(InputValue::OrderedMany(_)) => wrong_cardinality(name, "single"),
        None => Err(NodesError::MissingInput { name: name.to_owned() }),
    }
}

pub(crate) fn image_input<'a>(inputs: &'a NodeInputs, name: &str) -> Result<&'a str, NodesError> {
    match inputs.get(name) {
        Some(InputValue::Single(Value::Image(value))) => Ok(value),
        Some(InputValue::Single(_)) => wrong_input(name, "image"),
        Some(InputValue::OrderedMany(_)) => wrong_cardinality(name, "single"),
        None => Err(NodesError::MissingInput { name: name.to_owned() }),
    }
}

pub(crate) fn image_inputs<'a>(
    inputs: &'a NodeInputs,
    name: &str,
) -> Result<Vec<&'a str>, NodesError> {
    match inputs.get(name) {
        Some(InputValue::OrderedMany(values)) => values
            .iter()
            .map(|value| match value {
                Value::Image(image) => Ok(image.as_str()),
                _ => wrong_input(name, "image"),
            })
            .collect(),
        Some(InputValue::Single(_)) => wrong_cardinality(name, "ordered many"),
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

fn wrong_cardinality<T>(name: &str, expected: &'static str) -> Result<T, NodesError> {
    Err(NodesError::WrongInputCardinality { name: name.to_owned(), expected })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_input_errors_distinguish_missing_cardinality_and_value_type() {
        assert!(matches!(
            text_input(&NodeInputs::new(), "prompt"),
            Err(NodesError::MissingInput { .. })
        ));
        assert!(matches!(
            text_input(
                &NodeInputs::from([(
                    "prompt".to_owned(),
                    InputValue::OrderedMany(vec![Value::String("hello".to_owned())]),
                )]),
                "prompt",
            ),
            Err(NodesError::WrongInputCardinality { .. })
        ));
        assert!(matches!(
            text_input(
                &NodeInputs::from([(
                    "prompt".to_owned(),
                    InputValue::Single(Value::Image("asset".to_owned())),
                )]),
                "prompt",
            ),
            Err(NodesError::WrongInputType { .. })
        ));
    }
}
