use serde::Serialize;
use serde_json::Value;

use super::{OperationEffect, OperationRegistration};

/// Serializable model-facing projection of one Rust-owned operation registration.
#[derive(Clone, Debug, Serialize)]
pub struct OperationContract {
    id: String,
    version: u32,
    description: String,
    effect: OperationEffect,
    strict_json_schema: bool,
    needs_approval: bool,
    input_schema: Value,
    output_schema: Value,
}

impl OperationContract {
    pub(super) fn from_registration(registration: &OperationRegistration) -> Self {
        let effect = registration.effect();
        Self {
            id: registration.id().to_owned(),
            version: registration.version(),
            description: registration.description().to_owned(),
            effect,
            strict_json_schema: registration.input_schema_mode().sdk_strict_json_schema(),
            needs_approval: effect == OperationEffect::PreparedApprovalExecution,
            input_schema: registration.input_schema().clone(),
            output_schema: registration.output_schema().clone(),
        }
    }
}
