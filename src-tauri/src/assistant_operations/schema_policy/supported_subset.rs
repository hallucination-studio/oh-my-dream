use serde_json::{Map, Value};

pub(super) fn is_function_tool_root(object: &Map<String, Value>) -> bool {
    object.get("type").and_then(Value::as_str) == Some("object")
}

pub(super) fn is_object_schema(object: &Map<String, Value>) -> bool {
    schema_type_contains(object, "object") || object.keys().any(|key| is_object_keyword(key))
}

pub(super) fn unsupported_keyword(object: &Map<String, Value>) -> Option<&'static str> {
    const UNSUPPORTED: &[&str] = &[
        "if",
        "then",
        "else",
        "not",
        "additionalItems",
        "patternProperties",
        "dependencies",
        "propertyNames",
        "prefixItems",
        "unevaluatedItems",
        "unevaluatedProperties",
        "dependentSchemas",
        "contentSchema",
    ];
    UNSUPPORTED
        .iter()
        .copied()
        .find(|keyword| object.contains_key(*keyword))
        .or_else(|| object.get("items").is_some_and(Value::is_array).then_some("items"))
}

pub(super) fn has_schema_constraint(object: &Map<String, Value>) -> bool {
    let direct_constraint = object.keys().any(|key| {
        matches!(
            key.as_str(),
            "$ref"
                | "type"
                | "enum"
                | "const"
                | "multipleOf"
                | "maximum"
                | "exclusiveMaximum"
                | "minimum"
                | "exclusiveMinimum"
                | "maxLength"
                | "minLength"
                | "pattern"
                | "items"
                | "additionalItems"
                | "maxItems"
                | "minItems"
                | "uniqueItems"
                | "contains"
                | "maxProperties"
                | "minProperties"
                | "properties"
                | "patternProperties"
                | "additionalProperties"
                | "dependencies"
                | "propertyNames"
                | "anyOf"
                | "oneOf"
                | "not"
        )
    });
    direct_constraint
        || has_nonempty_array(object, "required")
        || has_nonempty_array(object, "allOf")
        || object.contains_key("if")
        || object.contains_key("then")
        || object.contains_key("else")
}

pub(super) fn is_subschema_keyword(keyword: &str) -> bool {
    matches!(keyword, "items" | "contains" | "additionalProperties" | "allOf" | "anyOf" | "oneOf")
}

pub(super) fn resolve_local_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    reference.strip_prefix('#').and_then(|pointer| root.pointer(pointer))
}

pub(super) fn escape_pointer(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn is_object_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "maxProperties"
            | "minProperties"
            | "required"
            | "properties"
            | "patternProperties"
            | "additionalProperties"
            | "dependencies"
            | "propertyNames"
    )
}

fn schema_type_contains(object: &Map<String, Value>, expected: &str) -> bool {
    match object.get("type") {
        Some(Value::String(value)) => value == expected,
        Some(Value::Array(values)) => values.iter().any(|value| value.as_str() == Some(expected)),
        _ => false,
    }
}

fn has_nonempty_array(object: &Map<String, Value>, keyword: &str) -> bool {
    object.get(keyword).and_then(Value::as_array).is_some_and(|values| !values.is_empty())
}
