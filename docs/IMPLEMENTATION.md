# oh-my-dream — Redesign & Feature Implementation Plan

Status: planning · Last updated 2026-07-08

This plan covers the professional-UI redesign (mockups in `docs/ui-pro.html`,
`docs/ui-assets.html`) and the backend capabilities it depends on.

Division of labor:

- **Part A — contracts + backend: dispatched by the user** (Codex or otherwise).
- **Part B — frontend styling/UX: implemented by Claude.**

Confirmed product decisions:

- **Full persistence**: projects and workflows are saved to disk and survive
  restart.
- **Progress events**: the engine reports per-node execution state/progress; the
  Tauri layer forwards them to the UI.
- **Cost is estimated** while the mock backend is active; real billing lands with
  a real provider.
- **SaveAsset node is removed**: any node that produces image/audio/video saves
  to the library automatically.

---

## Part A — contracts + backend (user-dispatched)

### A0. Contract layer (do FIRST — unlocks everything else)

This is the foundation; changing it after the fact breaks both sides. It touches
Wave 0 contracts, so it is intentionally a single coordinated change.

- **AssetDto / asset model** gains source + cost metadata:
  `prompt`, `project_id`, `project_name`, `source_node_id`, `source_node_type`,
  `model`, `seed`, `cost`, plus existing `kind/file_path/thumbnail_path/created_at`.
- **AssetKind** gains `Audio`.
- **Node progress event**: `NodeProgressEvent { node_id, state, progress?, cost? }`
  where `state ∈ idle | running | done | cached | error`.
- **Remove `SaveAsset`** from the node contract; replace with the "producer nodes
  auto-save" convention.
- **Project model**: `Project { id, name, created_at }`; a workflow belongs to a
  project.
- **Frontend mirror + fixtures**: update `ui/src/api/types.ts`,
  `ui/src/workflow/types.ts`, regenerate `src-tauri/tests/contract.rs` fixtures,
  update `ui/src/api/contract.test.ts`.
- Acceptance: `cargo build` + `cargo test` green; fixtures regenerated.

### A1. engine — execution events + cost

- Add an execution **observer callback** to `Executor::execute` that reports each
  node's start / cached / done / error and optional progress + cost. Keep the
  engine synchronous and pure — the observer is a trait, not async.
- Node execution results carry an optional `cost`.
- Acceptance: engine tests assert the observer fires per node in order, and that
  a cached node reports `cached` without re-running.

### A2. nodes — auto-save (remove SaveAsset)

- Delete the `SaveAsset` node.
- Producer nodes (TextToImage, ImageToVideo, and a future audio node) write their
  output into the `AssetStore` inside `run`, tagging it with prompt / model /
  seed / source node / project.
- Add an audio-producing node path (`AssetKind::Audio`).
- Acceptance: running a graph persists one asset per produced medium, each with
  full source metadata; no SaveAsset node exists.

### A3. assets — storage + professional query

- Extend the SQLite schema with: `prompt`, `project_id`, `project_name`,
  `source_node_type`, `model`, `seed`, `cost`.
- `list` supports filtering by kind / project / model, text search over `prompt`,
  and sort order (newest, cost, …).
- Full persistence: projects table + workflow persistence (save/load workflow
  JSON keyed by project).
- Acceptance: insert + query round-trips all new fields; text search and filters
  work; a saved project/workflow reloads after reopening the store.

### A4. Tauri — commands + progress forwarding

- `run_workflow` wires the engine observer to Tauri `emit` events so the UI gets
  live per-node state/progress/cost.
- Project commands: `list_projects`, `create_project`, `open_project`,
  `save_workflow`, `load_workflow`.
- `list_assets` command signature gains search/filter/sort params.
- Provider config commands: `get_providers`, `set_active_provider`,
  `set_provider_key` — stored locally, **keys never enter the repo**.
- Acceptance: backend tests cover the new commands via `AppState` without a live
  Tauri runtime, mirroring existing `run_workflow_with_state` tests.

### A5. mock backend — progress + cost simulation

- MockBackend `poll` returns increasing `progress` across calls and a `cost`
  estimate on completion, so the UI can demo the progress bar and cost badge.
- Acceptance: mock tests assert progress increases and a terminal cost is returned.

Dependency order: **A0 → A1 → A2 → A3 → A4 → A5** (A1–A3 can overlap once A0 is
merged; A4 depends on A1–A3; A5 supports A4).

Every step must leave `./scripts/e2e.sh` green.

---

## Part B — frontend styling / UX (Claude)

Design source of truth: `docs/ui-pro.html` (workbench) and `docs/ui-assets.html`
(library). React + React Flow kept; existing API seam and logic tests reused.

### B1. Design system + app shell

- Glass tokens: light glass surfaces, ambient gradient ground, data-type channel
  colors, Inter + JetBrains Mono.
- Shell: top bar (brand + **project switcher**, no tabs + run state + gear + Run),
  left icon rail, main region, right inspector.

### B2. Node library as a category tree

- Add a `category` field to the node catalog (Input / Image / Video / Audio /
  Utility).
- Collapsible category tree with search + Filter/Sort, draggable leaves.

### B3. Node states on canvas

- Node card shows: status pill (idle/running/done/cached/error), progress bar,
  **result preview** (image thumb / video poster), and a footer with cost / time /
  run count.
- Subscribe to Tauri progress events (from A4) to drive state; falls back to the
  mock’s simulated progress in-browser.

### B4. Asset library view

- Rail-switched full manager: large search, kind/project filters, big grid, right
  detail panel.
- **Drag an asset onto the canvas** → creates a "load this asset" node.
- **Jump**: asset → its source project / node.

### B5. Settings dialog

- Providers group (Mock active; fal / Replicate + API key inputs), Canvas,
  Storage, About. Keys stored locally only.

### B6. Tests

- Update serialize / validate / mockApi / contract tests; add node-state, asset
  search, and project-switch tests. Keep `./scripts/e2e.sh` green.

Part B ordering: B1 → B2 can start immediately (no backend dependency). B3/B4/B5
land against the A0 contracts and A4 events; the in-browser mock keeps them
demoable before the real backend is wired.
