# ADR-002: Use Typed Asset Sources And Structured Runtime Inputs

## Status

Accepted

## Date

2026-07-15

## Context

HELL-941 requires managed image, video, and audio Assets to become reusable Workflow sources. The
persisted Workflow `1.0` format already retains named outputs and ordered bindings, but runtime
execution flattens ordered inputs, patch operations infer the first output, and media-consuming
nodes depend on a concrete Asset store. The Strong Assistant also evaluates patches, stores immutable
candidates, attests Reviewer evidence, pauses for approval, and exactly replays approved work.

This wave must fix those foundations without implementing real media assembly, mixed-media
generation, provider expansion, durable missing-Asset readiness, or large-graph scale work. Changed
contracts use the user-approved hard cut rather than compatibility or data migration.

## Decision

### Preserve Workflow `1.0`

Keep the persisted binding union unchanged:

```text
InputBinding::Single { source: OutputRef }
InputBinding::OrderedMany { sources: Vec<OutputRef> }
```

It already stores exact output names and order. No Workflow document migration is required.

### Separate runtime inputs from outputs

Hard-switch the engine `Node` contract to:

```text
InputValue::Single(Value)
InputValue::OrderedMany(Vec<Value>)
NodeInputs = BTreeMap<String, InputValue>
ValueMap = BTreeMap<String, Value>
```

`NodeInputs` is input-only and `ValueMap` is output-only. Defaults enter execution as `Single`. No
scalar adapter or delimiter flattening remains. Cache identity includes input name, cardinality,
length, order, concrete value variant, and value.

### Require explicit patch outputs

Replace node-only binding sources with:

```text
PatchOutputRef { node: NodeRef, output: String }
InputBinding<PatchOutputRef>
```

Manual apply, evaluation, Assistant candidate preparation, candidate hashing, Reviewer evidence,
approval, replay, TypeScript, and mocks use this one semantic. Declaration order is never an output
selection rule.

### Hard-cut changed contracts

Keep one runtime API, patch shape, active operation version, Reviewer contract, and approval format.
Delete old schemas, parsers, fixtures, adapters, and version branches. A new Assistant contract epoch
selects a fresh state namespace; older ProductionPlans, candidates, receipts, approvals, repair
state, SDK Sessions, and Runner state are not parsed or converted. Assistant configuration, Assets,
and canonical Workflow documents remain because their formats do not change.

### Add contextual Asset Sources

Register exact `ImageAssetSource@1.0`, `VideoAssetSource@1.0`, and `AudioAssetSource@1.0`
capabilities under `Image / asset`, `Video / asset`, and `Audio / asset`. Their normalized params
contain only canonical `mode` and a stable non-empty `asset_id`. Paths, URLs, bytes, and presentation
DTO fields never enter Workflow semantics. They have no generic creation defaults.

### Put managed reads behind a consumer-owned boundary

Define `AssetReferenceResolver` in `crates/nodes`. It accepts trusted Project context, stable Asset
identity, and expected concrete media kind. A Tauri composition-root adapter implements it over
Asset persistence. One scoped policy admits current-Project and global local Assets and rejects
another Project's private Assets. Snapshot, UI creation, Assistant admission, approval
revalidation, and execution use that policy.

### Classify managed reads explicitly

Add `CapabilityEffect::LocalRead` without provider-health or external-approval semantics. A local
read resolves current availability every run instead of reusing its own output cache. Stable output
identity still permits unchanged downstream cache reuse.

### Preserve concrete media types

Do not add `PortType::Media`. A future mixed-media input may declare accepted concrete source types,
while outputs and runtime values remain image, video, or audio. That extension waits for a production
consumer.

### Extend the Strong Assistant authority chain

The UI sends selected stable IDs through trusted `selected_asset_ids`; the bounded snapshot exposes
identity, kind, and limited provenance, never paths, URLs, or bytes. Asset Sources are created only
through `workflow_prepare_patch`. The Reviewer fetches the exact Rust candidate, human approval
replays it once, and approved apply revalidates Asset scope, kind, and managed content.

## Consequences

- Ordered cardinality and order survive persistence, execution, and cache hashing.
- Manual and Assistant connections share one explicit-output contract.
- Existing Workflow `1.0` documents remain readable.
- Pre-cutover Assistant orchestration state is intentionally unavailable.
- Asset paths remain boundary data rather than identity.
- Missing content yields structured failure and an unavailable UI projection; durable readiness is
  deferred.
- `VideoConcat` remains a deterministic placeholder, not a codec implementation.
- A future `GeneratedAssetWriter` should remove the existing concrete write dependency separately.

## Rejected Alternatives

- Delimiter-flatten ordered inputs: loses structure and permits cache collisions.
- Workflow `2.0`: the current wire format already contains the required semantics.
- Node-only patch sources: declaration order is not a stable contract.
- Direct `AssetStore` use in nodes: reverses dependency direction.
- Paths or preview URLs in params: unstable and may expose local details.
- A vague `Media` output: erases concrete value semantics prematurely.
- Migrating old Assistant state: explicitly rejected in favor of a hard cut.

## Wave 1 Boundary

Included: structured inputs, ordered caching, explicit patch outputs, contextual Asset Sources,
scoped resolution, `LocalRead`, manual creation and preview, selected-Asset Assistant context, exact
review/approval/replay, and deterministic tests.

Deferred: real media assembly, mixed-media consumers, provider expansion, paid dispatch, unbounded
Assistant Asset search, filesystem access, durable missing-Asset readiness, generated-Asset writer
redesign, and HELL-965 scale work.
