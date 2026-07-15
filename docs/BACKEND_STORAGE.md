# Backend Storage Architecture

> Status: frozen MVP design
> Owner: consumer-owned interfaces; concrete adapters composed by `src-tauri`
> Scope: Project, Workflow, Run, Asset, Assistant, configuration, and credentials

Storage preserves only the state required to reopen a Workflow, inspect and recover a Run outcome,
use managed media, resume an Assistant approval, and connect configured external services. It owns
no business transition.

## Storage Topology

```text
Desktop application data root
  +-- metadata.sqlite
  |     Project, Workflow, Run, Asset, Assistant, and three closed post-commit effects
  +-- managed-media/
  |     immutable validated Image, Video, and Audio bytes
  +-- staging/
  |     incomplete Asset bytes awaiting finalization
  +-- assistant-epochs/<contract-epoch>/
  |     Python SDK Sessions and opaque model continuations
  +-- backend-config.json
        versioned non-secret startup configuration

Operating-system credential facility
  generation-provider and Assistant model credentials
```

There is no server, distributed queue, generic job database, generation-task table, provider-task
store, application cache, or cross-device synchronization in the MVP.

## Storage Rules

1. Aggregates and exact capabilities own semantics; adapters only encode approved state.
2. Every durable fact has one authoritative location.
3. SQLite transactions remain short and never include filesystem, provider, sidecar, credential-
   vault, or Tauri work.
4. Required state and idempotency evidence commit before external follow-up.
5. SQLite and filesystem publication are coordinated explicitly, not called one transaction.
6. Rows, paths, URLs, provider payloads, SDK bytes, and plaintext secrets never enter domain identity
   or public DTOs.
7. Startup recovery is bounded and does not resume paid provider work.

## State Placement

### Persisted

| State | Authoritative location |
| --- | --- |
| `ProjectAggregate` plus mutation request hashes and receipts | SQLite |
| current `WorkflowAggregate` and revision | SQLite |
| Workflow mutation request hashes and receipts | SQLite |
| `WorkflowRunAggregate`, frozen plan, node executions, outputs, and event delivery state | SQLite |
| Workflow Run request hashes and admission receipts | SQLite |
| `AssetAggregate`, node-output keys, and content finalization | SQLite |
| `AssistantProductionPlanAggregate` and items | SQLite |
| `AssistantWorkflowChangeAggregate`, review, decision, and Run link | SQLite |
| Assistant repair-activation deduplication | SQLite |
| three closed post-commit effect kinds and delivery state | SQLite |
| opaque Assistant continuation and SDK Session | contract-epoch storage owned by the model adapter |
| validated media bytes | managed-media directory |
| incomplete media bytes | staging directory |
| non-secret locations, limits, route/profile entries, credential IDs | configuration document |
| provider and Assistant API secrets | operating-system credential facility |

`ProjectId` is defined by `crates/projects` and stored on every Project-owned Workflow, Run, Asset,
Production Plan, and Assistant Workflow Change. Other contexts do not copy Project metadata.

### Process-Scoped

- one hydrated aggregate during one application call;
- active `DesktopPostCommitEffectWorker` handles and cancellation tokens;
- one active Assistant invocation lock per Session;
- provider transport, selected route, polling handle, and cancellation observation during one node;
- decrypted credential values during one bounded use;
- open source, staging, managed-content, sidecar, and preview handles;
- one process-local signing secret for preview leases;
- one bulk Generation Profile availability observation response.

No command assumes any process-scoped value survives restart.

### Never Persisted

- React selection, hover, drag, menu, viewport, zoom, seek, volume, or object URL;
- preview URLs, preview tokens, preview leases, or signing keys;
- user source paths or managed absolute paths in business records;
- plaintext credentials in SQLite/config/session state;
- provider request/response bodies, signed URLs, native model IDs, or remote task handles;
- current provider availability observations;
- generated media bytes inside SQLite;
- a separate Generation Task, arbitrary background-job kind, or provider-task lifecycle;
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

The frozen plan and outputs make a Run self-contained after admission. Reopening a Run does not
require a historical Workflow snapshot or provider task.

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
| `AssetRepositoryInterface` | load/query Assets and resolve node-output identity |
| `AssetIngestTransactionInterface` | commit Pending/finalization and availability transitions |
| `AssetManagedContentStoreInterface` | stage, publish, open, verify, and remove managed bytes |
| `AssistantProductionPlanRepositoryInterface` | load and revision-CAS one plan aggregate |
| `AssistantWorkflowChangeRepositoryInterface` | transition changes and query one pending approval per Session |
| `AssistantRepairActivationRepositoryInterface` | record-or-get one factual activation per failed Run |
| `AssistantModelContinuationStoreInterface` | store/load/consume versioned opaque continuation state |
| `DesktopPostCommitEffectOutboxInterface` | claim and finish one of the three closed effect kinds |
| `GenerationProviderCredentialVaultInterface` | save/load/delete generation-provider secrets in OS storage |
| `AssistantModelCredentialVaultInterface` | save/load/delete Assistant model secrets in OS storage |
| `DesktopBackendConfigReaderInterface` | read and validate non-secret startup configuration |

There is no global `Store`, `Database`, repository, unit of work, or credential interface. SQLite,
filesystem, epoch-storage, OS-vault, and config adapters depend inward on their consumers.

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

## Credentials

Production credentials use the platform facility: macOS Keychain, Windows Credential Manager, or
the supported Linux secret service. Non-secret configuration contains only typed credential IDs.

Vault adapters must use private access policy, return structured denied/not-found/unavailable
errors, zeroize temporary secret buffers where supported, and never fall back to plaintext files,
SQLite ciphertext with an embedded key, environment-variable persistence, or empty credentials.

Tests use an in-memory fault-injecting adapter that passes the same save/load/delete and isolation
contract. Environment variables may supply ephemeral development credentials but are never copied
into durable configuration.

## Reads And Preview

Project, Workflow, Run, Asset, and Assistant queries read bounded SQLite projections. Stable cursor
ordering is defined by the owning application contract.

`AssetManagedContentLease` owns an opaque read handle. Preview uses a short-lived process-signed
token rather than a database row or memory registry. Every protocol request validates signature,
expiry, Project, Asset state, and current descriptor. Restart invalidates all preview tokens.

## Startup And Restart

```text
resolve private application-data locations and validate non-secret DesktopBackendConfig
  -> open SQLite and apply known migrations
  -> connect OS credential facilities
  -> construct Project and other repositories, managed-content adapters, and application use cases
  -> construct provider routes, Node Capabilities, Assistant adapter, and bridges
  -> validate the active Assistant contract epoch
  -> replay bounded Asset finalization effects
  -> mark non-terminal Workflow Runs Failed with InterruptedByRestart and abandon their effects
  -> replay Assistant Applying effects through idempotency receipts
  -> emit bounded undispatched Workflow Run events
  -> start the post-commit worker and accept commands
```

MVP does not resume queued/running provider work. Pending Asset reconciliation is safe because it
completes only already-identified local bytes. Assistant apply recovery is safe because it invokes
canonical idempotent Workflow/Run admission, never a provider task directly.

## SQLite And Migration Policy

One metadata database belongs to one Desktop data root and one writable process. Transactions are
short; blocking SQL/filesystem work runs outside async runtime core threads. MVP requires foreign
keys, parameterized statements, private permissions, bounded queries, a bounded busy timeout, and
no application-managed pool or speculative WAL tuning.

Startup creates the current schema, applies known forward migrations transactionally, refuses newer
unsupported versions, and never deletes or silently recreates user data after integrity/migration
failure. Assistant contract-epoch storage is a hard compatibility boundary and is never parsed by a
new epoch.

## Errors And Verification

Storage adapters return structured categories: unavailable, busy, permission denied, unsupported
version, corruption, revision conflict, idempotency conflict, limit exceeded, content mismatch,
credential denied, and credential unavailable. Errors include safe operation and typed identity,
never paths, secrets, model prompts, provider bodies, signed URLs, or SDK bytes.

Verification proves:

- Project create/rename receipt behavior, stable list ordering, and current-Workflow discovery;
- Workflow revision CAS and mutation receipt replay/conflict;
- Run admission receipt, transition/output/event atomicity, and event ordering;
- Asset fault injection at every SQLite/filesystem boundary and node-output replay/conflict;
- Assistant plan CAS, exact change reconstruction, Applying recovery, and Run-link idempotency;
- restart interruption without provider-task resume;
- OS-vault round trip, denial, isolation, deletion, and no durable plaintext;
- bounded queries, Project isolation, preview expiry/Range, and path non-disclosure;
- translators reject unknown versions, variants, corrupt combinations, and oversized payloads.

## Post-MVP

Project archive/delete/duplicate, Workflow history/undo, provider task persistence, cross-Run
cache, Asset deletion/retention/GC, backup/restore, full database/media encryption, key rotation
UI, connection-pool/WAL tuning, multiple writers, distributed Assistant leases, cloud sync, and
collaboration require separate authority, migration, and recovery decisions.
