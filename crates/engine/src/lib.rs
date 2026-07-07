//! oh-my-dream workflow engine.
//!
//! This layer only concerns "how the node graph is organized and executed".
//! It knows nothing about what individual nodes do, nor about UI and network.
//! Inference backends, the asset library, and the interface all live outside
//! the engine.

#![forbid(unsafe_code)]

// Submodules are filled in as implementation proceeds; this placeholder pins
// the crate structure for now.
