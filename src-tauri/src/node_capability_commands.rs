//! Exact Node Capability and Generation Profile command projections.

use std::time::{Duration, Instant};

use engine::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
    NodeCapabilityContractVersion, NodeCapabilityExecutionKind, NodeCapabilityInputBindingContract,
    NodeCapabilityParameterConstraint, NodeCapabilityParameterPresence,
    NodeCapabilityParameterValue, WorkflowDataType,
};
use nodes::{
    GenerationProfileAvailabilityIndeterminateReason, GenerationProfileAvailabilityState,
    GenerationProfileError, GenerationProfileListForCapabilityQuery,
    GenerationProfileUnavailableReason,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

use crate::{
    composition::DesktopActivatedCommandDependencies,
    desktop_backend_config::{DesktopErrorCode, DesktopErrorContext, DesktopErrorDto},
};

/// Empty request for the immutable exact-seven capability list.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeCapabilityListRequestDto {}

/// Exact capability selection for compatible Generation Profiles.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationProfileListForCapabilityRequestDto {
    /// Stable capability family ID.
    pub capability_id: String,
    /// Exact `major.minor` version text.
    pub capability_version: String,
}

/// Exact capability identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NodeCapabilityRefDto {
    /// Stable family ID.
    pub id: String,
    /// Exact `major.minor` version.
    pub version: String,
}

/// One complete immutable capability contract projection.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NodeCapabilityContractDto {
    /// Exact immutable contract identity.
    pub capability_ref: NodeCapabilityRefDto,
    /// Parameter declarations in authoritative presentation order.
    pub parameters: Vec<NodeCapabilityParameterDto>,
    /// Input declarations in authoritative presentation order.
    pub inputs: Vec<NodeCapabilityInputDto>,
    /// Output declarations in authoritative presentation order.
    pub outputs: Vec<NodeCapabilityOutputDto>,
    /// Closed business execution classification.
    pub execution_kind: String,
}

/// One capability parameter declaration.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NodeCapabilityParameterDto {
    /// Capability-owned parameter key.
    pub key: String,
    /// Closed tagged constraint.
    pub constraint: Value,
    /// `required` or an exact default value.
    pub presence: Value,
}

/// One capability input declaration.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NodeCapabilityInputDto {
    /// Capability-owned input key.
    pub key: String,
    /// Closed tagged binding contract.
    pub binding: Value,
}

/// One mandatory capability output declaration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NodeCapabilityOutputDto {
    /// Capability-owned output key.
    pub key: String,
    /// Exact runtime data type.
    pub data_type: String,
    /// Whether this is the presentation output.
    pub is_primary: bool,
}

/// One selectable Generation Profile and current availability.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GenerationProfileForCapabilityDto {
    /// Canonical `id@version` profile ref.
    pub profile_ref: String,
    /// Provider-independent display name.
    pub display_name: String,
    /// Current expiring availability.
    pub availability: GenerationProfileAvailabilityDto,
}

/// Current expiring availability DTO.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GenerationProfileAvailabilityDto {
    /// `available`, `unavailable`, or `indeterminate`.
    pub state: String,
    /// Closed reason when state is not available.
    pub reason: Option<String>,
    /// Optional safe retry epoch as decimal text.
    pub retry_after_epoch_ms: Option<String>,
    /// Observation UTC epoch as decimal text.
    pub observed_at_epoch_ms: String,
    /// Expiry UTC epoch as decimal text.
    pub expires_at_epoch_ms: String,
}

/// Lists the exact seven active capability contracts.
#[tauri::command(rename_all = "snake_case")]
pub fn node_capability_list(
    _request: NodeCapabilityListRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Vec<NodeCapabilityContractDto> {
    node_capability_list_with_dependencies(&state)
}

/// Lists contracts against explicit already-composed dependencies.
pub fn node_capability_list_with_dependencies(
    state: &DesktopActivatedCommandDependencies,
) -> Vec<NodeCapabilityContractDto> {
    state.node_capability_list.list_node_capabilities().iter().map(contract_dto).collect()
}

/// Lists compatible active profiles with one current availability observation each.
#[tauri::command(rename_all = "snake_case")]
pub async fn generation_profile_list_for_capability(
    request: GenerationProfileListForCapabilityRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<Vec<GenerationProfileForCapabilityDto>, DesktopErrorDto> {
    generation_profile_list_with_dependencies(request, &state).await
}

/// Lists profiles against explicit already-composed dependencies.
pub async fn generation_profile_list_with_dependencies(
    request: GenerationProfileListForCapabilityRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<Vec<GenerationProfileForCapabilityDto>, DesktopErrorDto> {
    let capability_ref = capability_ref(&request)?;
    let items = state
        .generation_profile_list
        .list_generation_profiles_for_capability(GenerationProfileListForCapabilityQuery::new(
            capability_ref,
            Instant::now() + Duration::from_secs(5),
        ))
        .await
        .map_err(profile_error)?;
    Ok(items
        .into_iter()
        .map(|item| GenerationProfileForCapabilityDto {
            profile_ref: item.definition().profile_ref().to_string(),
            display_name: item.definition().display_name().as_str().to_owned(),
            availability: availability_dto(item.availability()),
        })
        .collect())
}

fn contract_dto(contract: &NodeCapabilityContract) -> NodeCapabilityContractDto {
    NodeCapabilityContractDto {
        capability_ref: NodeCapabilityRefDto {
            id: contract.contract_ref().id().as_str().to_owned(),
            version: format!(
                "{}.{}",
                contract.contract_ref().version().major(),
                contract.contract_ref().version().minor()
            ),
        },
        parameters: contract
            .parameters()
            .iter()
            .map(|parameter| NodeCapabilityParameterDto {
                key: parameter.key().as_str().to_owned(),
                constraint: constraint_value(parameter.constraint()),
                presence: presence_value(parameter.presence()),
            })
            .collect(),
        inputs: contract
            .inputs()
            .iter()
            .map(|input| NodeCapabilityInputDto {
                key: input.key().as_str().to_owned(),
                binding: binding_value(input.binding()),
            })
            .collect(),
        outputs: contract
            .outputs()
            .iter()
            .map(|output| NodeCapabilityOutputDto {
                key: output.key().as_str().to_owned(),
                data_type: data_type(output.data_type()).to_owned(),
                is_primary: output.is_primary(),
            })
            .collect(),
        execution_kind: match contract.execution_kind() {
            NodeCapabilityExecutionKind::PureValue => "pure_value",
            NodeCapabilityExecutionKind::ManagedAssetRead => "managed_asset_read",
            NodeCapabilityExecutionKind::ContentGeneration => "content_generation",
            NodeCapabilityExecutionKind::MediaTransformation => "media_transformation",
            NodeCapabilityExecutionKind::ContentAnalysis => "content_analysis",
        }
        .to_owned(),
    }
}

fn constraint_value(value: &NodeCapabilityParameterConstraint) -> Value {
    match value {
        NodeCapabilityParameterConstraint::UnsignedIntegerRange { minimum, maximum } => {
            json!({"kind":"unsigned_integer_range","minimum":minimum.to_string(),"maximum":maximum.to_string()})
        }
        NodeCapabilityParameterConstraint::UnsignedIntegerAllowedValues(values) => json!({
            "kind":"unsigned_integer_allowed_values",
            "values": values.iter().map(u64::to_string).collect::<Vec<_>>()
        }),
        NodeCapabilityParameterConstraint::TextUtf8Bytes { minimum, maximum } => {
            json!({"kind":"text_utf8_bytes","minimum":minimum,"maximum":maximum})
        }
        NodeCapabilityParameterConstraint::ChoiceAllowedKeys(values) => json!({
            "kind":"choice_allowed_keys",
            "values": values.iter().map(|value| value.as_str()).collect::<Vec<_>>()
        }),
        NodeCapabilityParameterConstraint::GenerationProfileRef => {
            json!({"kind":"generation_profile_ref"})
        }
        NodeCapabilityParameterConstraint::ManagedAssetId { media_kind } => {
            json!({"kind":"managed_asset_id","media_kind":data_type(*media_kind)})
        }
    }
}

fn presence_value(value: &NodeCapabilityParameterPresence) -> Value {
    match value {
        NodeCapabilityParameterPresence::Required => json!({"kind":"required"}),
        NodeCapabilityParameterPresence::OptionalWithDefault(value) => {
            json!({"kind":"optional_with_default","default":parameter_value(value)})
        }
    }
}

fn parameter_value(value: &NodeCapabilityParameterValue) -> Value {
    match value {
        NodeCapabilityParameterValue::UnsignedInteger(value) => {
            json!({"kind":"unsigned_integer","value":value.to_string()})
        }
        NodeCapabilityParameterValue::Text(value) => json!({"kind":"text","value":value}),
        NodeCapabilityParameterValue::Choice(value) => {
            json!({"kind":"choice","value":value.as_str()})
        }
        NodeCapabilityParameterValue::GenerationProfile(value) => json!({
            "kind":"generation_profile",
            "profile_id":value.profile_id(),
            "version":value.version().to_string()
        }),
        NodeCapabilityParameterValue::ManagedAsset(value) => json!({
            "kind":"managed_asset",
            "asset_id":uuid::Uuid::from_bytes(value.asset_id().as_bytes()).hyphenated().to_string()
        }),
    }
}

fn binding_value(value: &NodeCapabilityInputBindingContract) -> Value {
    match value {
        NodeCapabilityInputBindingContract::OptionalSingleValue { data_type: value } => {
            json!({"kind":"optional_single_value","data_type":data_type(*value)})
        }
        NodeCapabilityInputBindingContract::RequiredSingleValue { data_type: value } => {
            json!({"kind":"required_single_value","data_type":data_type(*value)})
        }
        NodeCapabilityInputBindingContract::OrderedReferences {
            minimum_items,
            maximum_items,
            accepted_data_types_by_role,
        } => json!({
            "kind":"ordered_references",
            "minimum_items":minimum_items,
            "maximum_items":maximum_items,
            "accepted_data_types_by_role":accepted_data_types_by_role.iter().map(|(role, accepted)| {
                let values = [WorkflowDataType::Image, WorkflowDataType::Video, WorkflowDataType::Audio]
                    .into_iter().filter(|value| accepted.contains(*value)).map(data_type).collect::<Vec<_>>();
                (role.as_str(), values)
            }).collect::<std::collections::BTreeMap<_,_>>()
        }),
    }
}

fn availability_dto(
    value: &nodes::GenerationProfileAvailabilityObservation,
) -> GenerationProfileAvailabilityDto {
    let (state, reason, retry_after) = match value.state() {
        GenerationProfileAvailabilityState::Available => ("available", None, None),
        GenerationProfileAvailabilityState::Unavailable { reason, retry_after } => (
            "unavailable",
            Some(unavailable_reason(*reason)),
            retry_after.map(|value| value.epoch_ms().to_string()),
        ),
        GenerationProfileAvailabilityState::Indeterminate { reason } => {
            ("indeterminate", Some(indeterminate_reason(*reason)), None)
        }
    };
    GenerationProfileAvailabilityDto {
        state: state.to_owned(),
        reason: reason.map(str::to_owned),
        retry_after_epoch_ms: retry_after,
        observed_at_epoch_ms: value.observed_at_epoch_ms().to_string(),
        expires_at_epoch_ms: value.expires_at_epoch_ms().to_string(),
    }
}

fn capability_ref(
    request: &GenerationProfileListForCapabilityRequestDto,
) -> Result<NodeCapabilityContractRef, DesktopErrorDto> {
    let (major, minor) = request.capability_version.split_once('.').ok_or_else(invalid_request)?;
    let version = NodeCapabilityContractVersion::new(
        major.parse().map_err(|_| invalid_request())?,
        minor.parse().map_err(|_| invalid_request())?,
    )
    .map_err(|_| invalid_request())?;
    if format!("{}.{}", version.major(), version.minor()) != request.capability_version {
        return Err(invalid_request());
    }
    Ok(NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(request.capability_id.clone())
            .map_err(|_| invalid_request())?,
        version,
    ))
}

fn data_type(value: WorkflowDataType) -> &'static str {
    match value {
        WorkflowDataType::Text => "text",
        WorkflowDataType::Image => "image",
        WorkflowDataType::Video => "video",
        WorkflowDataType::Audio => "audio",
    }
}

fn unavailable_reason(value: GenerationProfileUnavailableReason) -> &'static str {
    match value {
        GenerationProfileUnavailableReason::NoConfiguredRoute => "no_configured_route",
        GenerationProfileUnavailableReason::AuthenticationRequired => "authentication_required",
        GenerationProfileUnavailableReason::PolicyBlocked => "policy_blocked",
        GenerationProfileUnavailableReason::QuotaUnavailable => "quota_unavailable",
        GenerationProfileUnavailableReason::RateLimited => "rate_limited",
        GenerationProfileUnavailableReason::ProviderUnavailable => "provider_unavailable",
        GenerationProfileUnavailableReason::NativeModelUnavailable => "native_model_unavailable",
    }
}

fn indeterminate_reason(value: GenerationProfileAvailabilityIndeterminateReason) -> &'static str {
    match value {
        GenerationProfileAvailabilityIndeterminateReason::ProbeTimedOut => "probe_timed_out",
        GenerationProfileAvailabilityIndeterminateReason::NetworkOffline => "network_offline",
        GenerationProfileAvailabilityIndeterminateReason::UntrustedResponse => "untrusted_response",
    }
}

fn profile_error(error: GenerationProfileError) -> DesktopErrorDto {
    match error {
        GenerationProfileError::CapabilityNotFound
        | GenerationProfileError::InvalidProfileRef
        | GenerationProfileError::InvalidDefinition
        | GenerationProfileError::InvalidDisplayName
        | GenerationProfileError::ProfileNotFound
        | GenerationProfileError::ProfileIncompatible
        | GenerationProfileError::InvalidAvailabilityObservation
        | GenerationProfileError::AvailabilityRequestInvalid => invalid_request(),
        GenerationProfileError::AvailabilityReadFailed
        | GenerationProfileError::DeadlineExceeded => {
            DesktopErrorDto::from_context(DesktopErrorContext {
                code: DesktopErrorCode::ProviderUnavailable,
                retryable: true,
                retry_after_epoch_ms: None,
                target: None,
                correlation_id: None,
            })
        }
    }
}

fn invalid_request() -> DesktopErrorDto {
    DesktopErrorDto::from_context(DesktopErrorContext {
        code: DesktopErrorCode::NodeCapabilityInvalidRequest,
        retryable: false,
        retry_after_epoch_ms: None,
        target: None,
        correlation_id: None,
    })
}

#[cfg(test)]
mod tests;
