# Backend Desktop Application Architecture

> Status: frozen MVP design
> Owner: `src-tauri`
> Scope: Tauri admission, DTOs, cross-context bridges, post-commit effects, protocols, and composition

Desktop is an application host and infrastructure boundary. It owns no Workflow, Asset, Node
Capability, Generation Profile, or Assistant business transition.

## Responsibility

The Desktop host:

- attaches trusted Project and process context to untrusted DTOs;
- invokes one named application use case per command;
- translates domain/application values to boundary DTOs;
- consumes the three closed post-commit effect types;
- bridges consumer-owned interfaces across bounded contexts;
- emits only committed Run events;
- serves short-lived Asset preview access;
- loads configuration and credentials;
- constructs every concrete adapter in one composition root.

Tauri commands are thin entry points, not a second application layer.

## Capability-Oriented Layout

```text
src-tauri/src/
  projects/
    commands.rs       create, rename, get, list, and open
    dto.rs
    translation.rs
    sqlite.rs         Project repository adapter
    workflow_bridge.rs
  workflow/
    commands.rs       source-first Tauri commands
    dto.rs            Workflow request/response DTOs
    translation.rs    DTO/application conversion
    sqlite.rs         Workflow persistence adapters
    events.rs          committed-event publisher adapter
  assets/
    commands.rs
    dto.rs
    translation.rs
    sqlite.rs
    node_bridge.rs     node-owned media interface adapter
    preview.rs         local preview protocol adapter
  generation_profiles/
    commands.rs
    dto.rs
    availability.rs
  generation_providers/
    configuration.rs
    credentials.rs     operating-system credential-vault adapters
  assistant/
    commands.rs
    dto.rs
    translation.rs
    adapters/
  storage/
    sqlite/
    managed_content/
    configuration/
  configuration.rs
  post_commit_effects.rs  closed effect outbox and one worker
  composition.rs       only concrete construction point
  lib.rs               command/event/protocol registration only
```

There are no global `controllers`, `services`, `repositories`, `models`, or `dto` directories.

## Command Admission Pattern

```text
deserialize bounded <Module><Behavior>RequestDto
  -> validate transport shape and limits
  -> resolve ProjectId through ProjectGetUseCase when Project-scoped
  -> attach trusted Project/session/file-handle context
  -> translate to <Module><Behavior>Command or Query
  -> invoke one <Module><Behavior>UseCase method
  -> translate result or structured error once
```

Commands never call SQLite, filesystem, provider route, capability implementation, or Python
handler directly. They never duplicate graph compatibility, parameter normalization, Asset
visibility, profile compatibility, review evidence, or legal transitions.

## Frozen Tauri Surface

| Command | Boundary input | Application target |
| --- | --- | --- |
| `project_create` | `ProjectCreateRequestDto` | `ProjectCreateUseCase` |
| `project_rename` | `ProjectRenameRequestDto` | `ProjectRenameUseCase` |
| `project_get` | `ProjectGetRequestDto` | `ProjectGetUseCase` |
| `project_list` | `ProjectListRequestDto` | `ProjectListUseCase` |
| `project_open` | `ProjectOpenRequestDto` | `ProjectOpenUseCase` |
| `workflow_create` | `WorkflowCreateRequestDto` | `WorkflowCreateUseCase` |
| `workflow_get_current` | `WorkflowGetCurrentRequestDto` | `WorkflowGetCurrentUseCase` |
| `workflow_apply_mutation` | `WorkflowApplyMutationRequestDto` | `WorkflowApplyMutationUseCase` |
| `workflow_check_readiness` | `WorkflowCheckReadinessRequestDto` | `WorkflowCheckReadinessUseCase` |
| `workflow_start_run` | `WorkflowStartRunRequestDto` | `WorkflowStartRunUseCase` |
| `workflow_cancel_run` | `WorkflowCancelRunRequestDto` | `WorkflowCancelRunUseCase` |
| `workflow_get_run` | `WorkflowGetRunRequestDto` | `WorkflowGetRunUseCase` |
| `workflow_list_run_events` | `WorkflowListRunEventsRequestDto` | `WorkflowListRunEventsUseCase` |
| `workflow_get_node_presentation` | `WorkflowGetNodePresentationRequestDto` | `WorkflowGetNodePresentationUseCase` |
| `node_capability_list` | `NodeCapabilityListRequestDto` | `NodeCapabilityListUseCase` |
| `generation_profile_list_for_capability` | `GenerationProfileListForCapabilityRequestDto` | `GenerationProfileListForCapabilityUseCase` |
| `asset_import` | `AssetImportRequestDto` | `AssetImportUseCase` |
| `asset_get` | `AssetGetRequestDto` | `AssetGetUseCase` |
| `asset_list` | `AssetListRequestDto` | `AssetListUseCase` |
| `asset_issue_preview` | `AssetIssuePreviewRequestDto` | `AssetIssuePreviewUseCase` |
| `assistant_send_message` | `AssistantSendMessageRequestDto` | `AssistantSendMessageUseCase` |
| `assistant_get_pending_workflow_change` | `AssistantGetPendingWorkflowChangeRequestDto` | `AssistantGetPendingWorkflowChangeUseCase` |
| `assistant_decide_workflow_change` | `AssistantDecideWorkflowChangeRequestDto` | `AssistantDecideWorkflowChangeUseCase` |

`WorkflowStartRunRequestDto` contains one closed `WorkflowRunScopeDto` (`WholeWorkflow` or
`ThroughNode`). There is no duplicate through-node command. It returns a durable queued
`WorkflowRunDto` before external work begins.

Mutating requests carry stable request IDs. DTO validation checks shape and transport bounds;
domain owners validate meaning and repositories enforce idempotency receipts.

## Project Boundary

`ProjectId` comes from `crates/projects`. Project-scoped DTO references are untrusted until
`ProjectGetUseCase` resolves them. The Desktop boundary then passes the resolved ID to exactly one
target use case; there is no process-global active Project.

`ProjectOpenUseCase` calls `ProjectWorkflowSummaryReaderInterface`, implemented by
`DesktopProjectWorkflowBridgeAdapterImpl` over `WorkflowGetCurrentUseCase`. The bridge returns only a
Project-owned summary. If none exists, `workflow_create` creates the Project's single current
Workflow; Workflow persistence atomically rejects a second one.

## Workflow Editing Boundary

```text
WorkflowApplyMutationRequestDto
  -> WorkflowApplyMutationCommand + trusted Project scope
  -> WorkflowApplyMutationUseCase::apply_workflow_mutation
  -> WorkflowAggregate transition
  -> WorkflowAggregateRepositoryInterface::commit_workflow_mutation
  -> WorkflowDto + WorkflowReadinessIssueDto values
```

React submits a closed action list, never its entire editor store. The same canonical use case is
used by `DesktopAssistantWorkflowBridgeAdapterImpl` after approval. Canvas position persists, while
selection, viewport, dragging, menus, previews, and playback remain React state.

## Run Coordination

`WorkflowStartRunUseCase` atomically persists the queued Run and one `WorkflowExecuteRunEffect`,
then returns. `DesktopPostCommitEffectWorker` consumes that effect and calls
`WorkflowExecuteRunUseCase`.

The worker owns task handles, cancellation signals, effect delivery, and one configured concurrency
limit. It owns no business state, generic queue, or status setter. Startup converts non-terminal
Runs to `InterruptedByRestart` and abandons their unsafe Run effects.

`WorkflowExecuteRunUseCase` resolves exact implementations from the injected
`WorkflowNodeCapabilityRegistry`. Provider/filesystem calls occur outside SQLite transactions.

`WorkflowCancelRunUseCase` commits cancellation intent before the worker signals active tokens. A
late node output is accepted only if the Run aggregate still permits its transition.

## Node-To-Asset Bridge

`DesktopNodeCapabilityAssetBridgeAdapterImpl` implements both node-consumer interfaces:

```text
NodeCapabilityManagedMediaReaderInterface::read_managed_media
  -> AssetResolveContentUseCase::resolve_asset_content
  -> typed Workflow managed-media input

NodeCapabilityProducedMediaWriterInterface::write_node_output_media
  -> AssetRecordNodeOutputUseCase::record_asset_node_output
  -> AssetFinalizeContentUseCase::finalize_asset_content after the effect commit
  -> Available AssetAggregate
  -> typed Workflow managed-media output
```

The bridge translates Project, kind, provenance, Generation Profile ref, and source Asset IDs. It
converts `NodeCapabilityProducedMediaOutputKey` into the Asset-owned `AssetNodeOutputKey` and
exposes no Asset repository, row, path, or preview lease to node code.

`DesktopWorkflowMediaPreviewAdapterImpl` separately implements `WorkflowMediaPreviewIssuerInterface` over
`AssetIssuePreviewUseCase`. Asset application types never enter `crates/engine`.

## Assistant Bridges

`DesktopAssistantWorkflowBridgeAdapterImpl` implements Assistant-owned read/evaluate/apply/Run interfaces by
calling canonical Workflow use cases. `DesktopAssistantWorkspaceBridgeAdapterImpl` composes bounded
Workflow, Asset, capability, profile, and Run projections through their public queries.

The Assistant sidecar adapter receives only Rust-generated tool schemas and trusted invocation
context. Tool calls return to typed Rust handlers. Python never receives a repository, path, raw
credential, canonical mutation command, or direct Run-start operation.

After exact approval:

```text
AssistantDecideWorkflowChangeUseCase
  -> AssistantWorkflowMutationApplierInterface
  -> WorkflowApplyMutationUseCase
  -> AssistantWorkflowRunStarterInterface
  -> WorkflowStartRunUseCase
  -> WorkflowExecuteRunEffect
  -> DesktopPostCommitEffectWorker
```

This is the only Assistant execution path. Repair begins from committed Workflow Run facts and
re-enters the same candidate/review/approval chain.

## Durable State Before Effects

The Desktop host enforces these orderings:

1. Workflow snapshot and mutation receipt before returning the mutation result.
2. Queued Run, node executions, event, request receipt, and Run effect in one transaction.
3. Node/Run transition and event before Tauri emission.
4. Pending Asset, finalization, and Asset effect before managed-byte publication.
5. Assistant approval decision and Assistant effect before canonical apply/resume.

SQLite and filesystem/provider/sidecar work are never described as one transaction. Recovery uses
idempotency receipts, Pending Asset finalization, durable Run events, and conservative interruption.

The outbox is a closed boundary union, not a job framework:

```rust
pub enum DesktopPostCommitEffect {
    Workflow(WorkflowExecuteRunEffect),
    Asset(AssetFinalizeContentEffect),
    Assistant(AssistantApplyWorkflowChangeEffect),
}
```

Workflow Run effects are abandoned after restart if their Run was non-terminal. Asset effects are
idempotently replayed by exact finalization ID. Assistant effects are replayed through mutation and
Run request receipts. No arbitrary kind, payload, handler registration, or public task API exists.

Asset import and node-output use cases claim their just-committed finalization immediately and call
`AssetFinalizeContentUseCase`. The worker handles only an unfinished or recovered Asset effect.
This keeps publication after commit without making an executing Run wait on its own worker slot.

## Node Presentation And Preview

`WorkflowGetNodePresentationUseCase` creates `WorkflowNodePresentationView` from the canonical node,
contract, readiness, latest relevant execution/output, and optional preview.

`WorkflowNodePresentationDto` has only the four MVP variants: Text, Image, Video, and Audio. It is a
projection and is never valid input to `workflow_apply_mutation`.

`DesktopAssetPreviewProtocolAdapterImpl` validates each `AssetPreviewLease`, signature, expiry, Project,
current Asset state, and descriptor. Video and Audio support one bounded Range. Managed paths never
leave the adapter. React owns rendering and playback state.

## Event Delivery

Committed `WorkflowRunEvent` rows are their own delivery outbox. The post-commit worker passes only
undispatched rows to `TauriWorkflowRunEventPublisherAdapterImpl`, then records the delivery attempt.

```text
workflow_run_id, sequence, workflow_node_id?, event_kind,
progress?, structured_error?, occurred_at
```

Sequence is monotonic per Run. React deduplicates and repairs a gap through
`workflow_list_run_events(after_sequence, limit)`. Progress may be coalesced at the projection
boundary; state transitions and terminal errors remain durable and queryable.

## Composition Root

`DesktopCompositionRoot` in `composition.rs` is the only code that names concrete adapters:

```text
validate DesktopBackendConfig
  -> open/migrate SQLite and managed-content roots
  -> connect generation-provider and Assistant OS credential vault adapters
  -> construct Project, Workflow, Asset, and Assistant repositories
  -> construct Project use cases and DesktopProjectWorkflowBridgeAdapterImpl
  -> construct Asset storage/inspection use cases
  -> construct node/Asset and Workflow-preview bridges
  -> construct the frozen Generation Profile catalog
  -> construct deterministic/configured provider routes and three exact routers
  -> construct profile availability reader
  -> construct exactly seven Node Capability implementations and one registry
  -> construct Workflow use cases and DesktopPostCommitEffectWorker
  -> construct Assistant aggregates/use cases, Workflow bridges, and model runner adapter
  -> reconcile Pending Assets and interrupt non-terminal Runs
  -> register commands, post-commit effects, sidecar transport, and preview protocol
```

`DesktopApplicationHost` contains typed, already-constructed command dependencies. It is not a
service locator and is never passed into business code. Tests build the same graph with deterministic
adapters without starting Tauri.

## Configuration And Credentials

`DesktopBackendConfig` contains non-secret locations, bounds, concurrency, provider route entries,
profile mappings, credential IDs, availability/polling limits, preview expiry, Assistant model
selection, and protocol budgets. It does not contain API keys.

Configuration is validated once at startup. Missing provider credentials make only affected
Generation Profiles unavailable. Missing Assistant credentials disable only Assistant commands.

`GenerationProviderCredentialVaultInterface` and `AssistantModelCredentialVaultInterface` are separate
consumer-owned interfaces even when one OS adapter implements both. Production secrets live in the
operating-system credential store. Plaintext is call-scoped and never enters SQLite, config, DTOs,
errors, or logs.

## Representation Boundaries

Named translators keep models separate:

```text
WorkflowApplyMutationRequestDto -> WorkflowApplyMutationCommand
ProjectAggregate                 -> ProjectDto
ProjectWorkspaceView             -> ProjectWorkspaceDto
WorkflowRunAggregate            -> WorkflowRunDto
WorkflowRunEvent                -> WorkflowRunEventDto
NodeCapabilityContract          -> NodeCapabilityContractDto
GenerationProfileAvailabilityObservation -> GenerationProfileAvailabilityDto
AssetAggregate                  -> AssetDto
AssistantWorkflowChangeAggregate -> AssistantPendingWorkflowChangeDto

SqliteWorkflowRow               -> WorkflowAggregate
SqliteProjectRow                -> ProjectAggregate
SqliteWorkflowRunRow + children -> WorkflowRunAggregate
SqliteAssetRow                  -> AssetAggregate
SqliteAssistantWorkflowChangeRow -> AssistantWorkflowChangeAggregate
```

A persistence row, provider DTO, SDK state, credential, path, route ID, or provider task ID is never
returned to React.

## Error Translation

Tauri translates a structured context error once into `DesktopErrorDto`:

```text
{ code, message, retryable, retry_after?, target?, details?, correlation_id? }
```

Structured fields are contractual; `message` is safe presentation text. Unknown failures use an
internal code and correlation ID. Logs include stable typed IDs and exclude secrets, model prompts,
provider bodies, signed URLs, opaque SDK state, and unnecessary paths.

## Verification

- command tests cover DTO bounds, trusted context, source-first routing, and error translation;
- Project command/bridge tests cover create, rename, list, open, isolation, and one current Workflow;
- use-case tests use fake interfaces without Tauri;
- transaction tests prove every durable-before-effect ordering;
- post-commit worker tests cover the three effect types, concurrency, cancellation, and restart policy;
- bridge tests prove exact cross-context translation without copied semantics;
- event tests cover sequence, emission failure, duplicate/gap repair, and terminal query;
- preview tests cover Project isolation, expiry, MIME, Range, and path non-disclosure;
- composition tests assert exactly seven active capabilities and three exact provider routers;
- Assistant E2E proves proposal -> review -> approval -> canonical apply -> canonical Run -> repair;
- contract fixtures prove Rust, Python, and TypeScript DTO/schema alignment.

## Post-MVP

New roadmap capabilities, Project archive/delete/duplicate, remote task resume, server/background
workers, provider choice, dynamic plugins, durable backend undo/history, cross-Run cache, advanced
Asset lifecycle, multi-device Assistant coordination, cloud sync, 3D, and scenes remain outside the
frozen Desktop surface.
