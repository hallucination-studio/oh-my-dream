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
  generation_profiles/
    commands.rs
    dto.rs
    translation.rs
  generation_providers/
    configuration.rs
    credentials.rs      encrypted SQLite provider credential adapter
  storage/
    sqlite/           connection and migration mechanics
    configuration/    non-secret config-file adapter
  assistant/           existing boundary, unchanged
  configuration.rs     validated Desktop MVP configuration
  composition.rs       only concrete adapter construction point
  lib.rs               command, event, and protocol registration
```

The host is grouped by business capability, not global controller/service/repository/DTO folders.

## Tauri Command Pattern

```text
deserialize bounded *RequestDto
  -> attach trusted Project context
  -> translate to *Command or *Query
  -> invoke one *UseCase
  -> translate *Result, *View, or structured error to *Dto
```

Commands never call SQLite, the filesystem, a provider route, or a capability implementation directly. They do
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
| `list_node_capability_generation_profiles` | `ListNodeCapabilityGenerationProfilesRequestDto` | `ListNodeCapabilityGenerationProfilesUseCase` |
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
`WorkflowAggregate`, invokes its transition, and stores the new snapshot through revision
compare-and-swap. A revision conflict never overwrites newer state.

React submits a closed operation list rather than its complete editor state. The same use case is
available to approved Assistant edits. Node canvas position persists, while selection, viewport,
drag, menus, preview URLs, and playback remain client state.

## Run Coordination

`StartWorkflowRunUseCase` performs the admission transaction:

```text
load current WorkflowAggregate and verify the requested revision
  -> validate readiness and build WorkflowExecutionPlanValue
  -> commit queued WorkflowRunAggregate + node executions + first event
  -> return StartWorkflowRunResult
```

After commit, `DesktopWorkflowRunTaskHost` starts `ExecuteWorkflowRunUseCase` in a process-owned
task. That use case advances `WorkflowRunAggregate` and `WorkflowNodeExecutionEntity` through domain
methods, persists each transition/event, resolves the exact implementation through
`WorkflowNodeCapabilityRegistry`, and calls `WorkflowNodeCapabilityPort::execute`.

Independent branches may execute concurrently within one configured limit. The frozen plan, not
task timing, determines input/output association. No database transaction remains open during a
provider call.

`CancelWorkflowRunUseCase` records cancellation before the task host signals active tokens. Late
outputs are rejected when cancellation wins. On startup, the task host converts every non-terminal
MVP Run to a structured interrupted failure; queued work and remote provider tasks are not resumed.

## Node-To-Asset Bridge

`DesktopNodeAssetBridgeAdapter` implements both node-owned media ports:

```text
NodeCapabilityManagedMediaReaderPort
  -> ResolveAssetContentUseCase
  -> Workflow managed-media reference

NodeCapabilityProducedMediaWriterPort
  -> RecordNodeProducedAssetUseCase
  -> Available AssetAggregate
  -> Workflow managed-media reference
```

The adapter translates Project identity and produced-media origin explicitly. It never gives node code an
Asset repository, SQLite connection, path, or preview URL.

`DesktopWorkflowMediaPreviewAdapter` separately implements `WorkflowMediaPreviewIssuerPort` over
`IssueAssetPreviewUseCase`. It translates `AssetPreviewLease` into a Workflow-owned opaque preview
handle; no Asset application type enters `crates/engine`.

## Durable State Before External Effects

The MVP has two required orderings:

1. `WorkflowRunAggregate` is durably Queued before provider dispatch.
2. `AssetAggregate` and its managed-content finalization are durably Pending before content is
   published.

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
  -> open, verify, and migrate storage
  -> construct SqliteEncryptedProviderCredentialRepositoryAdapter
  -> load and decrypt configured provider credentials
  -> construct SQLite Workflow and Asset repository adapters
  -> construct filesystem content and media-inspection adapters
  -> construct Asset use cases
  -> construct DesktopNodeAssetBridgeAdapter and DesktopWorkflowMediaPreviewAdapter
  -> construct the generation-profile catalog
  -> construct deterministic or configured provider routes and routers
  -> construct generation-profile availability and query adapters
  -> construct exact WorkflowNodeCapabilityPort implementations and one registry
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

`DesktopBackendConfig` selects managed content location and limits, enabled provider accounts,
regions, route policy, provider deadlines, polling and availability-probe bounds, global Run
concurrency, and preview lease expiry. The composition root may register multiple equivalent
routes for one exact generation profile; configuration does not select one global model per
capability.

Configuration is validated once at startup. Missing provider wiring makes only the affected
generation profiles currently unavailable; it does not remove capability or profile definitions.
`DesktopBackendConfig` connects each `ProviderAccountId` to a `ProviderCredentialId`; it contains no
API key.
`DesktopProviderCredentialRepositoryPort` reads and writes authenticated ciphertext in SQLite.
Plaintext credentials exist only as short-lived `ProviderCredentialSecretValue` values and never
enter DTOs or logs. Nodes persist an exact provider-independent `GenerationProfileRef` and cannot
override endpoints, credentials, providers, native model IDs, or route priority.

## Representation Boundaries

Named translations keep layers separate:

```text
ApplyWorkflowMutationRequestDto -> ApplyWorkflowMutationCommand
WorkflowAggregate              -> WorkflowDto
WorkflowRunAggregate           -> WorkflowRunDto
WorkflowRunEvent               -> WorkflowRunEventDto
NodeCapabilityContract         -> NodeCapabilityContractDto
NodeCapabilityGenerationProfileView -> NodeCapabilityGenerationProfileDto
GenerationProfileAvailabilityObservation -> GenerationProfileAvailabilityDto
AssetAggregate                 -> AssetDto
WorkflowNodePresentationView   -> WorkflowNodePresentationDto
SqliteWorkflowAggregateRow               -> WorkflowAggregate
SqliteWorkflowRunAggregateRow + children -> WorkflowRunAggregate
SqliteAssetAggregateRow                  -> AssetAggregate
SqliteProviderCredentialRow              -> ProviderCredentialSecretValue
```

DTO validation checks shape and transport bounds. Domain owners enforce business semantics. A Row,
provider DTO, path, storage key, credential, or provider task ID is never returned to React.

Complete storage topology and lifecycle are defined in
[`BACKEND_STORAGE.md`](BACKEND_STORAGE.md).

## Assistant Boundary

The existing Python Assistant remains a catalog consumer. It uses the Rust-authoritative capability
catalog, Workflow mutation/review, and Run commands; it does not maintain an Assistant-specific
operation or provider list.

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
- composition tests prove every registered contract has one complete capability implementation;
- generation-profile query tests prove compatibility, live availability, pagination, expiry, and
  provider detail redaction;
- end-to-end tests exercise every capability family and preview every Workflow output type.

## Post-MVP

Separate layout revisions, durable backend undo, remote task resume, worker leases, user-defined
provider accounts, explicit provider choice, advanced Asset management, multiview, caching, and
batches are deferred. Per-node generation-profile selection is core behavior. 3D and scene use
cases are not product scope.
