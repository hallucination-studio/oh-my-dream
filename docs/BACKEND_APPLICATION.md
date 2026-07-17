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
- consumes the three closed Desktop post-commit effect types and the four closed Generation Task
  effect types through separate workers;
- bridges consumer-owned interfaces across bounded contexts;
- emits only committed Run events;
- serves short-lived Asset preview access;
- loads configuration and credentials;
- constructs every concrete adapter in one composition root.

Tauri commands are thin entry points, not a second application layer.

Code remains capability-oriented under `src-tauri`; commands, DTOs, translators, and adapters stay
inside their owning Project, Workflow, Generation Task, Asset, Generation Profile, provider, or
Assistant module. Composition, configuration, and the closed effect workers are the only shared Desktop modules. There
are no global `controllers`, `services`, `repositories`, `models`, or `dto` directories.

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
| `generation_provider_settings_get` | `GenerationProviderSettingsGetRequestDto` | `GenerationProviderSettingsGetUseCase` |
| `generation_provider_settings_apply` | `GenerationProviderSettingsApplyRequestDto` | `GenerationProviderSettingsApplyUseCase` |
| `generation_task_get` | `GenerationTaskGetRequestDto` | `GenerationTaskGetUseCase` |
| `generation_task_list` | `GenerationTaskListRequestDto` | `GenerationTaskListUseCase` |
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
Project-owned summary from the returned same-snapshot Workflow and readiness issues. It performs no
second Workflow load or readiness call. If none exists, `workflow_create` creates the Project's single current
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

The worker owns immediate capability handles, cancellation signals, effect delivery, and one
configured concurrency limit. It owns no business state, generic queue, or status setter.
Provider-backed capabilities durably hand off to Generation Task and return a waiting outcome.
Startup preserves Runs waiting on authoritative tasks and converts only unsafe non-terminal Runs to
`InterruptedByRestart`.

`WorkflowExecuteRunUseCase` resolves exact implementations from the injected
`WorkflowNodeCapabilityRegistry`. Provider/filesystem calls occur outside SQLite transactions.

`WorkflowCancelRunUseCase` commits cancellation intent before the worker signals active tokens. A
late node output is accepted only if the Run aggregate still permits its transition.

## Generation Task Coordination

`GenerationProviderSettingsGetUseCase` joins the frozen Generation Profile catalog, persisted
provider selection, credential presence, and safe `GenerationProviderContract` projections. For
each `(profile, generation kind)` pair it returns only provider choices whose matching focused
contract contains at least one route explicitly compatible with that exact profile. Each choice contains
the safe provider identity and its non-empty compatible route choices.
`GenerationProviderSettingsApplyUseCase` rejects a
`(profile_ref, generation_kind, provider_id, route_id)` selection
unless that exact tuple exists in the contract projection. React renders exactly those choices; a
provider without Voice, or without a Voice route compatible with the selected profile, cannot be
selected and no failed call is needed to discover that fact.

`GenerationProviderSettingsProfileDto` contains one `(profile, generationKind)` pair, its optional
sanitized selected binding, and `providerChoices`. Each
`GenerationProviderSettingsProviderChoiceDto` contains only provider ID/display name and a
non-empty `routes` list; each route choice contains only route ID and display name. The use case
filters before DTO construction, orders profiles by profile ref, providers by provider ID, and
routes by route ID, and rejects duplicate `(provider_id, kind, route_id)` keys. Consequently an
unrelated capability can never bring a provider back into the current profile's choice list.

`GenerationProviderSettingsDto` contains non-zero `settingsRevision` plus the ordered profile-kind
items. `GenerationProviderSettingsApplyRequestDto` contains `expectedSettingsRevision` and exactly
one closed action:

```text
SetBinding {
  profileRef, generationKind, providerId, routeId
}
RemoveBinding {
  profileRef, generationKind
}
```

Apply compare-and-swaps the config revision, commits the binding change, and returns the complete
Settings DTO with the new revision. A stale revision returns `CONFLICT`; the UI reloads and asks the
user to retry. Reapplying the already-current binding is a normal no-op result with no revision
increment. The Mock MVP accepts no account or credential input. Production-provider account and
credential mutation requires a separately reviewed Settings contract when such a provider is
activated.

`GenerationProviderSettingsRepositoryInterface`, owned by this Settings application capability,
has exactly `load_generation_provider_settings_snapshot` and
`apply_generation_provider_settings_mutation`. The latter accepts only the validated closed
mutation and expected revision and returns `Committed | Unchanged | RevisionConflict`. Its SQLite
adapter owns the config transaction; neither the use case nor a provider receives a connection.

Settings DTOs expose no trait object, endpoint, native model, route implementation, remote task ID,
secret, or `supports_*` boolean. The capability/route set is a safe contract projection used for
selection, not an optional execution method.

The Node palette keeps `NodeCapabilityListUseCase` as the authoritative registry read. For each
model-powered capability, React also consumes its authoritative
`generation_profile_list_for_capability` result. A non-empty compatible profile list proves
structural support and keeps the node in the add menu. If every observation is `Unavailable` or
`Indeterminate`, the node remains visible but disabled and presents the structured readiness
reason. Only an empty compatible list or a provider registry with no matching focused interface
makes the node absent. This presentation rule does not mutate the registry or reimplement profile
compatibility. Existing saved nodes always remain visible and explain their readiness issue.

The requesting Node Capability determines the generation kind; a profile may be compatible with
more than one capability, so Settings persists kind as part of its mapping key. Kind is not an
independent UI choice on a Workflow node and remains owned by the exact Task request at admission.
Provider construction rejects route ID reuse across focused capabilities.

`DesktopNodeCapabilityGenerationTaskStarterAdapterImpl` implements the node-owned task-start
interface over `GenerationTaskStartUseCase`. It translates the exact execution origin and semantic
request but does not expose a repository or provider adapter to node code.

`GenerationTaskEffectWorkerImpl` consumes only `SubmitTask`, `PollTask`, `CancelRemoteTask`, and
`NotifyWorkflow` from `generation_task_outbox`. It claims serially and executes on the bounded
in-flight pool sized by `generation_task_effect_concurrency`, with at most one in-flight effect per
task, as frozen in `BACKEND_TASK.md`. Submit/poll/cancel calls occur outside SQLite
transactions. Each result is committed with optimistic revision and the current effect consumed or
rescheduled atomically. Delayed polls and startup claim reset are task-delivery semantics, not Desktop
post-commit effects or a generic scheduler.

`DesktopGenerationTaskAssetSinkAdapterImpl` imports validated terminal media through canonical
Asset use cases. `DesktopGenerationTaskWorkflowCompletionAdapterImpl` invokes the Workflow-owned
completion use case. That use case accepts only the exact waiting Node Execution origin, commits the
node terminal state/output and event, and enqueues a new `WorkflowExecuteRunEffect` when necessary.
Replay after a crash is idempotent.

## Node-To-Asset Bridge

`DesktopNodeCapabilityAssetBridgeAdapterImpl` implements the node-consumer read interface:

```text
NodeCapabilityManagedMediaReaderInterface::read_managed_media
  -> AssetResolveContentUseCase::resolve_asset_content
  -> typed Workflow managed-media input

GenerationTaskAssetSinkInterface::recover_generation_task_asset
  -> AssetRecoverNodeOutputUseCase::recover_asset_node_output
  -> Available | Pending | SourceRequired
GenerationTaskAssetSinkInterface::store_generation_task_asset (SourceRequired only)
  -> AssetRecordNodeOutputUseCase::record_asset_node_output
  -> AssetFinalizeContentUseCase::finalize_asset_content after the effect commit
  -> Available AssetAggregate
  -> typed Workflow managed-media output
```

The separate `DesktopGenerationTaskAssetSinkAdapterImpl` owns the write flow. The bridges translate
kind, provenance, Generation Profile ref, source Asset IDs, the complete
`WorkflowNodeExecutionOrigin`, and the Project, Workflow Run, and node-execution coordinates from
`WorkflowNodeExecutionContext`. It converts
Generation Task output coordinates into the Asset-owned `AssetNodeOutputKey` and construct
`AssetWorkflowNodeOrigin` only from those supplied typed coordinates. It performs no Workflow
repository lookup or producer inference and exposes no Asset repository, row, path, or preview
lease to node or task business code.

`DesktopWorkflowMediaPreviewAdapterImpl` separately implements `WorkflowMediaPreviewIssuerInterface` over
`AssetIssuePreviewUseCase`. Asset application types never enter `crates/engine`.

## Assistant Bridges

`DesktopAssistantWorkflowBridgeAdapterImpl` implements Assistant-owned read/evaluate/apply/Run interfaces by
calling canonical Workflow use cases. `DesktopAssistantWorkspaceBridgeAdapterImpl` composes bounded
Workflow, Asset, capability, profile, and Run projections through their public queries.

The Workspace bridge accepts `AssistantWorkspaceSnapshotRequest`, calls
`WorkflowGetCurrentUseCase`, `WorkflowListActiveRunsUseCase`, `AssetGetUseCase`,
`AssetListUseCase`, `NodeCapabilityListUseCase`, and
`GenerationProfileListForCapabilityUseCase`, and emits one canonical bounded JSON snapshot.
Missing selected nodes or Assets are represented as unavailable selections rather than silently
dropped. The bridge performs no repository access and does not expose parameters, content leases,
paths, preview tokens, provider routes, or credentials.

The Workflow bridge parses only the strict Assistant-tool mutation proposal DTO frozen by
`BACKEND_ASSISTANT.md`, translates it to Workflow-owned typed actions, and emits Workflow's
canonical action bytes. It does not call or retain the superseded `WorkflowPatchService`,
`WorkflowAuthority`, legacy `NodeRegistry`, or legacy Asset store. Evaluation loads the current
Workflow once, requires the exact base revision, applies the typed actions in memory through the
same Workflow aggregate policy used by commit, and derives readiness and fingerprint from that
candidate. Apply decodes only the persisted canonical actions and invokes
`WorkflowApplyMutationUseCase`; it never reevaluates model JSON.

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
idempotency receipts, Pending Asset finalization, durable Generation Task effects, durable Run
events, and conservative interruption only where durable handoff cannot be proved.

The outbox is a closed boundary union, not a job framework:

```rust
pub enum DesktopPostCommitEffect {
    Workflow(WorkflowExecuteRunEffect),
    Asset(AssetFinalizeContentEffect),
    Assistant(AssistantApplyWorkflowChangeEffect),
}
```

`DesktopPostCommitEffectId` and `DesktopApplicationInstanceId` are UUIDv4 values.
`DesktopPostCommitEffectState` is
exactly `Ready`, `Claimed { instance_id, claimed_at }`, `Completed { completed_at }`, or
`Abandoned { abandoned_at, reason }`. `claim_next_post_commit_effect` orders Ready effects by
creation time then effect ID, atomically claims one, and increments a non-zero `u32` attempt count.
A Workflow effect is claimable only while its Run has no other Claimed Workflow effect, so effects
for the same Run execute serially and rely on optimistic revision only across restarts; different
Runs proceed concurrently within the configured bound. A Run-revision conflict observed during
effect execution is a transient consumer failure that releases the effect back to Ready.
Completion and abandonment require the claiming instance. Startup resets claimed Asset and
Assistant effects to Ready. Every safe non-terminal Workflow effect is replayed; the idempotent
Workflow executor schedules any independent ready nodes and naturally no-ops when all active nodes
are waiting for Generation Tasks. A Workflow effect is abandoned only after the Run was interrupted
or was already terminal. Abandon reason is exactly `WorkflowInterruptedByRestart` or
`OwningStateAlreadyTerminal`. A transient consumer/storage failure releases Ready and the worker
waits one second before another claim. There is no lease, priority, configurable retry policy,
poison state, or fourth Desktop effect kind. The separate Generation Task outbox is the closed
capability-specific protocol defined by `BACKEND_TASK.md`.

`DesktopPostCommitEffectOutboxInterface`, owned by this worker, has exactly
`claim_next_post_commit_effect`, `complete_claimed_post_commit_effect`,
`release_claimed_post_commit_effect`, `abandon_claimed_post_commit_effect`, and
`list_recoverable_post_commit_effects`, plus `recover_replayable_post_commit_effect` and
`recover_abandoned_post_commit_effect`. Every normal worker
transition is an atomic compare-and-swap over effect ID, `Claimed`, and the current claiming instance.
Claim returns at most one complete typed effect. The bounded recovery list returns every
prior-instance Claimed effect and every Ready Workflow effect, at most 100 per page, ordered by
creation time then effect ID. It includes the complete expected state required for recovery CAS;
its opaque cursor encodes that exact `(created_at, effect_id)` tuple.

Startup first acquires the OS-level exclusive database lock, resets every prior-process Generation
Task effect from `Claimed` to `Ready`, and invokes
`WorkflowClassifyRunsAfterRestartUseCase`. It preserves Runs with provable durable task handoff,
replays every safe non-terminal Run, and invokes `WorkflowInterruptRunsAfterRestartUseCase` only for
unsafe Runs. It then
pages all recoverable Desktop effects. Asset and Assistant
claims use `recover_replayable_post_commit_effect` to CAS directly from the observed prior claim to
Ready. A Workflow effect is restored to Ready for `ReplaySafe`; for `InterruptUnsafe`, Workflow first
commits `InterruptedByRestart` and then the effect is abandoned with
`WorkflowInterruptedByRestart`. An effect whose Run was already terminal is abandoned with
`OwningStateAlreadyTerminal`.
Each item commits independently; on failure startup stops
before accepting commands, and the next startup repeats the same ordered recovery. Thus a crash
between Run interruption and effect abandonment is closed by idempotent replay without putting
Workflow mutation inside the outbox adapter. The interface exposes no generic payload, handler
registration, unbounded list, delete, or arbitrary state setter.

Every Ready or prior-instance Claimed Workflow effect is replayed for safe non-terminal work or
abandoned after unsafe interruption. Asset effects are
idempotently replayed by exact finalization ID. Assistant effects are replayed through mutation and
Run request receipts. No arbitrary kind, payload, or handler registration exists. Generation Task
exposes only its bounded get/list API.

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

The adapter issues `desktop-asset://v1/<token>`. The unpadded base64url token is canonical bytes:
version byte `1`, lease UUID (16), Project UUID (16), Asset UUID (16), managed-content ID canonical
bytes (33), issued-at signed i64 Unix milliseconds (8, big-endian), expires-at signed i64 Unix
milliseconds (8, big-endian), then HMAC-SHA-256 (32) over every preceding byte. A fresh 32-byte
process secret is obtained from the operating-system CSPRNG at startup and is never persisted.
Issuance fails if entropy or time is unavailable; restart deliberately invalidates every token.

Protocol handling decodes the exact length and version, verifies the MAC in constant time before
using any embedded identifier, rejects expiry, then loads the Asset through `AssetGetUseCase` to
obtain its authoritative media kind and calls `AssetResolveContentUseCase` with that kind and a
protocol deadline. The resolved Available descriptor must have the same managed-content ID before
its opaque lease is read.
Only `GET` and `HEAD` are allowed. Image rejects Range; Video and Audio accept one satisfiable byte
range and return `206`, otherwise `416`. No signing interface, key rotation, token row, revocation
list, refresh token, path, or multi-version negotiation exists in MVP.

The decoder accepts only unpadded canonical base64url and rejects invalid alphabet, padding,
non-canonical encodings, wrong byte length, unknown version, a negative or future issued-at, expiry
other than exactly issued-at plus 300,000 milliseconds, or an expired token. Invalid token,
signature, Project visibility, or stale descriptor returns the same protocol `404`; expired returns
`410`, unsupported method `405`, invalid/unsatisfiable Range `416`, and internal storage failure
`500`. Successful responses set the verified `Content-Type`, `Content-Length`, descriptor digest as
ETag, `Cache-Control: private, no-store`, `Accept-Ranges: bytes` only for Video/Audio, and
`X-Content-Type-Options: nosniff`. `HEAD` returns identical status and headers without a body.

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

The Tauri event name is `workflow-run-event-v1`; its payload is the exact committed
`WorkflowRunEventDto`. Assistant model presentation uses process-scoped
`assistant-presentation-event-v1` with variants `TextDelta`, `ToolActivity`,
`WorkflowChangeReady`, `InvocationCompleted`, and `InvocationFailed`. Each carries invocation ID and
non-zero contiguous sequence; tool activity adds exact tool ID and `Started | Completed | Failed`,
change-ready adds change ID, and failure adds `DesktopErrorDto`. It never exposes a sidecar frame,
prompt, tool arguments/result, Reviewer prose, SDK state, or credential. Assistant events are not
durable authority; after a gap or restart React reloads pending change and canonical Run facts.

## Composition Root

`DesktopCompositionRoot` in `composition.rs` is the only code that names concrete adapters:

```text
open SQLite and managed-content roots; create fresh epoch-2 storage or validate its exact version
  -> construct SQLite backend-config and Assistant plaintext credential repository
  -> load and validate DesktopBackendConfig
  -> construct Project, Workflow, Generation Task, Asset, and Assistant repositories
  -> construct Project use cases and DesktopProjectWorkflowBridgeAdapterImpl
  -> construct Asset storage/inspection use cases
  -> construct node/Asset, node/task, task/Asset, task/Workflow, and Workflow-preview bridges
  -> construct the frozen Generation Profile catalog
  -> construct the immutable Mock provider composite, focused capabilities, and provider registry
  -> construct profile availability reader
  -> construct exactly seven Node Capability implementations and one registry
  -> construct Workflow use cases, DesktopPostCommitEffectWorker, and GenerationTaskEffectWorkerImpl
  -> construct Assistant aggregates/use cases, Workflow bridges, and model runner adapter
  -> reconcile Pending Assets, reset prior-process task claims, and classify non-terminal Runs
  -> register commands, both closed effect workers, sidecar transport, and preview protocol
```

`DesktopApplicationHost` contains typed, already-constructed command dependencies. It is not a
service locator and is never passed into business code. Tests build the same graph with deterministic
adapters without starting Tauri.

## Configuration And Credentials

`DesktopBackendConfig` schema version `2` is stored in `metadata.sqlite` and loaded through
`DesktopBackendConfigRepositoryInterface`. `DesktopBackendConfig` contains exactly
`sqlite_busy_timeout_ms`, `post_commit_effect_concurrency`, `workflow_run_concurrency`,
`workflow_node_concurrency`, `generation_task_effect_concurrency`, `asset_reconciliation_policy`,
`asset_preview_policy`, `generation_provider_routes`, `assistant_model`, and
`assistant_protocol_budgets`. Defaults for the
first five are 5,000, `4`, `1`, `2`, and `4`; every concurrency bound is `1..=8`. The remaining
nested values use
their owner-document exact fields, defaults, and maxima and cannot weaken or exceed them. Locations
are derived from the OS application-data root and are not config fields.

Each `generation_provider_routes` item has exactly `profile_ref`, `generation_kind`, `provider_id`,
and `route_id`. Endpoint, native model, account, credential, operation deadline, polling bounds,
response limits, and download host allowlist are not configuration fields. Adding a production
provider may extend the Settings and Task target schemas only through a separately reviewed design.
`assistant_protocol_budgets` has exactly
`invocation_deadline_ms`, `frame_max_bytes`, `json_max_depth`, `event_max_count`,
`tool_call_max_count`, `model_turn_max_count`, `direction_max_bytes`, `text_output_max_bytes`,
`snapshot_max_bytes`, `candidate_max_bytes`, `continuation_max_bytes`, and `approval_expiry_ms`, with
D0.5 exact values.

There is at most one active `generation_provider_routes` item per `(profile_ref, generation_kind)`;
its fields are one indivisible binding. The Mock Settings UI selects or removes that provider/route
binding and renders no account or credential fields. Duplicate mapping keys or duplicate bindings
are rejected.

The repository uses one canonical JSON payload of at most 256 KiB inside a revisioned SQLite row.
It rejects duplicate/unknown fields, wrong schema, Assistant native model overrides, credential
values inside the config payload, and paths. An absent row yields and atomically stores the exact
three `(profile, generation kind)`-to-Mock-route bindings from `BACKEND_PROVIDERS.md` and Assistant disabled. Its
Assistant default is schema `1`, enabled `false`,
`assistant.workflow_coauthor@1`, and credential ID `assistant.openai.default`. Configuration is
validated at startup, and every Settings mutation is validated against the same shipped provider/
route contracts before its transaction commits. Provider composites and their shipped route
registry are immutable and are not rebuilt after a Settings mutation. New task admission copies
the currently selected `(profile, generation kind, provider, route)` tuple into the immutable Task
target. A concurrent Settings change affects only a later binding resolution; the admitted Task
continues with its copied target. A missing Assistant credential disables only Assistant commands.

The Mock architecture starts in a new hard-cut Desktop storage epoch. Fresh storage writes canonical
version 2 with the exact three Mock bindings. There is no legacy config column, reader, importer,
translator, or migration. A non-empty database from any prior epoch fails startup closed and remains
untouched; current runtime code therefore cannot inspect, compose, or mutate its provider routes or
credential rows.

`AssistantModelCredentialRepositoryInterface` remains the active plaintext credential boundary.
Legacy provider credentials and Assistant credentials occupy separate SQLite tables. This provides
no encryption at rest: any actor able to read the database can read them. Plaintext never enters
the active config payload, public DTOs, errors, or logs. The Mock MVP defines no provider credential
mutation interface, revision lifecycle, tombstone, or Task foreign key. Those semantics belong to
the first separately reviewed production-provider design. There is no JSON-file, platform-vault,
encrypted-column, or embedded-key fallback.

## Representation Boundaries

All Desktop JSON uses `snake_case` fields and enum values. Tagged unions use required `kind`; every
declared field is present and optional values are explicit `null`. UUIDs use lowercase hyphenated
text, hashes use lowercase hex, `u64`/`i64` identities, revisions, sequences, and timestamps use
canonical decimal strings, and opaque cursors use unpadded base64url. Requests reject unknown or
duplicate fields, non-canonical encodings, non-finite numbers, and payloads over 2 MiB. Asset import
transfers only its trusted Tauri file handle, never media bytes or a path in JSON.

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

A persistence row, provider DTO, SDK state, credential, managed path, remote provider task ID, or
raw provider payload is never returned to React. A stable safe route ID appears only in the Models
Settings contract where the user selects that route; Task DTOs do not expose it.

## Error Translation

Tauri translates a structured context error once into `DesktopErrorDto`:

```text
{ code, message, retryable, retry_after_epoch_ms, target, correlation_id }
```

Every field is present; the last three may be null. `code` is a source-prefixed lowercase dot key.
`target` is one tagged Project, Workflow, Run, Node, Asset, Assistant Change, Generation Profile,
parameter, input, or output identity—never a map. Unknown failures use `desktop.internal` and a new
correlation UUID. `message` is safe presentation text and never drives logic. Logs exclude secrets,
model prompts, provider bodies, signed URLs, opaque SDK state, and unnecessary paths.

## Frozen Visual Baseline

Frontend activation preserves the current 1440×900 `No project` shell: top bar, icon rail, left
library, central canvas, right inspector, spacing, typography, colors, and component CSS classes.
The D0.6 repository-owned CSS manifest SHA-256 is
`f85b381fcb393ac96ac1ba1d0c5195b3f3b103253c14b35c93043840cf6a2d19`. Functional content may
change only through a V task, but changing this visual system requires new documented approval. The
hash covers sorted lines `<file SHA-256><two spaces><relative CSS path>`. Visual-equivalent CSS fixes
may change it with before/after browser evidence. Checks use this viewport plus 1024×768, keyboard
navigation, and zero console errors or warnings. The observed pre-cut React Flow container warning
belongs to V3 and is not fixed here.

## Verification

- command tests cover DTO bounds, trusted context, source-first routing, and error translation;
- Project command/bridge tests cover create, rename, list, open, isolation, and one current Workflow;
- use-case tests use fake interfaces without Tauri;
- transaction tests prove every durable-before-effect ordering;
- effect-worker tests cover the three Desktop effects, four task effects, single-worker claiming,
  cancellation, and restart policy;
- bridge tests prove exact cross-context translation without copied semantics;
- event tests cover sequence, emission failure, duplicate/gap repair, and terminal query;
- preview tests cover Project isolation, expiry, MIME, Range, and path non-disclosure;
- composition tests assert exactly seven active Node Capabilities, configured provider composites,
  and the exact three active profile routes;
- Assistant E2E proves proposal -> review -> approval -> canonical apply -> canonical Run -> repair
  when the Assistant implementation track starts (design intent until then);
- contract fixtures prove Rust, Python, and TypeScript DTO/schema alignment.

## Post-MVP

New roadmap capabilities, Project archive/delete/duplicate, standalone task creation, server/background
workers, provider choice, dynamic plugins, durable backend undo/history, cross-Run cache, advanced
Asset lifecycle, multi-device Assistant coordination, cloud sync, 3D, and scenes remain outside the
frozen Desktop surface.
