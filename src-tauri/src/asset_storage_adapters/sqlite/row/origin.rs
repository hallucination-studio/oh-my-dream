use super::*;

impl OriginRow {
    pub(super) fn from_domain(value: &AssetOrigin) -> Self {
        match value {
            AssetOrigin::Imported(value) => Self::Imported {
                import_id: *value.import_id().as_uuid().as_bytes(),
                original_file_name: value.original_file_name().as_str().to_owned(),
            },
            AssetOrigin::WorkflowNodeOutput(value) => {
                let producer = value.producer();
                let output = value.output_key();
                Self::WorkflowNodeOutput {
                    workflow_id: *producer.workflow_id().as_uuid().as_bytes(),
                    workflow_revision: producer.workflow_revision().get(),
                    workflow_run_id: *producer.workflow_run_id().as_uuid().as_bytes(),
                    workflow_node_id: *producer.workflow_node_id().as_uuid().as_bytes(),
                    node_execution_id: *producer.node_execution_id().as_uuid().as_bytes(),
                    capability_id: producer.capability_contract_ref().id().to_owned(),
                    capability_major: producer.capability_contract_ref().major(),
                    capability_minor: producer.capability_contract_ref().minor(),
                    production: ProductionRow::from_domain(value.production()),
                    output_key: output.output_key().as_str().to_owned(),
                    ordinal: output.ordinal(),
                }
            }
        }
    }

    pub(super) fn into_domain(self) -> Result<AssetOrigin, AssetApplicationError> {
        match self {
            Self::Imported { import_id, original_file_name } => Ok(AssetOrigin::imported(
                AssetImportId::from_uuid(Uuid::from_bytes(import_id)).map_err(|_| storage())?,
                AssetOriginalFileName::try_new(original_file_name).map_err(|_| storage())?,
            )),
            Self::WorkflowNodeOutput {
                workflow_id,
                workflow_revision,
                workflow_run_id,
                workflow_node_id,
                node_execution_id,
                capability_id,
                capability_major,
                capability_minor,
                production,
                output_key,
                ordinal,
            } => {
                let run_id = AssetOriginWorkflowRunId::from_uuid(Uuid::from_bytes(workflow_run_id))
                    .map_err(|_| storage())?;
                let execution_id = AssetOriginWorkflowNodeExecutionId::from_uuid(Uuid::from_bytes(
                    node_execution_id,
                ))
                .map_err(|_| storage())?;
                let producer = AssetWorkflowNodeOrigin::new(
                    AssetOriginWorkflowId::from_uuid(Uuid::from_bytes(workflow_id))
                        .map_err(|_| storage())?,
                    AssetOriginWorkflowRevision::new(workflow_revision).map_err(|_| storage())?,
                    run_id,
                    AssetOriginWorkflowNodeId::from_uuid(Uuid::from_bytes(workflow_node_id))
                        .map_err(|_| storage())?,
                    execution_id,
                    AssetOriginNodeCapabilityContractRef::try_new(
                        capability_id,
                        capability_major,
                        capability_minor,
                    )
                    .map_err(|_| storage())?,
                );
                AssetOrigin::workflow_node_output(
                    producer,
                    production.into_domain()?,
                    AssetNodeOutputKey::new(
                        run_id,
                        execution_id,
                        AssetOriginNodeOutputKey::try_new(output_key).map_err(|_| storage())?,
                        ordinal,
                    ),
                )
                .map_err(|_| storage())
            }
        }
    }
}

impl ProductionRow {
    fn from_domain(value: &AssetNodeOutputProduction) -> Self {
        match value {
            AssetNodeOutputProduction::ProviderGenerated { generation_profile_ref } => {
                Self::ProviderGenerated {
                    profile_id: generation_profile_ref.id().to_owned(),
                    profile_version: generation_profile_ref.version(),
                }
            }
            AssetNodeOutputProduction::DeterministicDerived { source_asset_ids } => {
                Self::DeterministicDerived {
                    source_asset_ids: source_asset_ids
                        .as_slice()
                        .iter()
                        .map(|value| *value.asset_id().as_uuid().as_bytes())
                        .collect(),
                }
            }
            AssetNodeOutputProduction::ProviderDerived {
                source_asset_ids,
                generation_profile_ref,
            } => Self::ProviderDerived {
                source_asset_ids: source_asset_ids
                    .as_slice()
                    .iter()
                    .map(|value| *value.asset_id().as_uuid().as_bytes())
                    .collect(),
                profile_id: generation_profile_ref.id().to_owned(),
                profile_version: generation_profile_ref.version(),
            },
        }
    }

    fn into_domain(self) -> Result<AssetNodeOutputProduction, AssetApplicationError> {
        match self {
            Self::ProviderGenerated { profile_id, profile_version } => {
                Ok(AssetNodeOutputProduction::ProviderGenerated {
                    generation_profile_ref: profile(profile_id, profile_version)?,
                })
            }
            Self::DeterministicDerived { source_asset_ids } => {
                Ok(AssetNodeOutputProduction::DeterministicDerived {
                    source_asset_ids: sources(source_asset_ids)?,
                })
            }
            Self::ProviderDerived { source_asset_ids, profile_id, profile_version } => {
                Ok(AssetNodeOutputProduction::ProviderDerived {
                    source_asset_ids: sources(source_asset_ids)?,
                    generation_profile_ref: profile(profile_id, profile_version)?,
                })
            }
        }
    }
}

fn sources(values: Vec<[u8; 16]>) -> Result<AssetOriginSourceAssetIds, AssetApplicationError> {
    AssetOriginSourceAssetIds::try_new(
        values
            .into_iter()
            .map(|value| {
                AssetId::from_uuid(Uuid::from_bytes(value))
                    .map(AssetOriginSourceAssetId::from_asset_id)
                    .map_err(|_| storage())
            })
            .collect::<Result<_, _>>()?,
    )
    .map_err(|_| storage())
}

fn profile(
    id: String,
    version: u32,
) -> Result<AssetOriginGenerationProfileRef, AssetApplicationError> {
    AssetOriginGenerationProfileRef::try_new(id, version).map_err(|_| storage())
}
