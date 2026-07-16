use std::collections::BTreeMap;

use schemars::{JsonSchema, schema_for};
use serde_json::Value;

use super::dto::{
    CapabilityCatalogOutput, ChangeOutput, CreatePlanInput, DescribeCapabilitiesInput, EmptyInput,
    PlanDto, PlanOutput, ReplacePlanInput, UpdatePlanItemInput, WorkflowChangeInput,
    WorkflowMutationsInput, WorkspaceOutput,
};
use crate::interfaces::AssistantApplicationError;

const TOOL_IDS: [&str; 11] = [
    "assistant.workspace.get_snapshot@1",
    "assistant.node_capability.list@1",
    "assistant.node_capability.describe@1",
    "assistant.production_plan.get@1",
    "assistant.production_plan.create@1",
    "assistant.production_plan.replace@1",
    "assistant.production_plan.update_item@1",
    "assistant.workflow.evaluate_mutation@1",
    "assistant.workflow.propose_change@1",
    "assistant.workflow.get_change@1",
    "assistant.workflow.request_apply@1",
];

/// Closed model-visible effect class; it never grants canonical Workflow or Run authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistantToolEffect {
    /// Reads one authoritative bounded projection.
    AuthoritativeRead,
    /// Mutates only Assistant-owned working memory or immutable proposals.
    AssistantStateMutation,
    /// Requests human approval without applying the Workflow.
    HumanApprovalRequest,
}

/// Exact versioned Assistant tool identity.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssistantToolId(&'static str);

impl AssistantToolId {
    /// Accepts only one of the eleven frozen tool identities.
    pub fn try_new(value: &str) -> Result<Self, AssistantApplicationError> {
        TOOL_IDS
            .iter()
            .copied()
            .find(|candidate| *candidate == value)
            .map(Self)
            .ok_or(AssistantApplicationError::ProtocolViolation)
    }

    /// Returns the canonical source-first versioned identity.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

/// One strict typed tool contract generated from canonical DTOs.
pub struct AssistantToolContract {
    id: AssistantToolId,
    input_schema: Value,
    output_schema: Value,
    description: &'static str,
    effect: AssistantToolEffect,
    requires_human_approval: bool,
}

impl AssistantToolContract {
    /// Returns the exact tool identity.
    #[must_use]
    pub const fn id(&self) -> &AssistantToolId {
        &self.id
    }

    /// Returns the generated strict input schema.
    #[must_use]
    pub const fn input_schema(&self) -> &Value {
        &self.input_schema
    }

    /// Returns the generated strict output schema.
    #[must_use]
    pub const fn output_schema(&self) -> &Value {
        &self.output_schema
    }

    /// Returns the bounded product-owned tool description.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        self.description
    }

    /// Returns the closed effect class.
    #[must_use]
    pub const fn effect(&self) -> AssistantToolEffect {
        self.effect
    }

    /// Reports whether the SDK must pause for exact human approval.
    #[must_use]
    pub const fn requires_human_approval(&self) -> bool {
        self.requires_human_approval
    }

    /// Validates untrusted JSON before typed deserialization.
    pub fn validate_input(&self, input: &Value) -> Result<(), AssistantApplicationError> {
        validate(&self.input_schema, input)
    }

    /// Validates typed serialized output before boundary delivery.
    pub fn validate_output(&self, output: &Value) -> Result<(), AssistantApplicationError> {
        validate(&self.output_schema, output)
    }
}

/// Closed catalog containing exactly the eleven Assistant tools.
pub struct AssistantToolCatalog {
    contracts: Vec<AssistantToolContract>,
    index_by_id: BTreeMap<AssistantToolId, usize>,
}

impl AssistantToolCatalog {
    /// Generates the complete exact catalog.
    pub fn try_new() -> Result<Self, AssistantApplicationError> {
        let contracts = vec![
            contract::<EmptyInput, WorkspaceOutput>(0)?,
            contract::<EmptyInput, CapabilityCatalogOutput>(1)?,
            contract::<DescribeCapabilitiesInput, CapabilityCatalogOutput>(2)?,
            contract::<EmptyInput, PlanOutput>(3)?,
            contract::<CreatePlanInput, PlanDto>(4)?,
            contract::<ReplacePlanInput, PlanDto>(5)?,
            contract::<UpdatePlanItemInput, PlanDto>(6)?,
            contract::<WorkflowMutationsInput, ChangeOutput>(7)?,
            contract::<WorkflowMutationsInput, ChangeOutput>(8)?,
            contract::<WorkflowChangeInput, ChangeOutput>(9)?,
            contract::<WorkflowChangeInput, ChangeOutput>(10)?,
        ];
        let index_by_id = contracts
            .iter()
            .enumerate()
            .map(|(index, contract)| (contract.id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        if index_by_id.len() != TOOL_IDS.len() {
            return Err(AssistantApplicationError::ProtocolViolation);
        }
        Ok(Self { contracts, index_by_id })
    }

    /// Returns contracts in the frozen order.
    #[must_use]
    pub fn contracts(&self) -> &[AssistantToolContract] {
        &self.contracts
    }

    /// Resolves one exact contract without accepting extensions.
    #[must_use]
    pub fn contract(&self, id: &str) -> Option<&AssistantToolContract> {
        let id = AssistantToolId::try_new(id).ok()?;
        self.index_by_id.get(&id).and_then(|index| self.contracts.get(*index))
    }
}

fn validate(schema: &Value, instance: &Value) -> Result<(), AssistantApplicationError> {
    jsonschema::validator_for(schema)
        .map_err(|_| AssistantApplicationError::ProtocolViolation)?
        .validate(instance)
        .map_err(|_| AssistantApplicationError::ProtocolViolation)
}

fn contract<I: JsonSchema, O: JsonSchema>(
    index: usize,
) -> Result<AssistantToolContract, AssistantApplicationError> {
    let input_schema = serde_json::to_value(schema_for!(I))
        .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    let output_schema = serde_json::to_value(schema_for!(O))
        .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    jsonschema::validator_for(&input_schema)
        .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    jsonschema::validator_for(&output_schema)
        .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    let id = *TOOL_IDS.get(index).ok_or(AssistantApplicationError::ProtocolViolation)?;
    Ok(AssistantToolContract {
        id: AssistantToolId(id),
        input_schema,
        output_schema,
        description: description(index)?,
        effect: effect(index)?,
        requires_human_approval: index == 10,
    })
}

fn effect(index: usize) -> Result<AssistantToolEffect, AssistantApplicationError> {
    match index {
        0..=3 | 7 | 9 => Ok(AssistantToolEffect::AuthoritativeRead),
        4..=6 | 8 => Ok(AssistantToolEffect::AssistantStateMutation),
        10 => Ok(AssistantToolEffect::HumanApprovalRequest),
        _ => Err(AssistantApplicationError::ProtocolViolation),
    }
}

fn description(index: usize) -> Result<&'static str, AssistantApplicationError> {
    [
        "Read the bounded authoritative workspace snapshot.",
        "List bounded active Node Capability contracts.",
        "Describe one to three exact Node Capability contracts.",
        "Read the Assistant-owned production plan.",
        "Create one Assistant-owned production plan.",
        "CAS-replace the Assistant-owned production plan.",
        "Transition one production-plan item.",
        "Evaluate ordered Workflow mutations without persistence.",
        "Persist one immutable evaluated Workflow Change.",
        "Read one exact persisted Workflow Change.",
        "Request human approval for one exact reviewed Workflow Change.",
    ]
    .get(index)
    .copied()
    .ok_or(AssistantApplicationError::ProtocolViolation)
}
