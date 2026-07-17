use serde_json::json;
use uuid::Uuid;

use super::*;

#[test]
fn defaults_match_the_frozen_non_secret_startup_contract() {
    let config = DesktopBackendConfig::default();

    assert_eq!(config.sqlite_busy_timeout_ms, 5_000);
    assert_eq!(config.post_commit_effect_concurrency, 4);
    assert_eq!(config.workflow_run_concurrency, 1);
    assert_eq!(config.workflow_node_concurrency, 2);
    assert_eq!(config.generation_provider_routes.len(), 3);
    assert_eq!(config.generation_provider_routes[0].provider_id, "mock");
    assert!(!config.assistant_model.enabled);
    assert_eq!(config.assistant_model.model_profile_ref, "assistant.workflow_coauthor@1");
    assert_eq!(config.assistant_model.credential_id, "assistant.openai.default");
    assert!(config.validate().is_ok());
}

#[test]
fn config_rejects_unknown_secret_path_weakened_budget_and_duplicate_route_shapes() {
    let mut value = serde_json::to_value(DesktopBackendConfig::default()).unwrap();
    value.as_object_mut().unwrap().insert("api_key".to_owned(), json!("secret"));
    assert!(serde_json::from_value::<DesktopBackendConfig>(value).is_err());

    let mut config = DesktopBackendConfig::default();
    config.assistant_protocol_budgets.frame_max_bytes -= 1;
    assert_eq!(config.validate(), Err(DesktopBackendConfigError::InvalidConfig));

    let route = route();
    config = DesktopBackendConfig::default();
    config.generation_provider_routes = vec![route.clone(), route];
    assert_eq!(config.validate(), Err(DesktopBackendConfigError::InvalidConfig));

    let mut legacy = serde_json::to_value(DesktopBackendConfig::default()).unwrap();
    legacy["generation_provider_routes"] = json!([{
        "profile_ref": "image.high_quality_general@1",
        "generation_kind": "image",
        "provider_id": "mock",
        "route_id": "mock.image.high-quality-general.v1",
        "legacy_options": {"provider": "legacy"}
    }]);
    let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
    assert_eq!(
        DesktopBackendConfig::from_canonical_json(&legacy_bytes),
        Err(DesktopBackendConfigError::InvalidConfig)
    );
}

#[test]
fn config_canonical_json_rejects_duplicate_keys_and_noncanonical_bytes() {
    let config = DesktopBackendConfig::default();
    let canonical = config.canonical_json().unwrap();
    assert_eq!(DesktopBackendConfig::from_canonical_json(&canonical).unwrap(), config);

    let duplicate = canonical
        .strip_suffix(b"}")
        .unwrap()
        .iter()
        .copied()
        .chain(b",\"sqlite_busy_timeout_ms\":5000}".iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(
        DesktopBackendConfig::from_canonical_json(&duplicate),
        Err(DesktopBackendConfigError::InvalidConfig)
    );

    let mut noncanonical = canonical;
    noncanonical.insert(1, b' ');
    assert_eq!(
        DesktopBackendConfig::from_canonical_json(&noncanonical),
        Err(DesktopBackendConfigError::InvalidConfig)
    );
}

#[test]
fn desktop_error_translation_exposes_only_closed_safe_fields() {
    let dto = DesktopErrorDto::from_context(DesktopErrorContext {
        code: DesktopErrorCode::AssistantProtocolViolation,
        retryable: false,
        retry_after_epoch_ms: None,
        target: Some(DesktopErrorTarget::AssistantChange {
            assistant_workflow_change_id: "change-1".to_owned(),
        }),
        correlation_id: None,
    });

    assert_eq!(dto.code, "assistant.protocol_violation");
    assert_eq!(dto.message, "The Assistant response was invalid.");
    assert!(!dto.retryable);
    assert!(!serde_json::to_string(&dto).unwrap().contains("secret"));

    let internal = DesktopErrorDto::internal(Uuid::from_u128(3));
    assert_eq!(internal.code, "desktop.internal");
    assert_eq!(internal.correlation_id.as_deref(), Some("00000000-0000-0000-0000-000000000003"));
}

fn route() -> GenerationProviderRouteConfig {
    GenerationProviderRouteConfig {
        profile_ref: "image.high_quality_general@1".to_owned(),
        generation_kind: "image".to_owned(),
        provider_id: "mock".to_owned(),
        route_id: "mock.image.high-quality-general.v1".to_owned(),
    }
}
