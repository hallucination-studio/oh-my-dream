use assets::asset::domain::{AssetContentDigest, AssetId, AssetMediaKind};
use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
};
use nodes::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileAvailabilityRequest,
    GenerationProfileAvailabilityState, GenerationProfileUnavailableReason,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tasks::generation_task::*;

use backends::mock_generation_provider::{
    GenerationProviderRegistryProfileAvailabilityReaderAdapterImpl,
    MockGenerationProviderAdapterImpl, MockGenerationProviderRegistryImpl,
};

#[test]
fn mock_composite_contributes_exact_image_video_and_voice_contracts() {
    let provider = MockGenerationProviderAdapterImpl::try_new().unwrap();
    let contract = GenerationProviderContract::from_provider(&provider);

    assert_eq!(contract.provider_id().as_str(), "mock");
    assert_eq!(contract.display_name().as_str(), "Mock");
    assert!(contract.text().is_none());
    assert_eq!(
        route_ids(contract.image().unwrap().routes()),
        ["mock.image.high-quality-general.v1"]
    );
    assert_eq!(
        route_ids(contract.video().unwrap().routes()),
        ["mock.video.cinematic-image-animation.v1"]
    );
    assert_eq!(
        route_ids(contract.voice().unwrap().routes()),
        ["mock.voice.multilingual-narration.v1"]
    );
}

#[test]
fn mock_registry_resolves_only_the_exact_profile_kind_and_route() {
    let registry = MockGenerationProviderRegistryImpl::try_new().unwrap();
    for (profile_id, kind, route_id) in [
        (
            "image.high_quality_general",
            GenerationTaskRequestKind::Image,
            "mock.image.high-quality-general.v1",
        ),
        (
            "video.cinematic_image_animation",
            GenerationTaskRequestKind::Video,
            "mock.video.cinematic-image-animation.v1",
        ),
        (
            "speech.multilingual_narration",
            GenerationTaskRequestKind::Voice,
            "mock.voice.multilingual-narration.v1",
        ),
    ] {
        let profile = nodes::GenerationProfileRef::new(
            nodes::GenerationProfileId::try_new(profile_id).unwrap(),
            nodes::GenerationProfileVersion::try_new(1).unwrap(),
        );
        let target = registry.target_for_profile(&profile, kind).unwrap();
        assert_eq!(target.provider_id().as_str(), "mock");
        assert_eq!(target.route_id().as_str(), route_id);
        assert_eq!(
            registry.resolve_generation_provider_route(&target, kind).unwrap().request_kind(),
            kind
        );
    }
}

#[tokio::test]
async fn mock_registry_availability_reports_exact_routes_and_structured_absence() {
    let registry = Arc::new(MockGenerationProviderRegistryImpl::try_new().unwrap());
    let reader = GenerationProviderRegistryProfileAvailabilityReaderAdapterImpl::new(registry);
    let observations = reader
        .read_generation_profile_availability(
            GenerationProfileAvailabilityRequest::try_new(
                capability_ref("image.generate_from_text"),
                vec![profile("video.cinematic_image_animation")],
                Instant::now() + Duration::from_secs(1),
            )
            .unwrap(),
        )
        .await
        .unwrap();

    assert!(matches!(
        observations[0].state(),
        GenerationProfileAvailabilityState::Unavailable {
            reason: GenerationProfileUnavailableReason::NoConfiguredRoute,
            retry_after: None,
        }
    ));
}

#[tokio::test]
async fn fresh_mock_composites_reconstruct_the_same_image_handle_and_terminal_result() {
    let first = MockGenerationProviderAdapterImpl::try_new().unwrap();
    let second = MockGenerationProviderAdapterImpl::try_new().unwrap();
    let context = call_context(
        now_milliseconds() - 2_000,
        "image.high_quality_general",
        "mock.image.high-quality-general.v1",
    );
    let spec = ImageGenerationSpec::new(
        GenerationTaskText::try_new("draw a moon").unwrap(),
        ImageAspectRatio::Square,
    );
    let first_execution = first
        .generation_provider_capabilities()
        .image()
        .unwrap()
        .resolve_image_generation_route(&route_id("mock.image.high-quality-general.v1"))
        .unwrap();
    let second_execution = second
        .generation_provider_capabilities()
        .image()
        .unwrap()
        .resolve_image_generation_route(&route_id("mock.image.high-quality-general.v1"))
        .unwrap();

    let first_handle = submit_image(&first_execution, &context, &spec).await;
    let second_handle = submit_image(&second_execution, &context, &spec).await;
    assert_eq!(first_handle, second_handle);

    let first_result = poll_image(&first_execution, &context, &first_handle).await;
    let replay = poll_image(&second_execution, &context, &first_handle).await;
    assert_eq!(first_result, replay);
    assert!(matches!(first_result, ImageGenerationPollOutcome::Completed(_)));
}

#[tokio::test]
async fn mock_image_route_rejects_malformed_handle() {
    let provider = MockGenerationProviderAdapterImpl::try_new().unwrap();
    let execution = provider
        .generation_provider_capabilities()
        .image()
        .unwrap()
        .resolve_image_generation_route(&route_id("mock.image.high-quality-general.v1"))
        .unwrap();
    let ImageGenerationProviderExecution::Remote { poller, .. } = execution else {
        panic!("Mock Image route is not Remote")
    };
    let error = poller
        .poll_image_generation(
            &call_context(
                now_milliseconds() - 2_000,
                "image.high_quality_general",
                "mock.image.high-quality-general.v1",
            ),
            &GenerationProviderTaskHandle::try_new("malformed").unwrap(),
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind(), GenerationProviderCallErrorKind::Permanent);
    assert_eq!(error.code(), "INVALID_MOCK_CALL");
}

#[tokio::test]
async fn mock_video_and_voice_routes_repeat_terminal_results() {
    let provider = MockGenerationProviderAdapterImpl::try_new().unwrap();
    let created_at = now_milliseconds() - 2_000;
    let video_context = call_context(
        created_at,
        "video.cinematic_image_animation",
        "mock.video.cinematic-image-animation.v1",
    );
    let voice_context = call_context(
        created_at,
        "speech.multilingual_narration",
        "mock.voice.multilingual-narration.v1",
    );

    let video = provider
        .generation_provider_capabilities()
        .video()
        .unwrap()
        .resolve_video_generation_route(&route_id("mock.video.cinematic-image-animation.v1"))
        .unwrap();
    let voice = provider
        .generation_provider_capabilities()
        .voice()
        .unwrap()
        .resolve_voice_generation_route(&route_id("mock.voice.multilingual-narration.v1"))
        .unwrap();

    assert_video_repeats(&video, &video_context).await;
    assert_voice_repeats(&voice, &voice_context).await;
}

#[tokio::test]
async fn mock_remote_route_reports_pending_before_completion() {
    let provider = MockGenerationProviderAdapterImpl::try_new().unwrap();
    let context = call_context(
        now_milliseconds() + 10_000,
        "image.high_quality_general",
        "mock.image.high-quality-general.v1",
    );
    let spec = ImageGenerationSpec::new(
        GenerationTaskText::try_new("draw a moon").unwrap(),
        ImageAspectRatio::Square,
    );
    let execution = provider
        .generation_provider_capabilities()
        .image()
        .unwrap()
        .resolve_image_generation_route(&route_id("mock.image.high-quality-general.v1"))
        .unwrap();
    let handle = submit_image(&execution, &context, &spec).await;

    let outcome = poll_image(&execution, &context, &handle).await;

    assert!(
        matches!(outcome, ImageGenerationPollOutcome::Pending(progress) if progress.percent() == Some(50))
    );
}

#[tokio::test]
async fn mock_route_rejects_mismatched_target_and_short_deadline() {
    let provider = MockGenerationProviderAdapterImpl::try_new().unwrap();
    let execution = provider
        .generation_provider_capabilities()
        .image()
        .unwrap()
        .resolve_image_generation_route(&route_id("mock.image.high-quality-general.v1"))
        .unwrap();
    let spec = ImageGenerationSpec::new(
        GenerationTaskText::try_new("draw a moon").unwrap(),
        ImageAspectRatio::Square,
    );
    let created_at = now_milliseconds();
    let mismatched = call_context(created_at, "image.high_quality_general", "mock.image.wrong.v1");
    let ImageGenerationProviderExecution::Remote { submitter, .. } = &execution else {
        panic!("Mock Image route is not Remote")
    };
    let mismatch = submitter.submit_image_generation(&mismatched, &spec).await.unwrap_err();
    assert_eq!(mismatch.kind(), GenerationProviderCallErrorKind::Permanent);

    let short = call_context_with_deadline(
        created_at,
        created_at + 500,
        "image.high_quality_general",
        "mock.image.high-quality-general.v1",
    );
    let outcome = submitter.submit_image_generation(&short, &spec).await.unwrap();
    assert!(
        matches!(outcome, ImageGenerationSubmitOutcome::Rejected(failure) if failure.kind() == GenerationProviderFailureKind::DeadlineExceeded)
    );

    let expired = call_context_with_deadline(
        created_at - 2_000,
        created_at - 1,
        "image.high_quality_general",
        "mock.image.high-quality-general.v1",
    );
    let outcome = submitter.submit_image_generation(&expired, &spec).await.unwrap();
    assert!(
        matches!(outcome, ImageGenerationSubmitOutcome::Rejected(failure) if failure.kind() == GenerationProviderFailureKind::DeadlineExceeded)
    );
}

#[test]
fn only_mock_implements_the_new_provider_composite_in_backends() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut implementations = Vec::new();
    collect_provider_implementations(&source_root, &mut implementations);

    assert_eq!(implementations.len(), 1);
    assert!(implementations[0].contains("MockGenerationProviderAdapterImpl"));
}

async fn submit_image(
    execution: &ImageGenerationProviderExecution,
    context: &GenerationProviderCallContext,
    spec: &ImageGenerationSpec,
) -> GenerationProviderTaskHandle {
    let ImageGenerationProviderExecution::Remote { submitter, .. } = execution else {
        panic!("Mock Image route is not Remote")
    };
    let ImageGenerationSubmitOutcome::Accepted(handle) =
        submitter.submit_image_generation(context, spec).await.unwrap()
    else {
        panic!("Mock Image submission was not accepted")
    };
    handle
}

async fn poll_image(
    execution: &ImageGenerationProviderExecution,
    context: &GenerationProviderCallContext,
    handle: &GenerationProviderTaskHandle,
) -> ImageGenerationPollOutcome {
    let ImageGenerationProviderExecution::Remote { poller, .. } = execution else {
        panic!("Mock Image route is not Remote")
    };
    poller.poll_image_generation(context, handle).await.unwrap()
}

async fn assert_video_repeats(
    execution: &VideoGenerationProviderExecution,
    context: &GenerationProviderCallContext,
) {
    let VideoGenerationProviderExecution::Remote { submitter, poller } = execution else {
        panic!("Mock Video route is not Remote")
    };
    let spec = VideoGenerationSpec::try_new(
        AssetSnapshotRef::new(
            AssetId::from_uuid(uuid::Uuid::from_bytes([
                2, 2, 2, 2, 2, 2, 0x42, 2, 0x82, 2, 2, 2, 2, 2, 2, 2,
            ]))
            .unwrap(),
            AssetMediaKind::Image,
            AssetContentDigest::from_bytes([2; 32]),
        ),
        VideoDurationSeconds::Five,
        None,
    )
    .unwrap();
    let VideoGenerationSubmitOutcome::Accepted(handle) =
        submitter.submit_video_generation(context, &spec).await.unwrap()
    else {
        panic!("Mock Video submission was not accepted")
    };
    let first = poller.poll_video_generation(context, &handle).await.unwrap();
    let replay = poller.poll_video_generation(context, &handle).await.unwrap();
    assert_eq!(first, replay);
    assert!(matches!(first, VideoGenerationPollOutcome::Completed(_)));
}

async fn assert_voice_repeats(
    execution: &VoiceGenerationProviderExecution,
    context: &GenerationProviderCallContext,
) {
    let VoiceGenerationProviderExecution::Remote { submitter, poller } = execution else {
        panic!("Mock Voice route is not Remote")
    };
    let spec = VoiceGenerationSpec::new(GenerationTaskText::try_new("hello").unwrap());
    let VoiceGenerationSubmitOutcome::Accepted(handle) =
        submitter.submit_voice_generation(context, &spec).await.unwrap()
    else {
        panic!("Mock Voice submission was not accepted")
    };
    let first = poller.poll_voice_generation(context, &handle).await.unwrap();
    let replay = poller.poll_voice_generation(context, &handle).await.unwrap();
    assert_eq!(first, replay);
    assert!(matches!(first, VoiceGenerationPollOutcome::Completed(_)));
}

fn call_context(created_at: i64, profile_id: &str, route: &str) -> GenerationProviderCallContext {
    call_context_with_deadline(created_at, created_at + 30_000, profile_id, route)
}

fn call_context_with_deadline(
    created_at: i64,
    deadline_at: i64,
    profile_id: &str,
    route: &str,
) -> GenerationProviderCallContext {
    GenerationProviderCallContext::try_new(
        GenerationTaskId::from_uuid(uuid::Uuid::from_bytes([
            1, 1, 1, 1, 1, 1, 0x41, 1, 0x81, 1, 1, 1, 1, 1, 1, 1,
        ]))
        .unwrap(),
        GenerationTaskTarget::new(
            nodes::GenerationProfileRef::new(
                nodes::GenerationProfileId::try_new(profile_id).unwrap(),
                nodes::GenerationProfileVersion::try_new(1).unwrap(),
            ),
            GenerationProviderId::try_new("mock").unwrap(),
            route_id(route),
        ),
        GenerationTaskTimestamp::from_utc_milliseconds(created_at).unwrap(),
        GenerationTaskTimestamp::from_utc_milliseconds(deadline_at).unwrap(),
    )
    .unwrap()
}

fn now_milliseconds() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

fn route_id(value: &str) -> GenerationProviderRouteId {
    GenerationProviderRouteId::try_new(value).unwrap()
}

fn profile(value: &str) -> nodes::GenerationProfileRef {
    nodes::GenerationProfileRef::new(
        nodes::GenerationProfileId::try_new(value).unwrap(),
        nodes::GenerationProfileVersion::try_new(1).unwrap(),
    )
}

fn capability_ref(value: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(value).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}

fn route_ids(routes: &[GenerationProviderRouteContract]) -> Vec<&str> {
    routes.iter().map(|route| route.route_id().as_str()).collect()
}

fn collect_provider_implementations(path: &std::path::Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_provider_implementations(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let source = std::fs::read_to_string(path).unwrap();
            found.extend(
                source
                    .lines()
                    .filter(|line| line.contains("impl GenerationProviderInterface for"))
                    .map(str::to_owned),
            );
        }
    }
}
