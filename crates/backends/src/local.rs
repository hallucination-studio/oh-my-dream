//! On-device backend placeholder.
//!
//! Intentionally unimplemented. The product runs no GPU today; this module
//! reserves the seam so a future local inference path can slot in behind the
//! same [`crate::traits::InferenceBackendInterface`] trait without touching callers.
