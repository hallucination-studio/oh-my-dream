# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Language policy

**English is the first language for everything in this repository** — code, comments, doc comments, documentation, commit messages, identifiers, log messages, and error text. Do not introduce non-English content into the repo.

## Project positioning

**oh-my-dream** is a local AI creation client that chains generative capabilities through a visual node workflow. The core pipeline is **text-to-image → image-to-video**.

The design takes inspiration from [ComfyUI](https://github.com/comfyanonymous/ComfyUI)'s node-workflow ideas, but **borrows selectively — it does not copy** ComfyUI's data structures or implementation.

Positioning:

- **Local client**: a desktop app for individual creators (Tauri is the plan).
- **No login**: no account system; usable out of the box.
- **No GPU on our side**: the machine runs no model inference. MVP generation happens through exact capability-owned provider interfaces and cloud-vendor routes. Do not introduce a broad inference interface or a speculative local-inference boundary.
- **Minimal by design**: build only what the core pipeline needs; do not reproduce all of ComfyUI's nodes and features.

First capabilities to build: text-to-image, image-to-video, a visual node workbench, and an asset library.

Repository map: [docs/DESIGN.md](docs/DESIGN.md). Frozen backend architecture and MVP scope:
[docs/BACKEND.md](docs/BACKEND.md). Backend names: [docs/BACKEND_GLOSSARY.md](docs/BACKEND_GLOSSARY.md).

## Tech stack

- Core and engine: **Rust** (cargo workspace, multiple crates)
- Desktop shell and frontend/backend bridge: Tauri (planned)
- Frontend canvas: web frontend + a node-canvas library (planned)
- Asset library: SQLite + local files

## Code standards

### Size limits

- **File ≤ 400 lines.** Exceeding it means the file does too much — split it.
- **Function ≤ 60 lines.** Extract helpers before crossing this.
- Do not add mechanical line breaks to fit the limit; rethink the responsibility split instead.

### Style

- **Formatting**: run `cargo fmt` before committing. Follow rustfmt defaults; no custom config.
- **Lints**: `cargo clippy --all-targets -- -D warnings` must be clean.
- **No unsafe**: keep `#![forbid(unsafe_code)]` at the top of every crate. Pure cloud-API orchestration needs no unsafe.
- **Naming**: types/enums `UpperCamelCase`, functions/variables `snake_case`, constants `SCREAMING_SNAKE_CASE`. Use whole English words; avoid abbreviations.
- **Comments**: comments explain *why*, not *what*. Public APIs get `///` doc comments. All comments in English.
- **Async**: I/O (cloud-API calls, files, DB) is async; pure computation (topological sort, type checking) stays synchronous — do not color it needlessly.
- **Dependencies**: manage versions centrally in the root `Cargo.toml` `[workspace.dependencies]`. Justify each new dependency.

### Error handling — surface, never swallow

- **Propagate errors to the top.** An error must bubble up to the highest layer that can meaningfully act on it (log it, show it to the user, decide a fallback). The engine and library layers propagate; only the application boundary decides final handling.
- **Never swallow errors.** No empty `catch`/`match` arms that drop an error, no `let _ = fallible()`, no ignoring a `Result`. If an error is truly benign, write a comment stating *why* it is safe to ignore.
- **Preserve context.** When propagating, add context (what operation, which inputs) rather than replacing the original error. Do not collapse a specific error into a generic string.
- **Library crates** (engine/nodes/backends/assets) define concrete error types with `thiserror`. The **application layer** (src-tauri) may use `anyhow`.
- **No `unwrap()` / `expect()` / `panic!()` in library code** (tests excepted). A `panic!` is not error handling.

### Logging — clear and mandatory

- **Always log.** Every meaningful step logs: cloud-API calls, profile routing, polling, node execution, Asset writes, post-commit effects, and failures. Do not leave code paths silent.
- **Be explicit.** A log line must be understandable on its own: state what happened, to which entity (Workflow Run ID, node execution ID, Asset ID), and the relevant values. No bare "error" or "done".
- **Use levels correctly**: `error` for failures needing attention, `warn` for recoverable anomalies, `info` for lifecycle/business milestones, `debug`/`trace` for diagnostics. Use structured logging (the `tracing` crate).
- **Log where you handle, propagate where you don't.** Log an error at the layer that finally handles it — do not log-and-rethrow at every level (double logging). But never let an error pass through completely unlogged.
- **Never log secrets** (API keys, tokens).

### Layering discipline

- `engine` is pure logic: it must **not** depend on UI, network, filesystem, or any specific cloud vendor.
- Each exact model operation uses its own consumer-owned provider interface; there is no broad inference-backend interface.
- Exported substitution traits end in `Interface`; every concrete implementation ends in `Impl`,
  using `CapabilityImpl`, `AdapterImpl`, `RouterImpl`, or `RouteImpl` where applicable.
- Cloud vendor API keys and other secrets **never enter the repo or plaintext configuration**. Production uses the operating-system credential facility; environment variables are development-only and ephemeral.

## Verification

After changing Rust code, run at least:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Before merging **any** change (Rust or frontend), the full suite must pass:

```bash
./scripts/e2e.sh
```

It runs the whole Rust workspace (`cargo test --workspace`), then the frontend typecheck and Vitest suite (`cd ui && npm run typecheck && npm run test`).

### Test layers

- **Rust unit/integration** (`crates/*/tests/`, `src-tauri/tests/`) — Workflow execution, deterministic provider routes, Asset adapters, and Node Capability pipelines.
- **Backend E2E** (`src-tauri/tests/e2e.rs`) — the whole Workflow Run path: idempotent admission, failure propagation, cancellation, restart interruption, typed-input rejection, and Asset read-back.
- **Cross-language contract** — `src-tauri/tests/contract.rs` writes fixtures to `ui/src/__fixtures__/`; `ui/src/api/contract.test.ts` validates them. This guards the frontend TS types against the backend DTO shapes so they cannot drift.
- **Frontend** (Vitest + jsdom, `ui/**/*.test.ts(x)`) — serialization, wiring validation, mock API, API selection, App run flow.

### When to update tests

Update tests in the **same change** that causes the need — never defer:

- **Changing a Tauri command signature or a DTO** (`WorkflowRunDto`, `AssetDto`, the nested node-output shape) → regenerate fixtures via `contract.rs` and update `contract.test.ts` and the affected frontend types. A DTO change with stale fixtures is a broken contract.
- **Adding/changing a Node Capability contract, interface, or the `Workflow` JSON schema** → update the engine/nodes tests and the frontend `serialize`/`validate` tests that mirror them.
- **Adding a Tauri command** → add a backend test for it, plus a `WorkflowApi` test if the frontend calls it.
- **Changing error / cancellation / restart / idempotency behavior** → extend the backend E2E cases asserting those outcomes and post-commit effect recovery.
- **Fixing a bug** → add a regression test that fails before the fix and passes after.
- **Adding/changing a production provider route** → keep deterministic route contract tests green and add vendor-route tests behind their own gate.

Every such change must leave `./scripts/e2e.sh` green.
