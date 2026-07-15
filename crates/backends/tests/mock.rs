use backends::{
    BackendError, ImageToVideoRequest, InferenceBackend, MockBackend,
    ReferenceImageGenerationRequest, ReferenceVideoGenerationRequest, TaskHandle, TaskProgress,
    TaskStatus, TextToAudioRequest, TextToImageRequest,
};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

#[test]
fn submits_and_polls_text_to_image_task_to_success() {
    let backend = MockBackend::new();

    let handle = block_on(backend.text_to_image(text_to_image_request()))
        .expect("text-to-image submission should succeed");

    assert_eq!(handle.backend, "mock");
    assert_eq!(handle.task_id, "task-1");
    assert_eq!(block_on(backend.poll(&handle)).expect("poll should succeed"), TaskStatus::Queued);
    assert_eq!(
        block_on(backend.poll(&handle)).expect("poll should succeed"),
        TaskStatus::Running { progress: TaskProgress(0.25) }
    );
    assert_eq!(
        block_on(backend.poll(&handle)).expect("poll should succeed"),
        TaskStatus::Running { progress: TaskProgress(0.75) }
    );
    assert_eq!(
        block_on(backend.poll(&handle)).expect("poll should succeed"),
        TaskStatus::Succeeded {
            output: "mock://mock/text-to-image/task-1".to_owned(),
            cost: Some(250)
        }
    );
}

#[test]
fn submits_reference_generation_tasks_to_distinct_backend_paths() {
    let backend = MockBackend::new();
    let image_handle = block_on(backend.reference_image_generation(reference_image_request()))
        .expect("reference-image submission should succeed");
    let video_handle = block_on(backend.reference_video_generation(reference_video_request()))
        .expect("reference-video submission should succeed");

    assert_succeeds_with_output(
        &backend,
        &image_handle,
        "mock://mock/reference-image-generation/task-1",
        400,
    );
    assert_succeeds_with_output(
        &backend,
        &video_handle,
        "mock://mock/reference-video-generation/task-2",
        1_200,
    );
}

#[test]
fn submits_and_polls_text_to_audio_task_to_success_with_cost() {
    let backend = MockBackend::new();

    let handle = block_on(backend.text_to_audio(text_to_audio_request()))
        .expect("text-to-audio submission should succeed");
    block_on(backend.poll(&handle)).expect("queued poll should succeed");
    block_on(backend.poll(&handle)).expect("running poll should succeed");
    block_on(backend.poll(&handle)).expect("running poll should succeed");

    assert_eq!(
        block_on(backend.poll(&handle)).expect("poll should succeed"),
        TaskStatus::Succeeded {
            output: "mock://mock/text-to-audio/task-1".to_owned(),
            cost: Some(125)
        }
    );
}

#[test]
fn cancel_marks_task_cancelled_for_later_polls() {
    let backend = MockBackend::new();
    let handle = block_on(backend.image_to_video(image_to_video_request()))
        .expect("image-to-video submission should succeed");

    block_on(backend.cancel(&handle)).expect("cancel should succeed");

    assert_eq!(
        block_on(backend.poll(&handle)).expect("poll should succeed"),
        TaskStatus::Cancelled
    );
}

#[test]
fn unknown_handle_returns_unknown_task() {
    let backend = MockBackend::new();
    let handle = TaskHandle { backend: "mock".to_owned(), task_id: "missing".to_owned() };

    let error = block_on(backend.poll(&handle)).expect_err("unknown task should fail");

    assert!(matches!(
        error,
        BackendError::UnknownTask {
            backend,
            task_id
        } if backend == "mock" && task_id == "missing"
    ));
}

#[test]
fn failing_backend_returns_failed_status() {
    let backend = MockBackend::always_fails("forced failure");
    let handle = block_on(backend.text_to_image(text_to_image_request()))
        .expect("submission should still succeed");

    assert_eq!(
        block_on(backend.poll(&handle)).expect("poll should succeed"),
        TaskStatus::Failed { reason: "forced failure".to_owned() }
    );
}

fn text_to_image_request() -> TextToImageRequest {
    TextToImageRequest {
        model: "mock-model".to_owned(),
        prompt: "a bright sky".to_owned(),
        negative_prompt: None,
        steps: Some(4),
        seed: Some(7),
    }
}

fn image_to_video_request() -> ImageToVideoRequest {
    ImageToVideoRequest {
        model: "mock-video-model".to_owned(),
        image: "asset://image".to_owned(),
        duration_seconds: Some(2.0),
        fps: Some(12),
    }
}

fn reference_image_request() -> ReferenceImageGenerationRequest {
    ReferenceImageGenerationRequest {
        model: "mock-reference-image".to_owned(),
        images: vec!["asset://first".to_owned(), "asset://second".to_owned()],
        prompt: "combine the references".to_owned(),
        negative_prompt: None,
        steps: Some(12),
        seed: Some(7),
    }
}

fn reference_video_request() -> ReferenceVideoGenerationRequest {
    ReferenceVideoGenerationRequest {
        model: "mock-reference-video".to_owned(),
        images: vec!["asset://first".to_owned(), "asset://second".to_owned()],
        prompt: "animate the references".to_owned(),
        duration_seconds: Some(3.0),
        aspect_ratio: Some("16:9".to_owned()),
        resolution: Some("720p".to_owned()),
        fps: Some(24),
    }
}

fn assert_succeeds_with_output(
    backend: &MockBackend,
    handle: &TaskHandle,
    expected_output: &str,
    expected_cost: i64,
) {
    for _ in 0..3 {
        block_on(backend.poll(handle)).expect("pending poll should succeed");
    }
    assert_eq!(
        block_on(backend.poll(handle)).expect("success poll should succeed"),
        TaskStatus::Succeeded { output: expected_output.to_owned(), cost: Some(expected_cost) }
    );
}

fn text_to_audio_request() -> TextToAudioRequest {
    TextToAudioRequest {
        model: "mock-audio-model".to_owned(),
        prompt: "rain on glass".to_owned(),
        seed: Some(7),
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);

    match Future::poll(future.as_mut(), &mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("mock backend futures should complete without a runtime"),
    }
}

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}
