use assets::asset::domain::{AssetContentDigest, AssetId, AssetMediaKind};
use tasks::generation_task::domain::*;

use crate::support::{
    asset_result, image_request, origin, result_for, target, task_with_request, text_request, time,
    uuid, video_request, voice_request,
};

#[test]
fn all_four_request_variants_accept_only_their_closed_result_kind() {
    for request in [text_request(), image_request("image"), voice_request(), video_request()] {
        let mut task = task_with_request(request.clone());
        task.begin_submission(time(101)).unwrap();
        task.complete(result_for(&request), time(102)).unwrap();
        assert!(matches!(task.state(), GenerationTaskState::Succeeded { .. }));
    }

    assert_result_rejected(text_request(), asset_result(AssetMediaKind::Image));
    assert_result_rejected(
        image_request("image"),
        GenerationTaskResult::Text { content: GenerationTaskText::try_new("text").unwrap() },
    );
    assert_result_rejected(image_request("image"), asset_result(AssetMediaKind::Audio));
    assert_result_rejected(voice_request(), asset_result(AssetMediaKind::Video));
    assert_result_rejected(video_request(), asset_result(AssetMediaKind::Image));
}

#[test]
fn video_request_requires_an_exact_image_snapshot() {
    let audio = AssetSnapshotRef::new(
        AssetId::from_uuid(uuid(90)).unwrap(),
        AssetMediaKind::Audio,
        AssetContentDigest::from_bytes([9; 32]),
    );
    assert_eq!(
        VideoGenerationSpec::try_new(audio, VideoDurationSeconds::Ten, None),
        Err(GenerationTaskDomainError::InvalidRequest)
    );
}

#[test]
fn canonical_hash_excludes_task_identity_time_and_idempotency_key() {
    let first = task_with_request(image_request("same"));
    let second = GenerationTaskAggregate::create(
        GenerationTaskId::from_uuid(uuid(91)).unwrap(),
        origin(1),
        GenerationTaskIdempotencyKey::try_new("another-key").unwrap(),
        target("mock.image.high-quality-general.v1"),
        image_request("same"),
        time(900),
        time(30_900),
    )
    .unwrap();
    assert_eq!(first.request_hash(), second.request_hash());
}

#[test]
fn canonical_hash_changes_with_each_immutable_admission_fact() {
    let baseline = task_with_request(image_request("same"));
    let changed_origin = GenerationTaskAggregate::create(
        GenerationTaskId::from_uuid(uuid(92)).unwrap(),
        origin(20),
        GenerationTaskIdempotencyKey::try_new("node-execution-1").unwrap(),
        target("mock.image.high-quality-general.v1"),
        image_request("same"),
        time(100),
        time(30_100),
    )
    .unwrap();
    let changed_request = task_with_request(image_request("different"));
    let changed_target = GenerationTaskAggregate::create(
        GenerationTaskId::from_uuid(uuid(93)).unwrap(),
        origin(1),
        GenerationTaskIdempotencyKey::try_new("node-execution-1").unwrap(),
        target("mock.image.alternate.v1"),
        image_request("same"),
        time(100),
        time(30_100),
    )
    .unwrap();

    assert_ne!(baseline.request_hash(), changed_origin.request_hash());
    assert_ne!(baseline.request_hash(), changed_request.request_hash());
    assert_ne!(baseline.request_hash(), changed_target.request_hash());
}

#[test]
fn canonical_hash_matches_the_schema_one_golden_vector() {
    let task = task_with_request(image_request("same"));
    assert_eq!(
        task.request_hash().as_bytes(),
        [
            207, 254, 48, 127, 18, 75, 154, 109, 5, 13, 175, 55, 6, 221, 30, 253, 83, 68, 223, 91,
            43, 159, 228, 137, 235, 52, 33, 95, 16, 220, 91, 127,
        ]
    );
}

#[test]
fn domain_values_reject_invalid_boundaries() {
    assert_eq!(GenerationTaskText::try_new(""), Err(GenerationTaskDomainError::InvalidText));
    assert_eq!(
        GenerationTaskIdempotencyKey::try_new("bad\nkey"),
        Err(GenerationTaskDomainError::InvalidIdempotencyKey)
    );
    assert_eq!(
        GenerationProviderId::try_new("Mock"),
        Err(GenerationTaskDomainError::InvalidProviderIdentity)
    );
    assert_eq!(
        GenerationProviderTaskHandle::try_new(""),
        Err(GenerationTaskDomainError::InvalidProviderTaskHandle)
    );
    assert_eq!(
        GenerationTaskTimestamp::from_utc_milliseconds(-1),
        Err(GenerationTaskDomainError::InvalidTimestamp)
    );
    assert_eq!(GenerationTaskRevision::try_new(0), Err(GenerationTaskDomainError::InvalidRevision));
}

fn assert_result_rejected(request: GenerationTaskRequest, result: GenerationTaskResult) {
    let mut task = task_with_request(request);
    task.begin_submission(time(101)).unwrap();
    assert_eq!(
        task.complete(result, time(102)),
        Err(GenerationTaskDomainError::ResultKindMismatch)
    );
}
