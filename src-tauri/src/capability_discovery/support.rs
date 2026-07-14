use super::{CapabilityDescription, CapabilitySearchInput, CapabilitySearchResult};
use crate::assistant_operations::OperationHandlerError;
use crate::dto::{
    CapabilityAvailabilityDto, CapabilityCatalogEntryDto, CapabilityRefDto, CapabilityStatusDto,
};
use engine::CapabilityRef;
use schemars::{JsonSchema, r#gen::SchemaGenerator, schema::Schema};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

const MAX_LEDGER_ENTRIES: usize = 256;
const LEDGER_TTL: Duration = Duration::from_secs(10 * 60);

impl JsonSchema for CapabilitySearchInput {
    fn schema_name() -> String {
        "CapabilitySearchInput".to_owned()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        schema_from_json(
            json!({
                "type": "object",
                "required": ["query", "kinds"],
                "properties": {
                    "query": { "type": "string" },
                    "kinds": {
                        "type": ["array", "null"],
                        "items": { "type": "string" }
                    }
                },
                "additionalProperties": false
            }),
            "capability search input",
        )
    }
}

fn schema_from_json(value: Value, label: &str) -> Schema {
    match serde_json::from_value(value) {
        Ok(schema) => schema,
        Err(error) => {
            tracing::error!(schema = label, error = %error, "failed to build static JSON Schema");
            Schema::Bool(false)
        }
    }
}

#[derive(Default)]
pub(super) struct DiscoveryLedger {
    states: HashMap<String, DiscoveryState>,
}

impl DiscoveryLedger {
    pub(super) fn state(
        &mut self,
        request_id: &str,
    ) -> Result<DiscoveryState, CapabilityDiscoveryError> {
        let state = self.entry_mut(request_id)?;
        state.last_seen = Instant::now();
        Ok(state.clone())
    }

    pub(super) fn record_search<'a>(
        &mut self,
        request_id: &str,
        refs: impl IntoIterator<Item = &'a CapabilityRefDto>,
    ) -> Result<(), CapabilityDiscoveryError> {
        let state = self.entry_mut(request_id)?;
        state.last_seen = Instant::now();
        state.search_refs.extend(refs.into_iter().map(capability_ref_from_dto));
        Ok(())
    }

    pub(super) fn record_descriptions(
        &mut self,
        request_id: &str,
        refs: BTreeSet<CapabilityRef>,
        schema_bytes: usize,
    ) -> Result<(), CapabilityDiscoveryError> {
        let state = self.entry_mut(request_id)?;
        state.last_seen = Instant::now();
        state.described_refs.extend(refs);
        state.schema_bytes = state.schema_bytes.saturating_add(schema_bytes);
        Ok(())
    }

    fn entry_mut(
        &mut self,
        request_id: &str,
    ) -> Result<&mut DiscoveryState, CapabilityDiscoveryError> {
        self.prune();
        if !self.states.contains_key(request_id) && self.states.len() >= MAX_LEDGER_ENTRIES {
            return Err(CapabilityDiscoveryError::TooManyInvocations);
        }
        Ok(self.states.entry(request_id.to_owned()).or_default())
    }

    fn prune(&mut self) {
        let now = Instant::now();
        self.states.retain(|_, state| now.duration_since(state.last_seen) <= LEDGER_TTL);
    }
}

#[derive(Clone)]
pub(super) struct DiscoveryState {
    pub(super) search_refs: BTreeSet<CapabilityRef>,
    pub(super) described_refs: BTreeSet<CapabilityRef>,
    pub(super) schema_bytes: usize,
    last_seen: Instant,
}

impl Default for DiscoveryState {
    fn default() -> Self {
        Self {
            search_refs: BTreeSet::new(),
            described_refs: BTreeSet::new(),
            schema_bytes: 0,
            last_seen: Instant::now(),
        }
    }
}

/// Structured failures returned by the bounded discovery operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CapabilityDiscoveryError {
    /// The search goal was empty after trimming.
    #[error("capability search goal must not be empty")]
    EmptyGoal,
    /// A supplied category was empty after trimming.
    #[error("capability kind `{kind}` must not be empty")]
    EmptyKind { kind: String },
    /// The describe call exceeded its per-call ref limit.
    #[error("capability describe accepts at most {maximum} refs")]
    TooManyRefs { maximum: usize },
    /// The describe call repeated an exact ref.
    #[error("capability describe refs must be distinct")]
    DuplicateRefs,
    /// A new ref was not admitted by search and is not persisted.
    #[error("capability ref `{reference:?}` was not admitted by search or the current Workflow")]
    NotAdmitted { reference: CapabilityRef },
    /// An admitted ref disappeared before it could be described.
    #[error("capability ref `{reference:?}` is stale or unavailable for new nodes")]
    StaleReference { reference: CapabilityRef },
    /// The distinct-ref invocation budget was exceeded.
    #[error("capability describe exceeds the {maximum} distinct-ref invocation budget")]
    DescribeBudgetExceeded { maximum: usize },
    /// The serialized schema budget was exceeded.
    #[error("capability describe exceeds the {maximum}-byte schema budget")]
    SchemaBudgetExceeded { maximum: usize },
    /// The bounded per-request ledger is full.
    #[error("capability discovery has too many active invocations")]
    TooManyInvocations,
    /// The in-memory discovery ledger could not be locked.
    #[error("capability discovery state is unavailable")]
    LedgerUnavailable,
    /// Persisted Workflow data could not be loaded or decoded.
    #[error("capability Workflow is unavailable: {message}")]
    WorkflowUnavailable { message: String },
    /// The registry projection violated its registration invariant.
    #[error("capability registry is unavailable: {message}")]
    RegistryUnavailable { message: String },
    /// The serialized schema body could not be measured.
    #[error("capability schema is unavailable: {message}")]
    SchemaUnavailable { message: String },
}

impl CapabilityDiscoveryError {
    pub(super) fn code(&self) -> &'static str {
        match self {
            Self::EmptyGoal => "CAPABILITY_GOAL_REQUIRED",
            Self::EmptyKind { .. } => "CAPABILITY_KIND_INVALID",
            Self::TooManyRefs { .. } | Self::DuplicateRefs => "CAPABILITY_DESCRIBE_INPUT_INVALID",
            Self::NotAdmitted { .. } => "CAPABILITY_NOT_ADMITTED",
            Self::StaleReference { .. } => "CAPABILITY_STALE",
            Self::DescribeBudgetExceeded { .. } => "CAPABILITY_REF_BUDGET_EXCEEDED",
            Self::SchemaBudgetExceeded { .. } => "CAPABILITY_SCHEMA_BUDGET_EXCEEDED",
            Self::TooManyInvocations => "CAPABILITY_INVOCATION_LIMIT",
            Self::LedgerUnavailable => "CAPABILITY_STATE_UNAVAILABLE",
            Self::WorkflowUnavailable { .. } => "CAPABILITY_WORKFLOW_UNAVAILABLE",
            Self::RegistryUnavailable { .. } => "CAPABILITY_REGISTRY_UNAVAILABLE",
            Self::SchemaUnavailable { .. } => "CAPABILITY_SCHEMA_UNAVAILABLE",
        }
    }
}

pub(super) fn to_handler_error(error: CapabilityDiscoveryError) -> OperationHandlerError {
    OperationHandlerError::new(error.code(), error.to_string())
}

pub(super) fn normalized_query(query: &str) -> Result<String, CapabilityDiscoveryError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(CapabilityDiscoveryError::EmptyGoal);
    }
    Ok(query.to_lowercase())
}

pub(super) fn normalized_kinds(
    kinds: Option<&[String]>,
) -> Result<Vec<String>, CapabilityDiscoveryError> {
    kinds
        .unwrap_or_default()
        .iter()
        .map(|kind| {
            let normalized = kind.trim().to_lowercase();
            if normalized.is_empty() {
                Err(CapabilityDiscoveryError::EmptyKind { kind: kind.clone() })
            } else {
                Ok(normalized)
            }
        })
        .collect()
}

pub(super) fn score_projection(
    projection: &nodes::CapabilityProjection,
    terms: &[&str],
    kinds: &[String],
) -> Option<u32> {
    let category = projection.presentation.category.to_lowercase();
    if !kinds.is_empty() && !kinds.iter().any(|kind| kind == &category) {
        return None;
    }
    let searchable = [
        projection.contract.reference.id.to_lowercase(),
        projection.presentation.label.to_lowercase(),
        projection.presentation.description.to_lowercase(),
        projection.presentation.category.to_lowercase(),
        projection.presentation.search_terms.join(" ").to_lowercase(),
    ];
    terms.iter().try_fold(0_u32, |score, term| {
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

pub(super) fn search_result(entry: &CapabilityCatalogEntryDto) -> CapabilitySearchResult {
    CapabilitySearchResult {
        selector: entry.selector.clone(),
        reference: entry.contract.reference.clone(),
        presentation: entry.presentation.clone(),
        status: entry.status.clone(),
    }
}

pub(super) fn validate_describe_refs(
    refs: &[CapabilityRefDto],
) -> Result<(), CapabilityDiscoveryError> {
    if refs.len() > super::MAX_DESCRIBE_REFS_PER_CALL {
        return Err(CapabilityDiscoveryError::TooManyRefs {
            maximum: super::MAX_DESCRIBE_REFS_PER_CALL,
        });
    }
    let distinct = refs.iter().map(capability_ref_from_dto).collect::<BTreeSet<_>>();
    if distinct.len() != refs.len() {
        return Err(CapabilityDiscoveryError::DuplicateRefs);
    }
    Ok(())
}

pub(super) fn schema_bytes(
    description: &CapabilityDescription,
) -> Result<usize, CapabilityDiscoveryError> {
    let Some(contract) = description.contract.as_ref() else {
        return Ok(0);
    };
    serde_json::to_vec(&contract.params_schema)
        .map(|bytes| bytes.len())
        .map_err(|error| CapabilityDiscoveryError::SchemaUnavailable { message: error.to_string() })
}

pub(super) fn capability_ref_from_dto(reference: &CapabilityRefDto) -> CapabilityRef {
    CapabilityRef::new(&reference.id, &reference.version)
}

pub(super) fn reference_to_dto(reference: &CapabilityRef) -> CapabilityRefDto {
    CapabilityRefDto { id: reference.id.clone(), version: reference.version.clone() }
}

pub(super) fn degraded_status(reason: &str) -> CapabilityStatusDto {
    CapabilityStatusDto {
        availability: CapabilityAvailabilityDto::Degraded,
        reason: Some(reason.to_owned()),
        provider_health: None,
        status_revision: 0,
    }
}
