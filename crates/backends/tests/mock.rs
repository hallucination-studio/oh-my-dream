use backends::{
    BackendError, ImageToVideoRequest, InferenceBackend, MockBackend, TaskHandle, TaskProgress,
    TaskStatus, TextToImageRequest,
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
        TaskStatus::Running { progress: TaskProgress(0.5) }
    );
    assert_eq!(
        block_on(backend.poll(&handle)).expect("poll should succeed"),
        TaskStatus::Succeeded { output: "mock://mock/text-to-image/task-1".to_owned() }
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
