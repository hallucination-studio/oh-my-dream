# Repository Guidelines

## Project Structure & Module Organization

`oh-my-dream` is a Rust workspace for a local desktop AI creation client. Keep Project identity and metadata in `crates/projects`, Workflow logic in `crates/engine`, and Assistant business logic in `crates/assistant`. These crates remain pure and must not depend on UI, network, filesystem, or specific vendors. Workspace modules follow [`docs/DESIGN.md`](docs/DESIGN.md): `crates/projects`, `crates/engine`, `crates/nodes`, `crates/backends`, `crates/assets`, `crates/assistant`, `src-tauri/`, and `ui/`. The root `Cargo.toml` owns workspace metadata and shared dependency versions. Do not commit `target/`, `data/`, local config, or generated runtime artifacts.

The frozen backend architecture starts at [`docs/BACKEND.md`](docs/BACKEND.md). Its document map identifies the single authority for Project, Workflow, Node Capability, Generation Profile/Provider, Asset, Assistant, Desktop, and Storage semantics. All exported backend names follow [`docs/BACKEND_GLOSSARY.md`](docs/BACKEND_GLOSSARY.md). Do not redefine those contracts in README files, implementation notes, DTO docs, or new ADRs.

## Architecture and Dependency Rules

> **Core principle:** Business code depends on stable abstractions, and concrete adapters depend inward on interfaces owned by the business code that consumes them. Each business concept has one authoritative source of semantics; every other form is only a boundary representation.

In this section, an interface means a Rust trait or an equivalent explicit protocol in another repository language.
Every exported substitution interface ends in `Interface`; every concrete type implementing one
ends in `Impl`. Use the more specific endings `CapabilityImpl`, `AdapterImpl`, `RouterImpl`, or
`RouteImpl` when they reveal the implementation's role. Boundary traits live in `interfaces/`.

1. **Organize code by business capability.** Group modules by the business reason they change, not by horizontal technical roles such as `controller`, `service`, or `store`. Technology-specific adapters and entry points belong at system boundaries.

2. **Give each concept one semantic owner.** Status predicates, legal state transitions, invariants, and business validation must be implemented exactly once by the authoritative aggregate, entity, value, capability implementation, or named policy. Persistence rows, API DTOs, and view models may represent the same data but must not reimplement those rules. Boundary-shape validation is allowed, but it must not become a second source of business semantics.

3. **Define a trait for every real substitution boundary.** A trait is required when multiple implementations already exist, multiple implementations are explicitly planned, the dependency crosses an external boundary such as a database, third-party API, filesystem, process, or clock, or tests require a fake or fault-injection implementation. A vague possibility of replacement is not enough.

4. **The consumer owns the trait.** Define the trait in the business or application capability that consumes it. PostgreSQL, JSON, cloud-provider, and other adapters depend inward on that trait and implement it. Never define a provider-shaped contract that forces business code to adapt to a concrete technology.

5. **Domain and application code depend on boundary traits, not concrete adapters.** For every dependency that meets the substitution criteria above, use generic trait bounds or trait objects as appropriate:

   ```rust
   pub struct WorkflowStartRunUseCase<R: WorkflowRunRepositoryInterface> {
       workflow_run_repository: R,
   }
   ```

   Do not name `SqliteWorkflowRunRepositoryAdapterImpl` or any other concrete adapter in a domain or application constructor, field, or function signature.

6. **Select concrete adapters only in the composition root.** Concrete types and their trait implementations live in adapter modules, but their construction, selection, and wiring belong only in an application entry point or a dedicated composition or dependency-injection module. Business code must not downcast concrete types, inspect implementation names, or branch on adapter-specific configuration or capability flags.

7. **Make all implementations behaviorally equivalent.** Matching method signatures is insufficient. Every implementation of a trait must preserve the same return values, error types, idempotency, concurrency semantics, transaction boundaries, ordering, and pagination rules. Run the same parameterized contract test suite against every implementation.

8. **Keep traits small and capability-scoped.** Prefer focused traits such as `WorkflowRunRepositoryInterface`, `AssetManagedContentStoreInterface`, and `TextToImageProviderInterface`. Do not grow a global `Store`, `Database`, or `Repository` trait with unrelated methods.

9. **Do not disguise unsupported behavior as an optional trait method.** An implementation must not satisfy a trait with `todo!()`, `unimplemented!()`, a panic, an `Unsupported` error, or a probe such as `supports_leases`. If some implementations cannot provide an operation, split that capability into a separate, semantically complete trait.

10. **Separate domain, persistence, API, and UI models.** Each representation has one responsibility:

    | Representation | Responsibility |
    | --- | --- |
    | Domain model | Business meaning and invariants |
    | Persistence row | Storage format |
    | API DTO | Boundary protocol |
    | View model | Presentation needs |

    Convert between them at boundaries through explicit, named translators such as `AssetDto::from_domain` or `TryFrom<AssetRow> for Asset`. Never pass or serialize a persistence row directly into an API response or UI model.

11. **Centralize state transitions.** All state changes go through the owning aggregate or its named domain policy. Repositories persist transitions that have already been validated; they must not expose arbitrary status setters to callers.

12. **Use structured types instead of magic strings.** Represent business concepts with enums, structs, newtypes, or tagged unions such as `WorkflowRunState`, `NodeCapabilityReadinessIssue`, and `AssetManagedContentState`. Never infer business state from string prefixes, human-readable error text, or implicit combinations of fields. Strings may carry ordinary domain text, but they must not stand in for structured business concepts.

13. **Persist state inside the transaction and perform side effects after it.** When a use case couples durable state with external effects, atomically persist the aggregate state, revision, domain events, outbox entries, and idempotency record. Execute provider calls, notifications, reports, and other external effects after commit by consuming the outbox.

14. **Do not abstract speculatively.** Pure functions, value objects, and stable internal implementations with no replacement requirement may remain concrete. Introduce a trait only for a real substitution boundary described above, not because replacement might someday be useful.

15. **Classify duplication before removing it.** Similar code may be the authoritative domain definition, a legitimate boundary DTO, a legitimate projection, a separate adapter implementation, or an accidental duplicate or legacy implementation. Only the last category should be merged or deleted solely because it is duplicated.

16. **Keep field semantics local to the authoritative model.** Define the business meaning of a new field exactly once in its owning domain model. Persistence, API, and UI changes should be mechanical translations and must not independently reinterpret that meaning.

17. **Protect dependency direction with automation.** Static architecture or import-boundary tests must reject domain or application code importing a concrete adapter, one capability accessing another capability's private implementation, cyclic module or crate dependencies, and concrete adapter selection outside the composition root.

## Build, Test, and Development Commands

- `cargo check` fast type-checks the workspace.
- `cargo fmt --all` formats all Rust code with rustfmt defaults.
- `cargo clippy --all-targets -- -D warnings` enforces lint cleanliness.
- `cargo test -p <affected-crate>` runs the unit and integration tests for a locally affected Rust crate.
- `npm --prefix ui run typecheck` and `npm --prefix ui run test` validate local frontend changes.
- `./scripts/e2e.sh` runs the complete Rust, Python, and frontend suite. It is a CI command and is not part of the normal local development loop.

Run fmt, clippy, and the tests affected by the change before committing. Do not run the complete E2E suite locally unless the user explicitly requests it or a CI failure must be reproduced. Pull requests run both the complete Rust workspace tests and `./scripts/e2e.sh`; those required CI checks are the final merge gate.

## Testing Guidelines

Put focused unit tests beside the code with `#[cfg(test)]`; use crate-level `tests/` for cross-module behavior. Name tests by behavior, for example `rejects_cyclic_workflow_graph`.

The suite is layered — know which layer a change touches so you update the right one:

- **Rust unit/integration** (`crates/*/tests/`, `src-tauri/tests/`): Workflow execution, deterministic provider routes, Asset adapters, and Node Capability pipelines.
- **Backend E2E** (`src-tauri/tests/e2e.rs`): the whole Workflow Run path — idempotent admission, failure propagation, cancellation, restart interruption, typed input rejection, and Asset read-back.
- **Cross-language contract** (`src-tauri/tests/contract.rs` writes fixtures to `ui/src/__fixtures__/`; `ui/src/api/contract.test.ts` validates them): guards the frontend TS types against the backend DTO shapes so they cannot drift.
- **Frontend** (Vitest + jsdom, `ui/**/*.test.ts(x)`): serialization, wiring validation, mock API, API selection, and the App run flow.

**When you MUST update tests (do this in the same change, never defer):**

- **Change a Tauri command signature or a DTO** (`WorkflowRunDto`, `AssetDto`, or the nested node-output shape) → regenerate the fixtures via `contract.rs` and update `contract.test.ts` and the affected frontend types. A DTO change with stale fixtures is a broken contract.
- **Add or change a Node Capability contract, interface, or the `Workflow` JSON schema** → update the authoritative engine/nodes tests plus frontend serialization and contract-conformance tests. Frontend UX preflight must consume or be mechanically derived from the engine-owned contract; it must not independently reimplement contract compatibility or Workflow business rules.
- **Add a new Tauri command** → add a backend test exercising it and, if the frontend calls it, a `WorkflowApi` test.
- **Change error/cancellation/restart/idempotency behavior** → extend the backend E2E cases that assert those outcomes and post-commit effect recovery.
- **Fix a bug** → add a regression test that fails before the fix and passes after.
- **Add or change a production provider route** → keep deterministic route contract tests green and add vendor-route tests behind their own gate.

Every such change must leave its focused local tests green. The pull request CI must leave both the complete Rust workspace tests and `./scripts/e2e.sh` green before merge.

## Rust Coding Standards

All repository content must be English: code, comments, docs, commit messages, identifiers, logs, and errors. Use Rust 2024. Every crate keeps `#![forbid(unsafe_code)]`. Files should be 400 lines or fewer and functions 60 lines or fewer; split responsibilities instead of adding mechanical line breaks. Use `UpperCamelCase` for types and enums, `snake_case` for functions and variables, and `SCREAMING_SNAKE_CASE` for constants. Public APIs need `///` docs. I/O is async; pure graph logic such as topological sorting and type checking stays synchronous. Add dependencies only through root `[workspace.dependencies]` and justify each one.

## Error Handling & Logging

Library crates define concrete errors with `thiserror`; the application boundary may use `anyhow`. Do not use `unwrap()`, `expect()`, or `panic!()` in library code outside tests. Never ignore a `Result` or swallow errors; preserve operation context when propagating. Use structured `tracing` logs for meaningful lifecycle events, node execution, profile routing, cloud calls, polling, Asset writes, post-commit effects, and failures. Log where an error is handled, not at every propagation layer. Never log secrets.

## Commit & Pull Request Guidelines

History currently uses short subjects such as `init` and `chore: empty commit`. Keep commit subjects concise and imperative; use `feat:`, `fix:`, or `chore:` when useful. Pull requests should describe intent, list verification commands, link issues, and include screenshots only for UI changes.
