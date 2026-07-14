//! Bounded Agent discovery over exact registered capability contracts.

mod support;

use crate::assistant_operations::{
    OperationEffect, OperationInputSchemaMode, OperationOutputSchemaMode, OperationRegistration,
    OperationRegistrationError, RequestContext,
};
use crate::capability_catalog::project_entry;
use crate::dto::{CapabilityBundleDto, CapabilityRefDto, CapabilitySummaryDto};
use crate::state::AppState;
use crate::workflow_authority::WorkflowAuthority;
use engine::{CapabilityRef, NodeRegistry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use support::{
    DiscoveryLedger, DiscoveryState, capability_ref_from_dto, degraded_status, normalized_kinds,
    normalized_query, reference_to_dto, schema_bytes, score_projection, search_result,
    to_handler_error, validate_describe_refs,
};

pub use support::CapabilityDiscoveryError;

/// Maximum number of search results returned to the Agent.
pub const MAX_SEARCH_RESULTS: usize = 5;
/// Maximum number of refs accepted by one describe call.
pub const MAX_DESCRIBE_REFS_PER_CALL: usize = 3;
/// Maximum number of distinct refs describable in one invocation.
pub const MAX_DESCRIBED_REFS: usize = 8;
/// Maximum serialized params-schema bytes describable in one invocation.
pub const MAX_SCHEMA_BYTES: usize = 96 * 1024;

/// Input for the bounded capability search operation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySearchInput {
    /// User goal or desired transformation.
    pub query: String,
    /// Optional presentation categories to search.
    pub kinds: Option<Vec<String>>,
}

/// One search result containing only a summary and live status.
pub type CapabilitySearchResult = CapabilitySummaryDto;

/// Output for the bounded capability search operation.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySearchOutput {
    /// At most [`MAX_SEARCH_RESULTS`] stable results.
    pub capabilities: Vec<CapabilitySearchResult>,
}

/// Input for exact capability description.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDescribeInput {
    /// Exact refs returned by search or already present in the Workflow.
    #[schemars(length(max = 3))]
    pub refs: Vec<CapabilityRefDto>,
}

/// One exact capability description, including degraded persisted refs.
pub type CapabilityDescription = CapabilityBundleDto;

/// Output for exact capability description.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDescribeOutput {
    /// Descriptions in the same order as the requested refs.
    pub capabilities: Vec<CapabilityDescription>,
}

/// Application-owned bounded capability discovery service.
pub struct CapabilityDiscovery {
    registry: Arc<NodeRegistry>,
    workflow_authority: Arc<WorkflowAuthority>,
    ledger: Mutex<DiscoveryLedger>,
}

impl CapabilityDiscovery {
    /// Creates discovery over the application registry and Workflow store.
    #[must_use]
    pub fn new(registry: Arc<NodeRegistry>, workflow_authority: Arc<WorkflowAuthority>) -> Self {
        Self { registry, workflow_authority, ledger: Mutex::new(DiscoveryLedger::default()) }
    }

    /// Creates discovery from the app composition root.
    #[must_use]
    pub fn from_state(state: &AppState) -> Self {
        Self::new(Arc::clone(&state.registry), Arc::clone(&state.workflow_authority))
    }

    /// Searches current exact refs and records the refs admitted for this request.
    pub fn search(
        &self,
        context: &RequestContext,
        input: CapabilitySearchInput,
    ) -> Result<CapabilitySearchOutput, CapabilityDiscoveryError> {
        let query = normalized_query(&input.query)?;
        let kinds = normalized_kinds(input.kinds.as_deref())?;
        let terms = query.split_whitespace().collect::<Vec<_>>();
        let mut ranked = Vec::new();
        for reference in self.registry.current_capability_refs() {
            let projection =
                nodes::project_capability(&self.registry, &reference).map_err(|error| {
                    CapabilityDiscoveryError::RegistryUnavailable { message: error.to_string() }
                })?;
            if let Some(score) = score_projection(&projection, &terms, &kinds) {
                ranked.push((score, projection));
            }
        }
        ranked.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.contract.reference.cmp(&right.1.contract.reference))
        });
        let entries = ranked
            .into_iter()
            .take(MAX_SEARCH_RESULTS)
            .map(|(_, projection)| project_entry(projection))
            .collect::<Vec<_>>();
        let results = entries.iter().map(search_result).collect::<Vec<_>>();
        self.record_search(context.request_id(), results.iter().map(|result| &result.reference))?;
        Ok(CapabilitySearchOutput { capabilities: results })
    }

    /// Describes refs admitted by search or persisted in the current Workflow.
    pub fn describe(
        &self,
        context: &RequestContext,
        input: CapabilityDescribeInput,
    ) -> Result<CapabilityDescribeOutput, CapabilityDiscoveryError> {
        validate_describe_refs(&input.refs)?;
        let requested = input.refs.iter().map(capability_ref_from_dto).collect::<Vec<_>>();
        let persisted = self.persisted_refs(context.project_id())?;
        let state = self.ledger_state(context.request_id())?;
        for reference in &requested {
            if !state.search_refs.contains(reference) && !persisted.contains(reference) {
                return Err(CapabilityDiscoveryError::NotAdmitted { reference: reference.clone() });
            }
        }
        let mut descriptions = Vec::with_capacity(requested.len());
        let mut new_refs = BTreeSet::new();
        let mut new_schema_bytes = 0_usize;
        for reference in &requested {
            let description = self.describe_one(reference, &persisted)?;
            if state.described_refs.contains(reference) {
                descriptions.push(description);
                continue;
            }
            new_refs.insert(reference.clone());
            new_schema_bytes = new_schema_bytes.saturating_add(schema_bytes(&description)?);
            descriptions.push(description);
        }
        if state.described_refs.len().saturating_add(new_refs.len()) > MAX_DESCRIBED_REFS {
            return Err(CapabilityDiscoveryError::DescribeBudgetExceeded {
                maximum: MAX_DESCRIBED_REFS,
            });
        }
        if state.schema_bytes.saturating_add(new_schema_bytes) > MAX_SCHEMA_BYTES {
            return Err(CapabilityDiscoveryError::SchemaBudgetExceeded {
                maximum: MAX_SCHEMA_BYTES,
            });
        }
        self.record_descriptions(context.request_id(), new_refs, new_schema_bytes)?;
        Ok(CapabilityDescribeOutput { capabilities: descriptions })
    }

    /// Builds the two fixed Agent operations backed by this service.
    pub fn operation_registrations(
        self: Arc<Self>,
    ) -> Result<Vec<OperationRegistration>, OperationRegistrationError> {
        let search_service = Arc::clone(&self);
        let search = OperationRegistration::new::<CapabilitySearchInput, CapabilitySearchOutput, _>(
            "capability_search",
            1,
            "Find a bounded set of exact workflow capabilities for a goal.",
            OperationEffect::LocalRead,
            OperationInputSchemaMode::Strict,
            move |context: &RequestContext, input: CapabilitySearchInput| {
                let context = context.clone();
                let service = Arc::clone(&search_service);
                async move { service.search(&context, input).map_err(to_handler_error) }
            },
        )?;
        let describe_service = Arc::clone(&self);
        let describe = OperationRegistration::new_with_output_mode::<
            CapabilityDescribeInput,
            CapabilityDescribeOutput,
            _,
        >(
            "capability_describe",
            1,
            "Describe up to three exact capabilities admitted by search or the current Workflow.",
            OperationEffect::LocalRead,
            OperationInputSchemaMode::Strict,
            OperationOutputSchemaMode::CapabilityDescribe,
            move |context: &RequestContext, input: CapabilityDescribeInput| {
                let context = context.clone();
                let service = Arc::clone(&describe_service);
                async move { service.describe(&context, input).map_err(to_handler_error) }
            },
        )?;
        Ok(vec![search, describe])
    }

    fn describe_one(
        &self,
        reference: &CapabilityRef,
        persisted: &BTreeSet<CapabilityRef>,
    ) -> Result<CapabilityDescription, CapabilityDiscoveryError> {
        match nodes::project_capability(&self.registry, reference) {
            Ok(projection) => {
                let entry = project_entry(projection);
                Ok(CapabilityDescription {
                    selector: Some(entry.selector),
                    reference: reference_to_dto(reference),
                    contract: Some(entry.contract),
                    presentation: Some(entry.presentation),
                    status: entry.status,
                })
            }
            Err(_) if persisted.contains(reference) => Ok(CapabilityDescription {
                selector: None,
                reference: reference_to_dto(reference),
                contract: None,
                presentation: None,
                status: degraded_status(
                    "exact capability version is unavailable; migrate or remove the persisted node",
                ),
            }),
            Err(_) => {
                Err(CapabilityDiscoveryError::StaleReference { reference: reference.clone() })
            }
        }
    }

    fn persisted_refs(
        &self,
        project_id: &str,
    ) -> Result<BTreeSet<CapabilityRef>, CapabilityDiscoveryError> {
        let head = self.workflow_authority.load_head(project_id).map_err(|source| {
            CapabilityDiscoveryError::WorkflowUnavailable { message: source.to_string() }
        })?;
        head
            .map(|head| head.workflow.nodes)
            .unwrap_or_default()
            .into_iter()
            .map(|node| {
                self.registry.persisted_workflow_capability_reference(
                    &node.id,
                    &node.type_id,
                    &node.contract_version,
                    &node.params,
                )
            })
            .collect::<Result<_, _>>()
            .map_err(|source| CapabilityDiscoveryError::WorkflowUnavailable {
                message: source.to_string(),
            })
    }

    fn ledger_state(&self, request_id: &str) -> Result<DiscoveryState, CapabilityDiscoveryError> {
        let mut ledger =
            self.ledger.lock().map_err(|_| CapabilityDiscoveryError::LedgerUnavailable)?;
        ledger.state(request_id)
    }

    fn record_search<'a>(
        &self,
        request_id: &str,
        refs: impl IntoIterator<Item = &'a CapabilityRefDto>,
    ) -> Result<(), CapabilityDiscoveryError> {
        let mut ledger =
            self.ledger.lock().map_err(|_| CapabilityDiscoveryError::LedgerUnavailable)?;
        ledger.record_search(request_id, refs)
    }

    fn record_descriptions(
        &self,
        request_id: &str,
        refs: BTreeSet<CapabilityRef>,
        schema_bytes: usize,
    ) -> Result<(), CapabilityDiscoveryError> {
        let mut ledger =
            self.ledger.lock().map_err(|_| CapabilityDiscoveryError::LedgerUnavailable)?;
        ledger.record_descriptions(request_id, refs, schema_bytes)
    }
}
