//! Canonical closed ten-action Workflow mutation command.

use std::collections::BTreeMap;

use engine::{
    node_capability::{
        NodeCapabilityChoiceKey, NodeCapabilityContractId, NodeCapabilityContractRef,
        NodeCapabilityContractVersion, NodeCapabilityGenerationProfileRefParameterValue,
        NodeCapabilityInputKey, NodeCapabilityInputRoleKey,
        NodeCapabilityManagedAssetIdParameterValue, NodeCapabilityOutputKey,
        NodeCapabilityParameterKey, NodeCapabilityParameterSet, NodeCapabilityParameterValue,
        WorkflowInputItemId, WorkflowManagedAssetIdBoundaryValue,
    },
    workflow_graph::{
        WorkflowAddNodeAction, WorkflowApplyMutationCommand, WorkflowBindSingleInputAction,
        WorkflowCanvasPosition, WorkflowInputItemEntity, WorkflowInputTarget,
        WorkflowInsertReferenceItemAction, WorkflowMoveNodeAction, WorkflowMoveReferenceItemAction,
        WorkflowMutationAction, WorkflowMutationRequestId, WorkflowRemoveInputItemAction,
        WorkflowRemoveNodeAction, WorkflowReplaceNodeParametersAction,
        WorkflowSelectNodeCapabilityAction, WorkflowSetInputItemRoleAction,
    },
};
use serde::Deserialize;
use tauri::State;

use crate::{
    composition::DesktopActivatedCommandDependencies,
    desktop_backend_config::DesktopErrorDto,
    workflow_command_dto::workflow_dto,
    workflow_commands::{
        canonical_u64, invalid_request, revision, trusted_project, uuid, workflow_error,
        workflow_id, workflow_node_id,
    },
    workflow_readiness_dto::{WorkflowWithReadinessDto, readiness_dto},
};

/// One idempotent, optimistic-concurrency Workflow mutation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowApplyMutationRequestDto {
    /// Owning Project identity.
    pub project_id: String,
    /// Stable UUIDv4 request identity.
    pub request_id: String,
    /// Workflow identity.
    pub workflow_id: String,
    /// Required current revision as decimal text.
    pub base_revision: String,
    /// Non-empty ordered list of at most 128 closed actions.
    pub actions: Vec<WorkflowMutationActionDto>,
}

/// Closed ten-action mutation boundary.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowMutationActionDto {
    /// Adds one node.
    AddNode {
        node_id: String,
        capability: CapabilityRefDto,
        parameters: Vec<ParameterEntryDto>,
        canvas_position: CanvasPositionDto,
    },
    /// Removes one node and incident bindings.
    RemoveNode { node_id: String },
    /// Replaces the complete parameter set.
    ReplaceNodeParameters { node_id: String, parameters: Vec<ParameterEntryDto> },
    /// Selects another exact capability and complete parameters.
    SelectNodeCapability {
        node_id: String,
        capability: CapabilityRefDto,
        parameters: Vec<ParameterEntryDto>,
    },
    /// Replaces the persisted canvas position.
    MoveNode { node_id: String, canvas_position: CanvasPositionDto },
    /// Binds one unoccupied single input.
    BindSingleInput { target: InputTargetDto, item: InputItemDto },
    /// Inserts one role-bearing ordered reference.
    InsertReferenceItem { target: InputTargetDto, item: InputItemDto, insertion_index: u32 },
    /// Reorders one stable ordered reference.
    MoveReferenceItem {
        target: InputTargetDto,
        input_item_id: String,
        insertion_index_after_removal: u32,
    },
    /// Removes one stable input item.
    RemoveInputItem { target: InputTargetDto, input_item_id: String },
    /// Replaces one ordered reference role.
    SetInputItemRole { target: InputTargetDto, input_item_id: String, input_role_key: String },
}

/// Exact capability identity.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRefDto {
    pub id: String,
    pub version: String,
}

/// One complete tagged D0.7 parameter value.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterEntryDto {
    pub key: String,
    pub value: ParameterValueDto,
}

/// Closed D0.7 parameter boundary value.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParameterValueDto {
    UnsignedInteger { value: String },
    Text { value: String },
    Choice { value: String },
    GenerationProfile { profile_id: String, version: String },
    ManagedAsset { asset_id: String },
}

/// Persisted canvas coordinates.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasPositionDto {
    pub x: f64,
    pub y: f64,
}

/// Exact target input.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputTargetDto {
    pub node_id: String,
    pub input_key: String,
}

/// Stable directed input item.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputItemDto {
    pub input_item_id: String,
    pub source_node_id: String,
    pub source_output_key: String,
    pub input_role_key: Option<String>,
}

/// Applies one canonical all-or-nothing mutation.
#[tauri::command(rename_all = "snake_case")]
pub async fn workflow_apply_mutation(
    request: WorkflowApplyMutationRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<WorkflowWithReadinessDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    let request_id = WorkflowMutationRequestId::from_uuid(uuid(&request.request_id)?)
        .map_err(|_| invalid_request())?;
    let command = WorkflowApplyMutationCommand::try_new(
        request_id,
        workflow_id(&request.workflow_id)?,
        revision(&request.base_revision)?,
        request.actions.into_iter().map(action).collect::<Result<_, _>>()?,
    )
    .map_err(|_| invalid_request())?;
    let result = state
        .workflow_apply_mutation
        .apply_project_workflow_mutation(project_id, command)
        .await
        .map_err(workflow_error)?;
    Ok(WorkflowWithReadinessDto {
        workflow: workflow_dto(&result.workflow),
        readiness: readiness_dto(result.readiness),
    })
}

fn action(value: WorkflowMutationActionDto) -> Result<WorkflowMutationAction, DesktopErrorDto> {
    Ok(match value {
        WorkflowMutationActionDto::AddNode { node_id, capability, parameters, canvas_position } => {
            add_node(node_id, capability, parameters, canvas_position)?
        }
        WorkflowMutationActionDto::RemoveNode { node_id } => {
            WorkflowMutationAction::RemoveNode(WorkflowRemoveNodeAction {
                node_id: workflow_node_id(&node_id)?,
            })
        }
        WorkflowMutationActionDto::ReplaceNodeParameters { node_id, parameters } => {
            replace_parameters(node_id, parameters)?
        }
        WorkflowMutationActionDto::SelectNodeCapability { node_id, capability, parameters } => {
            select_capability(node_id, capability, parameters)?
        }
        WorkflowMutationActionDto::MoveNode { node_id, canvas_position } => {
            WorkflowMutationAction::MoveNode(WorkflowMoveNodeAction {
                node_id: workflow_node_id(&node_id)?,
                canvas_position: position(canvas_position)?,
            })
        }
        input_action => return translate_input_action(input_action),
    })
}

fn translate_input_action(
    value: WorkflowMutationActionDto,
) -> Result<WorkflowMutationAction, DesktopErrorDto> {
    Ok(match value {
        WorkflowMutationActionDto::BindSingleInput { target, item } => {
            WorkflowMutationAction::BindSingleInput(WorkflowBindSingleInputAction {
                target: input_target(target)?,
                new_item: input_item(item)?,
            })
        }
        WorkflowMutationActionDto::InsertReferenceItem { target, item, insertion_index } => {
            WorkflowMutationAction::InsertReferenceItem(WorkflowInsertReferenceItemAction {
                target: input_target(target)?,
                new_item: input_item(item)?,
                insertion_index,
            })
        }
        WorkflowMutationActionDto::MoveReferenceItem {
            target,
            input_item_id,
            insertion_index_after_removal,
        } => WorkflowMutationAction::MoveReferenceItem(WorkflowMoveReferenceItemAction {
            target: input_target(target)?,
            input_item_id: item_id(&input_item_id)?,
            insertion_index_after_removal,
        }),
        WorkflowMutationActionDto::RemoveInputItem { target, input_item_id } => {
            WorkflowMutationAction::RemoveInputItem(WorkflowRemoveInputItemAction {
                target: input_target(target)?,
                input_item_id: item_id(&input_item_id)?,
            })
        }
        WorkflowMutationActionDto::SetInputItemRole { target, input_item_id, input_role_key } => {
            WorkflowMutationAction::SetInputItemRole(WorkflowSetInputItemRoleAction {
                target: input_target(target)?,
                input_item_id: item_id(&input_item_id)?,
                input_role_key: NodeCapabilityInputRoleKey::new(input_role_key)
                    .map_err(|_| invalid_request())?,
            })
        }
        WorkflowMutationActionDto::AddNode { .. }
        | WorkflowMutationActionDto::RemoveNode { .. }
        | WorkflowMutationActionDto::ReplaceNodeParameters { .. }
        | WorkflowMutationActionDto::SelectNodeCapability { .. }
        | WorkflowMutationActionDto::MoveNode { .. } => return Err(invalid_request()),
    })
}

fn add_node(
    node_id: String,
    capability: CapabilityRefDto,
    parameters: Vec<ParameterEntryDto>,
    canvas_position: CanvasPositionDto,
) -> Result<WorkflowMutationAction, DesktopErrorDto> {
    Ok(WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
        new_node_id: workflow_node_id(&node_id)?,
        capability_contract: capability_ref(capability)?,
        parameter_set: parameter_set(parameters)?,
        canvas_position: position(canvas_position)?,
    }))
}

fn replace_parameters(
    node_id: String,
    parameters: Vec<ParameterEntryDto>,
) -> Result<WorkflowMutationAction, DesktopErrorDto> {
    Ok(WorkflowMutationAction::ReplaceNodeParameters(WorkflowReplaceNodeParametersAction {
        node_id: workflow_node_id(&node_id)?,
        parameter_set: parameter_set(parameters)?,
    }))
}

fn select_capability(
    node_id: String,
    capability: CapabilityRefDto,
    parameters: Vec<ParameterEntryDto>,
) -> Result<WorkflowMutationAction, DesktopErrorDto> {
    Ok(WorkflowMutationAction::SelectNodeCapability(WorkflowSelectNodeCapabilityAction {
        node_id: workflow_node_id(&node_id)?,
        capability_contract: capability_ref(capability)?,
        parameter_set: parameter_set(parameters)?,
    }))
}

fn parameter_set(
    entries: Vec<ParameterEntryDto>,
) -> Result<NodeCapabilityParameterSet, DesktopErrorDto> {
    let mut values = BTreeMap::new();
    for entry in entries {
        let key = NodeCapabilityParameterKey::new(entry.key).map_err(|_| invalid_request())?;
        if values.insert(key, parameter_value(entry.value)?).is_some() {
            return Err(invalid_request());
        }
    }
    NodeCapabilityParameterSet::try_from_map(values).map_err(|_| invalid_request())
}

fn parameter_value(
    value: ParameterValueDto,
) -> Result<NodeCapabilityParameterValue, DesktopErrorDto> {
    Ok(match value {
        ParameterValueDto::UnsignedInteger { value } => {
            NodeCapabilityParameterValue::UnsignedInteger(canonical_u64(&value)?)
        }
        ParameterValueDto::Text { value } => NodeCapabilityParameterValue::Text(value),
        ParameterValueDto::Choice { value } => NodeCapabilityParameterValue::Choice(
            NodeCapabilityChoiceKey::new(value).map_err(|_| invalid_request())?,
        ),
        ParameterValueDto::GenerationProfile { profile_id, version } => {
            let version = u32::try_from(canonical_u64(&version)?).map_err(|_| invalid_request())?;
            NodeCapabilityParameterValue::GenerationProfile(
                NodeCapabilityGenerationProfileRefParameterValue::new(profile_id, version)
                    .map_err(|_| invalid_request())?,
            )
        }
        ParameterValueDto::ManagedAsset { asset_id } => NodeCapabilityParameterValue::ManagedAsset(
            NodeCapabilityManagedAssetIdParameterValue::new(
                WorkflowManagedAssetIdBoundaryValue::from_bytes(*uuid(&asset_id)?.as_bytes())
                    .map_err(|_| invalid_request())?,
            ),
        ),
    })
}

fn capability_ref(value: CapabilityRefDto) -> Result<NodeCapabilityContractRef, DesktopErrorDto> {
    let (major, minor) = value.version.split_once('.').ok_or_else(invalid_request)?;
    let major = u16::try_from(canonical_u64(major)?).map_err(|_| invalid_request())?;
    let minor = u16::try_from(canonical_u64(minor)?).map_err(|_| invalid_request())?;
    Ok(NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(value.id).map_err(|_| invalid_request())?,
        NodeCapabilityContractVersion::new(major, minor).map_err(|_| invalid_request())?,
    ))
}

fn position(value: CanvasPositionDto) -> Result<WorkflowCanvasPosition, DesktopErrorDto> {
    WorkflowCanvasPosition::try_new(value.x, value.y).map_err(|_| invalid_request())
}

fn input_target(value: InputTargetDto) -> Result<WorkflowInputTarget, DesktopErrorDto> {
    Ok(WorkflowInputTarget {
        node_id: workflow_node_id(&value.node_id)?,
        input_key: NodeCapabilityInputKey::new(value.input_key).map_err(|_| invalid_request())?,
    })
}

fn input_item(value: InputItemDto) -> Result<WorkflowInputItemEntity, DesktopErrorDto> {
    Ok(WorkflowInputItemEntity {
        id: item_id(&value.input_item_id)?,
        source_node_id: workflow_node_id(&value.source_node_id)?,
        source_output_key: NodeCapabilityOutputKey::new(value.source_output_key)
            .map_err(|_| invalid_request())?,
        input_role_key: value
            .input_role_key
            .map(NodeCapabilityInputRoleKey::new)
            .transpose()
            .map_err(|_| invalid_request())?,
    })
}

fn item_id(value: &str) -> Result<WorkflowInputItemId, DesktopErrorDto> {
    WorkflowInputItemId::from_uuid(uuid(value)?).ok_or_else(invalid_request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_translation_uses_only_closed_tagged_boundary_values() {
        assert_eq!(
            parameter_value(ParameterValueDto::GenerationProfile {
                profile_id: "image.high_quality_general".to_owned(),
                version: "1".to_owned(),
            })
            .unwrap(),
            NodeCapabilityParameterValue::GenerationProfile(
                NodeCapabilityGenerationProfileRefParameterValue::new(
                    "image.high_quality_general",
                    1,
                )
                .unwrap()
            )
        );
        assert!(
            parameter_value(ParameterValueDto::UnsignedInteger { value: "01".to_owned() }).is_err()
        );
    }
}
