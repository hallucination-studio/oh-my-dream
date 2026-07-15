# Backend MVP Architecture

> Status: proposed MVP design
> Scope: Rust backend for the first complete Text/Image/Video/Audio Workflow

## Purpose

The first backend version proves one complete local creation loop:

```text
create Text, Image, Video, and Audio nodes
  -> edit and connect a typed graph
  -> save and reopen it
  -> run the graph or one node with its dependencies
  -> persist generated media
  -> preview text, image, video, and audio
```

The architecture favors this vertical path over a general creative platform. It applies DDD and
explicit dependency injection where they protect real business or external boundaries, without
adding speculative abstractions.

## Document Map

| Document | Authority |
| --- | --- |
| [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md) | authoritative English terms, type naming, layers, and role suffixes |
| [`BACKEND_WORKFLOW.md`](BACKEND_WORKFLOW.md) | Workflow and Run aggregates, graph, editing, planning, execution, preview association |
| [`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md) | seven exact node capabilities and execution contracts |
| [`BACKEND_ASSETS.md`](BACKEND_ASSETS.md) | Asset aggregate, managed content, import, generated writes, resolution, and preview |
| [`BACKEND_PROVIDERS.md`](BACKEND_PROVIDERS.md) | three generation provider port adapters |
| [`BACKEND_APPLICATION.md`](BACKEND_APPLICATION.md) | Tauri DTO boundary, task host, preview protocol, and composition root |

The glossary owns names. Each detailed document owns its business semantics. This index owns only
cross-context boundaries and dependency direction.

## MVP Product Scope

React displays exactly four node shells backed by seven exact capabilities:

```text
Text:   text.literal
Image:  image.asset, image.text_to_image
Video:  video.asset, video.image_to_video
Audio:  audio.asset, audio.text_to_audio
```

The required graph is:

```text
Text -> Image -> Video
  |
  +------------> Audio
```

Imported Image, Video, and Audio Assets can enter through matching Asset source capabilities. Text
previews as text; managed Image, Video, and Audio outputs preview through the Asset boundary.

## DDD Context Map

```text
Workflow bounded context
  crates/engine     graph and Run domain, application use cases, consumer ports
  crates/nodes      seven built-in node capability contracts and executors
       |
       | NodeCapabilityGeneratedMediaWriterPort / NodeCapabilityManagedMediaReaderPort
       v
Asset bounded context
  crates/assets     Asset domain, application use cases, consumer ports, local adapters

Infrastructure boundaries
  crates/backends   provider adapters -> node-owned provider ports
  src-tauri         DTOs, SQLite adapters, bridges, task host, preview, composition
```

The Workflow and Asset bounded contexts each own their invariants, transitions, commands, queries,
results, and errors. NodeCapability is an executable sub-capability of Workflow. Provider and Desktop
code translate external protocols and never become business authorities.

## Dependency Direction

```text
React / unchanged Python Assistant
                 |
                 v
       src-tauri boundary and composition
          |            |             |
          v            v             v
    engine ports   Asset ports   provider adapters
          ^            ^             |
          |            |             v
    node capability ---+------> node-owned provider ports
```

Compile-time dependencies point toward the consumer-owned abstraction:

- `crates/engine` contains pure Workflow domain/application code and defines the execution ports it
  consumes;
- `crates/nodes` depends on engine contracts, implements its catalog/executor ports, and owns the
  media/provider ports its exact executors consume;
- `crates/backends` depends inward on the three node-owned provider ports;
- `crates/assets` owns media identity and content behavior without depending on Workflow or provider
  code;
- `src-tauri` implements persistence and cross-context bridge ports and selects concrete adapters;
- React and Python use DTOs and duplicate no Rust business semantics.

## Layer Rules

Every business context is capability-first and layered internally:

```text
<business capability>/
  domain/       aggregates, entities, values, policies, domain errors
  application/  named use cases, commands, queries, results
  ports/        focused consumer-owned traits
  infrastructure/ concrete adapters owned by that crate, when any
```

The rules are:

1. aggregates are the only authority for their invariants and state transitions;
2. application use cases coordinate aggregates and ports for one user intention;
3. repositories persist aggregates after domain validation and expose no arbitrary status setter;
4. DTOs, persistence Rows, Views, and provider DTOs contain no domain decisions;
5. cross-context translations are explicit and named;
6. there are no repository-wide `services`, `repositories`, or `models` buckets.

## Semantic Owners

| Concept | Authoritative owner | Primary types |
| --- | --- | --- |
| editable graph and revision | Workflow domain | `WorkflowAggregate`, `WorkflowNodeEntity`, `WorkflowEdgeEntity` |
| Run lifecycle and output association | Workflow domain | `WorkflowRunAggregate`, `WorkflowNodeExecutionEntity` |
| shared capability contract invariants | Workflow domain in `engine` | `NodeCapabilityContract` |
| seven exact parameter and execution semantics | exact modules in `nodes` | contract instances and `NodeCapabilityParameterSet` |
| managed media identity and availability | Asset domain | `AssetAggregate`, `AssetManagedContentState` |
| external generation protocol | provider adapter private code | private provider `*Dto` and `*AdapterError` |
| Tauri wire representation | Desktop boundary | `*RequestDto`, response `*Dto`, `DesktopErrorDto` |
| UI presentation state | React | editor and playback state, never Rust domain state |

Paths and URLs never identify Workflow values or Assets. Human-readable text, provider statuses,
node labels, filenames, and CSS types never determine business state.

## Dependency Injection

### Required Boundaries

A `*Port` trait is required for database, filesystem, provider, process event, clock, identity, media
inspection, and cross-context calls. Pure graph algorithms and stable value transformations remain
concrete.

Representative ports are:

| Consumer | Port |
| --- | --- |
| Workflow use cases | `WorkflowAggregateRepositoryPort`, `WorkflowRunRepositoryPort` |
| Workflow execution | `WorkflowNodeCapabilityCatalogPort`, `NodeCapabilityExecutorPort` |
| Workflow preview projection | `WorkflowMediaPreviewIssuerPort` |
| node Asset source/generation | `NodeCapabilityManagedMediaReaderPort`, `NodeCapabilityGeneratedMediaWriterPort` |
| exact generation capabilities | `TextToImageProviderPort`, `ImageToVideoProviderPort`, `TextToAudioProviderPort` |
| Asset use cases | `AssetAggregateRepositoryPort`, `AssetManagedContentStorePort`, `AssetMediaInspectorPort` |
| already-persisted Run events | `WorkflowRunEventPublisherPort` |

### Constructor Injection

Long-lived dependencies enter through constructors:

```rust
pub struct ImportAssetUseCase<R, T, S, M> {
    asset_repository: R,
    asset_ingest_transaction: T,
    managed_content_store: S,
    media_inspector: M,
}
```

Generic bounds are the matching `*Port` traits. Project, request identity, deadline, cancellation,
Workflow revision, and Run identity are call-scoped command or context values.

Only `src-tauri/composition.rs` constructs concrete adapters. No business code may use a service
locator, mutable global, concrete adapter parameter, runtime downcast, adapter-name branch, or
optional unsupported operation.

### Behavioral Equivalence

Every implementation of a port preserves the same errors, idempotency, concurrency, transaction,
ordering, pagination, cancellation, and retry semantics. Deterministic and production adapters run
the same parameterized port contract tests.

## Repository Shape

```text
crates/engine/src/workflow/
  domain/
  application/
  ports/

crates/nodes/src/node_capability/
  domain/
  application/
  ports/

crates/assets/src/asset/
  domain/
  application/
  ports/
  infrastructure/

crates/backends/src/
  mock/
  <provider>/

src-tauri/src/
  workflow/
  assets/
  providers/
  assistant/       unchanged
  configuration.rs
  composition.rs
```

Module paths add navigation; public type names still carry context and role as defined in the
glossary.

## State Ownership

| State | Owner |
| --- | --- |
| nodes, edges, revision, canvas position | `WorkflowAggregate` |
| Run/node execution state, progress, errors, outputs | `WorkflowRunAggregate` |
| fixed ports and parameter meaning | `NodeCapabilityContract` and exact capability module |
| Asset identity, content state, media facts, origin | `AssetAggregate` |
| provider task/protocol details | concrete provider adapter during active execution |
| concrete dependency graph | `DesktopCompositionRoot` |
| DTO and short-lived preview URL | Desktop boundary |
| selection, viewport, drag, playback, object URLs | React session state |

## Edit Flow

```text
ApplyWorkflowMutationRequestDto
  -> ApplyWorkflowMutationCommand
  -> ApplyWorkflowMutationUseCase
  -> WorkflowAggregate validates complete candidate
  -> SqliteWorkflowAggregateRepositoryAdapter compare-and-swap
  -> WorkflowDto
```

React never saves its entire editor store. Provider fields, Run state, and preview state cannot enter
the Workflow document.

## Run Flow

```text
StartWorkflowRunCommand
  -> readiness + deterministic WorkflowExecutionPlanValue
  -> persist Queued WorkflowRunAggregate and first event
  -> DesktopWorkflowRunTaskHost starts ExecuteWorkflowRunUseCase
  -> NodeCapabilityExecutorPort executes ready nodes
  -> persist aggregate transitions and events
  -> WorkflowRunEventPublisherPort emits projections
```

Provider work starts only after queued intent is durable. Independent branches may execute within a
bounded concurrency limit. The MVP has no cross-run result cache.

## Generated Media Flow

```text
provider adapter returns NodeCapabilityGeneratedMediaPayload
  -> NodeCapabilityGeneratedMediaWriterPort
  -> RecordGeneratedAssetUseCase
  -> persist Pending AssetAggregate and finalize job
  -> finalize and validate managed content
  -> transition AssetAggregate to Available
  -> return Workflow managed-media reference
```

A node succeeds only after its generated Asset is available. SQLite and external I/O are not treated
as one transaction; Pending finalization is idempotent and recoverable.

## Preview Flow

```text
GetWorkflowNodePresentationUseCase
  -> WorkflowMediaPreviewIssuerPort
  -> DesktopWorkflowMediaPreviewAdapter
  -> optional IssueAssetPreviewUseCase
  -> AssetPreviewLease
  -> WorkflowMediaPreviewValue
  -> WorkflowNodePresentationView
  -> WorkflowNodePresentationDto with short-lived Desktop URL
  -> React text/image/video/audio renderer
```

Video and audio support verified MIME and Range. Preview URLs and playback state never round-trip
into Workflow or Asset identity.

## Errors, Events, And Logs

Each context owns structured `*DomainError` and `*ApplicationError` types. Tauri translates once to
`DesktopErrorDto` with code, safe message, retryability, target, bounded details, and optional
correlation ID.

Run events have a monotonic sequence per Workflow Run and are committed before emission. React can
query missed events and terminal state. Logs contain stable typed IDs and omit secrets, provider
bodies, signed URLs, generated content, and unnecessary local paths.

## Verification Architecture

- Workflow tests prove graph invariants, readiness, planning, transitions, and cancellation;
- capability tests prove seven exact parameter, port, and execution contracts;
- provider port suites run against deterministic and configured adapters;
- Asset tests prove import/generated flow, Project isolation, resolution, recovery, and preview;
- Desktop tests prove transaction ordering, task hosting, DTO translation, and events;
- contract tests keep Rust DTOs and TypeScript types aligned;
- end-to-end tests run both required branches and preview all four shells;
- static checks reject concrete adapter construction outside `composition.rs` and inward dependency
  violations.

The implementation merge gate remains `./scripts/e2e.sh`.

## MVP Completion Criteria

1. Users can create, move, edit, connect, save, and reopen all four node shells.
2. Rust rejects invalid types and cycles while incomplete drafts remain editable.
3. Deterministic provider adapters complete `Text -> Image -> Video` and `Text -> Audio`.
4. Generated Image, Video, and Audio outputs are durable Project Assets.
5. All four preview types work after execution and after reopen.
6. Progress, failure, and cancellation are visible through durable Run state and events.
7. No DTO exposes provider state, credentials, managed paths, or persistence Rows.
8. Public Rust types satisfy the glossary naming rules and concrete dependencies are constructor
   injected only at the composition root.

## Post-MVP

Multiview, references, text generation, text-to-video, concat, timelines, dynamic ports, plugin
nodes, batches, cross-run cache, remote task resume, provider fallback, cost accounting, advanced
Asset management, collaboration, cloud sync, 3D, and scenes are outside this architecture revision.
