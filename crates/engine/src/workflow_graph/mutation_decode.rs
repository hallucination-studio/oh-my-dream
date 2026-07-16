//! Canonical per-action Workflow mutation encoding and decoding.

use crate::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
    NodeCapabilityInputKey, NodeCapabilityInputRoleKey, NodeCapabilityOutputKey,
    NodeCapabilityParameterSet, WorkflowInputItemId,
};

use super::{
    WorkflowAddNodeAction, WorkflowBindSingleInputAction, WorkflowCanvasPosition,
    WorkflowGraphError, WorkflowInputItemEntity, WorkflowInputTarget,
    WorkflowInsertReferenceItemAction, WorkflowMoveNodeAction, WorkflowMoveReferenceItemAction,
    WorkflowMutationAction, WorkflowNodeId, WorkflowRemoveInputItemAction,
    WorkflowRemoveNodeAction, WorkflowReplaceNodeParametersAction,
    WorkflowSelectNodeCapabilityAction, WorkflowSetInputItemRoleAction,
    mutation_hash::append_action,
};

impl WorkflowMutationAction {
    /// Encodes one action in the frozen canonical Workflow byte format.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        append_action(&mut bytes, self);
        bytes
    }

    /// Restores one action only from its exact canonical Workflow bytes.
    pub fn try_from_canonical_bytes(bytes: &[u8]) -> Result<Self, WorkflowGraphError> {
        let mut decoder = Decoder::new(bytes);
        let action = decoder.action()?;
        if !decoder.is_finished() || action.canonical_bytes() != bytes {
            return Err(WorkflowGraphError::CanonicalMutationInvalid);
        }
        Ok(action)
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn action(&mut self) -> Result<WorkflowMutationAction, WorkflowGraphError> {
        match self.u8()? {
            0 => Ok(WorkflowMutationAction::AddNode(WorkflowAddNodeAction {
                new_node_id: self.node_id()?,
                capability_contract: self.contract_ref()?,
                parameter_set: self.parameters()?,
                canvas_position: self.position()?,
            })),
            1 => Ok(WorkflowMutationAction::RemoveNode(WorkflowRemoveNodeAction {
                node_id: self.node_id()?,
            })),
            2 => Ok(WorkflowMutationAction::ReplaceNodeParameters(
                WorkflowReplaceNodeParametersAction {
                    node_id: self.node_id()?,
                    parameter_set: self.parameters()?,
                },
            )),
            3 => Ok(WorkflowMutationAction::SelectNodeCapability(
                WorkflowSelectNodeCapabilityAction {
                    node_id: self.node_id()?,
                    capability_contract: self.contract_ref()?,
                    parameter_set: self.parameters()?,
                },
            )),
            4 => Ok(WorkflowMutationAction::MoveNode(WorkflowMoveNodeAction {
                node_id: self.node_id()?,
                canvas_position: self.position()?,
            })),
            5 => Ok(WorkflowMutationAction::BindSingleInput(WorkflowBindSingleInputAction {
                target: self.target()?,
                new_item: self.item(false)?,
            })),
            6 => {
                Ok(WorkflowMutationAction::InsertReferenceItem(WorkflowInsertReferenceItemAction {
                    target: self.target()?,
                    new_item: self.item(true)?,
                    insertion_index: self.u32()?,
                }))
            }
            7 => Ok(WorkflowMutationAction::MoveReferenceItem(WorkflowMoveReferenceItemAction {
                target: self.target()?,
                input_item_id: self.input_item_id()?,
                insertion_index_after_removal: self.u32()?,
            })),
            8 => Ok(WorkflowMutationAction::RemoveInputItem(WorkflowRemoveInputItemAction {
                target: self.target()?,
                input_item_id: self.input_item_id()?,
            })),
            9 => Ok(WorkflowMutationAction::SetInputItemRole(WorkflowSetInputItemRoleAction {
                target: self.target()?,
                input_item_id: self.input_item_id()?,
                input_role_key: NodeCapabilityInputRoleKey::new(self.string()?)
                    .map_err(|_| invalid())?,
            })),
            _ => Err(invalid()),
        }
    }

    fn contract_ref(&mut self) -> Result<NodeCapabilityContractRef, WorkflowGraphError> {
        let id = NodeCapabilityContractId::new(self.string()?).map_err(|_| invalid())?;
        let version =
            NodeCapabilityContractVersion::new(self.u16()?, self.u16()?).map_err(|_| invalid())?;
        Ok(NodeCapabilityContractRef::new(id, version))
    }

    fn parameters(&mut self) -> Result<NodeCapabilityParameterSet, WorkflowGraphError> {
        NodeCapabilityParameterSet::try_from_canonical_bytes(self.variable_bytes()?)
            .map_err(|_| invalid())
    }

    fn position(&mut self) -> Result<WorkflowCanvasPosition, WorkflowGraphError> {
        WorkflowCanvasPosition::try_new(f64::from_bits(self.u64()?), f64::from_bits(self.u64()?))
            .map_err(|_| invalid())
    }

    fn target(&mut self) -> Result<WorkflowInputTarget, WorkflowGraphError> {
        Ok(WorkflowInputTarget {
            node_id: self.node_id()?,
            input_key: NodeCapabilityInputKey::new(self.string()?).map_err(|_| invalid())?,
        })
    }

    fn item(&mut self, with_role: bool) -> Result<WorkflowInputItemEntity, WorkflowGraphError> {
        Ok(WorkflowInputItemEntity {
            id: self.input_item_id()?,
            source_node_id: self.node_id()?,
            source_output_key: NodeCapabilityOutputKey::new(self.string()?)
                .map_err(|_| invalid())?,
            input_role_key: if with_role {
                Some(NodeCapabilityInputRoleKey::new(self.string()?).map_err(|_| invalid())?)
            } else {
                None
            },
        })
    }

    fn node_id(&mut self) -> Result<WorkflowNodeId, WorkflowGraphError> {
        WorkflowNodeId::from_uuid(self.uuid()?).map_err(|_| invalid())
    }

    fn input_item_id(&mut self) -> Result<WorkflowInputItemId, WorkflowGraphError> {
        WorkflowInputItemId::from_uuid(self.uuid()?).ok_or_else(invalid)
    }

    fn uuid(&mut self) -> Result<uuid::Uuid, WorkflowGraphError> {
        Ok(uuid::Uuid::from_bytes(self.array()?))
    }

    fn string(&mut self) -> Result<String, WorkflowGraphError> {
        String::from_utf8(self.variable_bytes()?.to_vec()).map_err(|_| invalid())
    }

    fn variable_bytes(&mut self) -> Result<&'a [u8], WorkflowGraphError> {
        let length = usize::try_from(self.u32()?).map_err(|_| invalid())?;
        self.take(length)
    }

    fn u8(&mut self) -> Result<u8, WorkflowGraphError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, WorkflowGraphError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, WorkflowGraphError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WorkflowGraphError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WorkflowGraphError> {
        self.take(N)?.try_into().map_err(|_| invalid())
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], WorkflowGraphError> {
        let end = self.offset.checked_add(length).ok_or_else(invalid)?;
        let value = self.bytes.get(self.offset..end).ok_or_else(invalid)?;
        self.offset = end;
        Ok(value)
    }

    const fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

const fn invalid() -> WorkflowGraphError {
    WorkflowGraphError::CanonicalMutationInvalid
}
