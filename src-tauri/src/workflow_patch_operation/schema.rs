//! Closed model-facing JSON Schemas for Workflow patch input and output.

use super::{WorkflowApplyPatchInput, WorkflowApplyPatchOutput, WorkflowEvaluatePatchOutput};
use schemars::{JsonSchema, r#gen::SchemaGenerator, schema::Schema};
use serde_json::{Value, json};

impl JsonSchema for WorkflowApplyPatchInput {
    fn schema_name() -> String {
        "WorkflowApplyPatchInput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        static_schema(json!({
            "type": "object",
            "required": ["expected_revision", "operations"],
            "properties": {
                "expected_revision": { "type": ["integer", "null"], "minimum": 0 },
                "operations": {
                    "type": "array",
                    "minItems": 0,
                    "maxItems": 128,
                    "items": { "oneOf": operation_schemas() }
                }
            },
            "additionalProperties": false
        }))
    }
}

impl JsonSchema for WorkflowApplyPatchOutput {
    fn schema_name() -> String {
        "WorkflowApplyPatchOutput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        static_schema(json!({
            "type": "object",
            "required": [
                "workflow_head", "aliases", "readiness_blockers", "changed",
                "deduplicated", "undo_id"
            ],
            "properties": {
                "workflow_head": workflow_head_schema(),
                "aliases": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["alias", "node_id"],
                        "properties": {
                            "alias": { "type": "string" },
                            "node_id": { "type": "string" }
                        },
                        "additionalProperties": false
                    }
                },
                "readiness_blockers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["code", "pointer", "constraint"],
                        "properties": {
                            "code": { "type": "string" },
                            "pointer": { "type": "string" },
                            "constraint": { "type": "string" }
                        },
                        "additionalProperties": false
                    }
                },
                "changed": { "type": "boolean" },
                "deduplicated": { "type": "boolean" },
                "undo_id": { "type": ["string", "null"] }
            },
            "additionalProperties": false
        }))
    }
}

impl JsonSchema for WorkflowEvaluatePatchOutput {
    fn schema_name() -> String {
        "WorkflowEvaluatePatchOutput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        static_schema(json!({
            "type": "object",
            "required": ["base_revision", "workflow", "aliases", "readiness_blockers"],
            "properties": {
                "base_revision": { "type": ["integer", "null"], "minimum": 0 },
                "workflow": { "type": "object" },
                "aliases": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["alias", "node_id"],
                        "properties": {
                            "alias": { "type": "string" },
                            "node_id": { "type": "string" }
                        },
                        "additionalProperties": false
                    }
                },
                "readiness_blockers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["code", "pointer", "constraint"],
                        "properties": {
                            "code": { "type": "string" },
                            "pointer": { "type": "string" },
                            "constraint": { "type": "string" }
                        },
                        "additionalProperties": false
                    }
                }
            },
            "additionalProperties": false
        }))
    }
}

fn workflow_head_schema() -> Value {
    json!({
        "type": ["object", "null"],
        "properties": {
            "project_id": { "type": "string" },
            "revision": { "type": "integer", "minimum": 0 },
            "workflow": { "type": "object" }
        },
        "required": ["project_id", "revision", "workflow"],
        "additionalProperties": false
    })
}

fn static_schema(value: Value) -> Schema {
    match serde_json::from_value(value) {
        Ok(schema) => schema,
        Err(_) => Schema::Bool(false),
    }
}

fn node_ref_schema() -> Value {
    json!({
        "oneOf": [
            { "type": "object", "required": ["kind", "id"], "properties": {
                "kind": { "const": "id" }, "id": { "type": "string" }
            }, "additionalProperties": false },
            { "type": "object", "required": ["kind", "alias"], "properties": {
                "kind": { "const": "alias" }, "alias": { "type": "string" }
            }, "additionalProperties": false }
        ]
    })
}

fn patch_output_ref_schema() -> Value {
    json!({
        "type": "object",
        "required": ["node", "output"],
        "properties": {
            "node": node_ref_schema(),
            "output": { "type": "string" }
        },
        "additionalProperties": false
    })
}

fn capability_ref_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "version"],
        "properties": { "id": { "type": "string" }, "version": { "type": "string" } },
        "additionalProperties": false
    })
}

fn binding_schema() -> Value {
    json!({
        "oneOf": [
            { "type": "object", "required": ["kind", "source"], "properties": {
                "kind": { "const": "single" }, "source": patch_output_ref_schema()
            }, "additionalProperties": false },
            { "type": "object", "required": ["kind", "sources"], "properties": {
                "kind": { "const": "ordered_many" }, "sources": {
                    "type": "array", "items": patch_output_ref_schema()
                }
            }, "additionalProperties": false }
        ]
    })
}

fn operation_schemas() -> Vec<Value> {
    vec![
        add_node_schema(),
        replace_params_schema(),
        set_input_schema(),
        clear_input_schema(),
        remove_node_schema(),
        set_position_schema(),
    ]
}

fn add_node_schema() -> Value {
    json!({
        "type": "object", "required": ["op", "alias", "capability", "params", "position"],
        "properties": { "op": { "const": "add_node" }, "alias": { "type": "string" },
            "capability": capability_ref_schema(), "params": { "type": "object", "additionalProperties": true },
            "position": { "type": ["array", "null"], "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }
        }, "additionalProperties": false
    })
}

fn replace_params_schema() -> Value {
    json!({
        "type": "object", "required": ["op", "node", "params"],
        "properties": { "op": { "const": "replace_params" }, "node": node_ref_schema(),
            "params": { "type": "object", "additionalProperties": true }
        }, "additionalProperties": false
    })
}

fn set_input_schema() -> Value {
    json!({
        "type": "object", "required": ["op", "node", "input", "binding"],
        "properties": { "op": { "const": "set_input" }, "node": node_ref_schema(),
            "input": { "type": "string" }, "binding": binding_schema()
        }, "additionalProperties": false
    })
}

fn clear_input_schema() -> Value {
    json!({
        "type": "object", "required": ["op", "node", "input"],
        "properties": { "op": { "const": "clear_input" }, "node": node_ref_schema(),
            "input": { "type": "string" }
        }, "additionalProperties": false
    })
}

fn remove_node_schema() -> Value {
    json!({
        "type": "object", "required": ["op", "node"],
        "properties": { "op": { "const": "remove_node" }, "node": node_ref_schema() },
        "additionalProperties": false
    })
}

fn set_position_schema() -> Value {
    json!({
        "type": "object", "required": ["op", "node", "position"],
        "properties": { "op": { "const": "set_position" }, "node": node_ref_schema(),
            "position": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }
        }, "additionalProperties": false
    })
}
