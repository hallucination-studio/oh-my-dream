use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use engine::node_capability::*;
use nodes::*;
use projects::project::domain::ProjectId;
use uuid::Uuid;

#[derive(Clone)]
struct AvailabilityRequestSnapshot {
    capability_ref: NodeCapabilityContractRef,
    profile_refs: Vec<GenerationProfileRef>,
    deadline: Instant,
}

#[derive(Clone)]
enum AvailabilityOutcome {
    State(GenerationProfileAvailabilityState),
    Error,
    WrongProfile,
}

#[derive(Clone)]
struct RecordingAvailabilityReaderImpl {
    outcome: AvailabilityOutcome,
    requests: Arc<Mutex<Vec<AvailabilityRequestSnapshot>>>,
}

#[async_trait]
impl GenerationProfileAvailabilityReaderInterface for RecordingAvailabilityReaderImpl {
    async fn read_generation_profile_availability(
        &self,
        request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError> {
        self.requests.lock().unwrap().push(AvailabilityRequestSnapshot {
            capability_ref: request.capability_ref().clone(),
            profile_refs: request.profile_refs().to_vec(),
            deadline: request.deadline(),
        });
        match &self.outcome {
            AvailabilityOutcome::State(state) => {
                Ok(vec![observation(request.profile_refs()[0].clone(), state.clone())])
            }
            AvailabilityOutcome::Error => Err(GenerationProfileError::AvailabilityReadFailed),
            AvailabilityOutcome::WrongProfile => Ok(vec![observation(
                profile_for("video.generate_from_image"),
                GenerationProfileAvailabilityState::Available,
            )]),
        }
    }
}

#[tokio::test]
async fn readiness_reads_exactly_one_selected_profile_with_the_unchanged_deadline() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let capability = capability(
        AvailabilityOutcome::State(GenerationProfileAvailabilityState::Available),
        requests.clone(),
    );
    let profile = profile_for("image.generate_from_text");
    let deadline = Instant::now() + Duration::from_secs(4);
    let issues = capability
        .check_node_external_readiness(readiness_request(&capability, profile.clone(), deadline))
        .await;
    assert!(issues.is_empty());
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].capability_ref, contract_ref("image.generate_from_text"));
    assert_eq!(requests[0].profile_refs, [profile]);
    assert_eq!(requests[0].deadline, deadline);
}

#[tokio::test]
async fn readiness_issues_preserve_the_exact_profile_parameter_target() {
    let profile = profile_for("image.generate_from_text");
    let cases = [
        (
            AvailabilityOutcome::State(GenerationProfileAvailabilityState::Unavailable {
                reason: GenerationProfileUnavailableReason::ProviderUnavailable,
                retry_after: None,
            }),
            NodeCapabilityReadinessCategory::GenerationProfileUnavailable,
        ),
        (
            AvailabilityOutcome::State(GenerationProfileAvailabilityState::Indeterminate {
                reason: GenerationProfileAvailabilityIndeterminateReason::NetworkOffline,
            }),
            NodeCapabilityReadinessCategory::GenerationProfileAvailabilityIndeterminate,
        ),
        (
            AvailabilityOutcome::Error,
            NodeCapabilityReadinessCategory::GenerationProfileAvailabilityIndeterminate,
        ),
        (
            AvailabilityOutcome::WrongProfile,
            NodeCapabilityReadinessCategory::GenerationProfileAvailabilityIndeterminate,
        ),
    ];
    for (outcome, category) in cases {
        let capability = capability(outcome, Arc::new(Mutex::new(Vec::new())));
        let issues = capability
            .check_node_external_readiness(readiness_request(
                &capability,
                profile.clone(),
                Instant::now() + Duration::from_secs(4),
            ))
            .await;
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].category(), category);
        assert_eq!(
            issues[0].target(),
            &NodeCapabilityReadinessTarget::GenerationProfile {
                parameter_key: NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
                generation_profile_ref: profile.to_node_capability_parameter_value().unwrap(),
            }
        );
        assert_eq!(issues[0].media_kind_mismatch(), None);
    }
}

#[tokio::test]
async fn malformed_normalized_parameters_are_rejected_before_availability_read() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let capability = capability(
        AvailabilityOutcome::State(GenerationProfileAvailabilityState::Available),
        requests.clone(),
    );
    let literal = ProvideLiteralTextCapabilityImpl::try_new().unwrap();
    let literal_parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new("text").unwrap(),
        NodeCapabilityParameterValue::Text("wrong contract".into()),
    )]))
    .unwrap();
    let issues = capability
        .check_node_external_readiness(NodeCapabilityReadinessRequest {
            project_id: project_id(1),
            normalized_parameters: literal.normalize_node_parameters(&literal_parameters).unwrap(),
            deadline: NodeCapabilityReadinessDeadline::at(Instant::now() + Duration::from_secs(4)),
        })
        .await;
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].category(), NodeCapabilityReadinessCategory::InvalidCapabilityInvocation);
    assert_eq!(issues[0].target(), &NodeCapabilityReadinessTarget::Capability);
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn incompatible_profile_is_rejected_before_availability_read_with_the_exact_target() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let capability = capability(
        AvailabilityOutcome::State(GenerationProfileAvailabilityState::Available),
        requests.clone(),
    );
    let profile = profile_for("video.generate_from_image");
    let issues = capability
        .check_node_external_readiness(readiness_request(
            &capability,
            profile.clone(),
            Instant::now() + Duration::from_secs(4),
        ))
        .await;
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].category(),
        NodeCapabilityReadinessCategory::GenerationProfileIncompatible
    );
    assert_eq!(
        issues[0].target(),
        &NodeCapabilityReadinessTarget::GenerationProfile {
            parameter_key: NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
            generation_profile_ref: profile.to_node_capability_parameter_value().unwrap(),
        }
    );
    assert!(requests.lock().unwrap().is_empty());
}

fn capability(
    outcome: AvailabilityOutcome,
    requests: Arc<Mutex<Vec<AvailabilityRequestSnapshot>>>,
) -> TextToImageCapabilityImpl<
    RecordingAvailabilityReaderImpl,
    NodeCapabilityGenerationTaskStarterFakeImpl,
> {
    TextToImageCapabilityImpl::try_new(
        catalog(),
        RecordingAvailabilityReaderImpl { outcome, requests },
        NodeCapabilityGenerationTaskStarterFakeImpl::default(),
    )
    .unwrap()
}

fn readiness_request(
    capability: &impl WorkflowNodeCapabilityInterface,
    profile: GenerationProfileRef,
    deadline: Instant,
) -> NodeCapabilityReadinessRequest {
    let parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
        NodeCapabilityParameterValue::GenerationProfile(
            profile.to_node_capability_parameter_value().unwrap(),
        ),
    )]))
    .unwrap();
    NodeCapabilityReadinessRequest {
        project_id: project_id(1),
        normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
        deadline: NodeCapabilityReadinessDeadline::at(deadline),
    }
}

fn observation(
    profile: GenerationProfileRef,
    state: GenerationProfileAvailabilityState,
) -> GenerationProfileAvailabilityObservation {
    GenerationProfileAvailabilityObservation::try_new(profile, state, 100, 1_000).unwrap()
}
fn catalog() -> Arc<GenerationProfileCatalog> {
    Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap())
}
fn profile_for(id: &str) -> GenerationProfileRef {
    catalog().list_active_generation_profiles_for_capability(&contract_ref(id))[0]
        .profile_ref()
        .clone()
}
fn contract_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
