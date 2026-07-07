//! oh-my-dream concrete nodes.
//!
//! These are the real workflow nodes for the first milestone. They implement
//! the `engine::Node` trait and, where they generate media, call into a
//! `backends::InferenceBackend` (a mock in the first milestone).
//!
//! Wave 2 (Track E) implements the node bodies and a `register_all` that
//! populates an `engine::NodeRegistry`. This crate depends on `engine`,
//! `backends`, and `assets`, which are built first.

#![forbid(unsafe_code)]

mod error;
mod image_to_video;
mod params;
mod polling;
mod ports;
mod save_asset;
mod text_prompt;
mod text_to_image;

use assets::AssetStore;
use backends::InferenceBackend;
use engine::NodeRegistry;
use std::sync::{Arc, Mutex};

/// Shared asset store used by node instances.
///
/// `AssetStore` owns a SQLite connection, which must not be shared directly
/// across concurrent node instances. The mutex serializes store access while
/// still allowing factories and nodes to hold cheap `Arc` clones.
pub type SharedAssetStore = Arc<Mutex<AssetStore>>;

/// Registers all first-milestone node factories into `registry`.
pub fn register_all(
    registry: &mut NodeRegistry,
    backend: Arc<dyn InferenceBackend>,
    store: SharedAssetStore,
) {
    text_prompt::register(registry);
    text_to_image::register(registry, Arc::clone(&backend));
    image_to_video::register(registry, backend);
    save_asset::register(registry, store);
}
