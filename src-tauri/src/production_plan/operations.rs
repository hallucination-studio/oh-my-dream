use super::{
    NewPlanItem, PlanItemStatus, ProductionPlan, ProductionPlanError, ProductionPlanService,
};
use crate::assistant_operations::{
    OperationEffect, OperationHandlerError, OperationInputSchemaMode, OperationRegistration,
    OperationRegistrationError, RequestContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetPlanInput {}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreatePlanInput {
    title: String,
    items: Vec<PlanItemInput>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReplacePlanInput {
    expected_revision: u64,
    title: String,
    items: Vec<PlanItemInput>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdatePlanItemInput {
    expected_revision: u64,
    item_id: String,
    action: PlanItemAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlanItemInput {
    id: String,
    summary: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum PlanItemAction {
    Start,
    Block { reason: String },
    Complete { acceptance_note: String },
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProductionPlanOutput {
    #[schemars(required)]
    plan: Option<ProductionPlanDto>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProductionPlanDto {
    project_id: String,
    revision: u64,
    title: String,
    items: Vec<PlanItemDto>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlanItemDto {
    id: String,
    summary: String,
    status: PlanItemStatusDto,
    #[schemars(required)]
    note: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum PlanItemStatusDto {
    Pending,
    InProgress,
    Blocked,
    Completed,
}

/// Model-facing operations owned by the production-plan capability.
pub struct ProductionPlanOperations {
    service: Arc<ProductionPlanService>,
}

impl ProductionPlanOperations {
    #[must_use]
    pub fn new(service: Arc<ProductionPlanService>) -> Self {
        Self { service }
    }

    pub fn registrations(self) -> Result<Vec<OperationRegistration>, OperationRegistrationError> {
        let service = self.service;
        Ok(vec![
            get_registration(Arc::clone(&service))?,
            create_registration(Arc::clone(&service))?,
            replace_registration(Arc::clone(&service))?,
            update_item_registration(service)?,
        ])
    }
}

fn get_registration(
    service: Arc<ProductionPlanService>,
) -> Result<OperationRegistration, OperationRegistrationError> {
    OperationRegistration::new::<GetPlanInput, ProductionPlanOutput, _>(
        "production_plan_get",
        1,
        "Read the Agent-owned production plan for the current Project.",
        OperationEffect::LocalRead,
        OperationInputSchemaMode::Strict,
        move |context: &RequestContext, _input: GetPlanInput| {
            let service = Arc::clone(&service);
            let project_id = context.project_id().to_owned();
            async move {
                let plan = service.get(&project_id).map_err(handler_error)?;
                Ok(ProductionPlanOutput { plan: plan.as_ref().map(ProductionPlanDto::from) })
            }
        },
    )
}

fn create_registration(
    service: Arc<ProductionPlanService>,
) -> Result<OperationRegistration, OperationRegistrationError> {
    OperationRegistration::new::<CreatePlanInput, ProductionPlanOutput, _>(
        "production_plan_create",
        1,
        "Create durable Agent-owned production memory without executing work.",
        OperationEffect::AssistantStateMutation,
        OperationInputSchemaMode::Strict,
        move |context: &RequestContext, input: CreatePlanInput| {
            let service = Arc::clone(&service);
            let project_id = context.project_id().to_owned();
            async move {
                let plan = service
                    .create(&project_id, input.title, domain_items(input.items))
                    .map_err(handler_error)?;
                Ok(ProductionPlanOutput { plan: Some(ProductionPlanDto::from(&plan)) })
            }
        },
    )
}

fn replace_registration(
    service: Arc<ProductionPlanService>,
) -> Result<OperationRegistration, OperationRegistrationError> {
    OperationRegistration::new::<ReplacePlanInput, ProductionPlanOutput, _>(
        "production_plan_replace",
        1,
        "CAS-replace the Agent-owned production plan without selecting a next item.",
        OperationEffect::AssistantStateMutation,
        OperationInputSchemaMode::Strict,
        move |context: &RequestContext, input: ReplacePlanInput| {
            let service = Arc::clone(&service);
            let project_id = context.project_id().to_owned();
            async move {
                let plan = service
                    .replace(
                        &project_id,
                        input.expected_revision,
                        input.title,
                        domain_items(input.items),
                    )
                    .map_err(handler_error)?;
                Ok(ProductionPlanOutput { plan: Some(ProductionPlanDto::from(&plan)) })
            }
        },
    )
}

fn update_item_registration(
    service: Arc<ProductionPlanService>,
) -> Result<OperationRegistration, OperationRegistrationError> {
    OperationRegistration::new::<UpdatePlanItemInput, ProductionPlanOutput, _>(
        "production_plan_update_item",
        1,
        "Update one chosen plan item; this tool never chooses the item for the Agent.",
        OperationEffect::AssistantStateMutation,
        OperationInputSchemaMode::Strict,
        move |context: &RequestContext, input: UpdatePlanItemInput| {
            let service = Arc::clone(&service);
            let project_id = context.project_id().to_owned();
            async move {
                let plan = match input.action {
                    PlanItemAction::Start => {
                        service.start_item(&project_id, input.expected_revision, &input.item_id)
                    }
                    PlanItemAction::Block { reason } => service.block_item(
                        &project_id,
                        input.expected_revision,
                        &input.item_id,
                        reason,
                    ),
                    PlanItemAction::Complete { acceptance_note } => service.complete_item(
                        &project_id,
                        input.expected_revision,
                        &input.item_id,
                        acceptance_note,
                    ),
                }
                .map_err(handler_error)?;
                Ok(ProductionPlanOutput { plan: Some(ProductionPlanDto::from(&plan)) })
            }
        },
    )
}

fn domain_items(items: Vec<PlanItemInput>) -> Vec<NewPlanItem> {
    items.into_iter().map(|item| NewPlanItem { id: item.id, summary: item.summary }).collect()
}

fn handler_error(error: ProductionPlanError) -> OperationHandlerError {
    OperationHandlerError::new("PRODUCTION_PLAN_FAILED", error.to_string())
}

impl From<&ProductionPlan> for ProductionPlanDto {
    fn from(plan: &ProductionPlan) -> Self {
        Self {
            project_id: plan.project_id().to_owned(),
            revision: plan.revision(),
            title: plan.title().to_owned(),
            items: plan
                .items()
                .iter()
                .map(|item| PlanItemDto {
                    id: item.id().to_owned(),
                    summary: item.summary().to_owned(),
                    status: match item.status() {
                        PlanItemStatus::Pending => PlanItemStatusDto::Pending,
                        PlanItemStatus::InProgress => PlanItemStatusDto::InProgress,
                        PlanItemStatus::Blocked => PlanItemStatusDto::Blocked,
                        PlanItemStatus::Completed => PlanItemStatusDto::Completed,
                    },
                    note: item.note().map(str::to_owned),
                })
                .collect(),
        }
    }
}
