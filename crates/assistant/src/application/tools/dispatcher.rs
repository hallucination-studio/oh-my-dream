mod output;

use projects::project::domain::ProjectId;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{
    AssistantToolCatalog,
    dto::{
        CapabilityCatalogOutput, CreatePlanInput, DescribeCapabilitiesInput, EmptyInput,
        PlanItemInput, PlanItemTransitionInput, PlanOutput, ReplacePlanInput, UpdatePlanItemInput,
        WorkflowChangeInput, WorkflowMutationsInput, WorkspaceOutput,
    },
};
use crate::{
    domain::{
        AssistantPlanItemEntity, AssistantPlanItemId, AssistantProductionPlanAggregate,
        AssistantProductionPlanError, AssistantProductionPlanId, AssistantSessionId,
        AssistantWorkflowChangeAggregate, AssistantWorkflowChangeId, AssistantWorkflowChangeState,
        AssistantWorkflowMutation, WorkflowRevisionBoundaryValue,
    },
    interfaces::{
        AssistantApplicationError, AssistantNodeCapabilityCatalogReaderInterface,
        AssistantNodeCapabilityCatalogRequest, AssistantProductionPlanRepositoryInterface,
        AssistantWorkflowChangeRepositoryInterface, AssistantWorkflowEvaluationRequest,
        AssistantWorkflowMutationEvaluatorInterface, AssistantWorkspaceSnapshotReaderInterface,
    },
};
use output::{change_value, plan_dto, plan_value, serialize};

/// Trusted scope supplied by Rust, never accepted from model arguments.
#[derive(Clone, Copy)]
pub struct AssistantToolExecutionContext {
    /// Authoritative Project scope.
    pub project_id: ProjectId,
    /// Authoritative Assistant Session scope.
    pub session_id: AssistantSessionId,
    /// Rust-created identity reserved for a plan-create call.
    pub production_plan_id: AssistantProductionPlanId,
}

/// Concrete typed dispatcher for exactly the eleven frozen Assistant tools.
pub struct AssistantToolDispatcherImpl<W, C, P, E, R> {
    catalog: AssistantToolCatalog,
    workspace: W,
    capabilities: C,
    plans: P,
    evaluator: E,
    changes: R,
}

impl<W, C, P, E, R> AssistantToolDispatcherImpl<W, C, P, E, R>
where
    W: AssistantWorkspaceSnapshotReaderInterface,
    C: AssistantNodeCapabilityCatalogReaderInterface,
    P: AssistantProductionPlanRepositoryInterface,
    E: AssistantWorkflowMutationEvaluatorInterface,
    R: AssistantWorkflowChangeRepositoryInterface,
{
    /// Constructs a dispatcher over the existing consumer-owned boundaries.
    pub fn try_new(
        workspace: W,
        capabilities: C,
        plans: P,
        evaluator: E,
        changes: R,
    ) -> Result<Self, AssistantApplicationError> {
        Ok(Self {
            catalog: AssistantToolCatalog::try_new()?,
            workspace,
            capabilities,
            plans,
            evaluator,
            changes,
        })
    }

    /// Validates, deserializes, executes, serializes, and revalidates one exact tool call.
    pub async fn execute(
        &self,
        context: AssistantToolExecutionContext,
        tool_id: &str,
        input: Value,
    ) -> Result<Value, AssistantApplicationError> {
        let contract =
            self.catalog.contract(tool_id).ok_or(AssistantApplicationError::ProtocolViolation)?;
        contract.validate_input(&input)?;
        let output = match tool_id {
            "assistant.workspace.get_snapshot@1" => {
                decode::<EmptyInput>(input)?;
                self.workspace(context).await
            }
            "assistant.node_capability.list@1" => {
                decode::<EmptyInput>(input)?;
                self.capability_catalog(None).await
            }
            "assistant.node_capability.describe@1" => {
                let input = decode::<DescribeCapabilitiesInput>(input)?;
                self.capability_catalog(Some(input.contract_refs)).await
            }
            "assistant.production_plan.get@1" => {
                decode::<EmptyInput>(input)?;
                self.get_plan(context).await
            }
            "assistant.production_plan.create@1" => self.create_plan(context, decode(input)?).await,
            "assistant.production_plan.replace@1" => {
                self.replace_plan(context, decode(input)?).await
            }
            "assistant.production_plan.update_item@1" => {
                self.update_plan_item(context, decode(input)?).await
            }
            "assistant.workflow.evaluate_mutation@1" => {
                self.evaluate(context, decode(input)?, false).await
            }
            "assistant.workflow.propose_change@1" => {
                self.evaluate(context, decode(input)?, true).await
            }
            "assistant.workflow.get_change@1" => {
                self.get_change(context, decode(input)?, false).await
            }
            "assistant.workflow.request_apply@1" => {
                self.get_change(context, decode(input)?, true).await
            }
            _ => Err(AssistantApplicationError::ProtocolViolation),
        }?;
        contract.validate_output(&output)?;
        Ok(output)
    }

    async fn workspace(
        &self,
        context: AssistantToolExecutionContext,
    ) -> Result<Value, AssistantApplicationError> {
        let snapshot = self
            .workspace
            .read_assistant_workspace_snapshot(context.project_id, context.session_id)
            .await?;
        serialize(&WorkspaceOutput { snapshot: carrier(snapshot.as_bytes())? })
    }

    async fn capability_catalog(
        &self,
        requested: Option<Vec<String>>,
    ) -> Result<Value, AssistantApplicationError> {
        let request = match requested {
            None => AssistantNodeCapabilityCatalogRequest::List,
            Some(refs) => AssistantNodeCapabilityCatalogRequest::describe(refs)?,
        };
        let snapshot =
            self.capabilities.read_assistant_node_capability_catalog(request.clone()).await?;
        serialize(&CapabilityCatalogOutput {
            catalog: carrier(snapshot.as_bytes())?,
            requested_contract_refs: match request {
                AssistantNodeCapabilityCatalogRequest::List => None,
                AssistantNodeCapabilityCatalogRequest::Describe { contract_refs } => {
                    Some(contract_refs)
                }
            },
        })
    }

    async fn get_plan(
        &self,
        context: AssistantToolExecutionContext,
    ) -> Result<Value, AssistantApplicationError> {
        let plan = self
            .plans
            .load_assistant_production_plan(context.project_id, context.session_id)
            .await?;
        serialize(&PlanOutput { plan: plan.as_ref().map(plan_dto) })
    }

    async fn create_plan(
        &self,
        context: AssistantToolExecutionContext,
        input: CreatePlanInput,
    ) -> Result<Value, AssistantApplicationError> {
        if self
            .plans
            .load_assistant_production_plan(context.project_id, context.session_id)
            .await?
            .is_some()
        {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        let plan = AssistantProductionPlanAggregate::new(
            context.production_plan_id,
            context.project_id,
            context.session_id,
            input.title,
            plan_items(input.items)?,
        )
        .map_err(plan_error)?;
        self.plans.compare_and_swap_assistant_production_plan(None, plan.clone()).await?;
        plan_value(&plan)
    }

    async fn replace_plan(
        &self,
        context: AssistantToolExecutionContext,
        input: ReplacePlanInput,
    ) -> Result<Value, AssistantApplicationError> {
        let mut plan = self.load_plan(context).await?;
        let expected = plan.revision();
        plan.replace(input.expected_revision, input.title, plan_items(input.items)?)
            .map_err(plan_error)?;
        self.plans.compare_and_swap_assistant_production_plan(Some(expected), plan.clone()).await?;
        plan_value(&plan)
    }

    async fn update_plan_item(
        &self,
        context: AssistantToolExecutionContext,
        input: UpdatePlanItemInput,
    ) -> Result<Value, AssistantApplicationError> {
        let mut plan = self.load_plan(context).await?;
        let expected = plan.revision();
        let item_id = AssistantPlanItemId::new(input.item_id).map_err(plan_error)?;
        match input.transition {
            PlanItemTransitionInput::Start => {
                plan.start_item(input.expected_revision, &item_id).map_err(plan_error)?
            }
            PlanItemTransitionInput::Block { reason } => {
                plan.block_item(input.expected_revision, &item_id, reason).map_err(plan_error)?
            }
            PlanItemTransitionInput::Complete { acceptance_note } => plan
                .complete_item(input.expected_revision, &item_id, acceptance_note)
                .map_err(plan_error)?,
        }
        self.plans.compare_and_swap_assistant_production_plan(Some(expected), plan.clone()).await?;
        plan_value(&plan)
    }

    async fn load_plan(
        &self,
        context: AssistantToolExecutionContext,
    ) -> Result<AssistantProductionPlanAggregate, AssistantApplicationError> {
        self.plans
            .load_assistant_production_plan(context.project_id, context.session_id)
            .await?
            .ok_or(AssistantApplicationError::NotFound)
    }

    async fn evaluate(
        &self,
        context: AssistantToolExecutionContext,
        input: WorkflowMutationsInput,
        persist: bool,
    ) -> Result<Value, AssistantApplicationError> {
        let request = evaluation_request(context, input)?;
        let expected = request.clone();
        let result = self.evaluator.evaluate_assistant_workflow_mutations(request).await?;
        let candidate = result.candidate;
        if candidate.project_id != expected.project_id
            || candidate.session_id != expected.session_id
            || candidate.base_workflow_revision != expected.base_workflow_revision
            || candidate.ordered_mutations != expected.ordered_mutations
        {
            return Err(AssistantApplicationError::CandidateFingerprintMismatch);
        }
        let change = AssistantWorkflowChangeAggregate::new(candidate)
            .map_err(|_| AssistantApplicationError::CandidateFingerprintMismatch)?;
        if persist {
            self.changes.insert_assistant_workflow_change(change.clone()).await?;
        }
        change_value(&change)
    }

    async fn get_change(
        &self,
        context: AssistantToolExecutionContext,
        input: WorkflowChangeInput,
        require_approval: bool,
    ) -> Result<Value, AssistantApplicationError> {
        let id = parse_change_id(&input.change_id)?;
        let change = self
            .changes
            .load_assistant_workflow_change(id)
            .await?
            .ok_or(AssistantApplicationError::NotFound)?;
        if change.project_id() != context.project_id || change.session_id() != context.session_id {
            return Err(AssistantApplicationError::NotVisible);
        }
        if require_approval && change.state() != AssistantWorkflowChangeState::AwaitingApproval {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        change_value(&change)
    }
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T, AssistantApplicationError> {
    serde_json::from_value(value).map_err(|_| AssistantApplicationError::ProtocolViolation)
}

fn carrier(bytes: &[u8]) -> Result<Value, AssistantApplicationError> {
    serde_json::from_slice(bytes).map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)
}

fn plan_items(
    values: Vec<PlanItemInput>,
) -> Result<Vec<AssistantPlanItemEntity>, AssistantApplicationError> {
    values
        .into_iter()
        .map(|item| AssistantPlanItemEntity::new(item.id, item.goal).map_err(plan_error))
        .collect()
}

fn evaluation_request(
    context: AssistantToolExecutionContext,
    input: WorkflowMutationsInput,
) -> Result<AssistantWorkflowEvaluationRequest, AssistantApplicationError> {
    let revision = WorkflowRevisionBoundaryValue::new(input.base_workflow_revision)
        .map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    let ordered_mutations = input
        .ordered_mutations
        .into_iter()
        .map(|value| {
            serde_json::to_vec(&value)
                .map_err(|_| AssistantApplicationError::ProtocolViolation)
                .and_then(|bytes| {
                    AssistantWorkflowMutation::new(bytes)
                        .map_err(|_| AssistantApplicationError::ProtocolViolation)
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AssistantWorkflowEvaluationRequest {
        project_id: context.project_id,
        session_id: context.session_id,
        base_workflow_revision: revision,
        ordered_mutations,
    })
}

fn parse_change_id(value: &str) -> Result<AssistantWorkflowChangeId, AssistantApplicationError> {
    let value =
        uuid::Uuid::parse_str(value).map_err(|_| AssistantApplicationError::ProtocolViolation)?;
    AssistantWorkflowChangeId::from_uuid(value)
        .map_err(|_| AssistantApplicationError::ProtocolViolation)
}

fn plan_error(error: AssistantProductionPlanError) -> AssistantApplicationError {
    match error {
        AssistantProductionPlanError::RevisionConflict { .. } => {
            AssistantApplicationError::RevisionConflict
        }
        AssistantProductionPlanError::ItemNotFound => AssistantApplicationError::NotFound,
        AssistantProductionPlanError::InvalidText
        | AssistantProductionPlanError::InvalidItemId
        | AssistantProductionPlanError::TooManyItems
        | AssistantProductionPlanError::DuplicateItemId => {
            AssistantApplicationError::ProtocolViolation
        }
        AssistantProductionPlanError::InvalidItemTransition
        | AssistantProductionPlanError::RevisionOverflow
        | AssistantProductionPlanError::InvalidRevision => {
            AssistantApplicationError::InvalidTransition
        }
    }
}
