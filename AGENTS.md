# Repository Guidelines

## Project Structure & Module Organization

`oh-my-dream` is a Rust workspace for a local desktop AI creation client. Keep workflow logic in `crates/engine`; it must remain pure logic and must not depend on UI, network, filesystem, or specific vendors. Planned crates follow `docs/DESIGN.md`: `crates/nodes`, `crates/backends`, `crates/assets`, plus future `src-tauri/` and `ui/`. The root `Cargo.toml` owns workspace metadata and shared dependency versions. Do not commit `target/`, `data/`, local config, or generated runtime artifacts.

## Build, Test, and Development Commands

- `cargo check` fast type-checks the workspace.
- `cargo fmt --all` formats all Rust code with rustfmt defaults.
- `cargo clippy --all-targets -- -D warnings` enforces lint cleanliness.
- `cargo test` runs unit and integration tests.
- `./scripts/e2e.sh` runs the full suite end to end: the entire Rust workspace (`cargo test --workspace`), then the frontend typecheck and Vitest suite (`cd ui && npm run typecheck && npm run test`). This is the single gate that must pass before merging any change.

Run fmt, clippy, and tests before committing Rust changes.

## Testing Guidelines

Put focused unit tests beside the code with `#[cfg(test)]`; use crate-level `tests/` for cross-module behavior. Name tests by behavior, for example `rejects_cyclic_workflow_graph`.

The suite is layered — know which layer a change touches so you update the right one:

- **Rust unit/integration** (`crates/*/tests/`, `src-tauri/tests/`): engine executor, mock backend, asset store, node pipeline.
- **Backend E2E** (`src-tauri/tests/e2e.rs`): the whole `run_workflow` path — cache reuse, failure propagation, type-mismatch rejection, asset snapshot read-back.
- **Cross-language contract** (`src-tauri/tests/contract.rs` writes fixtures to `ui/src/__fixtures__/`; `ui/src/api/contract.test.ts` validates them): guards the frontend TS types against the backend DTO shapes so they cannot drift.
- **Frontend** (Vitest + jsdom, `ui/**/*.test.ts(x)`): serialization, wiring validation, mock API, API selection, and the App run flow.

**When you MUST update tests (do this in the same change, never defer):**

- **Change a Tauri command signature or a DTO** (`RunWorkflowResultDto`, `AssetDto`, or the nested run-output shape) → regenerate the fixtures via `contract.rs` and update `contract.test.ts` and the affected frontend types. A DTO change with stale fixtures is a broken contract.
- **Add or change a node type, port, or the `Workflow` JSON schema** → update the engine/nodes tests and the frontend `serialize`/`validate` tests that mirror them.
- **Add a new Tauri command** → add a backend test exercising it and, if the frontend calls it, a `WorkflowApi` test.
- **Change error/cancellation/cache behavior** → extend the backend E2E cases that assert propagation, cancellation, and cache reuse.
- **Fix a bug** → add a regression test that fails before the fix and passes after.
- **Swap the mock backend for a real provider** → keep the mock-backed tests green (they are the deterministic contract) and add provider tests behind their own gate.

Every such change must leave `./scripts/e2e.sh` green.

## Rust Coding Standards

All repository content must be English: code, comments, docs, commit messages, identifiers, logs, and errors. Use Rust 2024. Every crate keeps `#![forbid(unsafe_code)]`. Files should be 400 lines or fewer and functions 60 lines or fewer; split responsibilities instead of adding mechanical line breaks. Use `UpperCamelCase` for types and enums, `snake_case` for functions and variables, and `SCREAMING_SNAKE_CASE` for constants. Public APIs need `///` docs. I/O is async; pure graph logic such as topological sorting and type checking stays synchronous. Add dependencies only through root `[workspace.dependencies]` and justify each one.

## Error Handling & Logging

Library crates define concrete errors with `thiserror`; the application boundary may use `anyhow`. Do not use `unwrap()`, `expect()`, or `panic!()` in library code outside tests. Never ignore a `Result` or swallow errors; preserve operation context when propagating. Use structured `tracing` logs for meaningful lifecycle events, node execution, cloud calls, polling, cache hits, asset writes, and failures. Log where an error is handled, not at every propagation layer. Never log secrets.

## Commit & Pull Request Guidelines

History currently uses short subjects such as `init` and `chore: empty commit`. Keep commit subjects concise and imperative; use `feat:`, `fix:`, or `chore:` when useful. Pull requests should describe intent, list verification commands, link issues, and include screenshots only for UI changes.
