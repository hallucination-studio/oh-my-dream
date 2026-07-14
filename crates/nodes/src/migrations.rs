//! Frozen legacy capability aliases and degraded-node resolution.

use engine::{CapabilityRef, DEFAULT_CAPABILITY_VERSION, NodeParams, NodeRegistry, WorkflowNode};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use thiserror::Error;

/// Result of resolving one persisted Workflow node against exact registrations.
#[derive(Debug, Clone)]
pub struct CapabilityNodeResolution {
    /// The node is preserved even when its capability is degraded.
    pub node: WorkflowNode,
    /// Current availability of the exact capability and params.
    pub status: CapabilityNodeStatus,
}

/// Availability state for a persisted capability node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityNodeStatus {
    /// The exact ref exists and params normalized successfully.
    Ready,
    /// The node belongs to a legacy non-capability registration.
    Legacy,
    /// The node remains readable but cannot currently be executed.
    Degraded(DegradedCapabilityReason),
}

/// Why a persisted node was reopened in degraded mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DegradedCapabilityReason {
    /// The id exists, but its persisted exact version is unavailable.
    MissingExactVersion { reference: CapabilityRef },
    /// The exact registration exists but stored params no longer validate.
    InvalidParams { message: String },
    /// No registration or legacy factory exists for the node type.
    UnknownCapability { reference: CapabilityRef },
}

/// Resolves one node without deleting it when its exact ref is unavailable.
pub fn resolve_workflow_node(
    registry: &NodeRegistry,
    node: &WorkflowNode,
) -> CapabilityNodeResolution {
    let persisted = CapabilityRef::new(node.type_id.clone(), node.contract_version.clone());
    if !registry.contains_capability_type(&node.type_id) {
        if registry.contains(&node.type_id) {
            return CapabilityNodeResolution {
                node: node.clone(),
                status: CapabilityNodeStatus::Legacy,
            };
        }
        return CapabilityNodeResolution {
            node: node.clone(),
            status: CapabilityNodeStatus::Degraded(DegradedCapabilityReason::UnknownCapability {
                reference: persisted,
            }),
        };
    }

    match registry.normalize_workflow_node(node) {
        Ok(normalized) => {
            CapabilityNodeResolution { node: normalized, status: CapabilityNodeStatus::Ready }
        }
        Err(engine::EngineError::UnknownCapabilityVersion {
            type_id,
            contract_version,
            ..
        }) => CapabilityNodeResolution {
            node: node.clone(),
            status: CapabilityNodeStatus::Degraded(
                DegradedCapabilityReason::MissingExactVersion {
                    reference: CapabilityRef::new(type_id, contract_version),
                },
            ),
        },
        Err(engine::EngineError::InvalidCapabilityParams { source, .. }) => {
            CapabilityNodeResolution {
                node: node.clone(),
                status: CapabilityNodeStatus::Degraded(
                    DegradedCapabilityReason::InvalidParams { message: source.to_string() },
                ),
            }
        }
        Err(engine::EngineError::InvalidCapabilitySelector { reason, .. }) => {
            CapabilityNodeResolution {
                node: node.clone(),
                status: CapabilityNodeStatus::Degraded(
                    DegradedCapabilityReason::InvalidParams { message: reason },
                ),
            }
        }
        Err(_) => CapabilityNodeResolution {
            node: node.clone(),
            status: CapabilityNodeStatus::Degraded(DegradedCapabilityReason::UnknownCapability {
                reference: CapabilityRef::new(node.type_id.clone(), node.contract_version.clone()),
            }),
        },
    }
}

/// Migrates a legacy node with omitted `contract_version` through frozen rules.
pub fn migrate_legacy_node(raw: Value) -> Result<WorkflowNode, CapabilityMigrationError> {
    let object = raw.as_object().ok_or(CapabilityMigrationError::NodeMustBeObject)?;
    if object.contains_key("contract_version") {
        return serde_json::from_value(raw).map_err(CapabilityMigrationError::InvalidNode);
    }
    let mut node: WorkflowNode =
        serde_json::from_value(raw).map_err(CapabilityMigrationError::InvalidNode)?;
    node.params = normalize_frozen_legacy_params(&node.type_id, &node.params)?;
    node.contract_version = DEFAULT_CAPABILITY_VERSION.to_owned();
    Ok(node)
}

/// Returns frozen migration examples used by compatibility tests and review.
#[must_use]
pub fn frozen_legacy_examples() -> Vec<(Value, NodeParams)> {
    vec![
        (
            json!({
                "id": "prompt",
                "type": "TextPrompt",
                "params": {"prompt": "a moonlit house"},
                "inputs": {}
            }),
            map([("text", json!("a moonlit house"))]),
        ),
        (
            json!({
                "id": "image",
                "type": "TextToImage",
                "params": {},
                "inputs": {}
            }),
            map([("model", json!("mock-image"))]),
        ),
        (
            json!({
                "id": "video",
                "type": "ImageToVideo",
                "params": {"duration_seconds": 4.0},
                "inputs": {}
            }),
            map([("model", json!("mock-video")), ("duration", json!(4.0))]),
        ),
    ]
}

fn normalize_frozen_legacy_params(
    type_id: &str,
    params: &NodeParams,
) -> Result<NodeParams, CapabilityMigrationError> {
    match type_id {
        "TextPrompt" => {
            reject_unknown(params, &["text", "prompt"])?;
            let text = string_value(params, &["text", "prompt"])?.unwrap_or_default();
            Ok(map([("text", json!(text))]))
        }
        "TextToImage" => {
            reject_unknown(params, &["model", "negative_prompt", "steps", "seed"])?;
            let model =
                string_value(params, &["model"])?.unwrap_or_else(|| "mock-image".to_owned());
            let negative_prompt = string_value(params, &["negative_prompt"])?;
            let steps = optional_value::<u32>(params, "steps")?;
            let seed = optional_value::<u64>(params, "seed")?;
            if steps == Some(0) {
                return Err(invalid_param("steps", "must be at least 1"));
            }
            let mut normalized = map([("model", json!(model))]);
            insert_optional(&mut normalized, "negative_prompt", negative_prompt)?;
            insert_optional(&mut normalized, "steps", steps)?;
            insert_optional(&mut normalized, "seed", seed)?;
            Ok(normalized)
        }
        "ImageToVideo" => {
            reject_unknown(params, &["model", "duration", "duration_seconds", "fps"])?;
            let model =
                string_value(params, &["model"])?.unwrap_or_else(|| "mock-video".to_owned());
            let duration = optional_alias_value::<f32>(params, &["duration", "duration_seconds"])?;
            let fps = optional_value::<u32>(params, "fps")?;
            if duration.is_some_and(|value| !value.is_finite() || value <= 0.0) {
                return Err(invalid_param("duration", "must be a positive finite number"));
            }
            if fps == Some(0) {
                return Err(invalid_param("fps", "must be at least 1"));
            }
            let mut normalized = map([("model", json!(model))]);
            insert_optional(&mut normalized, "duration", duration)?;
            insert_optional(&mut normalized, "fps", fps)?;
            Ok(normalized)
        }
        "VideoConcat" => {
            reject_unknown(params, &[])?;
            Ok(NodeParams::new())
        }
        _ => Err(CapabilityMigrationError::UnsupportedLegacyType { type_id: type_id.to_owned() }),
    }
}

fn string_value(
    params: &NodeParams,
    names: &[&str],
) -> Result<Option<String>, CapabilityMigrationError> {
    names
        .iter()
        .find_map(|name| params.get(*name).map(|value| (name, value)))
        .map(|(name, value)| {
            value.as_str().map(str::to_owned).ok_or_else(|| invalid_param(name, "must be a string"))
        })
        .transpose()
}

fn optional_alias_value<T: DeserializeOwned>(
    params: &NodeParams,
    names: &[&str],
) -> Result<Option<T>, CapabilityMigrationError> {
    names
        .iter()
        .find_map(|name| params.get(*name).map(|value| (*name, value)))
        .map(|(name, value)| {
            serde_json::from_value(value.clone())
                .map_err(|source| invalid_param(name, source.to_string()))
        })
        .transpose()
}

fn optional_value<T: DeserializeOwned>(
    params: &NodeParams,
    name: &str,
) -> Result<Option<T>, CapabilityMigrationError> {
    optional_alias_value(params, &[name])
}

fn insert_optional<T: serde::Serialize>(
    params: &mut NodeParams,
    name: &str,
    value: Option<T>,
) -> Result<(), CapabilityMigrationError> {
    if let Some(value) = value {
        let value = serde_json::to_value(value)
            .map_err(|source| invalid_param(name, source.to_string()))?;
        params.insert(name.to_owned(), value);
    }
    Ok(())
}

fn reject_unknown(params: &NodeParams, allowed: &[&str]) -> Result<(), CapabilityMigrationError> {
    if let Some(name) = params.keys().find(|name| !allowed.contains(&name.as_str())) {
        return Err(invalid_param(name, "unknown parameter"));
    }
    Ok(())
}

fn invalid_param(name: &str, reason: impl Into<String>) -> CapabilityMigrationError {
    CapabilityMigrationError::InvalidParam { name: name.to_owned(), reason: reason.into() }
}

fn map<const N: usize>(entries: [(&str, Value); N]) -> NodeParams {
    entries.into_iter().map(|(name, value)| (name.to_owned(), value)).collect()
}

/// Errors raised by frozen legacy Workflow migration.
#[derive(Debug, Error)]
pub enum CapabilityMigrationError {
    /// Legacy input was not an object.
    #[error("legacy capability node must be an object")]
    NodeMustBeObject,
    /// Legacy input could not be decoded as a Workflow node.
    #[error("invalid legacy capability node: {0}")]
    InvalidNode(serde_json::Error),
    /// The legacy type has no frozen migration contract.
    #[error("no frozen migration contract for capability `{type_id}`")]
    UnsupportedLegacyType { type_id: String },
    /// A legacy parameter did not match its frozen type contract.
    #[error("invalid legacy parameter `{name}`: {reason}")]
    InvalidParam { name: String, reason: String },
}
