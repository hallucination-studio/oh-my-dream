# Backend MVP Desktop Application

> Status: proposed MVP design
> Owner: `src-tauri`
> Scope: Tauri boundary, task hosting, preview protocol, and composition root

Naming follows [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md). Desktop is an application host and
infrastructure boundary, not a semantic owner of Workflow or Asset rules.

## Responsibility

The Desktop host exposes Rust-authoritative behavior to React. It owns Tauri command admission,
trusted Project context, DTO translation, process-owned Run tasks, event delivery, preview streaming,
configuration loading, and concrete adapter construction.

Workflow and Asset application use cases live with their bounded contexts. Tauri commands invoke
those use cases; they do not become a second application layer.

## Capability-Oriented Structure

```text
src-tauri/src/
  workflow/
    commands.rs       Tauri entry points
    dto.rs            request and response DTOs
    translation.rs    DTO/application conversion
    sqlite.rs         Workflow persistence adapters
    task_host.rs       process-owned Run task hosting
    events.rs          Tauri event adapter
  assets/
    commands.rs
    dto.rs
    translation.rs
    sqlite.rs
    node_bridge.rs     node-owned media port adapter
    preview.rs         local protocol adapter
  providers/
    configuration.rs
  assistant/           existing boundary, unchanged
  configuration.rs     validated Desktop MVP configuration
  composition.rs       only concrete adapter construction point
  lib.rs               command, event, and protocol registration
```

The host is grouped by business capability, not global controller/service/repository/DTO folders.

## Tauri Command Pattern

```text
deserialize bounded *RequestDto
  -> attach trusted Project and ApplicationRequestId
  -> translate to *Command or *Query
  -> invoke one *UseCase
  -> translate *Result, *View, or structured error to *Dto
```

Commands never call SQLite, the filesystem, a provider client, or a node executor directly. They do
not duplicate graph compatibility, parameter normalization, Asset visibility, or legal transitions.

## MVP Command Surface

| Tauri command | Boundary input | Application target |
| --- | --- | --- |
| `create_workflow` | `CreateWorkflowRequestDto` | `CreateWorkflowUseCase` |
| `get_workflow` | `GetWorkflowRequestDto` | `GetWorkflowUseCase` |
| `apply_workflow_mutation` | `ApplyWorkflowMutationRequestDto` | `ApplyWorkflowMutationUseCase` |
| `validate_workflow_readiness` | `ValidateWorkflowReadinessRequestDto` | `ValidateWorkflowReadinessUseCase` |
| `list_node_capabilities` | `ListNodeCapabilitiesRequestDto` | `ListNodeCapabilitiesUseCase` |
| `get_node_capability` | `GetNodeCapabilityRequestDto` | `GetNodeCapabilityUseCase` |
| `start_workflow_run` | `StartWorkflowRunRequestDto` | `StartWorkflowRunUseCase` |
| `start_workflow_node_run` | `StartWorkflowNodeRunRequestDto` | `StartWorkflowRunUseCase` |
| `cancel_workflow_run` | `CancelWorkflowRunRequestDto` | `CancelWorkflowRunUseCase` |
| `get_workflow_run` | `GetWorkflowRunRequestDto` | `GetWorkflowRunUseCase` |
| `get_workflow_run_events` | `GetWorkflowRunEventsRequestDto` | `GetWorkflowRunEventsUseCase` |
| `get_workflow_node_presentation` | `GetWorkflowNodePresentationRequestDto` | `GetWorkflowNodePresentationUseCase` |
| `import_asset` | `ImportAssetRequestDto` | `ImportAssetUseCase` |
| `get_asset` | `GetAssetRequestDto` | `GetAssetUseCase` |
| `list_assets` | `ListAssetsRequestDto` | `ListAssetsUseCase` |
| `issue_asset_preview` | `IssueAssetPreviewRequestDto` | `IssueAssetPreviewUseCase` |

`start_workflow_run` maps to `WorkflowRunScope::WholeWorkflow`; `start_workflow_node_run` maps to
`WorkflowRunScope::ThroughNode`. Both return a durable queued `WorkflowRunDto` before provider work
finishes.

## Workflow Editing Boundary

`ApplyWorkflowMutationUseCase` receives `ApplyWorkflowMutationCommand`, loads one
`WorkflowAggregate`, invokes its transition, and atomically stores the new revision plus idempotency
receipt. Revision conflict and request replay conflict remain distinct.

React submits a closed operation list rather than its complete editor state. The same use case is
available to approved Assistant edits. Node canvas position persists, while selection, viewport,
drag, menus, preview URLs, and playback remain client state.

## Run Coordination

`StartWorkflowRunUseCase` performs the admission transaction:

```text
load exact WorkflowAggregate revision
  -> validate readiness and build WorkflowExecutionPlanValue
  -> commit queued WorkflowRunAggregate + request receipt + first event
  -> return StartWorkflowRunResult
```

After commit, `DesktopWorkflowRunTaskHost` starts `ExecuteWorkflowRunUseCase` in a process-owned
task. That use case advances `WorkflowRunAggregate` and `WorkflowNodeExecutionEntity` through domain
methods, persists each transition/event, and dispatches prepared nodes through
`NodeCapabilityExecutorPort`.

Independent branches may execute concurrently within one configured limit. The frozen plan, not
task timing, determines input/output association. No database transaction remains open during a
provider call.

`CancelWorkflowRunUseCase` records cancellation before the task host signals active tokens. Late
outputs are rejected when cancellation wins. On startup, the task host converts previously Running
MVP Runs to a structured interrupted failure; remote provider task resume is deferred.

## Node-To-Asset Bridge

`DesktopNodeAssetBridgeAdapter` implements both node-owned media ports:

```text
NodeCapabilityManagedMediaReaderPort
  -> ResolveAssetContentUseCase
  -> Workflow managed-media reference

NodeCapabilityGeneratedMediaWriterPort
  -> RecordGeneratedAssetUseCase
  -> Available AssetAggregate
  -> Workflow managed-media reference
```

The adapter translates Project identity and generated origin explicitly. It never gives node code an
Asset repository, SQLite connection, path, or preview URL.

`DesktopWorkflowMediaPreviewAdapter` separately implements `WorkflowMediaPreviewIssuerPort` over
`IssueAssetPreviewUseCase`. It translates `AssetPreviewLease` into a Workflow-owned opaque preview
handle; no Asset application type enters `crates/engine`.

## Durable State Before External Effects

The MVP has two required orderings:

1. `WorkflowRunAggregate` is durably Queued before provider dispatch.
2. `AssetAggregate` and its finalize job are durably Pending before managed content is published.

SQLite and filesystem/network work are not presented as one transaction. Asset finalization is
idempotent and bounded startup recovery processes Pending work. A node succeeds only after the Asset
bridge returns an available managed reference.

## Node Presentation

`GetWorkflowNodePresentationUseCase` assembles `WorkflowNodePresentationView` from:

```text
WorkflowNodeEntity + NodeCapabilityContract
  + WorkflowReadinessIssueValue values
  + latest WorkflowNodeExecutionEntity
  + latest WorkflowNodeOutputSet
  + optional WorkflowMediaPreviewValue
```

`WorkflowNodePresentationDto` has Text, Image, Video, and Audio variants. It is a projection, not a
second domain node and never valid input to `apply_workflow_mutation`.

## Preview Protocol

`DesktopAssetPreviewProtocolAdapter` resolves `AssetPreviewLease` for each request, rechecks Project
scope and expiry, and streams verified MIME. Video and audio support one valid byte Range. Managed
paths never leave the adapter.

React owns image zoom, video/audio controls, playback position, volume, and object URL lifetime.

## Event Delivery

`TauriWorkflowRunEventPublisherAdapter` implements `WorkflowRunEventPublisherPort`. It emits
`WorkflowRunEventDto` only after the event record is committed.

```text
workflow_run_id, sequence, workflow_node_id?, event_kind,
progress?, structured_error?, occurred_at
```

Sequence is monotonic per Workflow Run. React deduplicates and repairs a gap through
`get_workflow_run_events(after_sequence, limit)`. Terminal state remains queryable through `get_workflow_run`.
Progress may be coalesced; state transitions and terminal errors remain durable.

## Composition Root

`DesktopCompositionRoot` in `composition.rs` is the only place that names concrete adapters:

```text
load and validate DesktopBackendConfig
  -> construct SQLite Workflow and Asset repository adapters
  -> construct filesystem content and media-inspection adapters
  -> construct Asset use cases
  -> construct DesktopNodeAssetBridgeAdapter and DesktopWorkflowMediaPreviewAdapter
  -> construct mock or configured provider adapters
  -> construct seven node capability executors and catalog
  -> construct Workflow use cases and DesktopWorkflowRunTaskHost
  -> register commands, event adapter, and preview protocol adapter
```

`DesktopApplicationHost` contains already-constructed use cases and task hosts required by Tauri
commands. It is not a service locator: commands access typed fields and business code never receives
`DesktopApplicationHost`. Tests construct the same dependency graph with deterministic adapters
without starting Tauri.

## Constructor Injection Rules

- every `*UseCase` receives focused `*Port` dependencies in `new`;
- every `*Adapter` receives validated configuration and lower-level boundary dependencies in `new`;
- Project, request, deadline, cancellation, and Run identity are call-scoped inputs;
- no mutable global, runtime adapter lookup, downcast, or concrete type appears in business code;
- no optional port method represents unsupported behavior;
- deterministic and production adapters run the same port contract suites.

## Configuration

`DesktopBackendConfig` selects managed content location and limits, one adapter/model per generation
capability, provider deadlines and polling bounds, global Run concurrency, and preview lease expiry.

Configuration is validated once at startup. Missing provider wiring makes only its related
capability unavailable. Credentials use redacted values and never enter DTOs or logs. Nodes cannot
override endpoint, model, or credential settings.

## Representation Boundaries

Named translations keep layers separate:

```text
ApplyWorkflowMutationRequestDto -> ApplyWorkflowMutationCommand
WorkflowAggregate              -> WorkflowDto
WorkflowRunAggregate           -> WorkflowRunDto
WorkflowRunEvent               -> WorkflowRunEventDto
NodeCapabilityContract         -> NodeCapabilityContractDto
AssetAggregate                 -> AssetDto
WorkflowNodePresentationView           -> WorkflowNodePresentationDto
SqliteWorkflowRow              -> WorkflowAggregate
SqliteAssetRow                 -> AssetAggregate
```

DTO validation checks shape and transport bounds. Domain owners enforce business semantics. A Row,
provider DTO, path, storage key, credential, or provider task ID is never returned to React.

## Assistant Boundary

The existing Python Assistant remains unchanged. It continues to use the Rust-authoritative
capability catalog, Workflow mutation/review, and Run commands. Assistant redesign is not required
for the four-node MVP.

## Error Translation

Tauri translates structured context errors once into `DesktopErrorDto`:

```text
{ code, message, retryable, retry_after?, target?, details?, correlation_id? }
```

Structured fields are contractual; `message` is safe presentation text. Unknown failures use an
internal code and correlation ID. Logs include stable typed IDs where relevant and exclude secrets,
provider bodies, signed URLs, and unnecessary paths.

## Verification

- command tests cover DTO bounds, trusted Project context, and error translation;
- use-case tests use fake ports without Tauri;
- transaction tests prove queued Run before dispatch and Pending Asset before finalization;
- task-host tests cover scheduling, concurrency, cancellation, and interrupted startup;
- event tests cover sequence, duplicate/gap recovery, progress, and terminal state;
- preview tests cover Project isolation, expiry, MIME, Range, and path non-disclosure;
- composition tests prove seven contracts exist and runnable contracts have complete executors;
- end-to-end tests run both required graph branches and preview all four shells.

## Post-MVP

Separate layout revisions, durable backend undo, remote task resume, worker leases, multiple provider
profiles, per-node provider selection, advanced Asset management, multiview, caching, and batches are
deferred. 3D and scene use cases are not product scope.
