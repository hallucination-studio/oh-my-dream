# Repository Guidelines

## Project Structure & Module Organization

`oh-my-dream` is a Rust workspace for a local desktop AI creation client. Keep workflow logic in `crates/engine`; it must remain pure logic and must not depend on UI, network, filesystem, or specific vendors. Planned crates follow `docs/DESIGN.md`: `crates/nodes`, `crates/backends`, `crates/assets`, plus future `src-tauri/` and `ui/`. The root `Cargo.toml` owns workspace metadata and shared dependency versions. Do not commit `target/`, `data/`, local config, or generated runtime artifacts.

## Build, Test, and Development Commands

- `cargo check` fast type-checks the workspace.
- `cargo fmt --all` formats all Rust code with rustfmt defaults.
- `cargo clippy --all-targets -- -D warnings` enforces lint cleanliness.
- `cargo test` runs unit and integration tests.

Run fmt, clippy, and tests before committing Rust changes.

## Rust Coding Standards

All repository content must be English: code, comments, docs, commit messages, identifiers, logs, and errors. Use Rust 2024. Every crate keeps `#![forbid(unsafe_code)]`. Files should be 400 lines or fewer and functions 60 lines or fewer; split responsibilities instead of adding mechanical line breaks. Use `UpperCamelCase` for types and enums, `snake_case` for functions and variables, and `SCREAMING_SNAKE_CASE` for constants. Public APIs need `///` docs. I/O is async; pure graph logic such as topological sorting and type checking stays synchronous. Add dependencies only through root `[workspace.dependencies]` and justify each one.

## Error Handling & Logging

Library crates define concrete errors with `thiserror`; the application boundary may use `anyhow`. Do not use `unwrap()`, `expect()`, or `panic!()` in library code outside tests. Never ignore a `Result` or swallow errors; preserve operation context when propagating. Use structured `tracing` logs for meaningful lifecycle events, node execution, cloud calls, polling, cache hits, asset writes, and failures. Log where an error is handled, not at every propagation layer. Never log secrets.

## Testing Guidelines

Put focused unit tests beside the code with `#[cfg(test)]`; use crate-level `tests/` for cross-module behavior. Name tests by behavior, for example `rejects_cyclic_workflow_graph`.

## Commit & Pull Request Guidelines

History currently uses short subjects such as `init` and `chore: empty commit`. Keep commit subjects concise and imperative; use `feat:`, `fix:`, or `chore:` when useful. Pull requests should describe intent, list verification commands, link issues, and include screenshots only for UI changes.
