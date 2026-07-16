use engine::node_capability::WorkflowDataType;
use nodes::{GenerationProfileAvailabilityIndeterminateReason, GenerationProfileUnavailableReason};

pub(super) fn data_type(value: WorkflowDataType) -> &'static str {
    match value {
        WorkflowDataType::Text => "text",
        WorkflowDataType::Image => "image",
        WorkflowDataType::Video => "video",
        WorkflowDataType::Audio => "audio",
    }
}

pub(super) fn unavailable_reason(value: GenerationProfileUnavailableReason) -> &'static str {
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

pub(super) fn indeterminate_reason(
    value: GenerationProfileAvailabilityIndeterminateReason,
) -> &'static str {
    match value {
        GenerationProfileAvailabilityIndeterminateReason::ProbeTimedOut => "probe_timed_out",
        GenerationProfileAvailabilityIndeterminateReason::NetworkOffline => "network_offline",
        GenerationProfileAvailabilityIndeterminateReason::UntrustedResponse => "untrusted_response",
    }
}
