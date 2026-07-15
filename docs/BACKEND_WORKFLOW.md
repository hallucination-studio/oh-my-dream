# Backend Workflow Architecture

> Status: proposed MVP design
> Owner: `crates/engine`
> Scope: four visible node shells, graph editing, execution, and preview association

Naming follows [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md). `WorkflowAggregate` and
`WorkflowRunAggregate` are the aggregate roots in this bounded context.

## MVP Goal

The first backend version supports one complete local creative flow:

```text
Text -> Image -> Video
  |
  +------------> Audio
```

Users can create and edit Text, Image, Video, and Audio nodes, connect compatible ports, run the
whole Workflow or one node with its dependencies, observe progress and errors, save and reopen the
graph, and preview the latest successful output.

The Workflow bounded context is the sole authority for graph identity, revision, nodes, edges,
validity, execution planning, Run state, and output association. Domain code performs no UI,
network, filesystem, database, or provider work.

## Deliberate MVP Boundary

The MVP excludes multiview, reference batches, concat, timelines, dynamic ports, groups, subgraphs,
conditional flow, cross-run cache, collaboration, 3D, and scenes.

DVStudio contributes only the useful patterns of stable node identity, typed ports, first-class
edges, and media previews. Its combined UI/runtime/persistence node object, URL inference, and
provider task fields are not copied.

## DDD Layers

```text
crates/engine/src/workflow/
  domain/       Workflow and Run aggregates, entities, values, policies, errors
  application/  edit, readiness, start, cancel, query, and presentation use cases
  ports/        repositories, capability catalog/executor, clock, IDs, events
```

Aggregates own invariants and transitions. Application use cases load aggregates, invoke domain
behavior, and persist through focused ports. Infrastructure and node capability implementations
depend inward on those consumer-owned ports.

## Visible Node Shells

React displays four node shells backed by seven exact capability contracts:

| `WorkflowNodeShellKindDto` | Capability contracts |
| --- | --- |
| `Text` | `text.literal@1.0` |
| `Image` | `image.asset@1.0`, `image.text_to_image@1.0` |
| `Video` | `video.asset@1.0`, `video.image_to_video@1.0` |
| `Audio` | `audio.asset@1.0`, `audio.text_to_audio@1.0` |

There is no domain `NodeKind`. `WorkflowNodeShellKindDto` is derived from the primary output's
`WorkflowDataType` and controls presentation only. Each persisted node selects one exact
`NodeCapabilityContractRef`.

## Workflow Aggregate

```rust
pub struct WorkflowAggregate {
    pub schema_version: WorkflowSchemaVersion,
    pub id: WorkflowId,
    pub project_id: ProjectId,
    pub revision: WorkflowRevision,
    pub nodes: BTreeMap<WorkflowNodeId, WorkflowNodeEntity>,
    pub edges: BTreeMap<WorkflowEdgeId, WorkflowEdgeEntity>,
}

pub struct WorkflowNodeEntity {
    pub id: WorkflowNodeId,
    pub capability_contract: NodeCapabilityContractRef,
    pub parameter_set: NodeCapabilityParameterSet,
    pub canvas_position: WorkflowCanvasPositionValue,
}

pub struct WorkflowEdgeEntity {
    pub id: WorkflowEdgeId,
    pub source_node_id: WorkflowNodeId,
    pub source_output_key: NodeCapabilityOutputPortKey,
    pub target_node_id: WorkflowNodeId,
    pub target_input_key: NodeCapabilityInputPortKey,
}
```

The selected capability normalizes `NodeCapabilityParameterSet`; graph code treats it as opaque
structured data. Canvas position is persisted for reopen but excluded from readiness and execution.

Nodes never persist port definitions, connected values, outputs, progress, errors, provider task
IDs, URLs, paths, previews, or playback state. Selection, hover, drag state, viewport, open menus,
playback position, and volume remain React session state.

## Fixed Port Model

```rust
pub enum WorkflowDataType {
    Text,
    Image,
    Video,
    Audio,
}
```

Port keys and types come from the exact capability contract version. Each MVP input accepts zero or
one value, so `WorkflowEdgeEntity` needs no ordering field. Types match exactly; there is no generic
media wildcard or implicit conversion. An Image becomes a Video only through
`video.image_to_video`.

## Edge Invariants

- an edge names one exact output port and one exact input port;
- a single input has at most one incoming edge;
- an output may fan out to several inputs;
- duplicate endpoints, self-edges, missing endpoints, and cycles are rejected;
- nodes hold no connection state;
- removing a node removes its incident edges in the same aggregate transition;
- incoming and outgoing indexes are derived, never persisted.

Connecting to an occupied input does not silently replace it. The caller explicitly disconnects the
old edge and connects the new edge in one atomic mutation.

## Draft Validity And Run Readiness

An incomplete node remains editable, so the domain exposes two checks.

Draft validity always enforces:

- unique typed node and edge IDs;
- known capability contract versions;
- valid values for parameters that are present;
- existing nodes and named ports;
- exact type compatibility and single-input cardinality;
- no duplicate edge, self-edge, or cycle.

Run readiness additionally requires:

- every required parameter and input;
- every referenced Asset visible in the current Project and available;
- one executor wired for every node in the requested scope.

`WorkflowReadinessPolicy` returns all structured issues by node, parameter, or input. It never
repairs the graph, and `StartWorkflowRunUseCase` checks it again before persistence.

## Editing Use Case

All semantic and position edits enter one compare-and-swap use case:

```text
ApplyWorkflowMutationCommand {
  workflow_id,
  base_revision,
  application_request_id,
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
ConnectWorkflowEdge
DisconnectWorkflowEdge
```

Selecting another capability never silently drops edges. The same command must explicitly remove
edges invalid under the new fixed contract. `WorkflowAggregate` validates the complete candidate;
`ApplyWorkflowMutationUseCase` persists all or none.

The caller assigns opaque final node and edge IDs. `base_revision` prevents lost updates. Repeating
the same request ID and payload returns the original `ApplyWorkflowMutationResult`; reusing the ID
with another payload returns a conflict.

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
bindings, and dependency order.

## Runtime Values

```rust
pub enum WorkflowRuntimeValue {
    Text(WorkflowTextValue),
    Image(WorkflowManagedImageRefValue),
    Video(WorkflowManagedVideoRefValue),
    Audio(WorkflowManagedAudioRefValue),
}

pub type WorkflowNodeInputSet =
    BTreeMap<NodeCapabilityInputPortKey, WorkflowRuntimeValue>;
pub type WorkflowNodeOutputSet =
    BTreeMap<NodeCapabilityOutputPortKey, WorkflowRuntimeValue>;
```

Text is bounded immutable text. Managed media references contain Asset identity and a content
fingerprint, never bytes, paths, provider URLs, or preview URLs. An executor returns every declared
output or one structured failure. Generated media becomes a runtime value only after Asset storage
succeeds.

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
stops new dispatch, signals active executors, and rejects late outputs. Retry creates a new
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

`GetWorkflowNodePresentationUseCase` joins the node contract, readiness issues, latest relevant node
execution, and `WorkflowMediaPreviewValue` into `WorkflowNodePresentationView`. It obtains media
preview access through `WorkflowMediaPreviewIssuerPort`; Workflow code never imports an Asset lease.
Tauri translates the View to `WorkflowNodePresentationDto`, which may contain a short-lived URL that
is never accepted as Workflow input.

Each output records the producing Workflow revision. When the current node or an ancestor changes,
the projection marks the previous preview stale.

## No Cross-Run Cache

One Run naturally reuses a dependency output when it fans out. The MVP has no cross-run result
cache. Starting another Run intentionally generates another result.

## Application Ports And Injection

| Port | Required capability |
| --- | --- |
| `WorkflowAggregateRepositoryPort` | load and compare-and-swap one revision |
| `WorkflowMutationReceiptRepositoryPort` | persist idempotent mutation results |
| `WorkflowRunRepositoryPort` | persist Run aggregates, outputs, and ordered events |
| `WorkflowNodeCapabilityCatalogPort` | expose fixed contracts, normalization, and readiness |
| `NodeCapabilityExecutorPort` | execute one prepared node capability |
| `WorkflowMediaPreviewIssuerPort` | translate a managed-media reference into scoped preview access |
| `WorkflowClockPort` | provide deterministic timestamps |
| `WorkflowIdentityGeneratorPort` | create production or deterministic IDs |
| `WorkflowRunEventPublisherPort` | publish already-persisted Run events |

Use cases receive ports through constructors. For example, `StartWorkflowRunUseCase` receives the
Workflow repositories, capability catalog, executor, clock, and event publisher it consumes.
Concrete adapters are selected only in `src-tauri/composition.rs`. Pure graph algorithms remain
synchronous concrete domain code.

## Structured Errors

`WorkflowDomainError` covers graph invariants and transitions. `WorkflowApplicationError` adds port
and orchestration failures. Stable categories include revision conflict, replay conflict, unknown
capability, invalid parameters, missing node/port, type mismatch, occupied input, duplicate edge,
cycle, not ready, unavailable Asset, unavailable executor, upstream failure, cancellation, and
execution failure.

Errors contain safe typed IDs and structured details. Behavior is never inferred from message text.

## Verification

- aggregate tests cover graph mutations, exact types, fan-out, duplicates, and cycles;
- readiness tests prove incomplete drafts remain editable and cannot run;
- plan tests prove whole-graph and through-node dependency order;
- Run aggregate tests cover transitions, independent branches, failure, and cancellation;
- port contract tests cover repository concurrency and idempotency semantics;
- preview tests cover all four projections and stale-output labeling;
- boundary contract tests prove React consumes Rust-owned ports and errors.

## Post-MVP

Multiple references, ordered-many inputs, text generation, text-to-video, concat, durable backend
undo, cross-run cache, restart-resumable provider tasks, and dynamic capabilities require separate
design after the four-node flow is proven. 3D and scene nodes are not product scope.
