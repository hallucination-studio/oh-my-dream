# oh-my-dream Architecture

> Status: active design baseline
> Last updated: 2026-07-15

`oh-my-dream` is a local desktop AI creation client. Users compose typed Workflows, reuse managed
media, collaborate with an in-app Assistant, and execute generation capabilities through replaceable
backends.

## Capability-Oriented Structure

```text
ui/                 React presentation and interaction
src-tauri/          application boundary and composition root
crates/engine/      pure Workflow semantics and execution
crates/nodes/       generation and managed-media capabilities
crates/backends/    provider adapters
crates/assets/      SQLite metadata and managed files
assistant/          Agents SDK runtime and read-only Reviewer
```

Business code depends on consumer-owned traits. Concrete filesystem, database, process, provider,
and clock adapters depend inward and are selected only in the composition root. Each business rule
has one semantic owner; DTOs and view models are boundary representations.

## Workflow Authority

The canonical Workflow is the only executable and editable creative state. Rust owns graph
validation, revision, undo, patch dedupe, and execution semantics. React projects the current head
and submits bounded commands. Assistant candidates are immutable proposals, not a second graph.

Workflow documents remain at version `1.0`. An input binding is either one named `OutputRef` or an
ordered collection of named `OutputRef` values. Layout does not define execution semantics.

## Runtime Inputs And Outputs

Runtime cardinality is explicit:

```text
InputValue::Single(Value)
InputValue::OrderedMany(Vec<Value>)
NodeInputs = BTreeMap<String, InputValue>
```

`ValueMap = BTreeMap<String, Value>` is output-only. Ordered inputs are never delimiter-joined. Cache
identity includes cardinality, length, order, concrete variant, and value.

Workflow patches name both the source node or alias and its exact output through `PatchOutputRef`.
Selecting the first declared output is not a supported fallback. See
[ADR-002](decisions/002-typed-media-sources-and-ordered-inputs.md).

## Capability Identity

Each node persists a selector representation plus an exact capability contract version. The engine
registry owns selector resolution, normalized params, ports, and executable registration. Boundary
projections derive from that authority. Generic creation defaults are optional; contextual
capabilities require trusted caller context.

## Managed Assets

Asset identity is a stable `asset_id`; paths and presentation URLs are boundary data. Current-Project
and global local Assets are visible, while another Project's private Assets are not. One scoped
policy is shared by snapshots, UI creation, Assistant admission, approval revalidation, and
execution.

Nodes consume managed media through the nodes-owned `AssetReferenceResolver`. A Tauri adapter
implements it over `AssetStore` and verifies scope, concrete kind, and managed content availability.
Managed reads use `CapabilityEffect::LocalRead`: they execute every run to observe availability,
while unchanged downstream work remains cacheable.

## Assistant Authority

The Strong Assistant uses the Rust-authoritative chain:

```text
bounded snapshot -> ProductionPlan -> immutable candidate
-> read-only Reviewer -> Rust receipt -> human approval -> exact replay
```

The Agents SDK owns the model/tool loop and run state. Rust owns tools, candidates, receipts,
approval scope, and canonical mutation. React displays candidate and approval state. Assistant
changes never call the canonical mutating patch operation directly.

Selected Assets enter through trusted `selected_asset_ids`; the model receives bounded identity and
provenance but no local path, converted URL, bytes, or filesystem authority.

Changed runtime, patch, and Assistant contracts use a hard cut. A new contract epoch selects a fresh
Assistant state namespace. Previous orchestration state is not parsed or migrated; canonical
Workflow, Assets, and Assistant configuration remain.

## Side Effects And Persistence

Provider calls and external effects occur behind capability-scoped traits. Durable state and
idempotency decisions are committed before external follow-up where a use case couples them. Errors
remain structured across boundaries, and logs do not expose secrets or unnecessary path details.

## Current Delivery Boundary

Wave 1 delivers typed image, video, and audio Asset Sources; structured ordered inputs; explicit
patch outputs; manual and Assistant creation paths; managed previews; and deterministic tests. It
does not deliver real media assembly, mixed-media generation, provider expansion, durable
missing-Asset readiness, arbitrary Assistant Asset search, or large-graph scale work.
