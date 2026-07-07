//! oh-my-dream asset library.
//!
//! Generated media (images, videos) live as files on disk; their metadata lives
//! in SQLite. Every asset stores a snapshot of the workflow parameters that
//! produced it, so the UI can trace an asset back to a reproducible recipe.
//!
//! Wave 1 (Track C) implements storage, thumbnails, and queries. This module
//! defines the data model those pieces share.

#![forbid(unsafe_code)]

pub mod error;
pub mod model;
pub mod store;
pub mod thumbnail;

pub use error::{AssetError, Result};
pub use model::{Asset, AssetKind, NewAsset};
