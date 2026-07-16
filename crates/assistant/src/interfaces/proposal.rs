//! Strict Assistant-tool Workflow mutation proposal protocol.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One strict tagged proposal for a frozen Workflow mutation action.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssistantWorkflowMutationProposalDto {
    /// Adds one exact capability node under a proposal-local alias.
    AddNode {
        /// Alias available to later proposals.
        alias: String,
        /// Exact selected capability.
        capability: AssistantCapabilityRefDto,
        /// Complete tagged parameter map.
        parameters: BTreeMap<String, AssistantParameterValueDto>,
        /// Initial persisted canvas position.
        position: AssistantCanvasPositionDto,
    },
    /// Removes one existing or earlier aliased node.
    RemoveNode {
        /// Node to remove.
        node: AssistantNodeRefDto,
    },
    /// Replaces one node's complete parameter set.
    ReplaceNodeParameters {
        /// Node to update.
        node: AssistantNodeRefDto,
        /// Complete replacement parameter map.
        parameters: BTreeMap<String, AssistantParameterValueDto>,
    },
    /// Replaces one node's capability and complete parameters.
    SelectNodeCapability {
        /// Node to update.
        node: AssistantNodeRefDto,
        /// Exact replacement capability.
        capability: AssistantCapabilityRefDto,
        /// Complete replacement parameter map.
        parameters: BTreeMap<String, AssistantParameterValueDto>,
    },
    /// Replaces one node's canvas position.
    MoveNode {
        /// Node to move.
        node: AssistantNodeRefDto,
        /// Replacement position.
        position: AssistantCanvasPositionDto,
    },
    /// Binds one role-free single input item.
    BindSingleInput {
        /// Exact target input.
        target: AssistantInputTargetDto,
        /// Exact source output.
        source: AssistantOutputSourceDto,
    },
    /// Inserts one role-bearing ordered reference item.
    InsertReferenceItem {
        /// Exact target input.
        target: AssistantInputTargetDto,
        /// Exact source output.
        source: AssistantOutputSourceDto,
        /// Capability-owned role key.
        role: String,
        /// Insertion index.
        insertion_index: u32,
    },
    /// Reorders one existing reference item.
    MoveReferenceItem {
        /// Exact target input.
        target: AssistantInputTargetDto,
        /// Stable input-item UUID text.
        input_item_id: String,
        /// Index after first removing the item.
        insertion_index_after_removal: u32,
    },
    /// Removes one stable input item.
    RemoveInputItem {
        /// Exact target input.
        target: AssistantInputTargetDto,
        /// Stable input-item UUID text.
        input_item_id: String,
    },
    /// Replaces one ordered item's role.
    SetInputItemRole {
        /// Exact target input.
        target: AssistantInputTargetDto,
        /// Stable input-item UUID text.
        input_item_id: String,
        /// Replacement capability-owned role key.
        role: String,
    },
}

/// Exact node-capability contract reference proposal.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantCapabilityRefDto {
    /// Canonical dot-separated capability ID.
    pub id: String,
    /// Non-zero major version.
    pub major: u16,
    /// Minor version.
    pub minor: u16,
}

/// Existing or earlier proposal-local node reference.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssistantNodeRefDto {
    /// Existing Workflow node UUID text.
    Id { id: String },
    /// Alias introduced by an earlier Add Node proposal.
    Alias { alias: String },
}

/// Exact proposed canvas position.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantCanvasPositionDto {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

/// Target node and declared input key.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantInputTargetDto {
    /// Existing or prior aliased target node.
    pub node: AssistantNodeRefDto,
    /// Declared input key.
    pub input: String,
}

/// Source node and declared output key.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantOutputSourceDto {
    /// Existing or prior aliased source node.
    pub node: AssistantNodeRefDto,
    /// Declared output key.
    pub output: String,
}

/// Closed Node Capability parameter boundary values accepted from the Assistant tool.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssistantParameterValueDto {
    /// Unsigned integer value.
    UnsignedInteger { value: u64 },
    /// UTF-8 text value.
    Text { value: String },
    /// Capability-owned choice key.
    Choice { value: String },
    /// Provider-independent Generation Profile reference.
    GenerationProfile { id: String, version: u32 },
    /// Managed Asset UUID text.
    ManagedAsset { id: String },
}
