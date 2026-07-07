//! Neutral request models shared by every backend.
//!
//! Providers translate these into their own API shapes; the engine and nodes
//! only ever speak this vendor-neutral vocabulary.

use serde::{Deserialize, Serialize};

/// A text-to-image generation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextToImageRequest {
    /// The (opaque) model identifier to run.
    pub model: String,
    /// Positive prompt.
    pub prompt: String,
    /// Optional negative prompt.
    #[serde(default)]
    pub negative_prompt: Option<String>,
    /// Sampling steps, when the provider exposes it.
    #[serde(default)]
    pub steps: Option<u32>,
    /// Seed for reproducibility, when supported.
    #[serde(default)]
    pub seed: Option<u64>,
}

/// An image-to-video generation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageToVideoRequest {
    /// The (opaque) model identifier to run.
    pub model: String,
    /// Reference to the source image (asset id / URL).
    pub image: String,
    /// Desired clip duration in seconds.
    #[serde(default)]
    pub duration_seconds: Option<f32>,
    /// Desired frames per second.
    #[serde(default)]
    pub fps: Option<u32>,
}
