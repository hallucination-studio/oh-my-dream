# oh-my-dream Architecture

> Status: active project map
> Last updated: 2026-07-15

`oh-my-dream` is a local desktop AI creation client. The detailed frozen backend architecture,
cross-module flow, and MVP boundary are owned by [`BACKEND.md`](BACKEND.md). This document only maps
the repository; it does not redefine backend contracts.

## Repository Map

```text
ui/                 React presentation and interaction
src-tauri/          Desktop boundary, bridges, effects, and composition
crates/projects/    pure Project identity and workspace metadata
crates/engine/      pure Workflow graph and Run semantics
crates/nodes/       exact Node Capabilities, Generation Profile, and Generation Settings semantics
crates/tasks/       pure Generation Task lifecycle and application interfaces
crates/backends/    provider composites, focused capabilities, and private routes
crates/assets/      Asset semantics and managed-media adapters
crates/assistant/   pure Assistant plan, proposal, approval, and repair semantics
assistant/          Python Agents SDK model and Reviewer adapter
```

## Authority Map

| Area | Architecture authority |
| --- | --- |
| backend system and MVP freeze | [`BACKEND.md`](BACKEND.md) |
| naming | [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md) |
| Project | [`BACKEND_PROJECT.md`](BACKEND_PROJECT.md) |
| Workflow graph and Run | [`BACKEND_WORKFLOW_GRAPH.md`](BACKEND_WORKFLOW_GRAPH.md), [`BACKEND_WORKFLOW.md`](BACKEND_WORKFLOW.md) |
| Node Capability and Provider | [`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md), [`BACKEND_PROVIDERS.md`](BACKEND_PROVIDERS.md) |
| Generation Task | [`BACKEND_TASK.md`](BACKEND_TASK.md) |
| Asset | [`BACKEND_ASSETS.md`](BACKEND_ASSETS.md) |
| Assistant | [`BACKEND_ASSISTANT.md`](BACKEND_ASSISTANT.md) |
| Desktop composition and storage | [`BACKEND_APPLICATION.md`](BACKEND_APPLICATION.md), [`BACKEND_STORAGE.md`](BACKEND_STORAGE.md) |

Business code depends on consumer-owned interfaces. Concrete database, filesystem, process,
provider, credential, event, and clock adapters depend inward and are selected only by
`DesktopCompositionRoot`. DTOs, Rows, Views, model messages, and provider payloads are boundary
representations, never a second source of business semantics.

## Dependency Direction

```text
React / Python adapter
        -> Desktop boundary and composition
             -> Project, Workflow, Generation Settings, Generation Task, Asset, and Assistant use cases/interfaces

Workflow, Asset, Assistant       -> ProjectId from Project
Node Capability implementations -> WorkflowNodeCapabilityInterface
Node Capability implementations -> Generation Task start and managed-media interfaces
Generation Settings             -> nodes-owned aggregates, policies, use cases, and repository interface
Generation Task application     -> task-owned provider, Asset, Workflow, storage, and clock interfaces
Provider composites/routes       -> provider-level and focused task-owned interfaces
Desktop bridges                  -> managed-media, preview, and cross-context interfaces
```

`crates/projects` and `crates/engine` remain pure. Cross-context calls use explicit bridges in
`src-tauri`. The Desktop
post-commit worker consumes only the three closed Desktop effect types. The separate Generation
Task worker consumes its five closed delayed effect types. Neither is a generic job framework.

The required merge gate is `./scripts/e2e.sh`.
