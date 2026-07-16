use engine::{
    node_capability::WorkflowNodeCapabilityRegistry,
    workflow::{
        WorkflowApplicationError, WorkflowCreateCommandHash, WorkflowCreateReceipt,
        WorkflowCreateRequestId,
    },
    workflow_graph::{
        WorkflowMutationCommandHash, WorkflowMutationReceipt, WorkflowMutationRequestId,
    },
};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{
    graph::{decode_workflow, encode_workflow},
    persistence,
};

type ReceiptRow = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

pub(super) fn load_creation_receipt(
    connection: &Connection,
    capabilities: &WorkflowNodeCapabilityRegistry,
    request_id: WorkflowCreateRequestId,
) -> Result<Option<WorkflowCreateReceipt>, WorkflowApplicationError> {
    let row = load_row(connection, "workflow_create_receipts", request_id.as_uuid().as_bytes())?;
    row.map(|row| decode_creation(row, capabilities)).transpose()
}

pub(super) fn load_mutation_receipt(
    connection: &Connection,
    capabilities: &WorkflowNodeCapabilityRegistry,
    request_id: WorkflowMutationRequestId,
) -> Result<Option<WorkflowMutationReceipt>, WorkflowApplicationError> {
    let row = load_row(connection, "workflow_mutation_receipts", request_id.as_uuid().as_bytes())?;
    row.map(|row| decode_mutation(row, capabilities)).transpose()
}

pub(super) fn insert_creation_receipt(
    connection: &Connection,
    receipt: &WorkflowCreateReceipt,
) -> Result<(), WorkflowApplicationError> {
    insert_row(
        connection,
        "workflow_create_receipts",
        receipt.request_id().as_uuid().as_bytes(),
        &receipt.command_hash().as_bytes(),
        &encode_workflow(receipt.created_workflow())?,
        &receipt.result_fingerprint(),
    )
}

pub(super) fn insert_mutation_receipt(
    connection: &Connection,
    receipt: &WorkflowMutationReceipt,
) -> Result<(), WorkflowApplicationError> {
    insert_row(
        connection,
        "workflow_mutation_receipts",
        receipt.request_id().as_uuid().as_bytes(),
        &receipt.command_hash().as_bytes(),
        &encode_workflow(receipt.committed_workflow())?,
        &receipt.result_fingerprint().as_bytes(),
    )
}

fn load_row(
    connection: &Connection,
    table: &str,
    request_id: &[u8; 16],
) -> Result<Option<ReceiptRow>, WorkflowApplicationError> {
    let sql = format!(
        "SELECT request_id, command_hash, workflow_snapshot, result_fingerprint
         FROM {table} WHERE request_id = ?1"
    );
    connection
        .query_row(&sql, [request_id.as_slice()], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .optional()
        .map_err(|_| persistence())
}

fn insert_row(
    connection: &Connection,
    table: &str,
    request_id: &[u8; 16],
    command_hash: &[u8; 32],
    snapshot: &[u8],
    fingerprint: &[u8; 32],
) -> Result<(), WorkflowApplicationError> {
    let sql = format!(
        "INSERT INTO {table}(request_id, command_hash, workflow_snapshot, result_fingerprint)
         VALUES (?1, ?2, ?3, ?4)"
    );
    connection
        .execute(
            &sql,
            params![
                request_id.as_slice(),
                command_hash.as_slice(),
                snapshot,
                fingerprint.as_slice()
            ],
        )
        .map(|_| ())
        .map_err(|_| persistence())
}

fn decode_creation(
    row: ReceiptRow,
    capabilities: &WorkflowNodeCapabilityRegistry,
) -> Result<WorkflowCreateReceipt, WorkflowApplicationError> {
    let (request_id, command_hash, snapshot, fingerprint) = row;
    WorkflowCreateReceipt::try_restore(
        WorkflowCreateRequestId::from_uuid(uuid(&request_id)?).ok_or_else(persistence)?,
        WorkflowCreateCommandHash::from_bytes(array(command_hash)?),
        decode_workflow(&snapshot, capabilities)?,
        array(fingerprint)?,
    )
}

fn decode_mutation(
    row: ReceiptRow,
    capabilities: &WorkflowNodeCapabilityRegistry,
) -> Result<WorkflowMutationReceipt, WorkflowApplicationError> {
    let (request_id, command_hash, snapshot, fingerprint) = row;
    WorkflowMutationReceipt::try_restore(
        WorkflowMutationRequestId::from_uuid(uuid(&request_id)?)?,
        WorkflowMutationCommandHash::from_bytes(array(command_hash)?),
        decode_workflow(&snapshot, capabilities)?,
        array(fingerprint)?,
    )
    .map_err(Into::into)
}

fn uuid(bytes: &[u8]) -> Result<Uuid, WorkflowApplicationError> {
    Uuid::from_slice(bytes).map_err(|_| persistence())
}

fn array(bytes: Vec<u8>) -> Result<[u8; 32], WorkflowApplicationError> {
    bytes.try_into().map_err(|_| persistence())
}
