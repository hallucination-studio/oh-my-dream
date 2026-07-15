# Backend Workflow Architecture

> Status: proposed backend design
> Owner: `crates/engine`
> Scope: graph editing, typed input binding, execution, and preview association

Naming follows [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md). `WorkflowAggregate` and
`WorkflowRunAggregate` are the aggregate roots in this bounded context.

## MVP Goal

The first backend version supports one complete local creative flow:

```text
Text -> Image -> Video
  |
  +------------> Audio
```

Users can create and edit exact Text, Image, Video, Audio, Image Sequence, and Video Storyboard
capabilities, connect compatible ports, run the whole Workflow or one node with its dependencies,
observe progress and errors, save and reopen the graph, and preview the latest successful output.

The Workflow bounded context is the sole authority for graph identity, revision, nodes, input
bindings, validity, execution planning, Run state, and output association. Domain code performs no
UI, network, filesystem, database, or provider work.

## Deliberate Boundary

The Workflow model includes ordered reference inputs, mixed-media items, per-item reference roles,
and stable prompt-to-item references. This design does not add a node capability, provider
operation, UI interaction, or cross-run cache. Multiview, concat, timelines, dynamic ports, groups,
subgraphs, conditional flow, collaboration, 3D, and scenes remain outside this scope.

DVStudio contributes only the useful patterns of stable node identity, typed ports, first-class
edges, and media previews. Its combined UI/runtime/persistence node object, URL inference, and
provider task fields are not copied.

## DDD Layers

```text
crates/engine/src/workflow/
  domain/       Workflow and Run aggregates, entities, values, policies, errors
  application/  edit, readiness, start, cancel, query, and presentation use cases
  ports/        repositories, node capability interface, clock, IDs, events
```

Aggregates own invariants and transitions. Application use cases load aggregates, invoke domain
behavior, and persist through focused ports. Infrastructure and node capability implementations
depend inward on those consumer-owned ports.

## Visible Node Shells

React derives presentation shells from the primary output type. The authoritative operation list is
the catalog in [`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md); representative projections are:

| `WorkflowNodeShellKindDto` | Capability contracts |
| --- | --- |
| `Text` | literal and text-generation capabilities |
| `Image` | Asset read, image generation, and crop capabilities |
| `Video` | Asset read, video generation, upscale, and concatenation capabilities |
| `Audio` | Asset read, speech synthesis, and music generation capabilities |
| `ImageSequence` | video frame extraction |
| `VideoStoryboard` | video storyboard analysis |

There is no domain `NodeKind`. `WorkflowNodeShellKindDto` is derived from the primary output's
`WorkflowDataType` and controls presentation only. Each persisted node selects one exact
`NodeCapabilityContractRef`.

## Graph Contract

[`BACKEND_WORKFLOW_GRAPH.md`](BACKEND_WORKFLOW_GRAPH.md) is the authoritative definition of
`WorkflowAggregate`, typed single and ordered input bindings, stable input-item identity,
capability-owned role keys, persistence ordering, and graph invariants. This document consumes that
model for readiness, editing, execution planning, Run lifecycle, and previews without redefining it.

## Draft Validity And Run Readiness

An incomplete node remains editable, so the domain exposes two checks.

Draft validity always enforces:

- unique typed node and input-item IDs;
- known capability contract versions;
- valid values for parameters that are present;
- existing nodes and named ports;
- binding shape, item identity, role, and concrete type compatibility;
- ordered-reference maximum constraints;
- valid structured prompt references;
- no self-edge or cycle.

Run readiness additionally requires:

- every required parameter and input;
- every ordered-reference minimum constraint;
- every referenced Asset visible in the current Project and available;
- one capability implementation registered for every node in the requested scope.

`WorkflowReadinessPolicy` returns all structured issues by node, parameter, or input. It never
repairs the graph, and `StartWorkflowRunUseCase` checks it again before persistence.

## Editing Use Case

All semantic and position edits enter one compare-and-swap use case:

```text
ApplyWorkflowMutationCommand {
  workflow_id,
  base_revision,
  operations: Vec<WorkflowMutationAction>
}
```

`WorkflowMutationAction` is a closed union:

```text
AddWorkflowNode
RemoveWorkflowNode
ReplaceWorkflowNodeParameters
SelectWorkflowNodeCapability
MoveWorkflowNode
BindWorkflowSingleInput
InsertWorkflowReferenceItem
MoveWorkflowReferenceItem
RemoveWorkflowInputItem
SetWorkflowInputItemRole
```

`MoveWorkflowReferenceItem` identifies the item by `WorkflowInputItemId` and supplies its new
index. It preserves the item ID, source, and role. Inserting, moving, removing, and changing a role
are semantic edits: each increments `WorkflowRevision`; position-only canvas movement remains
non-executable presentation state.

Selecting another capability never silently drops input items. The same command must explicitly
remove bindings invalid under the new exact contract. `WorkflowAggregate` validates the complete
candidate; `ApplyWorkflowMutationUseCase` persists all or none.

The caller assigns opaque final node and input-item IDs. `base_revision` prevents lost updates.
The local Desktop MVP does not persist mutation replay receipts. After an uncertain command result,
the caller reloads the current Workflow snapshot before issuing another mutation.

## Run Scope And Plan

```rust
pub enum WorkflowRunScope {
    WholeWorkflow,
    ThroughNode(WorkflowNodeId),
}
```

`WholeWorkflow` includes every node. `ThroughNode` includes the selected node and all transitive
ancestors, which supports the Run action on one node without running unrelated branches.

Starting a Run freezes one exact Workflow revision. The domain checks readiness, topologically sorts
the selected subgraph, and binds named inputs before any provider call. The immutable
`WorkflowExecutionPlanValue` contains only node IDs, capability refs, normalized parameters, input
items with roles in their persisted order, and dependency order.

## Runtime Values

```rust
pub struct WorkflowTextValue {
    pub parts: Vec<WorkflowTextPartValue>,
}

pub enum WorkflowTextPartValue {
    Literal(WorkflowBoundedTextValue),
    InputItemReference(WorkflowInputItemId),
}

pub enum WorkflowRuntimeValue {
    Text(WorkflowTextValue),
    Image(WorkflowManagedImageRefValue),
    Video(WorkflowManagedVideoRefValue),
    Audio(WorkflowManagedAudioRefValue),
    ImageSequence(WorkflowImageSequenceValue),
    VideoStoryboard(WorkflowVideoStoryboardValue),
}

pub struct WorkflowRuntimeInputItemValue {
    pub input_item_id: WorkflowInputItemId,
    pub input_role_key: Option<NodeCapabilityInputRoleKey>,
    pub value: WorkflowRuntimeValue,
}

pub enum WorkflowNodeInputValue {
    Single(WorkflowRuntimeInputItemValue),
    OrderedReferences(NonEmptyVec<WorkflowRuntimeInputItemValue>),
}

pub type WorkflowNodeInputSet =
    BTreeMap<NodeCapabilityInputPortKey, WorkflowNodeInputValue>;
pub type WorkflowNodeOutputSet =
    BTreeMap<NodeCapabilityOutputPortKey, WorkflowRuntimeValue>;
```

Text is an immutable structured sequence with bounded total literal text and bounded part count.
Managed media references contain Asset identity and a content fingerprint, never bytes, paths,
provider URLs, or preview URLs. A capability implementation returns every declared output or one
structured failure.
Generated media becomes a runtime value only after Asset storage succeeds.

Binding never erases structure. Executors receive the input item's stable ID, explicit role,
concrete runtime variant, and vector position. They do not reconstruct any of these from filenames,
MIME strings, prompt contents, or provider DTOs.

## Stable Prompt References

A prompt that mentions reference material uses `WorkflowTextValue` rather than a string containing a
magic token. Plain text contains only `Literal` parts. A material mention contains the stable input
item ID; its display label is a projection and is not persisted as identity. For a node to be
Run-ready, every `InputItemReference` consumed by that node must resolve to a reference item bound
to that same node. This prevents prompt text from creating hidden graph dependencies. Removing a
referenced item requires removing or replacing its prompt parts in the same atomic mutation, while
reordering the item requires no prompt change.

The exact Text-producing capability owns text normalization and returns referenced item IDs inside
`NormalizedNodeCapabilityParameters`. Workflow traces each Text output to its consuming node and
owns cross-node referential integrity. Execution preparation rechecks the resolved value
before dispatch. This design admits only references that are statically inspectable from the frozen
Workflow revision; dynamically generated prompt references are outside scope.

The exact prompt-consuming capability owns provider request mapping. Provider-specific placeholder
syntax is produced at the provider boundary and never persisted.

## Execution Identity

Every planned node has a versioned `WorkflowPlannedNodeIdentityValue` derived from a canonical
encoding of:

- the exact capability contract reference and normalized parameters;
- input keys and their `Single` or `OrderedReferences` discriminator;
- each ordered item's stable ID, explicit role, and exact source output reference;
- structured prompt parts and their referenced input item IDs.

Map keys use canonical key order, while `OrderedReferences` items are encoded in vector order. The
encoding is length-delimited and includes variant tags, so it cannot collide through string
concatenation. Consequently, changing item order, source, role, or prompt-to-item association
changes planned identity. Moving a node on the canvas does not.

Immediately before dispatch, after upstream values resolve, the application derives
`WorkflowNodeDispatchIdentityValue` from the planned identity plus each input item's concrete
runtime variant and content identity. For media, content identity is the managed Asset content
fingerprint; URLs and paths are excluded. Because planned identity is an input, reordering always
changes dispatch identity even when two items resolve to identical bytes.

`WorkflowRunId` and `WorkflowNodeExecutionId` identify execution records. Planned identity
identifies the frozen structural work; dispatch identity identifies one fully bound operation for
provider idempotency. Neither is permission to introduce cross-run caching.

## Run Aggregate

`WorkflowRunAggregate` represents one execution of one frozen revision.
`WorkflowNodeExecutionEntity` owns one node's state, progress, structured failure, and output set.
Neither is embedded in `WorkflowAggregate`.

```text
WorkflowRunState:
  Queued -> Running -> Succeeded | Failed | Cancelled

WorkflowNodeExecutionState:
  Pending -> Running -> Succeeded | Failed | Cancelled | Blocked
```

A failure blocks descendants but does not stop independent branches. Cancellation records intent,
stops new dispatch, signals active capability executions, and rejects late outputs. Retry creates a new
`WorkflowRunAggregate`; terminal Runs are immutable.

`WorkflowRunEvent` records a monotonic sequence per Run and is persisted before Desktop emission.
Clients deduplicate by `workflow_run_id + sequence`.

## Preview Association

Preview is a read projection, not aggregate state:

| Shell | Preview source |
| --- | --- |
| Text | literal text or latest successful text output |
| Image | short-lived Asset image preview |
| Video | short-lived Asset stream with MIME and Range support |
| Audio | short-lived Asset stream with MIME and Range support |
| ImageSequence | ordered timestamped Image previews |
| VideoStoryboard | ordered scene timeline and bounded analysis text |

`GetWorkflowNodePresentationUseCase` joins the node contract, readiness issues, latest relevant node
execution, and `WorkflowMediaPreviewValue` into `WorkflowNodePresentationView`. It obtains media
preview access through `WorkflowMediaPreviewIssuerPort`; Workflow code never imports an Asset lease.
Tauri translates the View to `WorkflowNodePresentationDto`, which may contain a short-lived URL that
is never accepted as Workflow input.

Each output records the producing Workflow revision. When the current node or an ancestor changes,
the projection marks the previous preview stale.

## No Cross-Run Cache

One Run naturally reuses a dependency output when it fans out. This architecture defines no
cross-run result cache; starting another Run intentionally produces another result.

## Application Ports And Injection

| Port | Required capability |
| --- | --- |
| `WorkflowAggregateRepositoryPort` | load and revision-CAS the current Workflow snapshot |
| `WorkflowRunRepositoryPort` | atomically admit and transition Runs, outputs, and events |
| `WorkflowNodeCapabilityPort` | expose one exact contract, normalize parameters, check readiness, and execute |
| `WorkflowMediaPreviewIssuerPort` | translate a managed-media reference into scoped preview access |
| `WorkflowClockPort` | provide deterministic timestamps |
| `WorkflowIdentityGeneratorPort` | create production or deterministic IDs |
| `WorkflowRunEventPublisherPort` | publish already-persisted Run events |

Use cases receive ports through constructors. `StartWorkflowRunUseCase` receives the Workflow
repositories, one concrete `WorkflowNodeCapabilityRegistry`, clock, and event publisher. Tests build
that registry from fake or real `WorkflowNodeCapabilityPort` implementations.
Concrete adapters are selected only in `src-tauri/composition.rs`. Pure graph algorithms remain
synchronous concrete domain code.

Logical records, transaction boundaries, and restart behavior are defined in
[`BACKEND_STORAGE.md`](BACKEND_STORAGE.md).

## Structured Errors

`WorkflowDomainError` covers graph invariants and transitions. `WorkflowApplicationError` adds port
and orchestration failures. Stable categories include revision conflict, unknown
capability, invalid parameters, missing node/port, type mismatch, occupied input, invalid
cardinality, duplicate input item, invalid reference role, role/type mismatch, invalid prompt
reference, cycle, not ready, unavailable Asset, unavailable capability implementation, upstream failure,
cancellation, and execution failure.

Errors contain safe typed IDs and structured details. Behavior is never inferred from message text.

## Verification

- graph and persistence contract suites are owned by `BACKEND_WORKFLOW_GRAPH.md`;
- readiness tests prove incomplete drafts remain editable and cannot run;
- prompt tests prove item references survive reorder and reject missing or foreign-node items;
- plan tests prove whole-graph and through-node dependency order and preserve input-item order;
- execution identity tests prove reorder, role changes, source changes, concrete type changes,
  content changes, and prompt-reference changes alter the correct planned or dispatch identity while
  canvas movement does not;
- Run aggregate tests cover transitions, independent branches, failure, and cancellation;
- port contract tests cover repository concurrency and transition atomicity;
- preview tests cover all six output projections and stale-output labeling;
- boundary contract tests prove React consumes Rust-owned ports and errors.

## Post-MVP

Durable backend undo, cross-run cache, restart-resumable provider tasks, dynamic capabilities,
batch generation, 3D, and scene generation require separate designs. The built-in generation,
transformation, and analysis catalog is defined in `BACKEND_CAPABILITIES.md`.
