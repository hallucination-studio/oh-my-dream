use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use engine::node_capability::*;
use nodes::{
    GenerationProfileAvailabilityObservation, GenerationProfileAvailabilityReaderInterface,
    GenerationProfileAvailabilityRequest, GenerationProfileAvailabilityState,
    GenerationProfileCatalog, GenerationProfileError, GenerationProfileListForCapabilityQuery,
    GenerationProfileListForCapabilityUseCase, NodeCapabilityListUseCase,
};

struct AvailabilityReaderFakeImpl;

struct UnexpectedAvailabilityReadFakeImpl;

struct MismatchedAvailabilityReaderFakeImpl;

#[async_trait]
impl GenerationProfileAvailabilityReaderInterface for UnexpectedAvailabilityReadFakeImpl {
    async fn read_generation_profile_availability(
        &self,
        _request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError> {
        panic!("availability reader must not be called for an empty compatible profile set")
    }
}

#[async_trait]
impl GenerationProfileAvailabilityReaderInterface for MismatchedAvailabilityReaderFakeImpl {
    async fn read_generation_profile_availability(
        &self,
        _request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError> {
        let speech_definition = GenerationProfileCatalog::frozen_mvp()?
            .list_active_generation_profiles_for_capability(&capability_ref(
                "audio.synthesize_speech_from_text",
            ))
            .into_iter()
            .next()
            .ok_or(GenerationProfileError::ProfileNotFound)?
            .clone();
        Ok(vec![GenerationProfileAvailabilityObservation::try_new(
            speech_definition.profile_ref().clone(),
            GenerationProfileAvailabilityState::Available,
            100,
            1_000,
        )?])
    }
}

#[async_trait]
impl GenerationProfileAvailabilityReaderInterface for AvailabilityReaderFakeImpl {
    async fn read_generation_profile_availability(
        &self,
        request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError> {
        request
            .profile_refs()
            .iter()
            .cloned()
            .map(|profile_ref| {
                GenerationProfileAvailabilityObservation::try_new(
                    profile_ref,
                    GenerationProfileAvailabilityState::Available,
                    100,
                    1_000,
                )
            })
            .collect()
    }
}

#[tokio::test]
async fn profile_list_joins_the_registered_capability_with_one_bulk_observation() {
    let implementation: Arc<dyn WorkflowNodeCapabilityInterface> =
        Arc::new(CapabilityFakeImpl::new("image.generate_from_text"));
    let registry = Arc::new(WorkflowNodeCapabilityRegistry::try_new([implementation]).unwrap());
    let use_case = GenerationProfileListForCapabilityUseCase::new(
        registry.clone(),
        Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap()),
        Arc::new(AvailabilityReaderFakeImpl),
    );
    let capability_ref = capability_ref("image.generate_from_text");

    let items = use_case
        .list_generation_profiles_for_capability(GenerationProfileListForCapabilityQuery::new(
            capability_ref,
            Instant::now() + Duration::from_secs(4),
        ))
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].definition().profile_ref().to_string(), "image.high_quality_general@1");
    assert!(matches!(
        items[0].availability().state(),
        GenerationProfileAvailabilityState::Available
    ));
    assert_eq!(NodeCapabilityListUseCase::new(registry).list_node_capabilities().len(), 1);
}

#[tokio::test]
async fn capability_without_profiles_returns_empty_without_reading_availability() {
    let implementation: Arc<dyn WorkflowNodeCapabilityInterface> =
        Arc::new(CapabilityFakeImpl::new("text.provide_literal"));
    let registry = Arc::new(WorkflowNodeCapabilityRegistry::try_new([implementation]).unwrap());
    let use_case = GenerationProfileListForCapabilityUseCase::new(
        registry,
        Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap()),
        Arc::new(UnexpectedAvailabilityReadFakeImpl),
    );

    let items = use_case
        .list_generation_profiles_for_capability(GenerationProfileListForCapabilityQuery::new(
            capability_ref("text.provide_literal"),
            Instant::now() + Duration::from_secs(4),
        ))
        .await
        .unwrap();

    assert!(items.is_empty());
}

#[tokio::test]
async fn profile_list_rejects_an_observation_for_a_different_profile_ref() {
    let implementation: Arc<dyn WorkflowNodeCapabilityInterface> =
        Arc::new(CapabilityFakeImpl::new("image.generate_from_text"));
    let registry = Arc::new(WorkflowNodeCapabilityRegistry::try_new([implementation]).unwrap());
    let use_case = GenerationProfileListForCapabilityUseCase::new(
        registry,
        Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap()),
        Arc::new(MismatchedAvailabilityReaderFakeImpl),
    );

    let result = use_case
        .list_generation_profiles_for_capability(GenerationProfileListForCapabilityQuery::new(
            capability_ref("image.generate_from_text"),
            Instant::now() + Duration::from_secs(4),
        ))
        .await;
    let error = match result {
        Ok(_) => panic!("mismatched profile observation was accepted"),
        Err(error) => error,
    };

    assert_eq!(error, GenerationProfileError::InvalidAvailabilityObservation);
}

#[tokio::test]
async fn profile_list_rejects_an_unregistered_capability_before_availability_read() {
    let registry = Arc::new(
        WorkflowNodeCapabilityRegistry::try_new(
            Vec::<Arc<dyn WorkflowNodeCapabilityInterface>>::new(),
        )
        .unwrap(),
    );
    let use_case = GenerationProfileListForCapabilityUseCase::new(
        registry,
        Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap()),
        Arc::new(UnexpectedAvailabilityReadFakeImpl),
    );

    let result = use_case
        .list_generation_profiles_for_capability(GenerationProfileListForCapabilityQuery::new(
            capability_ref("image.generate_from_text"),
            Instant::now() + Duration::from_secs(4),
        ))
        .await;
    let error = match result {
        Ok(_) => panic!("unregistered capability was accepted"),
        Err(error) => error,
    };

    assert_eq!(error, GenerationProfileError::CapabilityNotFound);
}

#[test]
fn availability_values_reject_expired_observations_and_overlong_requests() {
    let profile_ref = GenerationProfileCatalog::frozen_mvp()
        .unwrap()
        .list_active_generation_profiles_for_capability(&capability_ref(
            "image.generate_from_text",
        ))[0]
        .profile_ref()
        .clone();
    assert_eq!(
        GenerationProfileAvailabilityObservation::try_new(
            profile_ref.clone(),
            GenerationProfileAvailabilityState::Available,
            100,
            30_101,
        )
        .unwrap_err(),
        GenerationProfileError::InvalidAvailabilityObservation
    );
    assert!(
        GenerationProfileAvailabilityRequest::try_new(
            capability_ref("image.generate_from_text"),
            vec![profile_ref],
            Instant::now() + Duration::from_secs(6),
        )
        .is_err()
    );
}

struct CapabilityFakeImpl {
    contract: NodeCapabilityContract,
}

impl CapabilityFakeImpl {
    fn new(id: &str) -> Self {
        let output = NodeCapabilityOutputContract::new(
            NodeCapabilityOutputKey::new("output").unwrap(),
            WorkflowDataType::Image,
            true,
        );
        Self {
            contract: NodeCapabilityContract::try_new(
                capability_ref(id),
                vec![],
                vec![],
                vec![output],
                NodeCapabilityExecutionKind::ContentGeneration,
            )
            .unwrap(),
        }
    }
}

#[async_trait]
impl WorkflowNodeCapabilityInterface for CapabilityFakeImpl {
    fn node_capability_contract(&self) -> &NodeCapabilityContract {
        &self.contract
    }
    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError> {
        self.contract.normalize_node_parameters(parameters)
    }
    async fn check_node_external_readiness(
        &self,
        _request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue> {
        Vec::new()
    }
    async fn execute_node_capability(
        &self,
        _request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError> {
        unreachable!()
    }
}

fn capability_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
