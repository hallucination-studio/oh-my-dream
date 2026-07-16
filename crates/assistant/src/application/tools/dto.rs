use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DescribeCapabilitiesInput {
    #[schemars(length(min = 1, max = 3))]
    #[schemars(inner(length(min = 1, max = 256)))]
    pub contract_refs: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CreatePlanInput {
    #[schemars(length(min = 1, max = 120))]
    pub title: String,
    #[schemars(length(max = 128))]
    pub items: Vec<PlanItemInput>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ReplacePlanInput {
    pub expected_revision: u64,
    #[schemars(length(min = 1, max = 120))]
    pub title: String,
    #[schemars(length(max = 128))]
    pub items: Vec<PlanItemInput>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PlanItemInput {
    #[schemars(length(min = 1, max = 64))]
    pub id: String,
    #[schemars(length(min = 1, max = 2000))]
    pub goal: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdatePlanItemInput {
    pub expected_revision: u64,
    pub item_id: String,
    pub transition: PlanItemTransitionInput,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum PlanItemTransitionInput {
    Start,
    Block { reason: String },
    Complete { acceptance_note: String },
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkflowMutationsInput {
    pub base_workflow_revision: u64,
    #[schemars(length(min = 1, max = 128))]
    pub ordered_mutations: Vec<Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkflowChangeInput {
    #[schemars(length(min = 36, max = 36))]
    pub change_id: String,
}

#[allow(dead_code)]
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkspaceOutput {
    pub snapshot: Value,
}

#[allow(dead_code)]
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CapabilityCatalogOutput {
    pub catalog: Value,
    pub requested_contract_refs: Option<Vec<String>>,
}

#[allow(dead_code)]
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PlanOutput {
    pub plan: Option<PlanDto>,
}

#[allow(dead_code)]
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PlanDto {
    pub id: String,
    pub revision: u64,
    pub title: String,
    pub items: Vec<PlanItemDto>,
}

#[allow(dead_code)]
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PlanItemDto {
    pub id: String,
    pub goal: String,
    pub state: PlanItemStateDto,
    pub note: Option<String>,
}

#[allow(dead_code)]
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum PlanItemStateDto {
    Pending,
    InProgress,
    Blocked,
    Completed,
}

#[allow(dead_code)]
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ChangeOutput {
    pub change_id: String,
    pub base_workflow_revision: u64,
    pub ordered_mutations: Vec<Value>,
    pub mutation_digest_hex: String,
    pub resulting_workflow_fingerprint_hex: String,
    pub approval_scope_id: String,
    pub expires_at_epoch_ms: i64,
    pub state: ChangeStateDto,
}

#[allow(dead_code)]
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum ChangeStateDto {
    Proposed,
    ReviewRejected,
    AwaitingApproval,
    Rejected,
    Applying,
    Applied,
    ApplyFailed,
    Expired,
}
