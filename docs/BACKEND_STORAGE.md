# Backend Storage Architecture

> Status: frozen MVP design
> Owner: consumer-owned interfaces; concrete adapters composed by `src-tauri`
> Scope: Project, Workflow, Run, Generation Task, Asset, Assistant, configuration, and credentials

Storage preserves only the state required to reopen a Workflow, inspect and recover a Run outcome,
use managed media, resume an Assistant approval, and connect configured external services. It owns
no business transition.

## Storage Topology

```text
Desktop application data root
  +-- metadata.sqlite
  |     Project, Workflow, Run, Generation Task, Asset, Assistant, backend configuration,
  |     plaintext provider/Assistant credentials, three closed post-commit effects,
  |     and the closed Generation Task outbox
  +-- managed-media/
  |     immutable validated Image, Video, and Audio bytes
  +-- staging/
  |     incomplete Asset bytes awaiting finalization
  +-- assistant-epochs/<contract-epoch>/
        Python SDK Sessions and opaque model continuations
```

There is no server, distributed queue, generic job database, provider-specific task table,
application cache, or cross-device synchronization in the MVP.

## Storage Rules

1. Aggregates and exact capabilities own semantics; adapters only encode approved state.
2. Every durable fact has one authoritative location.
3. SQLite transactions remain short and never include filesystem, provider, sidecar, or Tauri
   work.
4. Required state and idempotency evidence commit before external follow-up.
5. SQLite and filesystem publication are coordinated explicitly, not called one transaction.
6. Rows, paths, URLs, provider payloads, SDK bytes, and plaintext secrets never enter domain identity
   or public DTOs.
7. Startup recovery is bounded; accepted provider work resumes only by an authoritative persisted
   Generation Task and its exact remote handle.

## State Placement

### Persisted

| State | Authoritative location |
| --- | --- |
| `ProjectAggregate` plus mutation request hashes and receipts | SQLite |
| current `WorkflowAggregate` and revision | SQLite |
| Workflow mutation request hashes and receipts | SQLite |
| `WorkflowRunAggregate`, frozen plan, node executions, outputs, and event delivery state | SQLite |
| Workflow Run request hashes and admission receipts | SQLite |
| `GenerationTaskAggregate`, immutable request/target, remote handle, optional result, and revision | SQLite |
| closed Generation Task submit/poll/cancel/notify effects and lease state | SQLite |
| `AssetAggregate`, node-output keys, and content finalization | SQLite |
| `AssistantProductionPlanAggregate` and items | SQLite |
| `AssistantWorkflowChangeAggregate`, review, decision, and Run link | SQLite |
| Assistant repair-activation deduplication | SQLite |
| three closed post-commit effect kinds and delivery state | SQLite |
| opaque Assistant continuation and SDK Session | contract-epoch storage owned by the model adapter |
| validated media bytes | managed-media directory |
| incomplete media bytes | staging directory |
| validated startup limits, route/profile entries, credential IDs | SQLite |
| provider and Assistant API secrets | plaintext SQLite credential rows |

`ProjectId` is defined by `crates/projects` and stored on every Project-owned Workflow, Run, Asset,
Production Plan, and Assistant Workflow Change. Other contexts do not copy Project metadata.

### Process-Scoped

- one hydrated aggregate during one application call;
- active `DesktopPostCommitEffectWorker` handles and cancellation tokens;
- one active Assistant invocation lock per Session;
- provider transport and one in-flight submit/poll/cancel call;
- loaded credential values during one bounded use;
- open source, staging, managed-content, sidecar, and preview handles;
- one process-local signing secret for preview leases;
- one bulk Generation Profile availability observation response.

No command assumes any process-scoped value survives restart.

### Never Persisted

- React selection, hover, drag, menu, viewport, zoom, seek, volume, or object URL;
- preview URLs, preview tokens, preview leases, or signing keys;
- user source paths or managed absolute paths in business records;
- credentials outside their dedicated SQLite rows or call-scoped secret values;
- provider request/response bodies, signed URLs, or native model IDs;
- current provider availability observations;
- generated media bytes inside SQLite;
- arbitrary background-job kinds or provider-specific task lifecycles;
- a cross-Run result cache;
- unbounded model stream events or raw Reviewer prose as review authority.

## Persistence Names

Private types follow:

```text
<Technology><OwningModule><BusinessObject><RepresentationRole>
```

Examples are `SqliteWorkflowRunRow`, `SqliteWorkflowNodeExecutionRow`,
`SqliteAssetContentFinalizationRow`, and `SqliteAssistantWorkflowChangeRow`. These are Rust types,
not physical table names.

Rows contain no transition methods. Named translators reconstruct authoritative types through their
validated restore paths and reject corrupt or unsupported combinations.

## Logical Project Records

`SqliteProjectRow` stores Project identity as the exact 16 UUID bytes, normalized name, revision,
and timestamps as signed integers.
`SqliteProjectMutationReceiptRow` stores request ID, canonical command hash, operation, the exact
committed Project outcome fields, and result fingerprint. This snapshot is idempotency evidence, not
Project history: it exists only so replay returns the original result after a later rename. Project
rows store no Workflow, Asset, or Assistant payload.

Project creation and rename atomically write the row and receipt. Names need not be unique; IDs are
unique. Project deletion is not an MVP transition.

## Logical Workflow Records

`SqliteWorkflowRow` stores Project/Workflow identity, schema version, current revision, a bounded
versioned graph payload, and timestamps. UUIDs are exact 16-byte values; timestamps are signed
UTC-millisecond integers.

`SqliteWorkflowCreateReceiptRow` stores creation request ID, command hash, the exact created
Workflow snapshot, and result fingerprint.

`SqliteWorkflowMutationReceiptRow` stores:

- `WorkflowMutationRequestId` and canonical command hash;
- the exact committed Workflow snapshot and result fingerprint.

The unique request ID makes mutation replay deterministic. Matching content returns the prior
receipt; mismatched reuse is an idempotency conflict. A receipt is not Workflow history or undo.

## Logical Run Records

`SqliteWorkflowRunRow` stores Project, Workflow, Run, source revision, scope, the frozen
`WorkflowExecutionPlan`, current `WorkflowRunState`, timestamps, and structured terminal failure.

Child records are:

- `SqliteWorkflowNodeExecutionRow`: node/execution identity, state, progress, timestamps, and
  structured failure; capability and profile selection remain in the one frozen plan;
- `SqliteWorkflowNodeExecutionOutputRow`: complete named Text or typed Asset output;
- `SqliteWorkflowRunEventRow`: monotonic per-Run event plus Desktop delivery state;
- `SqliteWorkflowRunRequestReceiptRow`: request ID, canonical admission hash, and admitted Run ID.

Run/event UUIDs are exact 16-byte values, event sequence is non-zero `u64`, progress is `u16`
basis points, and event timestamps are signed UTC-millisecond integers.

The frozen plan and outputs make a Run self-contained after admission. A waiting Node Execution
correlates to Generation Task by its exact typed origin through a bridge query; Workflow rows never
store a remote task handle.

## Logical Generation Task Records

`SqliteGenerationTaskRow` stores the exact Workflow origin, immutable request snapshot and hash,
stable profile and non-secret route binding, normalized state, optional progress and opaque remote
handle, structured failure, timestamps, and optimistic revision.

The Generation Task row stores one optional tagged result: either bounded inline Text or one Asset
reference from finalization. Row constraints require a representation matching the request kind,
and the named translator restores the matching `GenerationTaskResult` variant.
`SqliteGenerationTaskOutboxRow` stores one of `SubmitTask`, `PollTask`, `CancelRemoteTask`, or
`NotifyWorkflow`, plus availability time, bounded delivery count, lease, completion, and safe last
failure. The payload contains identifiers only. Unique origin/idempotency and provider-handle
indexes prevent duplicate local tasks and ambiguous lookup. Exact schema and transaction semantics
are owned by `BACKEND_TASK.md`.

## Logical Asset Records

`SqliteAssetRow` stores Asset/Project identity, media kind, `AssetManagedContentState`,
`AssetContentDescriptor`, `AssetMediaFacts`, `AssetOrigin`, display name, and creation time.

`SqliteAssetContentFinalizationRow` stores only recovery facts: Asset/content/finalization identity,
adapter-private staging reference, expected digest/length, state, and last structured failure.

For node-produced Assets, `AssetNodeOutputKey` is unique. The record stores the verified digest so
the Asset application can return an exact replay or reject different bytes. Only the filesystem
adapter interprets staging references or derives a managed path.

## Logical Assistant Records

`SqliteAssistantProductionPlanRow` and `SqliteAssistantPlanItemRow` encode one plan aggregate and its
revisioned items.

`SqliteAssistantWorkflowChangeRow` plus private child representations encode:

- Project/Session/change identity and base Workflow revision;
- exact ordered mutations, aliases, digest, fingerprint, lineage, and expiry;
- verified review receipt and Reviewer contract version;
- approval scope, human decision, and change state;
- `AssistantModelContinuationRef`, never opaque SDK bytes;
- Workflow mutation receipt and optional Assistant-to-Run link after apply.

`SqliteAssistantRepairActivationRow` deduplicates one factual repair activation per failed Run. It
stores safe structured Run facts, not generated diagnosis or a chosen repair step.

Opaque continuation bytes and SDK Session records stay under the active Assistant contract epoch.
Their adapter validates envelope version, epoch, SDK version, Agent identity, tool-version set,
Project/Session scope, and size before returning a typed continuation handle.

## Closed Post-Commit Effect Record

`SqliteDesktopPostCommitEffectRow` stores effect ID, one closed kind, owning business ID, delivery
state, attempt count, and safe last failure. The only kinds are:

```text
WorkflowExecuteRun { workflow_run_id }
AssetFinalizeContent { finalization_id }
AssistantApplyWorkflowChange { assistant_workflow_change_id }
```

There is no arbitrary payload, handler name, user-facing task API, priority, workflow graph, or
plugin registration. Domain/application code creates the typed effect; the SQLite adapter
mechanically encodes it in the same transaction as the owning state.

## Consumer-Owned Interfaces

| Consumer interface | Storage behavior |
| --- | --- |
| `ProjectRepositoryInterface` | load/list Projects and atomically commit create/rename plus receipt |
| `WorkflowAggregateRepositoryInterface` | load current Workflow; atomically CAS snapshot and mutation receipt |
| `WorkflowRunRepositoryInterface` | idempotently admit and atomically transition Runs, outputs, and events |
| `GenerationTaskRepositoryInterface` | atomically create/transition Tasks with consumed and enqueued task effects |
| `GenerationTaskOutboxReaderInterface` | claim, reschedule, and recover the closed task-effect protocol |
| `AssetRepositoryInterface` | load/query Assets and resolve node-output identity |
| `AssetIngestTransactionInterface` | commit Pending/finalization and availability transitions |
| `AssetManagedContentStoreInterface` | stage, publish, open, verify, and remove stale staged bytes |
| `AssistantProductionPlanRepositoryInterface` | load and revision-CAS one plan aggregate |
| `AssistantWorkflowChangeRepositoryInterface` | transition changes and query one pending approval per Session |
| `AssistantRepairActivationRepositoryInterface` | record-or-get one factual activation per failed Run |
| `AssistantModelContinuationStoreInterface` | store/load/consume versioned opaque continuation state |
| `DesktopPostCommitEffectOutboxInterface` | claim/finish one closed effect and page/CAS prior-instance claims plus Ready Workflow effects for startup recovery |
| `GenerationProviderCredentialRepositoryInterface` | save/load/delete plaintext generation-provider secrets in SQLite |
| `AssistantModelCredentialRepositoryInterface` | save/load/delete plaintext Assistant model secrets in SQLite |
| `DesktopBackendConfigRepositoryInterface` | load/save and validate startup configuration in SQLite |

There is no global `Store`, `Database`, repository, unit of work, or credential interface. SQLite,
filesystem, and epoch-storage adapters depend inward on their consumers.

## Required Atomic Writes

### Project Create Or Rename

```text
verify request identity and optional base revision
  -> write Project creation or next name/revision
  -> write mutation receipt
  -> commit or return revision/idempotency conflict
```

### Workflow Mutation

```text
verify base revision and request identity
  -> write next Workflow snapshot
  -> write mutation receipt
  -> commit or return revision/idempotency conflict
```

### Run Admission And Transition

```text
write Queued WorkflowRunAggregate + frozen plan
  -> write planned node executions
  -> append first event
  -> write Run request receipt
  -> write WorkflowExecuteRunEffect
  -> commit
  -> post-commit worker executes the Run
```

Later transactions persist only aggregate-approved changes. One transition transaction may update
the Run root, affected node executions, a complete output set, and ordered events. Provider calls
occur while the already-committed Run effect is being consumed; no per-node effect records are
created. Tauri emission consumes committed event rows after their transaction.

### Asset Publication

```text
commit Pending Asset + finalization + node-output identity + AssetFinalizeContentEffect
  -> publish or verify exact bytes outside SQLite
  -> commit Pending -> Available, finalization Completed, and effect Completed
```

A generated output is not attached to a Run until Asset is Available.

### Assistant Approval And Apply

```text
commit Assistant change AwaitingApproval -> Applying + AssistantApplyWorkflowChangeEffect
  -> call canonical Workflow mutation with stable request ID
  -> commit Applied + Workflow mutation receipt reference
  -> start canonical Run with stable Run request ID
  -> commit optional Assistant-to-Run link
```

These are intentionally separate short transactions consumed under one Assistant effect. An
`Applying` change is recoverable because
Workflow and Run request receipts make each follow-up idempotent. A permanent stale/fingerprint
failure transitions the change to `ApplyFailed`; a transient storage failure leaves it recoverable.
An ambiguous sidecar-continuation resume is marked interrupted rather than replayed; canonical
Workflow apply and Run admission remain authoritative.

## Managed Media Publication

Import and node output share one protocol:

```text
bounded stream -> restricted staging -> digest + MIME + media facts
  -> Pending metadata commit -> atomic publish/verify -> Available metadata commit
```

The filesystem adapter derives final location from `AssetManagedContentId` and rejects traversal,
symlinks, unexpected file types, digest/length/kind mismatch, oversized content, and unsafe
permissions. Managed bytes are immutable after availability.

| Failure point | Durable outcome |
| --- | --- |
| before Pending commit | no Asset; remove staging when possible |
| after Pending commit | Pending finalization is retried at startup |
| after publication, before Available commit | verify exact bytes and complete transition |
| expected bytes absent | mark Asset Missing |
| Available bytes later absent | detect and mark Asset Missing |

## Configuration And Credentials

`metadata.sqlite` stores the validated `DesktopBackendConfig` and the two credential classes in
separate focused tables. Credential values are deliberately stored as plaintext blobs. The MVP
does not claim encryption at rest, does not derive or embed an encryption key, and does not use
macOS Keychain, Windows Credential Manager, Linux Secret Service, a JSON config file, or an
environment-variable persistence fallback.

The physical rows are exact:

| Table | Key and payload |
| --- | --- |
| `desktop_backend_config` | singleton key `1`, schema version, non-zero monotonic revision, canonical JSON blob `1..=262,144` bytes |
| `generation_provider_settings_receipts` | request ID primary key, canonical action hash, committed config revision, and sanitized result snapshot |
| `generation_provider_credentials` | composite primary key `(credential_id, revision)`, nullable plaintext secret blob `1..=16,384` bytes, and `is_active` with a unique partial index allowing at most one active revision per credential ID; null secret is a retained tombstone |
| `assistant_model_credentials` | typed credential ID primary key, plaintext secret blob `1..=16,384` bytes |

Saving config or one credential is an atomic revisioned mutation. Provider credential replacement
inserts a new immutable revision and atomically advances its active pointer. Generation Task
admission compares the observed config revision and exact `is_active` credential revision in the
same write transaction as Task insertion; mismatch retries resolution before any provider call.
The exact ID/revision foreign key preserves the accepted snapshot but is not the concurrency guard.
Settings deletion retires a revision;
only after no other active Settings binding or non-terminal Task needs it are its secret bytes
cleared while the tombstone row preserves every Task foreign key. Only an entirely unreferenced tombstone may be physically deleted. Loading
an absent exact revision returns `NotFound`; loading a tombstone for an authenticated call returns
`CredentialRetired`. The two credential tables cannot be queried through one broad
interface or joined into config DTOs. Config initialization writes the frozen default only
when the singleton row is absent; a corrupt, oversized, unsupported-version, or non-canonical row
fails startup and is never silently replaced.
Settings apply atomically compare-and-swaps the config revision, mutates any credential rows, and
inserts its request receipt. A repeated request ID with the same canonical action hash returns the
sanitized recorded result; a different hash is an idempotency conflict. Receipt rows never contain
secret bytes.

The database and parent data directory use private user-only permissions where the platform
supports them. This reduces accidental disclosure but is not encryption: any process or user able
to read the database can read every stored credential. Public DTOs expose only configured presence
and typed credential IDs. Provider routes and the Assistant runner load one secret immediately
before an authenticated call; request construction may zeroize temporary header buffers, but the
durable SQLite value remains plaintext by design.

The generation-provider and Assistant credential repositories remain separate consumer-owned
interfaces and separate tables even when one SQLite adapter implements both. Tests prove
save/load/delete, Project-independent credential-ID isolation, missing/unavailable behavior,
plaintext round trip, and absence from DTOs, errors, and logs.

## Reads And Preview

Project, Workflow, Run, Generation Task, Asset, and Assistant queries read bounded SQLite projections. Stable cursor
ordering is defined by the owning application contract.

`AssetManagedContentLease` owns an opaque read handle. Preview uses a short-lived process-signed
token rather than a database row or memory registry. Every protocol request validates signature,
expiry, Project, Asset state, and current descriptor. Restart invalidates all preview tokens.
The exact v1 token wire and signing rules are owned by `BACKEND_APPLICATION.md`; Storage owns only
the process-secret lifetime and the opaque managed-content read used after validation.

## Startup And Restart

```text
resolve private application-data locations
  -> open SQLite and apply known migrations
  -> load or initialize validated DesktopBackendConfig and credential repositories
  -> construct Project and other repositories, managed-content adapters, and application use cases
  -> construct provider routes, Node Capabilities, Assistant adapter, and bridges
  -> validate the active Assistant contract epoch
  -> replay bounded Asset finalization effects and reclaim prior-instance or expired Generation Task leases
  -> classify non-terminal Workflow Runs against exact durable task handoffs
  -> preserve waiting Runs, replay queued Runs, and interrupt only unsafe Runs
  -> replay Assistant Applying effects through idempotency receipts
  -> emit bounded undispatched Workflow Run events
  -> start the post-commit and Generation Task workers and accept commands
```

An accepted Generation Task resumes only by its persisted remote handle and exact route binding.
An ambiguous uncommitted submission is never blindly repeated. Pending Asset reconciliation is safe
because it completes only already-identified local bytes. Assistant apply recovery remains isolated
and invokes canonical idempotent Workflow/Run admission.

## SQLite And Migration Policy

One metadata database belongs to one Desktop data root and one writable process. Transactions are
short; blocking SQL/filesystem work runs outside async runtime core threads. MVP requires foreign
keys, parameterized statements, private permissions, bounded queries, a bounded busy timeout, and
no application-managed pool or speculative WAL tuning.

Startup creates the current schema, applies known forward migrations transactionally, refuses newer
unsupported versions, and never deletes or silently recreates user data after integrity/migration
failure. Assistant contract-epoch storage is a hard compatibility boundary and is never parsed by a
new epoch.
The Desktop backend-config v1-to-v2 translation is the exact Mock-binding migration owned by
`BACKEND_APPLICATION.md`; it runs in the same transaction that advances the stored config schema
version and does not delete retained credential rows.

The hard-cut Desktop storage epoch is `1`: SQLite `application_id` is `0x4f4d4431` and
`user_version` starts at `1`. An absent database is created at that pair; an existing non-empty
database with a missing/different application ID is `UnsupportedLegacyStorageEpoch` and remains
untouched with adjacent files. Within epoch `1`, only explicitly shipped sequential forward
migrations run, each in one transaction; gaps, downgrades, partial migrations, and unknown newer
versions fail closed. The abandoned architecture has no reader, importer, compatibility table, or
destructive reset path.

## Errors And Verification

Storage adapters return structured categories: unavailable, busy, permission denied, unsupported
version, corruption, revision conflict, idempotency conflict, limit exceeded, content mismatch,
credential not found, invalid credential, and credential unavailable. Errors include safe
operation and typed identity,
never paths, secrets, model prompts, provider bodies, signed URLs, or SDK bytes.

Verification proves:

- Project create/rename receipt behavior, stable list ordering, and current-Workflow discovery;
- Workflow revision CAS and mutation receipt replay/conflict;
- Run admission receipt, transition/output/event atomicity, and event ordering;
- Asset fault injection at every SQLite/filesystem boundary and node-output replay/conflict;
- Assistant plan CAS, exact change reconstruction, Applying recovery, and Run-link idempotency;
- Generation Task transaction/outbox atomicity, expired-lease recovery, query-by-ID resume, and
  safe/unsafe Workflow restart classification;
- SQLite configuration and plaintext credential round trip, isolation, deletion, and redaction
  from DTOs/errors/logs;
- bounded queries, Project isolation, preview expiry/Range, and path non-disclosure;
- translators reject unknown versions, variants, corrupt combinations, and oversized payloads.

## Post-MVP

Project archive/delete/duplicate, Workflow history/undo, Generation Task attempt history/archive,
cross-Run cache, Asset deletion/retention/GC, backup/restore, full database/media encryption, key rotation
UI, connection-pool/WAL tuning, multiple writers, distributed Assistant leases, cloud sync, and
collaboration require separate authority, migration, and recovery decisions.
