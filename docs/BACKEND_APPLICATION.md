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

Code remains capability-oriented under `src-tauri`; commands, DTOs, translators, and adapters stay
inside their owning Project, Workflow, Asset, Generation Profile, provider, or Assistant module.
Composition, configuration, and the closed effect worker are the only shared Desktop modules. There
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

The bridge translates kind, provenance, Generation Profile ref, source Asset IDs, the complete
`WorkflowNodeExecutionOrigin`, and the Project, Workflow Run, and node-execution coordinates from
`WorkflowNodeExecutionContext`. It converts
`NodeCapabilityProducedMediaOutputKey` into the Asset-owned `AssetNodeOutputKey` and constructs
`AssetWorkflowNodeOrigin` only from those supplied typed coordinates. It performs no Workflow
repository lookup or producer inference and exposes no Asset repository, row, path, or preview
lease to node code.

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

`DesktopPostCommitEffectId` and `DesktopApplicationInstanceId` are UUIDv4 values.
`DesktopPostCommitEffectState` is
exactly `Ready`, `Claimed { instance_id, claimed_at }`, `Completed { completed_at }`, or
`Abandoned { abandoned_at, reason }`. `claim_next_post_commit_effect` orders Ready effects by
creation time then effect ID, atomically claims one, and increments a non-zero `u32` attempt count.
Completion and abandonment require the claiming instance. Startup resets claimed Asset and
Assistant effects to Ready, but abandons a claimed Workflow effect after marking its Run
`InterruptedByRestart`. Abandon reason is exactly `WorkflowInterruptedByRestart` or
`OwningStateAlreadyTerminal`. A transient consumer/storage failure releases Ready and the worker
waits one second before another claim. There is no lease, priority, configurable retry policy,
poison state, or fourth effect kind.

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

Startup first invokes `WorkflowInterruptRunsAfterRestartUseCase`, which idempotently marks every
non-terminal Run `InterruptedByRestart`. It then pages all recoverable effects. Asset and Assistant
claims use `recover_replayable_post_commit_effect` to CAS directly from the observed prior claim to
Ready. A Workflow effect uses `recover_abandoned_post_commit_effect` to CAS from its observed Ready
or prior-instance Claimed state to Abandoned only after its Run is observed terminal; the reason is
`WorkflowInterruptedByRestart` when that is its terminal cause and
`OwningStateAlreadyTerminal` otherwise. Each item commits independently; on failure startup stops
before accepting commands, and the next startup repeats the same ordered recovery. Thus a crash
between Run interruption and effect abandonment is closed by idempotent replay without putting
Workflow mutation inside the outbox adapter. The interface exposes no generic payload, handler
registration, unbounded list, delete, or arbitrary state setter.

Every Ready or prior-instance Claimed Workflow effect is abandoned after restart once its Run is
confirmed terminal, using the terminal-cause reason above. Asset effects are
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

`backend-config.json` has schema version `1` and is read only by
`JsonFileDesktopBackendConfigReaderAdapterImpl`. `DesktopBackendConfig` contains exactly
`sqlite_busy_timeout_ms`, `post_commit_effect_concurrency`, `workflow_run_concurrency`,
`workflow_node_concurrency`, `asset_reconciliation_policy`, `asset_preview_policy`,
`generation_provider_routes`, `assistant_model`, and `assistant_protocol_budgets`. Defaults for the
first four are 5,000, `4`, `1`, and `2`; concurrency is `1..=8`. The remaining nested values use
their owner-document exact fields, defaults, and maxima and cannot weaken or exceed them. Locations
are derived from the OS application-data root and are not config fields.

Each `generation_provider_routes` item has exactly `profile_ref`, `route_id`, `account_id`,
`endpoint`, `native_model_id`, `credential_id`, `operation_deadline_ms`, `poll_min_delay_ms`,
`poll_max_delay_ms`, and sorted unique `download_host_allowlist`; values must equal the matching
D0.3 profile/route contract. `assistant_protocol_budgets` has exactly
`invocation_deadline_ms`, `frame_max_bytes`, `json_max_depth`, `event_max_count`,
`tool_call_max_count`, `model_turn_max_count`, `direction_max_bytes`, `text_output_max_bytes`,
`snapshot_max_bytes`, `candidate_max_bytes`, `continuation_max_bytes`, and `approval_expiry_ms`, with
D0.5 exact values.

The file is UTF-8 JSON, at most 256 KiB, rejects duplicate/unknown fields, symlinks, group/other
writable POSIX permissions, non-private Windows ACLs, wrong schema, Assistant native model
overrides, plaintext credentials, and paths.
An absent file yields the exact defaults with no provider routes and Assistant disabled; it is not
written implicitly. Its Assistant default is schema `1`, enabled `false`,
`assistant.workflow_coauthor@1`, and credential ID `assistant.openai.default`. Configuration is
validated once at startup. Missing provider credentials make only affected Generation Profiles
unavailable; a missing Assistant
credential disables only Assistant commands.

`GenerationProviderCredentialVaultInterface` and `AssistantModelCredentialVaultInterface` are separate
consumer-owned interfaces even when one OS adapter implements both. Production secrets live in the
operating-system credential store. Plaintext is call-scoped and never enters SQLite, config, DTOs,
errors, or logs.

Credential IDs follow lowercase dot-segment identity rules and are at most 128 bytes. Production
adapters are the distinct generation/Assistant pairs prefixed `MacOsKeychain`,
`WindowsCredentialManager`, or `LinuxSecretService`. Their service scopes are
`oh-my-dream/generation-provider` and `oh-my-dream/assistant-model`; account is the credential ID.
They implement only save, load, and delete, and never enumerate or fall back to a file.

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

A persistence row, provider DTO, SDK state, credential, path, route ID, or provider task ID is never
returned to React.

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
