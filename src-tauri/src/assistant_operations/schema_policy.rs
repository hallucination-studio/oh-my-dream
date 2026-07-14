use std::collections::HashSet;

use schemars::{JsonSchema, schema_for};
use serde_json::{Map, Value};
use thiserror::Error;

/// Input schema policy owned by an assistant operation registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationInputSchemaMode {
    /// Every object in the canonical input schema is closed.
    Strict,
    /// Only the canonical Workflow patch `params` payload remains open.
    WorkflowPatchParamsOpen,
}

impl OperationInputSchemaMode {
    /// Returns the value Task 3 should pass to SDK `strict_json_schema`.
    #[must_use]
    pub const fn sdk_strict_json_schema(self) -> bool {
        matches!(self, Self::Strict)
    }
}

/// Output schema policy for operations that return bounded dynamic documents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationOutputSchemaMode {
    /// Every output object and property is closed and strict.
    Strict,
    /// Capability descriptions keep a closed envelope around dynamic schema bodies.
    CapabilityDescribe,
    /// A closed result envelope around a portable Workflow document whose
    /// params and input maps are dynamic.
    WorkflowDocument,
}

/// Failure while creating an operation contract from canonical Rust DTOs.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum OperationRegistrationError {
    /// A canonical DTO schema could not be represented as JSON.
    #[error("failed to generate the {schema_kind} schema: {message}")]
    SchemaGeneration {
        /// Whether input or output schema generation failed.
        schema_kind: &'static str,
        /// Serde serialization detail.
        message: String,
    },
    /// The generated input schema could not initialize the Draft 7 validator.
    #[error("failed to compile the canonical input schema: {message}")]
    ValidatorCompilation {
        /// Validator compilation detail.
        message: String,
    },
    /// An operation input root was not an exact JSON object schema.
    #[error("the canonical input schema at {schema_path} must have type object")]
    InvalidInputRootSchema {
        /// Logical path to the invalid input root.
        schema_path: String,
    },
    /// An object schema was open outside the one permitted patch payload.
    #[error("the canonical {schema_kind} object at {schema_path} must be closed")]
    OpenObjectSchema {
        /// Whether the open object was in the input or output schema.
        schema_kind: &'static str,
        /// Logical JSON Schema path reached after following references.
        schema_path: String,
    },
    /// A strict object property was not listed in its required set.
    #[error("the canonical {schema_kind} object at {object_path} must require {property}")]
    MissingRequiredProperty {
        /// Whether the object was in the input or output schema.
        schema_kind: &'static str,
        /// Logical path to the object schema.
        object_path: String,
        /// Property omitted from the required set.
        property: String,
    },
    /// Patch mode did not expose exactly the canonical open params payload.
    #[error("the canonical input schema must contain an open object at #/properties/params")]
    InvalidPatchParamsSchema,
}

pub(super) fn operation_schemas<I: JsonSchema, O: JsonSchema>(
    input_mode: OperationInputSchemaMode,
    output_mode: OperationOutputSchemaMode,
) -> Result<(Value, Value), OperationRegistrationError> {
    let input = schema_value::<I>("input")?;
    validate_schema(&input, "input", input_mode, OperationOutputSchemaMode::Strict)?;
    let output = schema_value::<O>("output")?;
    validate_schema(&output, "output", OperationInputSchemaMode::Strict, output_mode)?;
    Ok((input, output))
}

fn schema_value<T: JsonSchema>(
    schema_kind: &'static str,
) -> Result<Value, OperationRegistrationError> {
    serde_json::to_value(schema_for!(T)).map_err(|error| {
        OperationRegistrationError::SchemaGeneration { schema_kind, message: error.to_string() }
    })
}

fn validate_schema(
    root: &Value,
    schema_kind: &'static str,
    input_mode: OperationInputSchemaMode,
    output_mode: OperationOutputSchemaMode,
) -> Result<(), OperationRegistrationError> {
    let mut state = ValidationState {
        root,
        schema_kind,
        input_mode,
        output_mode,
        visited_refs: HashSet::new(),
        found_open_patch_params: false,
    };
    state.validate_node(root, "#")?;
    if input_mode == OperationInputSchemaMode::WorkflowPatchParamsOpen
        && !state.found_open_patch_params
    {
        return Err(OperationRegistrationError::InvalidPatchParamsSchema);
    }
    Ok(())
}

struct ValidationState<'a> {
    root: &'a Value,
    schema_kind: &'static str,
    input_mode: OperationInputSchemaMode,
    output_mode: OperationOutputSchemaMode,
    visited_refs: HashSet<(String, bool)>,
    found_open_patch_params: bool,
}

impl ValidationState<'_> {
    fn validate_node(
        &mut self,
        node: &Value,
        schema_path: &str,
    ) -> Result<(), OperationRegistrationError> {
        if self.allows_capability_output_body(schema_path) {
            return Ok(());
        }
        let Some(object) = node.as_object() else {
            return Ok(());
        };
        self.validate_input_root(object, schema_path)?;
        self.validate_ref(object, schema_path)?;
        if is_object_schema(object) {
            self.validate_object(object, schema_path)?;
        }
        self.validate_children(object, schema_path)
    }

    fn validate_input_root(
        &self,
        object: &Map<String, Value>,
        schema_path: &str,
    ) -> Result<(), OperationRegistrationError> {
        if self.schema_kind != "input"
            || schema_path != "#"
            || object.get("type").and_then(Value::as_str) == Some("object")
        {
            return Ok(());
        }
        Err(OperationRegistrationError::InvalidInputRootSchema {
            schema_path: schema_path.to_owned(),
        })
    }

    fn validate_ref(
        &mut self,
        object: &Map<String, Value>,
        schema_path: &str,
    ) -> Result<(), OperationRegistrationError> {
        let Some(reference) = object.get("$ref").and_then(Value::as_str) else {
            return Ok(());
        };
        let allows_open = self.allows_open_patch_params(schema_path);
        if !self.visited_refs.insert((reference.to_owned(), allows_open)) {
            return Ok(());
        }
        if let Some(target) = resolve_local_ref(self.root, reference) {
            self.validate_node(target, schema_path)?;
        }
        Ok(())
    }

    fn validate_object(
        &mut self,
        object: &Map<String, Value>,
        schema_path: &str,
    ) -> Result<(), OperationRegistrationError> {
        if self.allows_open_patch_params(schema_path) {
            if object.get("additionalProperties") == Some(&Value::Bool(true)) {
                self.found_open_patch_params = true;
                return Ok(());
            }
            return Err(OperationRegistrationError::InvalidPatchParamsSchema);
        }
        if object.get("additionalProperties") == Some(&Value::Bool(false)) {
            return self.validate_required_properties(object, schema_path);
        }
        Err(OperationRegistrationError::OpenObjectSchema {
            schema_kind: self.schema_kind,
            schema_path: schema_path.to_owned(),
        })
    }

    fn validate_required_properties(
        &self,
        object: &Map<String, Value>,
        schema_path: &str,
    ) -> Result<(), OperationRegistrationError> {
        if self.input_mode != OperationInputSchemaMode::Strict
            || (self.schema_kind == "output"
                && matches!(self.output_mode, OperationOutputSchemaMode::CapabilityDescribe))
        {
            return Ok(());
        }
        let Some(properties) = object.get("properties").and_then(Value::as_object) else {
            return Ok(());
        };
        let required = object
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<HashSet<_>>();
        let Some(property) = properties.keys().find(|name| !required.contains(name.as_str()))
        else {
            return Ok(());
        };
        Err(OperationRegistrationError::MissingRequiredProperty {
            schema_kind: self.schema_kind,
            object_path: schema_path.to_owned(),
            property: property.to_owned(),
        })
    }

    fn validate_children(
        &mut self,
        object: &Map<String, Value>,
        schema_path: &str,
    ) -> Result<(), OperationRegistrationError> {
        if self.allows_open_patch_params(schema_path) && self.found_open_patch_params {
            return Ok(());
        }
        for (key, value) in object {
            match key.as_str() {
                "properties" => {
                    self.validate_schema_map(key, value, schema_path)?;
                }
                "items"
                | "contains"
                | "additionalProperties"
                | "propertyNames"
                | "not"
                | "if"
                | "then"
                | "else"
                | "allOf"
                | "anyOf"
                | "oneOf" => {
                    let path = format!("{schema_path}/{}", escape_pointer(key));
                    self.validate_child(value, &path)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn validate_schema_map(
        &mut self,
        keyword: &str,
        value: &Value,
        schema_path: &str,
    ) -> Result<(), OperationRegistrationError> {
        let Some(schemas) = value.as_object() else {
            return Ok(());
        };
        for (name, schema) in schemas {
            let path =
                format!("{schema_path}/{}/{}", escape_pointer(keyword), escape_pointer(name));
            self.validate_node(schema, &path)?;
        }
        Ok(())
    }

    fn validate_child(
        &mut self,
        value: &Value,
        schema_path: &str,
    ) -> Result<(), OperationRegistrationError> {
        match value {
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate() {
                    self.validate_node(child, &format!("{schema_path}/{index}"))?;
                }
                Ok(())
            }
            _ => self.validate_node(value, schema_path),
        }
    }

    fn allows_open_patch_params(&self, schema_path: &str) -> bool {
        self.schema_kind == "input"
            && self.input_mode == OperationInputSchemaMode::WorkflowPatchParamsOpen
            && schema_path.ends_with("/properties/params")
    }

    fn allows_capability_output_body(&self, schema_path: &str) -> bool {
        if self.schema_kind != "output" {
            return false;
        }
        if matches!(self.output_mode, OperationOutputSchemaMode::CapabilityDescribe)
            && (schema_path.ends_with("/params_schema") || schema_path.ends_with("/default_params"))
        {
            return true;
        }
        self.output_mode == OperationOutputSchemaMode::WorkflowDocument
            && (schema_path.ends_with("/workflow")
                || schema_path.ends_with("/params")
                || schema_path.ends_with("/inputs"))
    }
}

fn is_object_schema(object: &Map<String, Value>) -> bool {
    matches!(object.get("type"), Some(Value::String(value)) if value == "object")
        || matches!(
            object.get("type"),
            Some(Value::Array(values))
                if values.iter().any(|value| value.as_str() == Some("object"))
        )
        || object
            .keys()
            .any(|key| matches!(key.as_str(), "properties" | "required" | "additionalProperties"))
}

fn resolve_local_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    reference.strip_prefix('#').and_then(|pointer| root.pointer(pointer))
}

fn escape_pointer(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
