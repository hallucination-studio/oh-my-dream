//! Application boundary projections for versioned workflow capabilities.

use crate::command_error::command_error;
use crate::dto::{
    CapabilityAvailabilityDto, CapabilityCardinalityDto, CapabilityCatalogDto,
    CapabilityCatalogEntryDto, CapabilityContractDto, CapabilityEffectDto, CapabilityPortDto,
    CapabilityPresentationDto, CapabilityRefDto, CapabilityStatusDto,
};
use crate::state::AppState;
use engine::{CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityRef, NodeRegistry};
use nodes::CapabilityProjection;
use std::collections::BTreeMap;
use tauri::State;

/// Returns all registered exact capabilities as separate boundary projections.
#[tauri::command(rename_all = "snake_case")]
pub fn get_capability_catalog(state: State<'_, AppState>) -> Result<CapabilityCatalogDto, String> {
    get_capability_catalog_with_state(&state)
}

/// Returns the capability catalog against an explicit app state.
pub fn get_capability_catalog_with_state(state: &AppState) -> Result<CapabilityCatalogDto, String> {
    project_catalog(&state.registry)
        .map_err(|source| command_error("project capability catalog", source))
}

fn project_catalog(
    registry: &NodeRegistry,
) -> Result<CapabilityCatalogDto, nodes::CapabilityProjectionError> {
    let projections = nodes::project_capabilities(registry)?;
    Ok(CapabilityCatalogDto { capabilities: projections.into_iter().map(project_entry).collect() })
}

fn project_entry(projection: CapabilityProjection) -> CapabilityCatalogEntryDto {
    let status = status_for(&projection.contract);
    CapabilityCatalogEntryDto {
        contract: contract_to_dto(&projection.contract),
        presentation: presentation_to_dto(projection.presentation),
        status,
    }
}

fn contract_to_dto(contract: &CapabilityContract) -> CapabilityContractDto {
    CapabilityContractDto {
        reference: reference_to_dto(&contract.reference),
        inputs: contract.inputs.iter().map(port_to_dto).collect(),
        outputs: contract.outputs.iter().map(port_to_dto).collect(),
        params_schema: contract.params_schema.clone(),
        default_params: contract
            .default_params
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>(),
        effects: contract.effects.iter().copied().map(effect_to_dto).collect(),
    }
}

fn reference_to_dto(reference: &CapabilityRef) -> CapabilityRefDto {
    CapabilityRefDto { id: reference.id.clone(), version: reference.version.clone() }
}

fn port_to_dto(port: &CapabilityPort) -> CapabilityPortDto {
    CapabilityPortDto {
        name: port.name.clone(),
        port_type: port_type_to_string(port.port_type),
        cardinality: cardinality_to_dto(port.cardinality),
        required: port.required,
    }
}

fn port_type_to_string(port_type: engine::PortType) -> String {
    match port_type {
        engine::PortType::String => "string",
        engine::PortType::Image => "image",
        engine::PortType::Video => "video",
        engine::PortType::Audio => "audio",
        engine::PortType::Model => "model",
        engine::PortType::Int => "int",
        engine::PortType::Float => "float",
    }
    .to_owned()
}

fn cardinality_to_dto(cardinality: engine::PortCardinality) -> CapabilityCardinalityDto {
    match cardinality {
        engine::PortCardinality::One => CapabilityCardinalityDto::One,
        engine::PortCardinality::Many { minimum, maximum } => {
            CapabilityCardinalityDto::Many { minimum, maximum }
        }
    }
}

fn effect_to_dto(effect: CapabilityEffect) -> CapabilityEffectDto {
    match effect {
        CapabilityEffect::Pure => CapabilityEffectDto::Pure,
        CapabilityEffect::External => CapabilityEffectDto::External,
    }
}

fn presentation_to_dto(presentation: engine::CapabilityPresentation) -> CapabilityPresentationDto {
    CapabilityPresentationDto {
        label: presentation.label,
        description: presentation.description,
        category: presentation.category,
        search_terms: presentation.search_terms,
    }
}

fn status_for(contract: &CapabilityContract) -> CapabilityStatusDto {
    let provider_health =
        contract.effects.contains(&CapabilityEffect::External).then(|| "unknown".to_owned());
    CapabilityStatusDto {
        availability: CapabilityAvailabilityDto::Available,
        reason: None,
        provider_health,
        status_revision: 0,
    }
}
