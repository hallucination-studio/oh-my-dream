# Backend Architecture

> Status: proposed target architecture
> Scope: Rust backend for provider-independent creative Workflows

## Purpose

The backend supports one durable local creation loop across exact content operations:

```text
select an exact generation, transformation, analysis, or Asset-read capability
  -> edit and connect a typed graph
  -> save and reopen it
  -> run the graph or one node with its dependencies
  -> persist generated and derived media
  -> preview scalar media, image sequences, and structured storyboards
```

The architecture applies DDD and explicit dependency injection at real business and external
boundaries. Exact capability semantics remain independent from provider topology, storage, and UI.

## Document Map

| Document | Authority |
| --- | --- |
| [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md) | authoritative English terms, type naming, layers, and role suffixes |
| [`BACKEND_WORKFLOW_GRAPH.md`](BACKEND_WORKFLOW_GRAPH.md) | Workflow aggregate, typed bindings, ordered references, and graph invariants |
| [`BACKEND_WORKFLOW.md`](BACKEND_WORKFLOW.md) | readiness, editing use case, planning, Run lifecycle, execution, and preview association |
| [`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md) | authoritative capability catalog, requests, results, errors, and consumer ports |
| [`BACKEND_ASSETS.md`](BACKEND_ASSETS.md) | Asset aggregate, managed content, import, generated writes, resolution, and preview |
| [`BACKEND_PROVIDERS.md`](BACKEND_PROVIDERS.md) | stable generation profiles, availability, routing, and provider adapters |
| [`BACKEND_APPLICATION.md`](BACKEND_APPLICATION.md) | Tauri DTO boundary, task host, preview protocol, and composition root |
| [`BACKEND_STORAGE.md`](BACKEND_STORAGE.md) | local metadata, managed media, and restart durability |

The glossary owns names. Each detailed document owns its business semantics. This index owns only
cross-context boundaries and dependency direction.

## Product Capability Scope

The target catalog covers:

```text
Foundation: literal Text and Image/Video/Audio Asset reads
Image:      text, image, and multi-reference generation; crop
Video:      text, image, multi-reference, first-frame, first-and-last-frame,
            and mixed-media generation; upscale, frame extraction, concatenation,
            and storyboard analysis
Text:       text-only and mixed-media generation
Audio:      independent speech synthesis and music generation
```

[`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md) owns the exact versioned contracts. UI shells
and forms are projections of that catalog, not a second hard-coded capability list.

## DDD Context Map

```text
Workflow bounded context
  crates/engine     graph and Run domain, application use cases, consumer ports
  crates/nodes      exact built-in capabilities plus generation-profile semantics
       |
       | NodeCapabilityProducedMediaWriterPort / NodeCapabilityManagedMediaReaderPort
       v
Asset bounded context
  crates/assets     Asset domain, application use cases, consumer ports, local adapters

Infrastructure boundaries
  crates/backends   profile routers and provider adapters -> node-owned provider ports
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
- `crates/nodes` depends on the engine-owned `WorkflowNodeCapabilityPort`, implements one exact
  capability type per operation, and owns the profile plus media/provider ports those types consume;
- `crates/backends` depends inward on the generation-profile availability port and behavior-named
  node-owned provider ports;
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
| editable graph and revision | Workflow domain | `WorkflowAggregate`, `WorkflowNodeEntity`, `WorkflowInputBindingValue`, `WorkflowInputItemEntity` |
| Run lifecycle and output association | Workflow domain | `WorkflowRunAggregate`, `WorkflowNodeExecutionEntity` |
| shared capability contract invariants | Workflow domain in `engine` | `NodeCapabilityContract` |
| exact parameter, input, result, and execution semantics | capability implementations in `nodes` | `TextToVideoCapability`, `NodeCapabilityParameterSet` |
| stable generation profile identity and capability compatibility | generation-profile domain in `nodes` | `GenerationProfileDefinition`, `GenerationProfileRef` |
| current generation profile availability | provider-availability adapter through a consumer-owned reader port | `GenerationProfileAvailabilityObservation` |
| managed media identity and availability | Asset domain | `AssetAggregate`, `AssetManagedContentState` |
| provider model-operation protocol | provider adapter private code | private provider `*Dto` and `*AdapterError` |
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
| exact capability behavior | `WorkflowNodeCapabilityPort` |
| Workflow preview projection | `WorkflowMediaPreviewIssuerPort` |
| node Asset reads and produced-media writes | `NodeCapabilityManagedMediaReaderPort`, `NodeCapabilityProducedMediaWriterPort` |
| exact model-powered capabilities | `TextToVideoProviderPort`, `TextToSpeechProviderPort`, and the complete capability-owned port set |
| deterministic media transformations | `ImageCropPort`, `VideoFrameExtractionPort`, `VideoConcatenationPort` |
| generation profile availability reads | `GenerationProfileAvailabilityReaderPort` |
| Asset use cases | `AssetAggregateRepositoryPort`, `AssetIngestTransactionPort`, `AssetManagedContentStorePort`, `AssetMediaInspectorPort` |
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

crates/nodes/src/
  generation_profile/{domain,application,ports}/
  text_to_video/{capability,parameters,provider_port}.rs
  video_concatenation/{capability,parameters,media_port}.rs
  <exact_capability>/

crates/assets/src/asset/
  domain/
  application/
  ports/
  infrastructure/

crates/backends/src/
  provider_routing/
  deterministic_provider/
  <provider_name>/

src-tauri/src/
  workflow/
  assets/
  generation_profiles/
  generation_providers/
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
| fixed ports and parameter meaning | exact `WorkflowNodeCapabilityPort` implementation |
| selected provider-independent generation profile | `WorkflowNodeEntity` parameter set |
| profile identity and capability compatibility | generation-profile catalog in `nodes` |
| current profile availability and equivalent routes | composition-built provider routing adapters |
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
  -> WorkflowNodeCapabilityRegistry resolves WorkflowNodeCapabilityPort
  -> WorkflowNodeCapabilityPort::execute runs each ready node
  -> persist aggregate transitions and events
  -> WorkflowRunEventPublisherPort emits projections
```

Provider work starts only after queued intent is durable. Independent branches may execute within a
bounded concurrency limit. Each model-powered node keeps its selected exact
`GenerationProfileRef`; the provider router fixes one equivalent route per dispatch before paid
submission. This architecture defines no cross-run result cache.

## Node-Produced Media Flow

```text
provider or media-operation adapter returns an exact result payload
  -> NodeCapabilityProducedMediaWriterPort
  -> RecordNodeProducedAssetUseCase
  -> persist Pending AssetAggregate and managed-content finalization
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
- capability tests prove every registered parameter, input, result, error, port, and execution contract;
- generation-profile tests prove stable node selection, exact compatibility, availability, and
  provider-independent routing;
- provider port suites run against deterministic and configured capability routers;
- Asset tests prove import/generated flow, Project isolation, resolution, recovery, and preview;
- Desktop tests prove transaction ordering, task hosting, DTO translation, and events;
- contract tests keep Rust DTOs and TypeScript types aligned;
- end-to-end tests exercise every capability family and preview every Workflow output type;
- static checks reject concrete adapter construction outside `composition.rs` and inward dependency
  violations.

The implementation merge gate remains `./scripts/e2e.sh`.

## Architecture Completion Criteria

1. Users can create, move, edit, connect, save, and reopen every registered exact capability.
2. Rust rejects invalid types and cycles while incomplete drafts remain editable.
3. Users can query currently available compatible profiles and select a stable profile on each
   model-powered node.
4. Every provider port and media-operation port has a deterministic contract implementation.
5. Generated and transformed media, including frame sequences, become durable Project Assets.
6. Text, Image, Video, Audio, Image Sequence, and Video Storyboard previews work after execution and
   reopen.
7. Progress, failure, and cancellation are visible through durable Run state and events.
8. No DTO exposes provider state, native model IDs, credentials, managed paths, or persistence Rows.
9. Public Rust types satisfy the glossary naming rules and concrete dependencies are constructor
   injected only at the composition root.

## Deferred Product Areas

Dynamic ports, plugin-supplied capabilities, batch generation, timelines, cross-run cache, remote
task resume, cross-profile fallback, cost accounting, advanced Asset management, collaboration,
cloud sync, 3D, and scene generation require separate designs. Equivalent provider routing within
one exact profile is core behavior.
