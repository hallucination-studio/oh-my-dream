//! Application boundary projections for versioned workflow capabilities.

use crate::command_error::command_error;
use crate::dto::{
    CapabilityAvailabilityDto, CapabilityBundleDto, CapabilityBundlesDto, CapabilityCardinalityDto,
    CapabilityCatalogDto, CapabilityCatalogEntryDto, CapabilityContractDto, CapabilityEffectDto,
    CapabilityPortDto, CapabilityPresentationDto, CapabilityRefDto, CapabilitySearchPageDto,
    CapabilitySelectorDto, CapabilityStatusDto, CapabilitySummaryDto,
};
use crate::state::AppState;
use engine::{CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityRef, NodeRegistry};
use nodes::CapabilityProjection;
use std::collections::BTreeMap;
use tauri::State;

const MAX_PALETTE_PAGE: usize = 24;
const MAX_BUNDLE_REFS: usize = 32;

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

/// Searches current capability summaries for the paged React palette.
#[tauri::command(rename_all = "snake_case")]
pub fn search_capabilities(
    query: String,
    category: Option<String>,
    type_id: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<CapabilitySearchPageDto, String> {
    search_capabilities_with_state(query, category, type_id, cursor, limit, &state)
}

/// Searches capability summaries without creating or revising a Workflow.
pub fn search_capabilities_with_state(
    query: String,
    category: Option<String>,
    type_id: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
    state: &AppState,
) -> Result<CapabilitySearchPageDto, String> {
    let offset = parse_cursor(cursor)?;
    let page_size = limit.unwrap_or(MAX_PALETTE_PAGE).clamp(1, MAX_PALETTE_PAGE);
    let query_terms = query.split_whitespace().map(|term| term.to_lowercase()).collect::<Vec<_>>();
    let category =
        category.as_deref().map(str::trim).filter(|value| !value.is_empty()).map(str::to_lowercase);
    let type_id = type_id.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let mut ranked = state
        .registry
        .current_capability_refs()
        .into_iter()
        .filter_map(|reference| {
            let projection = nodes::project_capability(&state.registry, &reference).ok()?;
            if type_id.is_some_and(|value| value != projection.selector.type_id) {
                return None;
            }
            let score = summary_score(&projection, &query_terms, category.as_deref())?;
            Some((score, project_entry(projection)))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.contract.reference.cmp(&right.1.contract.reference))
    });
    let total = ranked.len();
    let capabilities = ranked
        .into_iter()
        .skip(offset)
        .take(page_size)
        .map(|(_, entry)| CapabilitySummaryDto {
            selector: entry.selector,
            reference: entry.contract.reference,
            presentation: entry.presentation,
            status: entry.status,
        })
        .collect::<Vec<_>>();
    let next_cursor =
        (offset + capabilities.len() < total).then(|| (offset + capabilities.len()).to_string());
    Ok(CapabilitySearchPageDto { capabilities, next_cursor })
}

/// Loads exact capability bundles for a project-open batch or one lazy node.
#[tauri::command(rename_all = "snake_case")]
pub fn get_capability_bundles(
    refs: Vec<CapabilityRefDto>,
    state: State<'_, AppState>,
) -> Result<CapabilityBundlesDto, String> {
    get_capability_bundles_with_state(refs, &state)
}

/// Resolves exact refs while preserving missing refs as degraded placeholders.
pub fn get_capability_bundles_with_state(
    refs: Vec<CapabilityRefDto>,
    state: &AppState,
) -> Result<CapabilityBundlesDto, String> {
    if refs.len() > MAX_BUNDLE_REFS {
        return Err(command_error(
            "load capability bundles",
            format!("at most {MAX_BUNDLE_REFS} refs may be loaded at once"),
        ));
    }
    let mut capabilities = Vec::with_capacity(refs.len());
    for reference in refs {
        let exact = CapabilityRef::new(reference.id.clone(), reference.version.clone());
        let bundle = match nodes::project_capability(&state.registry, &exact) {
            Ok(projection) => {
                let entry = project_entry(projection);
                CapabilityBundleDto {
                    selector: Some(entry.selector),
                    reference,
                    contract: Some(entry.contract),
                    presentation: Some(entry.presentation),
                    status: entry.status,
                }
            }
            Err(_) => CapabilityBundleDto {
                selector: None,
                reference,
                contract: None,
                presentation: None,
                status: degraded_status("exact capability version is unavailable"),
            },
        };
        capabilities.push(bundle);
    }
    Ok(CapabilityBundlesDto { capabilities })
}

fn project_catalog(
    registry: &NodeRegistry,
) -> Result<CapabilityCatalogDto, nodes::CapabilityProjectionError> {
    let projections = nodes::project_capabilities(registry)?;
    Ok(CapabilityCatalogDto { capabilities: projections.into_iter().map(project_entry).collect() })
}

pub(crate) fn project_entry(projection: CapabilityProjection) -> CapabilityCatalogEntryDto {
    let status = status_for(&projection.contract);
    CapabilityCatalogEntryDto {
        selector: CapabilitySelectorDto {
            type_id: projection.selector.type_id,
            mode: projection.selector.mode,
        },
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
        CapabilityEffect::LocalRead => CapabilityEffectDto::LocalRead,
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

pub(crate) fn status_for(contract: &CapabilityContract) -> CapabilityStatusDto {
    let provider_health =
        contract.effects.contains(&CapabilityEffect::External).then(|| "unknown".to_owned());
    CapabilityStatusDto {
        availability: CapabilityAvailabilityDto::Available,
        reason: None,
        provider_health,
        status_revision: 0,
    }
}

fn parse_cursor(cursor: Option<String>) -> Result<usize, String> {
    cursor
        .as_deref()
        .unwrap_or("0")
        .parse::<usize>()
        .map_err(|source| command_error("parse capability cursor", source))
}

fn summary_score(
    projection: &CapabilityProjection,
    query_terms: &[String],
    category: Option<&str>,
) -> Option<u32> {
    let presentation_category = projection.presentation.category.to_lowercase();
    if category.is_some_and(|value| value != presentation_category) {
        return None;
    }
    let searchable = [
        projection.contract.reference.id.to_lowercase(),
        projection.presentation.label.to_lowercase(),
        projection.presentation.description.to_lowercase(),
        presentation_category,
        projection.presentation.search_terms.join(" ").to_lowercase(),
    ];
    query_terms.iter().try_fold(0_u32, |score, term| {
        searchable.iter().enumerate().find_map(|(index, field)| {
            field.contains(term).then_some(
                score
                    + match index {
                        0 => 8,
                        1 => 6,
                        2 => 4,
                        3 => 3,
                        _ => 2,
                    },
            )
        })
    })
}

fn degraded_status(reason: &str) -> CapabilityStatusDto {
    CapabilityStatusDto {
        availability: CapabilityAvailabilityDto::Degraded,
        reason: Some(reason.to_owned()),
        provider_health: None,
        status_revision: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::status_for;
    use engine::{CapabilityContract, CapabilityEffect, CapabilityRef};

    #[test]
    fn local_read_has_no_provider_health_marker() {
        let contract = CapabilityContract::new(
            CapabilityRef::new("ManagedLocalRead", "1.0"),
            Vec::new(),
            Vec::new(),
            serde_json::json!({}),
            serde_json::Map::new(),
            vec![CapabilityEffect::LocalRead],
        );

        assert!(status_for(&contract).provider_health.is_none());
    }
}
