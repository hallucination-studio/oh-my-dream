# Backend MVP Storage Architecture

> Status: proposed MVP design
> Owner: infrastructure adapters composed by `src-tauri`
> Scope: local durability for Workflow, Run, Asset, provider credentials, and configuration

Naming follows [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md). This document defines storage
responsibilities and logical persistence records. It does not define physical table names or SQL
DDL.

## Purpose

The product is a single-user desktop client. The MVP storage design preserves only the data needed
to save and reopen a Workflow, execute it, inspect a Run, use a configured provider, and preview
imported or generated Image, Video, and Audio Assets.

The design uses:

- SQLite for business metadata and encrypted provider credentials;
- an application-managed directory for immutable media bytes;
- a restricted staging directory for incomplete media writes;
- a versioned file for non-secret startup configuration.

It does not introduce a server, distributed coordination, background workers, application caches,
or cross-device synchronization.

## Storage Rules

1. Domain aggregates own invariants and legal transitions. Storage persists approved state.
2. Every durable fact has one authoritative location.
3. Memory is command-scoped or task-scoped and never becomes a second source of business state.
4. Provider and filesystem effects start only after required SQLite state is committed.
5. SQLite and the filesystem are coordinated explicitly, not described as one transaction.
6. Paths, URLs, provider payloads, plaintext credentials, and Rows never enter domain identity or
   public DTOs.
7. Startup recovery is bounded and does not resume provider work.

## Storage Topology

```text
Desktop application data root
  |
  +-- SQLite metadata database
  |     current Workflow, Runs, Assets, encrypted provider credentials
  |
  +-- managed media directory
  |     immutable validated Image, Video, and Audio bytes
  |
  +-- staging directory
  |     incomplete Asset bytes awaiting finalization
  |
  +-- non-secret startup configuration document

Process memory
  one-command domain objects and active execution resources only
```

A user-selected source file stays outside the application data root and is read only during
`ImportAssetUseCase`. The MVP has no persisted Project aggregate; `ProjectId` is trusted Desktop
scope recorded on each Project-owned Workflow, Run, and Asset.

## State Placement

### Persisted On Disk

| Data or state | Authoritative location |
| --- | --- |
| current `WorkflowAggregate` snapshot and revision | SQLite |
| `WorkflowRunAggregate` and frozen execution plan | SQLite |
| node execution state, outputs, and Run events | SQLite |
| `AssetAggregate` metadata and content state | SQLite |
| Asset managed-content finalization state | SQLite |
| encrypted provider credentials | SQLite |
| validated media bytes | managed media directory |
| incomplete media bytes | staging directory |
| non-secret Desktop and provider settings | configuration document |

SQLite is authoritative for metadata. The managed media directory is authoritative for the bytes of
an Available Asset. Provider credentials are authoritative only after successful authenticated
decryption from SQLite.

### Held Temporarily In Memory

The MVP permits only runtime state that cannot usefully be persisted:

- a hydrated aggregate during one command;
- an active `DesktopWorkflowRunTaskHost` task handle;
- a cancellation signal after cancellation state is committed;
- provider transport and polling state during one active call;
- one decrypted `ProviderCredentialSecretValue` during provider adapter construction or use;
- open source, staging, managed-content, and preview stream handles;
- a process-local signing secret for short-lived preview access.

Every item is dropped after its command, call, stream, or process ends. No command assumes that a
Workflow, Run, Asset, provider task, credential, or preview handle from an earlier command remains
in memory.

### Never Stored

The MVP does not persist:

- React selection, hover, drag, menus, viewport, zoom, seek, volume, or playback state;
- object URLs, preview URLs, preview tokens, or preview leases;
- user source paths, managed absolute paths, staging paths, or filesystem handles in business data;
- plaintext provider credentials;
- provider request bodies, response bodies, signed URLs, or native task payloads;
- provider-native task IDs after the active process exits;
- capability definitions shipped with the application;
- current provider availability observations;
- raw generated media bytes inside SQLite;
- a backend undo stack or historical Workflow snapshots.

The current Workflow snapshot is the saved document. Undo and redo remain editor-session behavior
for the MVP.

## No-Cache Policy

The MVP has no application-level cache:

- no in-memory Workflow, Run, or Asset aggregate cache;
- no cross-Run node-result cache;
- no provider-response or provider-availability cache;
- no duplicate-media cache separate from managed Assets;
- no thumbnail, poster, waveform, or transcoded-preview cache.

Queries read SQLite. Media previews stream managed Asset bytes. Starting another Run executes again.
SQLite page caching and operating-system filesystem caching are implementation details, not another
source of business state.

## Persistence Naming Rule

Private persistence types use:

```text
<Technology><BusinessConcept><RepresentationRole>
```

Examples:

```text
SqliteWorkflowRunAggregateRow
SqliteWorkflowNodeExecutionOutputRow
SqliteAssetManagedContentFinalizationRow
SqliteEncryptedProviderCredentialRepositoryAdapter
```

`Sqlite` identifies the technology, the middle words identify the exact owned concept, and `Row` or
`Adapter` identifies the infrastructure role. These are Rust type names, not physical table names.

## Logical Persistence Records

### Workflow Records

`SqliteWorkflowAggregateRow` stores the current aggregate snapshot:

- Project and Workflow identity;
- current `WorkflowRevision` and `WorkflowSchemaVersion`;
- versioned Workflow persistence payload;
- creation and update timestamps.

`WorkflowAggregateRepositoryPort::save_if_revision` updates the snapshot only when the stored
revision matches the command's base revision. The MVP does not retain historical snapshots or
durable mutation receipts.

### Workflow Run Records

`SqliteWorkflowRunAggregateRow` stores:

- Project, Workflow, Run, and source Workflow revision identity;
- `WorkflowRunScope` and current `WorkflowRunState`;
- the frozen `WorkflowExecutionPlanValue`;
- request, start, update, and terminal timestamps;
- structured terminal failure when present.

`SqliteWorkflowNodeExecutionRow` stores one planned node execution:

- Run, node, and node-execution identity;
- exact `NodeCapabilityContractRef` and selected `GenerationProfileRef` when applicable;
- current `WorkflowNodeExecutionState`, bounded progress, and timestamps;
- structured terminal failure when present.

`SqliteWorkflowNodeExecutionOutputRow` stores one successful named output:

- Run, node execution, and output port identity;
- exact `WorkflowDataType`;
- bounded Text or a typed Asset reference for Image, Video, or Audio.

`SqliteWorkflowRunEventRow` stores the monotonic per-Run event sequence used to repair missed Desktop
event delivery.

The frozen execution plan and outputs make a Run self-contained after admission. Reopening a Run
does not require a historical Workflow snapshot.

### Asset Records

`SqliteAssetAggregateRow` stores:

- Asset and Project identity;
- `AssetMediaKind` and `AssetManagedContentState`;
- `AssetManagedContentDescriptorValue`;
- versioned origin and verified media facts;
- display name and creation timestamp.

`SqliteAssetManagedContentFinalizationRow` stores only what restart recovery needs:

- Asset, managed-content, and `AssetManagedContentFinalizationId`;
- an adapter-private staging reference;
- expected digest and byte length;
- Pending, Completed, or Failed state and the last structured failure.

Only the filesystem adapter interprets the staging reference or derives a final managed path.

### Provider Credential Record

`SqliteProviderCredentialRow` stores:

- `ProviderCredentialId` and owning `ProviderAccountId`;
- authenticated-encryption version;
- random nonce;
- ciphertext;
- creation and update timestamps.

The Row never stores plaintext. The repository returns `ProviderCredentialSecretValue`, which is a
short-lived application value and is never serialized into a DTO or log.

## Representation Boundaries

```text
WorkflowAggregate
  <-> SqliteWorkflowAggregateRow

WorkflowRunAggregate
  <-> SqliteWorkflowRunAggregateRow
      + SqliteWorkflowNodeExecutionRow
      + SqliteWorkflowNodeExecutionOutputRow
      + SqliteWorkflowRunEventRow

AssetAggregate
  <-> SqliteAssetAggregateRow
      + SqliteAssetManagedContentFinalizationRow

ProviderCredentialSecretValue
  <-> SqliteProviderCredentialRow
```

Named translators reconstruct aggregates and reject corrupt or unsupported data. A Row does not
validate business rules, perform a domain transition, or become a DTO.

Persistence encoding is explicit:

- IDs use the canonical encoding of their owning newtype;
- timestamps use one UTC representation inside typed wrappers;
- enum variants use stable explicit codes, never Rust ordinal positions;
- structured payloads include a schema version and bounded size;
- every SQL value is bound as a parameter;
- unknown required variants and unsupported payload versions fail explicitly.

## Consumer-Owned Storage Ports

There is no global `Store`, `Database`, generic repository, or ambient unit of work.

| Port | Storage responsibility |
| --- | --- |
| `WorkflowAggregateRepositoryPort` | load and revision-CAS the current Workflow snapshot |
| `WorkflowRunRepositoryPort` | admit and transition Runs, outputs, and events atomically |
| `AssetAggregateRepositoryPort` | load one Asset and query a stable bounded page |
| `AssetIngestTransactionPort` | persist Pending Asset, finalization state, and availability transition |
| `AssetManagedContentStorePort` | stage, finalize, open, verify, and remove managed bytes |
| `DesktopBackendConfigReaderPort` | read validated non-secret startup configuration |
| `DesktopProviderCredentialRepositoryPort` | save, load, and delete encrypted provider credentials |

Each Port is owned by the application capability that consumes it. SQLite, filesystem, and config-
file adapters depend inward on these Ports. Concrete adapters are selected only in
`src-tauri/composition.rs`.

## Required Atomic Writes

### Workflow Save

`WorkflowAggregateRepositoryPort::save_if_revision` performs one conditional update:

```text
compare base revision with stored revision
  -> write the next WorkflowAggregate snapshot
  -> commit or return WorkflowRevisionConflict
```

The aggregate validates the candidate before the repository call. A local Desktop command is not
automatically retried. After an uncertain command result, the caller reloads the current snapshot.

### Workflow Run Admission And Transition

`WorkflowRunRepositoryPort` commits Run admission as one unit:

```text
write Queued WorkflowRunAggregate with frozen execution plan
  -> write planned node executions
  -> append the first Run event
  -> commit
  -> start execution
```

Later transactions persist only transitions approved by `WorkflowRunAggregate`. A transaction may
update the Run root, update affected node executions, insert a complete output set, and append
ordered events. Provider calls and Tauri emission occur after commit.

### Asset Ingest

`AssetIngestTransactionPort` uses two short SQLite transactions around filesystem publication:

```text
commit Pending AssetAggregate + managed-content finalization state
  -> publish or verify exact managed bytes outside SQLite
  -> commit Pending -> Available and mark finalization Completed
```

If publication cannot complete, the Asset remains Pending for bounded startup reconciliation. A
generated media output is not attached to its Run until the Asset is Available.

## Managed Media Flow

Import and generated media use the same protocol:

```text
stream into restricted staging
  -> enforce size limit
  -> calculate digest and sniff MIME
  -> extract and validate media facts
  -> flush and close staging content
  -> commit Pending Asset and finalization state
  -> atomically move or verify exact managed bytes
  -> commit Available Asset state
  -> remove obsolete staging entry
```

The filesystem adapter derives the final location from `AssetManagedContentId`; callers cannot
choose it. It rejects traversal, symlinks, unexpected file types, digest mismatch, length mismatch,
and media-kind mismatch. Managed bytes are immutable after availability.

| Failure point | Durable state | MVP response |
| --- | --- | --- |
| before Pending commit | no Asset | remove staging when possible |
| after Pending commit | Pending and finalization state | retry at startup |
| after publication, before Available commit | Pending; final bytes may exist | verify and mark Available |
| Pending with no expected bytes | Pending cannot complete | mark Missing |
| Available but bytes disappear | inconsistent Asset | detect and mark Missing |

Recovery compares content identity, digest, and length. It never searches by filename, scans another
Project, or binds similar content.

## Provider Credential Encryption

`SqliteEncryptedProviderCredentialRepositoryAdapter` performs encryption directly. The MVP does not
define a generic Cipher Port, key-provider service, operating-system vault adapter, or key-rotation
workflow.

The adapter uses:

- a maintained authenticated-encryption library;
- XChaCha20-Poly1305;
- the code-embedded `PROVIDER_CREDENTIAL_ENCRYPTION_KEY_V1`;
- a new cryptographically random nonce for every save;
- `ProviderCredentialId` and `ProviderAccountId` as authenticated associated data.

Decryption failure returns `ProviderCredentialDecryptionFailed`; it never falls back to plaintext or
an empty credential. Saving replaces the credential in one SQLite transaction. Deleting removes the
credential record.

This is deliberately basic local obfuscation. It prevents a casually opened database from exposing
the API key, but an attacker who can inspect or modify the installed binary can recover the embedded
key. The MVP does not claim protection from a fully compromised local machine.

## Reads And Preview Access

Workflow and Asset queries read SQLite directly and return bounded results. Asset lists use the
stable cursor contract from [`BACKEND_ASSETS.md`](BACKEND_ASSETS.md).

`AssetManagedContentLease` owns an open read handle and exposes no path. Provider uploads and Desktop
previews stream through that handle.

Preview access uses no database record or memory registry. The Desktop boundary issues a short-lived
process-scoped signed token. Each protocol request validates signature, expiry, Project scope,
current Asset state, and current content descriptor. Restart invalidates every preview token.

Image previews stream verified original bytes. Video and Audio previews additionally support the
bounded Range behavior from [`BACKEND_APPLICATION.md`](BACKEND_APPLICATION.md). Derived previews are
not part of the MVP.

## Startup And Restart

```text
resolve private application-data locations
  -> open SQLite and apply known migrations
  -> load non-secret DesktopBackendConfig
  -> load and decrypt required provider credentials
  -> reconcile a bounded batch of Pending Assets
  -> mark every non-terminal Workflow Run as Failed(InterruptedByRestart)
  -> construct use cases and accept Desktop commands
```

The MVP does not resume Queued Runs, Running nodes, provider polling, or paid provider tasks. The user
starts a new Run explicitly. Pending Asset reconciliation is different: it only completes local
publication already described by durable finalization state.

Correctness does not depend on graceful shutdown. Startup applies the same conservative recovery
after a crash or forced exit.

## SQLite And Migration Policy

One SQLite database belongs to one Desktop data root. The process owns its writable connection and
keeps transactions short. Blocking SQLite and filesystem work runs outside async runtime core
threads.

The MVP requires foreign-key enforcement, parameterized statements, private file permissions,
bounded queries, a bounded busy timeout, and no transaction held during filesystem or provider I/O.
It adds no application-managed connection pool or WAL-specific tuning.

The database records one schema version. Startup creates the current schema, applies known forward
migrations transactionally, refuses newer unsupported schemas, and never deletes or silently
recreates user data after a migration or integrity failure.

## Non-Secret Configuration

`DesktopBackendConfig` contains startup locations, limits, provider bindings, `ProviderAccountId`
values, and `ProviderCredentialId` references. It contains no API key.
`DesktopBackendConfigReaderPort` loads and validates the versioned configuration document once at
startup.

Development and tests may override non-secret values through environment variables. Production
provider credentials are loaded from `DesktopProviderCredentialRepositoryPort`.

## Storage Errors

Storage failures are translated into the consuming application context. Required categories include
unavailable, busy, permission denied, unsupported version, corruption, revision conflict, limit
exceeded, managed-content mismatch, and provider-credential decryption failure.

Errors contain the operation and safe typed identity needed for diagnosis. They never contain
plaintext credentials, provider bodies, signed URLs, generated text, local paths, or media bytes.

## Verification

- Workflow repository tests prove revision compare-and-swap;
- Run repository tests prove admission, transition, complete-output, and ordered-event atomicity;
- Asset fault-injection tests stop before and after each SQLite/filesystem boundary;
- restart tests prove every non-terminal Run becomes interrupted and is not resumed;
- reconciliation tests prove Pending Asset completion and exact-content Missing behavior;
- credential tests prove authenticated round-trip, unique nonce, tamper rejection, deletion, and no
  plaintext database value;
- query and preview tests prove bounds, Project isolation, expiry, Range, and path non-disclosure;
- persistence translation tests reject unknown versions, variants, and corrupt payloads.

Domain invariants remain covered by Workflow and Asset tests rather than being duplicated in storage
tests.

## Features

The following are future features, not MVP requirements:

- durable Workflow history and backend undo/redo;
- provider task persistence and Run resume;
- result, provider-response, and media-derivative caching;
- Asset deletion, retention, deduplication, and garbage collection;
- backup, restore, export/import, and downgrade support;
- full database or managed-media encryption;
- replaceable credential key storage and key rotation;
- measured SQLite connection-pool or WAL tuning;
- multiple writable processes, cloud synchronization, and collaboration;
- advanced Asset metadata and search.

Each feature must define its authority, migration, failure, and recovery behavior before changing
this MVP baseline.
