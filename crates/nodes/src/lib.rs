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

// Wave 2 fills in: text_prompt, text_to_image, image_to_video, save_asset,
// and `register_all(&mut NodeRegistry)`.
