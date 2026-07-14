//! Closed operation output schema for the bounded workspace snapshot.

use super::{MAX_WORKSPACE_ASSET_SUMMARIES, MAX_WORKSPACE_RUN_SUMMARIES, MAX_WORKSPACE_SELECTIONS};
use crate::dto::{MAX_WORKSPACE_PROMPT_CHARS, WorkspaceSnapshotInput, WorkspaceSnapshotOutput};
use schemars::{JsonSchema, r#gen::SchemaGenerator, schema::Schema};
use serde_json::{Value, json};

impl JsonSchema for WorkspaceSnapshotInput {
    fn schema_name() -> String {
        "WorkspaceSnapshotInput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        static_schema(json!({
            "type": "object",
            "required": [],
            "properties": {},
            "additionalProperties": false
        }))
    }
}

impl JsonSchema for WorkspaceSnapshotOutput {
    fn schema_name() -> String {
        "WorkspaceSnapshotOutput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        static_schema(json!({
            "type": "object",
            "required": [
                "scope", "project", "workflow_head", "selected_assets", "selected_nodes",
                "readiness_blockers", "assets", "runs"
            ],
            "properties": {
                "scope": scope_schema(),
                "project": project_schema(),
                "workflow_head": workflow_head_schema(),
                "selected_assets": {
                    "type": "array", "maxItems": MAX_WORKSPACE_SELECTIONS, "items": asset_schema()
                },
                "selected_nodes": {
                    "type": "array", "maxItems": MAX_WORKSPACE_SELECTIONS, "items": node_schema()
                },
                "readiness_blockers": {
                    "type": "array", "items": blocker_schema()
                },
                "assets": {
                    "type": "array", "maxItems": MAX_WORKSPACE_ASSET_SUMMARIES, "items": asset_schema()
                },
                "runs": {
                    "type": "array", "maxItems": MAX_WORKSPACE_RUN_SUMMARIES, "items": run_schema()
                }
            },
            "additionalProperties": false
        }))
    }
}

fn scope_schema() -> Value {
    closed_object(
        &["project_id", "session_id", "request_id"],
        json!({
            "project_id": { "type": "string" },
            "session_id": { "type": "string" },
            "request_id": { "type": "string" }
        }),
    )
}

fn project_schema() -> Value {
    closed_object(
        &["id", "name", "created_at"],
        json!({
            "id": { "type": "string" },
            "name": { "type": "string" },
            "created_at": { "type": "integer" }
        }),
    )
}

fn workflow_head_schema() -> Value {
    json!({
        "type": ["object", "null"],
        "required": ["project_id", "revision", "workflow"],
        "properties": {
            "project_id": { "type": "string" },
            "revision": { "type": "integer", "minimum": 1 },
            "workflow": { "type": "object" }
        },
        "additionalProperties": false
    })
}

fn asset_schema() -> Value {
    closed_object(
        &[
            "id",
            "kind",
            "project_id",
            "source_node_id",
            "source_node_type",
            "model",
            "prompt",
            "prompt_truncated",
            "created_at",
        ],
        json!({
            "id": { "type": "string" },
            "kind": { "type": "string", "enum": ["image", "video", "audio"] },
            "project_id": { "type": ["string", "null"] },
            "source_node_id": { "type": ["string", "null"] },
            "source_node_type": { "type": ["string", "null"] },
            "model": { "type": ["string", "null"] },
            "prompt": { "type": ["string", "null"], "maxLength": MAX_WORKSPACE_PROMPT_CHARS },
            "prompt_truncated": { "type": "boolean" },
            "created_at": { "type": "integer" }
        }),
    )
}

fn node_schema() -> Value {
    closed_object(
        &["id", "capability"],
        json!({
            "id": { "type": "string" },
            "capability": closed_object(
                &["id", "version"],
                json!({ "id": { "type": "string" }, "version": { "type": "string" } }),
            )
        }),
    )
}

fn blocker_schema() -> Value {
    closed_object(
        &["code", "pointer", "constraint"],
        json!({
            "code": { "type": "string" },
            "pointer": { "type": "string" },
            "constraint": { "type": "string" }
        }),
    )
}

fn run_schema() -> Value {
    closed_object(
        &["run_id", "status"],
        json!({
            "run_id": { "type": "string" },
            "status": { "type": "string", "enum": ["active"] }
        }),
    )
}

fn closed_object(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "required": required,
        "properties": properties,
        "additionalProperties": false
    })
}

fn static_schema(value: Value) -> Schema {
    match serde_json::from_value(value) {
        Ok(schema) => schema,
        Err(_) => Schema::Bool(false),
    }
}
