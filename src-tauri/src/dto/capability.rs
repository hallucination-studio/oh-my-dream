use schemars::{JsonSchema, r#gen::SchemaGenerator, schema::Schema};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn static_schema(value: serde_json::Value, label: &str) -> Schema {
    match serde_json::from_value(value) {
        Ok(schema) => schema,
        Err(error) => {
            tracing::error!(schema = label, error = %error, "failed to build static JSON Schema");
            Schema::Bool(false)
        }
    }
}

/// Exact identity of one versioned workflow capability.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRefDto {
    /// Stable capability identifier.
    pub id: String,
    /// Exact semantic contract version.
    pub version: String,
}

/// Workflow-facing modality and mode selecting one exact capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySelectorDto {
    /// Output modality persisted as the Workflow node type.
    pub type_id: String,
    /// Discriminator persisted in `params.mode`.
    pub mode: String,
}

/// Cardinality of a capability port at the application boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityCardinalityDto {
    /// Exactly one value is accepted or produced.
    One,
    /// An ordered collection with explicit bounds is accepted.
    Many {
        /// Inclusive minimum number of values.
        minimum: usize,
        /// Inclusive maximum number of values, when bounded.
        maximum: Option<usize>,
    },
}

impl JsonSchema for CapabilityCardinalityDto {
    fn schema_name() -> String {
        "CapabilityCardinalityDto".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        static_schema(
            serde_json::json!({
                "oneOf": [
                    { "type": "string", "enum": ["one"] },
                    {
                        "type": "object",
                        "required": ["many"],
                        "properties": {
                            "many": {
                                "type": "object",
                                "required": ["minimum", "maximum"],
                                "properties": {
                                    "minimum": { "type": "integer", "minimum": 0 },
                                    "maximum": { "type": ["integer", "null"], "minimum": 0 }
                                },
                                "additionalProperties": false
                            }
                        },
                        "additionalProperties": false
                    }
                ]
            }),
            "capability cardinality",
        )
    }
}

/// Execution port metadata exposed by a capability contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPortDto {
    /// Stable named port.
    pub name: String,
    /// Canonical engine port type in snake case.
    pub port_type: String,
    /// Port cardinality and bounds.
    pub cardinality: CapabilityCardinalityDto,
    /// Whether the input must be connected or have a default.
    pub required: bool,
}

/// Immutable execution contract for one exact capability reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityContractDto {
    /// Exact capability identity represented by this contract.
    pub reference: CapabilityRefDto,
    /// Named input ports.
    pub inputs: Vec<CapabilityPortDto>,
    /// Named output ports.
    pub outputs: Vec<CapabilityPortDto>,
    /// JSON Schema for the normalized params object.
    pub params_schema: serde_json::Value,
    /// Canonical params used when no params are supplied.
    pub default_params: BTreeMap<String, serde_json::Value>,
    /// Policy-relevant execution effects.
    pub effects: Vec<CapabilityEffectDto>,
}

/// Effect classification owned by an immutable capability contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityEffectDto {
    /// Deterministic local transformation with no external effect.
    Pure,
    /// Read from managed local state without contacting an external provider.
    LocalRead,
    /// Provider, filesystem, or other external effect.
    External,
}

/// Non-authoritative display metadata for one exact capability reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPresentationDto {
    /// Short label shown in palettes and node headers.
    pub label: String,
    /// User-facing description.
    pub description: String,
    /// Presentation grouping.
    pub category: String,
    /// Search terms used by discovery and UI filtering.
    pub search_terms: Vec<String>,
}

/// Live availability projection kept separate from execution identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailabilityDto {
    /// The exact registration is available for admission or execution.
    Available,
    /// The exact registration is known but currently unavailable.
    Unavailable,
    /// The registration can be inspected but needs repair or migration.
    Degraded,
}

/// Status metadata supplied independently of a capability contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityStatusDto {
    /// Current availability state.
    pub availability: CapabilityAvailabilityDto,
    /// Safe explanation when availability is not fully ready.
    pub reason: Option<String>,
    /// Provider health marker, when the capability has an external effect.
    pub provider_health: Option<String>,
    /// Monotonic status revision for cache revalidation.
    pub status_revision: u64,
}

/// A compact palette result containing presentation and live status only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySummaryDto {
    /// Workflow selector for this exact capability.
    pub selector: CapabilitySelectorDto,
    /// Exact capability identity that may be loaded into the editor.
    pub reference: CapabilityRefDto,
    /// Non-authoritative display metadata.
    pub presentation: CapabilityPresentationDto,
    /// Current availability metadata.
    pub status: CapabilityStatusDto,
}

/// One exact capability bundle returned to the React contract cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityBundleDto {
    /// Workflow selector, absent only for an unknown exact reference.
    pub selector: Option<CapabilitySelectorDto>,
    /// Exact capability identity.
    pub reference: CapabilityRefDto,
    /// Immutable contract when the exact registration is available.
    pub contract: Option<CapabilityContractDto>,
    /// Presentation metadata when the exact registration is available.
    pub presentation: Option<CapabilityPresentationDto>,
    /// Availability and degradation reason.
    pub status: CapabilityStatusDto,
}

/// A bounded page of palette summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySearchPageDto {
    /// Summary results in stable server-defined order.
    pub capabilities: Vec<CapabilitySummaryDto>,
    /// Opaque offset cursor for the next page, when more results exist.
    pub next_cursor: Option<String>,
}

/// Exact bundles requested by the canvas cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityBundlesDto {
    /// Results preserve request order and include degraded unknown refs.
    pub capabilities: Vec<CapabilityBundleDto>,
}

impl JsonSchema for CapabilityStatusDto {
    fn schema_name() -> String {
        "CapabilityStatusDto".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        static_schema(
            serde_json::json!({
                "type": "object",
                "required": ["availability", "reason", "provider_health", "status_revision"],
                "properties": {
                    "availability": {
                        "type": "string",
                        "enum": ["available", "unavailable", "degraded"]
                    },
                    "reason": { "type": ["string", "null"] },
                    "provider_health": { "type": ["string", "null"] },
                    "status_revision": { "type": "integer", "minimum": 0 }
                },
                "additionalProperties": false
            }),
            "capability status",
        )
    }
}

/// One catalog entry combining independent contract, presentation, and status projections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityCatalogEntryDto {
    /// Workflow selector for this exact capability.
    pub selector: CapabilitySelectorDto,
    /// Immutable execution contract.
    pub contract: CapabilityContractDto,
    /// Non-authoritative presentation metadata.
    pub presentation: CapabilityPresentationDto,
    /// Current availability metadata.
    pub status: CapabilityStatusDto,
}

/// Capability catalog returned to the application and UI boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityCatalogDto {
    /// Entries in stable exact-reference order.
    pub capabilities: Vec<CapabilityCatalogEntryDto>,
}
