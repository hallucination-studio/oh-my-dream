use std::collections::BTreeMap;

use assistant::{
    domain::AssistantWorkflowChangeId,
    interfaces::{
        AssistantApplicationError, AssistantCanvasPositionDto as CanvasPositionDto,
        AssistantCapabilityRefDto as CapabilityRefDto, AssistantInputTargetDto as InputTargetDto,
        AssistantNodeRefDto as NodeRefDto, AssistantOutputSourceDto as OutputSourceDto,
        AssistantParameterValueDto as ParameterValueDto, AssistantWorkflowMutationProposal,
        AssistantWorkflowMutationProposalDto as ProposalAction,
    },
};
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
        WorkflowAddNodeAction, WorkflowBindSingleInputAction, WorkflowCanvasPosition,
        WorkflowInputItemEntity, WorkflowInputTarget, WorkflowInsertReferenceItemAction,
        WorkflowMoveNodeAction, WorkflowMoveReferenceItemAction, WorkflowMutationAction,
        WorkflowNodeId, WorkflowRemoveInputItemAction, WorkflowRemoveNodeAction,
        WorkflowReplaceNodeParametersAction, WorkflowSelectNodeCapabilityAction,
        WorkflowSetInputItemRoleAction,
    },
};
use sha2::{Digest, Sha256};

/// Translates strict proposal JSON to exact Workflow actions and resolved aliases.
pub fn translate_proposals(
    change_id: AssistantWorkflowChangeId,
    proposals: &[AssistantWorkflowMutationProposal],
) -> Result<
    (Vec<WorkflowMutationAction>, BTreeMap<String, WorkflowNodeId>),
    AssistantApplicationError,
> {
    let mut aliases = BTreeMap::new();
    let mut actions = Vec::with_capacity(proposals.len());
    for (index, proposal) in proposals.iter().enumerate() {
        let value: ProposalAction =
            serde_json::from_slice(proposal.as_bytes()).map_err(protocol)?;
        if serde_json::to_vec(&value).map_err(protocol)? != proposal.as_bytes() {
            return Err(AssistantApplicationError::ProtocolViolation);
        }
        actions.push(translate_action(value, change_id, index, &mut aliases)?);
    }
    Ok((actions, aliases))
}

fn translate_action(
    action: ProposalAction,
    change_id: AssistantWorkflowChangeId,
    index: usize,
    aliases: &mut BTreeMap<String, WorkflowNodeId>,
) -> Result<WorkflowMutationAction, AssistantApplicationError> {
    Ok(match action {
        ProposalAction::AddNode { alias, capability, parameters, position } => {
            if aliases.contains_key(&alias) {
                return Err(AssistantApplicationError::ProtocolViolation);
            }
            let node_id = derived_node_id(change_id, index, &alias)?;
            aliases.insert(alias, node_id);
            WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
                new_node_id: node_id,
                capability_contract: capability_ref(capability)?,
                parameter_set: parameters_set(parameters)?,
                canvas_position: canvas_position(position)?,
            })
        }
        ProposalAction::RemoveNode { node } => {
            WorkflowMutationAction::RemoveNode(WorkflowRemoveNodeAction {
                node_id: resolve_node(node, aliases)?,
            })
        }
        ProposalAction::ReplaceNodeParameters { node, parameters } => {
            WorkflowMutationAction::ReplaceNodeParameters(WorkflowReplaceNodeParametersAction {
                node_id: resolve_node(node, aliases)?,
                parameter_set: parameters_set(parameters)?,
            })
        }
        ProposalAction::SelectNodeCapability { node, capability, parameters } => {
            WorkflowMutationAction::SelectNodeCapability(WorkflowSelectNodeCapabilityAction {
                node_id: resolve_node(node, aliases)?,
                capability_contract: capability_ref(capability)?,
                parameter_set: parameters_set(parameters)?,
            })
        }
        ProposalAction::MoveNode { node, position } => {
            WorkflowMutationAction::MoveNode(WorkflowMoveNodeAction {
                node_id: resolve_node(node, aliases)?,
                canvas_position: canvas_position(position)?,
            })
        }
        ProposalAction::BindSingleInput { target, source } => {
            WorkflowMutationAction::BindSingleInput(WorkflowBindSingleInputAction {
                target: input_target(target, aliases)?,
                new_item: source_item(source, change_id, index, aliases, None)?,
            })
        }
        ProposalAction::InsertReferenceItem { target, source, role, insertion_index } => {
            WorkflowMutationAction::InsertReferenceItem(WorkflowInsertReferenceItemAction {
                target: input_target(target, aliases)?,
                new_item: source_item(source, change_id, index, aliases, Some(role))?,
                insertion_index,
            })
        }
        ProposalAction::MoveReferenceItem {
            target,
            input_item_id,
            insertion_index_after_removal,
        } => WorkflowMutationAction::MoveReferenceItem(WorkflowMoveReferenceItemAction {
            target: input_target(target, aliases)?,
            input_item_id: input_item_id_value(&input_item_id)?,
            insertion_index_after_removal,
        }),
        ProposalAction::RemoveInputItem { target, input_item_id } => {
            WorkflowMutationAction::RemoveInputItem(WorkflowRemoveInputItemAction {
                target: input_target(target, aliases)?,
                input_item_id: input_item_id_value(&input_item_id)?,
            })
        }
        ProposalAction::SetInputItemRole { target, input_item_id, role } => {
            WorkflowMutationAction::SetInputItemRole(WorkflowSetInputItemRoleAction {
                target: input_target(target, aliases)?,
                input_item_id: input_item_id_value(&input_item_id)?,
                input_role_key: NodeCapabilityInputRoleKey::new(role).map_err(protocol)?,
            })
        }
    })
}

fn capability_ref(
    value: CapabilityRefDto,
) -> Result<NodeCapabilityContractRef, AssistantApplicationError> {
    Ok(NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(value.id).map_err(protocol)?,
        NodeCapabilityContractVersion::new(value.major, value.minor).map_err(protocol)?,
    ))
}

fn resolve_node(
    value: NodeRefDto,
    aliases: &BTreeMap<String, WorkflowNodeId>,
) -> Result<WorkflowNodeId, AssistantApplicationError> {
    match value {
        NodeRefDto::Id { id } => workflow_node_id(&id),
        NodeRefDto::Alias { alias } => {
            aliases.get(&alias).copied().ok_or(AssistantApplicationError::ProtocolViolation)
        }
    }
}

fn canvas_position(
    value: CanvasPositionDto,
) -> Result<WorkflowCanvasPosition, AssistantApplicationError> {
    WorkflowCanvasPosition::try_new(value.x, value.y).map_err(protocol)
}

fn input_target(
    value: InputTargetDto,
    aliases: &BTreeMap<String, WorkflowNodeId>,
) -> Result<WorkflowInputTarget, AssistantApplicationError> {
    Ok(WorkflowInputTarget {
        node_id: resolve_node(value.node, aliases)?,
        input_key: NodeCapabilityInputKey::new(value.input).map_err(protocol)?,
    })
}

fn source_item(
    value: OutputSourceDto,
    change_id: AssistantWorkflowChangeId,
    index: usize,
    aliases: &BTreeMap<String, WorkflowNodeId>,
    role: Option<String>,
) -> Result<WorkflowInputItemEntity, AssistantApplicationError> {
    Ok(WorkflowInputItemEntity {
        id: derived_item_id(change_id, index)?,
        source_node_id: resolve_node(value.node, aliases)?,
        source_output_key: NodeCapabilityOutputKey::new(value.output).map_err(protocol)?,
        input_role_key: role.map(NodeCapabilityInputRoleKey::new).transpose().map_err(protocol)?,
    })
}

fn parameters_set(
    values: BTreeMap<String, ParameterValueDto>,
) -> Result<NodeCapabilityParameterSet, AssistantApplicationError> {
    let values = values
        .into_iter()
        .map(|(key, value)| {
            Ok((NodeCapabilityParameterKey::new(key).map_err(protocol)?, parameter_value(value)?))
        })
        .collect::<Result<BTreeMap<_, _>, AssistantApplicationError>>()?;
    NodeCapabilityParameterSet::try_from_map(values).map_err(protocol)
}

fn parameter_value(
    value: ParameterValueDto,
) -> Result<NodeCapabilityParameterValue, AssistantApplicationError> {
    Ok(match value {
        ParameterValueDto::UnsignedInteger { value } => {
            NodeCapabilityParameterValue::UnsignedInteger(value)
        }
        ParameterValueDto::Text { value } => NodeCapabilityParameterValue::Text(value),
        ParameterValueDto::Choice { value } => NodeCapabilityParameterValue::Choice(
            NodeCapabilityChoiceKey::new(value).map_err(protocol)?,
        ),
        ParameterValueDto::GenerationProfile { id, version } => {
            NodeCapabilityParameterValue::GenerationProfile(
                NodeCapabilityGenerationProfileRefParameterValue::new(id, version)
                    .map_err(protocol)?,
            )
        }
        ParameterValueDto::ManagedAsset { id } => {
            let id = uuid::Uuid::parse_str(&id).map_err(protocol)?;
            let id = WorkflowManagedAssetIdBoundaryValue::from_bytes(id.into_bytes())
                .map_err(protocol)?;
            NodeCapabilityParameterValue::ManagedAsset(
                NodeCapabilityManagedAssetIdParameterValue::new(id),
            )
        }
    })
}

fn workflow_node_id(value: &str) -> Result<WorkflowNodeId, AssistantApplicationError> {
    WorkflowNodeId::from_uuid(uuid::Uuid::parse_str(value).map_err(protocol)?).map_err(protocol)
}

fn input_item_id_value(value: &str) -> Result<WorkflowInputItemId, AssistantApplicationError> {
    WorkflowInputItemId::from_uuid(uuid::Uuid::parse_str(value).map_err(protocol)?)
        .ok_or(AssistantApplicationError::ProtocolViolation)
}

fn derived_node_id(
    change_id: AssistantWorkflowChangeId,
    index: usize,
    alias: &str,
) -> Result<WorkflowNodeId, AssistantApplicationError> {
    WorkflowNodeId::from_uuid(derived_uuid(change_id, index, b"node", alias.as_bytes()))
        .map_err(protocol)
}

fn derived_item_id(
    change_id: AssistantWorkflowChangeId,
    index: usize,
) -> Result<WorkflowInputItemId, AssistantApplicationError> {
    WorkflowInputItemId::from_uuid(derived_uuid(change_id, index, b"item", &[]))
        .ok_or(AssistantApplicationError::ProtocolViolation)
}

fn derived_uuid(
    change_id: AssistantWorkflowChangeId,
    index: usize,
    domain: &[u8],
    extra: &[u8],
) -> uuid::Uuid {
    let digest = Sha256::new()
        .chain_update(b"oh-my-dream/assistant-workflow-identity/v1")
        .chain_update(change_id.as_uuid().as_bytes())
        .chain_update(index.to_be_bytes())
        .chain_update(domain)
        .chain_update(extra)
        .finalize();
    let mut bytes: [u8; 16] = digest[..16].try_into().unwrap_or([0; 16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

fn protocol<E>(_error: E) -> AssistantApplicationError {
    AssistantApplicationError::ProtocolViolation
}
