//! oh-my-dream inference backends.
//!
//! Generation happens through pluggable backends behind a single trait. Each
//! provider translates a neutral request into its own API and normalizes the
//! response back. The first product milestone ships **only a mock backend** —
//! no real cloud vendor, no network, no API keys — so the whole pipeline is
//! locally testable. A `local` on-device backend is intentionally kept as a
//! placeholder for the future.

#![forbid(unsafe_code)]

pub mod error;
pub mod request;
pub mod task;
pub mod traits;

pub mod local;
pub mod mock;

pub use error::{BackendError, Result};
pub use request::{ImageToVideoRequest, TextToImageRequest};
pub use task::{TaskHandle, TaskProgress, TaskStatus};
pub use traits::InferenceBackend;
