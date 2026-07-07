//! Asset store contract: SQLite metadata plus on-disk files.
//!
//! Wave 1 (Track C) implements the SQLite schema, inserts, queries (list, filter
//! by kind), and workflow-snapshot lookup. The trait is defined here so the
//! `nodes` and `src-tauri` crates can build against a stable surface.

// Wave 1 fills in the concrete `AssetStore` and its query methods.
