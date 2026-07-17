# Backend Asset Architecture

> Status: frozen MVP design
> Owner: `crates/assets`
> Scope: Project-local Image, Video, and Audio used by Workflow execution and preview

The Asset bounded context owns media identity, verified managed bytes, availability, technical
facts, provenance, and preview permission. It owns no Workflow edge, node parameter, provider task,
canvas state, or playback state.

Assets import the authoritative `ProjectId` from `crates/projects`. The Desktop boundary resolves
Project existence before an Asset use case; Asset then owns media visibility within that scope.

## MVP Goal

```text
user import or node-produced byte stream
  -> validate and identify managed content
  -> persist Pending Asset
  -> publish immutable bytes
  -> transition Asset to Available
  -> return one stable managed-media reference
  -> resolve for execution or issue a preview lease
```

Text is not an Asset. A path, storage key, digest, provider task, signed URL, preview URL, and SQLite
row are never Asset identity.

## DDD Structure

```text
crates/assets/src/asset/
  domain/          aggregate, content state, media facts, origin, errors
  application/     import, node output, get, list, resolve, preview, recovery
  interfaces/   repository, ingest transaction, bytes, inspection, clock, IDs
  infrastructure/  local adapters owned by this crate when appropriate
```

`AssetAggregate` approves transitions. Application use cases coordinate focused interfaces. Desktop
constructs adapters and translates cross-context values.

## Asset Aggregate

```rust
pub struct AssetAggregate {
    pub id: AssetId,
    pub project_id: ProjectId,
    pub media_kind: AssetMediaKind,
    pub content_state: AssetManagedContentState,
    pub media_facts: AssetMediaFacts,
    pub origin: AssetOrigin,
    pub display_name: AssetDisplayName,
    pub created_at: AssetCreatedAt,
}

pub enum AssetMediaKind {
    Image,
    Video,
    Audio,
}
```

`AssetId` identifies one logical media item. `AssetManagedContentId` identifies one immutable byte
object. Two Assets remain distinct even when their verified bytes match. MVP Assets are visible only
inside their owning Project.

## Frozen Asset Values

`AssetId`, `AssetImportId`, `AssetContentFinalizationId`, and `AssetPreviewLeaseId` are UUIDv4 values
stored as exact 16 bytes and displayed as lowercase hyphenated UUIDs. `AssetCreatedAt` is a
non-negative signed UTC-millisecond integer. `AssetDisplayName` is trimmed, contains 1..=255 Unicode
scalar values and no control character. `AssetOriginalFileName` has the same bound, stores only the
final file name, and rejects path separators.

`AssetContentDigest` is exactly 32 SHA-256 bytes. `AssetManagedContentId` has canonical bytes
`1 || digest`, where `1` is the one-byte scheme version; its display form is
`sha256-v1:<64 lowercase hex digits>`. Neither value is accepted from an import caller. Identity
generators reject nil and non-v4 UUIDs; ingest transactions reject identity conflicts.

## Managed Content State

```rust
pub struct AssetContentDescriptor {
    pub content_id: AssetManagedContentId,
    pub digest: AssetContentDigest,
    pub byte_length: u64,
    pub mime_type: AssetMediaMimeType,
    pub media_kind: AssetMediaKind,
}

pub enum AssetManagedContentState {
    Pending {
        descriptor: AssetContentDescriptor,
        finalization_id: AssetContentFinalizationId,
    },
    Available {
        descriptor: AssetContentDescriptor,
    },
    Missing {
        expected: AssetContentDescriptor,
        reason: AssetContentMissingReason,
    },
}
```

`AssetContentMissingReason` is exactly `FinalizationSourceMissing` or `ManagedContentMissing`.
`AssetFinalizeContentEffect` contains only `AssetContentFinalizationId`; the finalization record owns
the expected descriptor and adapter-private staging reference.

`AssetManagedContentId` is derived from a versioned SHA-256 scheme. Only the filesystem adapter maps
it to a private location. MIME is sniffed from bytes and verified against media kind; caller MIME
and extension are hints only.

Bytes are never replaced in place. Importing or producing new content creates another Asset, so an
earlier Run always references the exact bytes it recorded.

## Verified Media Facts

`AssetMediaFacts` is a closed enum:

```text
Image { width, height }
Video { width, height, duration_ms, has_audio }
Audio { duration_ms, sample_rate_hz, channels }
```

Facts are immutable observations extracted before availability. Invalid, unreadable, oversized, or
unsupported media is rejected rather than stored with guessed metadata.

The frozen accepted media contracts are:

| Kind | MIME | Byte maximum | Required fact bounds |
| --- | --- | --- | --- |
| Image | `image/png`, `image/jpeg`, `image/webp` | 32 MiB | width and height `1..=16,384` |
| Video | `video/mp4`, `video/webm` | 512 MiB | width and height `1..=16,384`; duration `1..=86,400,000` ms |
| Audio | `audio/mpeg`, `audio/wav`, `audio/ogg` | 64 MiB | duration `1..=86,400,000` ms; sample rate `8,000..=192,000` Hz; channels `1..=8` |

Video `has_audio` is an inspected Boolean. Dimensions and duration use non-zero `u32` and `u64`
values respectively. The inspector rejects animated images, multiple video programs, unknown
duration, and a container/MIME mismatch. It never transcodes or repairs content.

## Provenance

```rust
pub enum AssetOrigin {
    Imported {
        import_id: AssetImportId,
        original_file_name: AssetOriginalFileName,
    },
    WorkflowNodeOutput {
        producer: AssetWorkflowNodeOrigin,
        production: AssetNodeOutputProduction,
        output_key: AssetNodeOutputKey,
    },
}

pub enum AssetNodeOutputProduction {
    ProviderGenerated {
        generation_profile_ref: AssetOriginGenerationProfileRef,
    },
    DeterministicDerived {
        source_asset_ids: NonEmptyVec<AssetOriginSourceAssetId>,
    },
    ProviderDerived {
        source_asset_ids: NonEmptyVec<AssetOriginSourceAssetId>,
        generation_profile_ref: AssetOriginGenerationProfileRef,
    },
}
```

`AssetWorkflowNodeOrigin` records Workflow, revision, Run, node, node execution, capability, and
output through Asset-owned integration value types. `DesktopNodeCapabilityAssetBridgeAdapterImpl`
translates the Workflow values explicitly, so `crates/assets` does not depend on `crates/engine`.

The Asset-owned integration shapes are exact. `AssetOriginWorkflowId`,
`AssetOriginWorkflowRunId`, `AssetOriginWorkflowNodeId`, and
`AssetOriginWorkflowNodeExecutionId` are distinct RFC 9562 UUIDv4 values.
`AssetOriginWorkflowRevision` is a non-zero `u64`. `AssetWorkflowNodeOrigin` contains those four
identities, the revision, and one `AssetOriginNodeCapabilityContractRef`. The capability ref uses
the D0.3 capability ID grammar and non-zero-major `{ u16 major, u16 minor }` version without owning
capability semantics. `AssetOriginNodeOutputKey` uses the D0.3 1..=64-byte
`[a-z][a-z0-9_]*` key grammar. `AssetNodeOutputKey` contains Run ID, node-execution ID, output key,
and unrestricted `u32` ordinal; its Run and node-execution IDs must equal those in the producer.

`AssetOriginGenerationProfileRef` mechanically stores a Generation Profile ID using the exact
3..=128-byte dot-separated grammar from `BACKEND_PROVIDERS.md` and a non-zero `u32` version. It
does not own catalog, lifecycle, compatibility, availability, provider, or model semantics.
`AssetOriginSourceAssetId` mechanically wraps one Asset-owned `AssetId`. Derived source lists are
non-empty, preserve supplied order, and do not infer, sort, or deduplicate sources. Imported and
node-output origin construction rejects every inconsistent shape as `InvalidOrigin`.

Provenance never copies prompt text, provider/native model, provider task, route, progress, cost, or
path. Imported origin never retains the user's absolute source path.

## Node-Output Idempotency

```rust
pub struct AssetNodeOutputKey {
    pub workflow_run_id: AssetOriginWorkflowRunId,
    pub node_execution_id: AssetOriginWorkflowNodeExecutionId,
    pub output_key: AssetOriginNodeOutputKey,
    pub ordinal: u32,
}
```

The key identifies one durable media output slot. `AssetRecordNodeOutputUseCase` returns the existing
Asset when the key and content digest match. The same key with different bytes returns
`AssetNodeOutputConflict`; it never silently rebinds the slot.

`AssetNodeOutputSourceLease` is the node-output equivalent of `AssetImportSourceLease`: it owns one
already-open `Pin<Box<dyn AsyncRead + Send>>`, one caller deadline, and one consuming
`try_take_stream` operation. It is process-local, non-cloneable, non-serializable, non-persisted, and
has no path conversion. It returns `DeadlineExceeded` when consumed at or after its deadline and
does not buffer, rewind, retry, inspect, or own cancellation.

`AssetRecordNodeOutputCommand` contains exactly the trusted Project ID, expected media kind, display
name, translated `AssetWorkflowNodeOrigin`, `AssetNodeOutputProduction`, matching
`AssetNodeOutputKey`, and one `AssetNodeOutputSourceLease`. It accepts no provider/model/route,
caller identity, MIME, digest, media facts, path, URL, prompt, or original filename. Construction
rejects producer/output-key disagreement as `IdentityConflict`. The source deadline bounds staging,
inspection, transaction, inline finalization, and any cleanup.

`record_asset_node_output` returns an `AssetAggregate` only when it is Available. It stages once and
calculates the digest before replay lookup. The use case owns this exact decision:

1. Check the deadline, observe one `AssetCreatedAt`, stage with that timestamp, and perform replay
   lookup from the calculated digest. If no Asset is bound, inspect the staged bytes and generate
   Asset/finalization IDs once, then attempt the atomic node-output Pending commit. Replay paths do
   not generate identities; their observed staging time is used only for stale cleanup ordering.
2. If lookup or the atomic commit returns an existing binding, require the same Project, media kind,
   producer, production, output key, descriptor digest, and byte length. Any difference removes the
   new staging once and returns `NodeOutputConflict`.
3. An exact existing Available Asset removes the new staging once and returns that Asset. An exact
   Pending Asset removes the new staging, immediately replays its own committed finalization, and
   returns it only if it becomes Available. An exact Missing Asset removes the new staging and
   returns `ContentMissing`; the new staging is never substituted into an old finalization.
4. A newly committed Pending Asset immediately finalizes through
   `AssetFinalizeContentUseCase`. Available is returned; Pending returns `ContentPending`; Missing
   returns `ContentMissing`. Every finalization error propagates unchanged rather than being
   converted to a successful Pending result.

Inspection, value construction, or transaction failure before a successful Pending commit attempts
one idempotent removal of the new staging and preserves the primary error. The transaction race
result is evaluated by the same replay decision as the initial lookup. Cleanup failure is logged and
left for stale-staging reconciliation; it never replaces an existing-Asset result or the primary
error. The use case does not attach a Workflow output, claim/complete an outbox effect, retry,
restage, replace old staging, or run outputs concurrently. Workflow owns the later all-or-nothing
`WorkflowNodeOutputSet` commit.

This closes the failure window where content becomes Available but the Workflow output commit is
uncertain. A late Asset may remain durable after cancellation, but Workflow rejects its late output
association and never reports the cancelled node as succeeded.

## Aggregate Invariants

- identity, Project, media kind, descriptor, facts, and origin are immutable;
- descriptor digest, length, MIME, media kind, and verified bytes agree;
- legal MVP transitions are `Pending -> Available`, `Pending -> Missing`, and
  `Available -> Missing`;
- `Missing -> Available` requires the exact expected digest and length;
- finalization is idempotent by `AssetContentFinalizationId`;
- node-output identity is idempotent by `AssetNodeOutputKey`;
- only the owning Project may get, list, resolve, or preview an Asset;
- paths and preview URLs never enter the aggregate.

Repositories persist transitions approved by `AssetAggregate`; they expose no generic status
setter.

## Frozen MVP Use Cases

| Use case and method | Responsibility |
| --- | --- |
| `AssetImportUseCase::import_asset` | validate a trusted user file handle and create an Asset |
| `AssetRecordNodeOutputUseCase::record_asset_node_output` | validate one node-produced stream and create/reuse its exact output Asset |
| `AssetRecoverNodeOutputUseCase::recover_asset_node_output` | inspect an exact node-output key without a source stream and return Available, Pending with durable finalization, or SourceRequired |
| `AssetFinalizeContentUseCase::finalize_asset_content` | consume one committed finalization effect and publish exact managed bytes |
| `AssetGetUseCase::get_asset` | return one Project-visible Asset |
| `AssetListUseCase::list_assets` | return one stable bounded Project page |
| `AssetResolveContentUseCase::resolve_asset_content` | return opaque managed-byte access for execution |
| `AssetIssuePreviewUseCase::issue_asset_preview` | grant short-lived preview permission |
| `AssetReconcileContentUseCase::reconcile_asset_content` | finish a bounded batch of interrupted local publication |

Delete, archive, purge, tags, collections, search, export, remote import, derivatives, and garbage
collection are not MVP use cases.

## Import Flow

`AssetImportCommand` contains the trusted Project ID, expected `AssetMediaKind`, display name, final
original file name, and one `AssetImportSourceLease` created from a Tauri-owned already-open file
handle. It never accepts a reusable path, caller MIME, digest, content ID, media facts, or identity.
The source lease deadline is the deadline for staging, inspection, the Pending commit, and the first
inline finalization attempt.

`import_asset` returns the committed `AssetAggregate` directly. A successful inline finalization
returns it as Available. A transient post-commit finalization error is handled and logged by the use
case and returns the still-Pending aggregate, because its durable effect remains replayable. A
confirmed absence of both exact staged and managed bytes returns the durably Missing aggregate.
Errors before the Pending commit return `AssetApplicationError` and no Asset.

For the inline attempt, exactly `ManagedStorageFailed`, `Cancelled`, and `DeadlineExceeded` are
deferred transient errors and therefore return the committed Pending aggregate. `NotFound`,
`FinalizationFailed`, `IdentityConflict`, and every other error category propagate because they
indicate a contract, identity, or input failure rather than an approved deferred publication. The
Pending aggregate and effect remain durable even when such an error propagates. Import does not add
an outcome wrapper or reinterpret an error from the finalization use case.

```text
open bounded source stream
  -> write restricted staging content
  -> calculate digest and sniff MIME
  -> extract and validate AssetMediaFacts
  -> atomically commit Pending Asset + finalization + AssetFinalizeContentEffect
  -> after commit, AssetImportUseCase calls AssetFinalizeContentUseCase
  -> publish exact managed bytes and transition Asset to Available
```

Validation or initial database failure leaves no aggregate and removes staging when possible. A
failure after the Pending commit remains recoverable and is never reported as Available. Import may
return Pending; the post-commit worker or startup reconciliation retries the same exact effect.

Import observes one `AssetCreatedAt`, then generates Asset, import, and finalization identities
exactly once before staging. It stages, reopens the exact staged reference for inspection, constructs
the descriptor/origin/Pending aggregate/finalization/effect, and calls
`commit_imported_pending_asset`. If staging succeeds but inspection, value construction, or the
Pending transaction fails, it attempts one idempotent staged-content removal. Cleanup failure is
logged where handled and never replaces the primary error. There is no identity retry, source
rewind, second staging attempt, or filename-derived behavior.

## Finalization

`AssetFinalizeContentCommand` contains one committed `AssetFinalizeContentEffect` and the caller's
process-monotonic deadline. `finalize_asset_content` returns the current durable `AssetAggregate`
directly.

`AssetFinalizeContentUseCase::finalize_asset_content` performs exactly this idempotent decision:

1. Find the finalization and its Asset. An absent finalization returns `NotFound`; an absent or
   identity-inconsistent owning Asset returns `FinalizationFailed`.
2. If the Asset is Available with the exact descriptor, return it without store mutation. If it is
   Missing with the exact descriptor, return it. A different descriptor, finalization, or state
   returns `FinalizationFailed`.
3. For the matching Pending Asset, open the staged source. When present, publish it under the exact
   descriptor, commit Available, attempt one idempotent staged removal, and return Available.
4. When staging is absent, verify exact managed bytes. A match commits Available and returns it.
   Otherwise commit Missing with `FinalizationSourceMissing` and return Missing.

Store/publish/transaction/cancellation/deadline errors leave the already-committed Pending state and
propagate their frozen category. Staged cleanup failure after an Available commit is logged and does
not change the successful result; stale-staging reconciliation owns later cleanup. This use case
does not claim or complete the Desktop outbox effect, retry, sleep, or schedule work.

## Node-Produced Media Flow

```text
GenerationTaskEffectWorkerImpl finalization
  -> GenerationTaskAssetSinkInterface::recover_generation_task_asset
  -> AssetRecoverNodeOutputUseCase::recover_asset_node_output
  -> Available | Pending | SourceRequired
  -> only SourceRequired continues with provider poll/result bytes
  -> GenerationTaskAssetSinkInterface::store_generation_task_asset
  -> DesktopGenerationTaskAssetSinkAdapterImpl
  -> AssetRecordNodeOutputUseCase::record_asset_node_output
  -> Pending + AssetFinalizeContentEffect
  -> AssetFinalizeContentUseCase::finalize_asset_content after commit
  -> Available
  -> typed Workflow managed-media reference
```

The bridge translates Project scope, media kind, node provenance, source Asset IDs, profile ref,
and Generation Task output coordinates into Asset-owned values including
`AssetNodeOutputKey`. It never gives node code an Asset repository, SQLite connection, path, or
preview URL.

A Generation Task publishes its result only after its required Asset is Available. The sink uses
the canonical Asset-owned Pending/finalization protocol and returns only an Available Asset. The
Task remains active while the Asset effect may finalize inline, in its worker, or during startup;
it commits the Asset result only with Task success. Workflow attaches the result only after task
notification. The task worker waits within the persisted Generation Task deadline, never a
process-only Node deadline. `AssetRecoverNodeOutputUseCase` performs the key-only recovery read: an
Available Asset is reused, Pending causes safe Task rescheduling while the durable Asset effect
finishes, and only SourceRequired permits polling the persisted provider handle for result bytes.
No code publishes bytes before the Asset effect is durable.

## Resolve And List

```rust
pub struct AssetGetQuery {
    pub project_id: ProjectId,
    pub asset_id: AssetId,
}

pub struct AssetResolveContentQuery {
    pub project_id: ProjectId,
    pub asset_id: AssetId,
    pub expected_media_kind: AssetMediaKind,
    pub deadline: Instant,
}

pub struct AssetIssuePreviewCommand {
    pub project_id: ProjectId,
    pub asset_id: AssetId,
}
```

These are illustrative field listings; the Rust values keep fields private and expose noun-specific
constructors and accessors. `AssetGetUseCase::get_asset` loads by global Asset identity, returns
`NotFound` for absence, `NotVisible` for a different Project, and otherwise returns the aggregate
without changing or hiding its content state.

`AssetListUseCase::list_assets` accepts the existing `AssetListQuery` and returns exactly the
repository's `AssetListPage`. The use case adds no default limit, filtering, sorting, cursor
translation, total, state suppression, or content verification. Project, optional kind, cursor, and
validated limit remain mandatory repository filters, and repository errors propagate unchanged.

`AssetResolveContentUseCase::resolve_asset_content` checks the caller deadline, loads by Asset ID,
then verifies Project visibility, exact kind, and content state in that order. Absence returns
`NotFound`; a different Project returns `NotVisible`; a different kind returns
`MediaKindMismatch { expected: AssetMediaKind, observed: AssetMediaKind }`; Pending returns
`ContentPending`; Missing returns `ContentMissing`. For
Available, it calls `open_managed_asset_content` once with the exact descriptor and deadline.
`None` returns `ContentMissing`; an open failure propagates unchanged. Success returns an
`AssetResolvedContent` containing that exact `AssetContentDescriptor` and
`AssetManagedContentLease`. The lease is bounded and opaque; provider upload and Desktop preview
can stream it but cannot discover a path. Resolution does not mark content Missing, issue a preview,
retry, or repair storage.

`AssetManagedContentLease` is call-scoped to one exact content ID and byte length, supports one
forward asynchronous stream, and expires at its caller-supplied deadline. `AssetImportSourceLease`
is the equivalent one-shot opaque stream over the already-open trusted file handle. Neither lease
is cloneable, serializable, persisted, or convertible to a path.

The two stream leases are application values. Each owns one
`Pin<Box<dyn AsyncRead + Send>>` and a process-monotonic `Instant` deadline. Construction accepts an
already-open stream and validates no bytes. `AssetManagedContentLease` additionally carries its
exact `AssetManagedContentId` and byte length; `AssetImportSourceLease` carries no file identity or
path.
The only stream operation consumes the lease. It returns `DeadlineExceeded` when observed at or
after the deadline and otherwise returns the owned stream. Reading after that handoff remains
bounded by the same caller deadline at the consuming use case; the lease does not spawn work,
retry, buffer, rewind, inspect, or orchestrate replay.

`AssetListQuery` accepts Project, optional media kind, opaque cursor, and limit `1..=100`. Ordering
is always `(created_at DESC, asset_id DESC)`, and the cursor contains both values.

Pending, Missing, wrong-kind, wrong-Project, and absent content are distinct structured outcomes.

## Preview

`AssetIssuePreviewUseCase` creates a short-lived `AssetPreviewLease`. The Desktop protocol adapter
translates it into a process-scoped `desktop-asset://` URL:

```text
Image -> verified image MIME
Video -> MIME, Content-Length, ETag, and one valid byte Range
Audio -> MIME, Content-Length, ETag, and one valid byte Range
```

Every request rechecks token signature, expiry, Project, current Asset state, and content
descriptor. It supports bounded `GET` and `HEAD`, sets `nosniff`, and exposes no managed path. React
owns zoom, seek, playback, volume, and object-URL lifetime.

`issue_asset_preview` loads by Asset ID, then verifies Project visibility and content state in that
order. Absence returns `NotFound`, a different Project returns `NotVisible`, Pending returns
`ContentPending`, and Missing returns `ContentMissing`. Only for Available content does it observe
the Asset clock once, generate one preview lease ID, and construct the lease from the exact current
content ID. Clock, identity, and lease-construction errors propagate unchanged. Issuance does not
open bytes, sign a token, choose a URL, accept a caller expiry, retry identity generation, or perform
protocol Range handling. The protocol request performs its documented fresh Asset and content
checks before access.

`AssetPreviewLease` contains an `AssetPreviewLeaseId`, Project ID, Asset ID, exact content ID,
issued-at, and expiry. Its lifetime is exactly five minutes; the signed protocol token is
process-scoped and is not persisted. Image rejects Range. Video and Audio accept either no Range or
one `bytes=start-end`, `bytes=start-`, or `bytes=-suffix` range. Multiple ranges, invalid syntax,
zero suffix, unsatisfiable bounds, or a range spanning more than 16 MiB is rejected. `HEAD` returns
the same status and headers as `GET` without opening or returning a body.

Preview lease timestamps use the same non-negative epoch-millisecond representation as Asset
timestamps. Construction derives expiry as exactly `issued_at + 300_000`; callers cannot supply a
different expiry. A negative issue time or timestamp overflow returns `PreviewLeaseInvalid`.
`AssetPreviewLease` is an immutable process-local application value. It contains no signed token and
grants no content access by itself.

`AssetPreviewPolicy` is the non-secret Desktop configuration representation of these fixed protocol
bounds. It contains exactly `lease_lifetime_ms: 300_000` and
`max_range_bytes: 16_777_216`. Both values must equal those constants; configuration cannot weaken,
extend, or disable either bound.

## Recovery

`AssetReconcileContentUseCase` and the post-commit worker process bounded recovery candidates at
startup:

- publish or verify the exact expected managed bytes when staging exists;
- mark Pending content Missing when neither exact staging nor managed bytes can complete it;
- mark Available content Missing when its exact managed bytes are absent;
- remove stale unreferenced staging entries after a configured retention window.

One reconciliation call accepts limit `1..=100`, default `50`, independently for unfinished
finalizations, Available-content checks, and stale staging; it returns one opaque continuation cursor
for each non-exhausted class. Finalizations and Assets order by creation time then identity ascending;
adapter-private staging orders by creation time then private reference. Each candidate is claimed at
most once in that call. Unreferenced staging is stale after 24 hours. Effect delivery is at-least-once;
`AssetContentFinalizationId` is its business and idempotency identity. Attempts are bounded by the
caller deadline, not by an Asset retry counter or new terminal state.

`AssetReconcileContentCommand` contains the caller deadline, optional matching cursors for unfinished
finalizations, Available verification, and stale staging, plus an optional `AssetPageLimit` that
defaults to `50`. The same effective limit applies independently to all three pages.
`AssetReconcileContentResult` contains only the three optional next cursors returned by those pages;
it contains no totals, retry state, failure list, or scheduling hint.

One call observes the clock once, derives the stale cutoff by saturating `now - 24 hours` at epoch
zero, and processes classes in this exact order:

1. call `finalize_asset_content` once for each unfinished finalization page item;
2. verify each Available Asset's exact descriptor and commit `ManagedContentMissing` when absent;
3. for each stale staged item, query its exact reference and remove it only when unreferenced.

Each class preserves its interface page order. The deadline is checked before each candidate. A
confirmed missing object is a successful state transition and processing continues. Cancellation,
deadline, repository, store, or transaction failure stops the call immediately with its frozen
error; already-committed earlier work remains valid and an error result exposes no advanced cursor.
Replaying the same input is safe through finalization, transition, verification, and removal
idempotency. Reconciliation does not claim outbox effects, run candidates concurrently, retry a
candidate, sleep, or invent a recovery state.

`AssetReconciliationPolicy` contains exactly `page_limit: 50`,
`operation_deadline_ms: 30_000`, and `stale_staging_after_ms: 86_400_000`. These are fixed startup
configuration values: all three must equal their documented constants. The Desktop recovery caller
derives one monotonic deadline 30 seconds from invocation and passes the page limit to the command;
the Asset use case remains the sole owner of class order, stale-cutoff derivation, and fail-fast
behavior.

Recovery compares content ID, digest, and length. It never searches by filename, crosses Project
scope, substitutes similar bytes, or changes an output key.

## Consumer-Owned Interfaces

| Interface | Explicit behavior |
| --- | --- |
| `AssetRepositoryInterface` | load Assets, resolve an output key, and perform stable bounded queries |
| `AssetIngestTransactionInterface` | atomically commit Pending/finalization and approved availability transitions |
| `AssetManagedContentStoreInterface` | stage, publish, open, verify, and remove stale staged bytes |
| `AssetMediaInspectorInterface` | sniff MIME and extract verified media facts |
| `AssetClockInterface` | provide deterministic Asset timestamps |
| `AssetIdentityGeneratorInterface` | create Asset, import, finalization, and preview-lease identities |

Fake and production implementations run the same ordering, idempotency, Project-isolation,
transaction, and failure contract suites.

### Interface-Owned Values

The six interfaces use only Asset domain/application values and the values frozen here. They do not
accept rows, paths, provider values, Desktop effect envelopes, or generic byte payloads.

- `AssetStagedContentRef` is an opaque, equality-comparable `1..=512`-byte value originating from a
  managed-content store implementation. Consumers may retain and return its bytes but cannot
  interpret them. It is persisted only inside an Asset finalization record, has no text/path
  conversion, and is never returned to Desktop or node callers.
- `AssetStagedContent` contains one `AssetStagedContentRef`, exact digest, byte length, and the
  eventual aggregate's `AssetCreatedAt`. Staging calculates digest and length while copying the
  source once; it does not inspect media.
- `AssetInspectedMedia` contains only verified MIME and `AssetMediaFacts`. Its kind must equal the
  requested kind; descriptor construction remains application/domain behavior.
- `AssetContentFinalization` contains finalization ID, Asset ID, exact descriptor, staged-content
  reference, and the aggregate's `AssetCreatedAt`. It contains no retry count, path, effect state, or
  failure text.
- `AssetFinalizeContentEffect` contains only the finalization ID.
- `AssetGetQuery` contains Project ID and Asset ID. `AssetResolveContentQuery` contains Project ID,
  Asset ID, expected media kind, and the caller's process-monotonic deadline.
- `AssetResolvedContent` contains exactly one descriptor and one managed-content lease; it is not
  cloneable or serializable and exposes no path or preview token.
- `AssetIssuePreviewCommand` contains only Project ID and Asset ID. Preview lifetime and identity are
  application-owned and cannot be supplied by a caller.
- `AssetListCursor` contains `(created_at, asset_id)`. `AssetListQuery` contains Project ID, optional
  media kind, optional cursor, and `AssetPageLimit`. `AssetPageLimit` is one shared validated
  `1..=100` value. `AssetListPage` contains ordered Assets and an optional cursor for the next
  non-empty page; it contains no total count.
- `AssetFinalizationRecoveryCursor` contains `(created_at, finalization_id)` and
  `AssetAvailableContentRecoveryCursor` contains `(created_at, asset_id)`. Staged-content recovery
  uses `AssetStagedContentRecoveryCursor { created_at, staged_content_ref }`, with reference bytes
  ordered lexicographically.
- `AssetContentFinalizationRecoveryPage`, `AssetAvailableContentRecoveryPage`, and
  `AssetStagedContentRecoveryPage` each contain only their named ordered items and an optional next
  cursor of the matching kind.

No cursor is accepted across cursor kinds. A page returns a next cursor only when another matching
record exists. Reusing the same query and cursor against unchanged state returns the same ordered
page.

`AssetPageLimit::from_u16` returns `None` outside `1..=100`; `get` returns the accepted `u16`.
`AssetStagedContentRef::try_from_store_bytes` returns `ManagedStorageFailed` for an empty or oversized
value, and `as_store_bytes` exposes the opaque borrowed bytes only for persistence/store adapters.
All cursor constructors are infallible from their already-validated typed fields. Finalization and
command constructors reject inconsistent Asset, descriptor, staging, output-key, or finalization
identities as `IdentityConflict`; their fields remain private and are exposed through noun-specific
accessors.

### `AssetRepositoryInterface`

The repository is read-only from the consumer's perspective and exposes exactly these methods:

| Method | Exact input | Result and invariant |
| --- | --- | --- |
| `find_asset_by_id` | `AssetId` | one Asset by global identity, or `None`; the use case owns Project visibility |
| `find_asset_by_node_output_key` | `AssetNodeOutputKey` | one Asset bound to the exact output key, or `None` |
| `list_project_assets` | `AssetListQuery` | one stable `AssetListPage`, ordered `(created_at DESC, asset_id DESC)` |
| `find_asset_content_finalization` | `AssetContentFinalizationId` | one finalization by exact identity, or `None` |
| `list_unfinished_asset_content_finalizations` | optional finalization cursor and validated limit | ascending recovery page by `(created_at, finalization_id)` |
| `list_available_assets_for_content_verification` | optional Available cursor and validated limit | ascending recovery page by `(created_at, asset_id)` |
| `is_asset_staged_content_referenced` | `AssetStagedContentRef` | whether any unfinished finalization owns the exact reference |

Every method is asynchronous. Reads return `ManagedStorageFailed` only for repository I/O or decode
failure. They never translate absence into an application error and never apply Project visibility,
media-kind, content-state, replay, or transition rules.

### `AssetIngestTransactionInterface`

This interface owns exactly four atomic writes; it does not expose a generic transaction callback:

| Method | Atomic behavior |
| --- | --- |
| `commit_imported_pending_asset` | insert one imported Pending aggregate, its finalization, and its Asset effect |
| `commit_workflow_node_output_pending_asset` | insert one node-output key, Pending aggregate, finalization, and Asset effect, or return the Asset already bound to that key |
| `commit_finalized_asset_content_available` | persist an already-approved exact aggregate transition and complete its finalization |
| `commit_asset_content_missing` | persist an already-approved Missing transition and complete any supplied finalization |

`AssetCommitPendingContentCommand` contains the complete Pending aggregate,
`AssetContentFinalization`, and `AssetFinalizeContentEffect`; all three Asset, descriptor, and
finalization identities must agree. Both Pending methods accept exactly this command. The
node-output commit result is exactly `Committed` or `OutputKeyAlreadyBound { asset }`; the imported
commit returns only success. The transaction implementation reports the existing binding but never
decides same-content replay; `AssetRecordNodeOutputUseCase` remains the sole replay/conflict owner.

`AssetCommitFinalizedContentAvailableCommand` contains the already-transitioned Available aggregate
and exact finalization ID. `AssetCommitContentMissingCommand` contains the already-transitioned
Missing aggregate and an optional finalization ID; the ID is required for a Pending-origin
transition and absent for an Available-origin transition. These are the exact inputs of the two
transition methods.

Pending commits write the closed Asset effect into the Desktop outbox as part of the same storage
transaction, but this interface never claims, releases, completes, or abandons that effect. Those
operations remain owned exclusively by `DesktopPostCommitEffectOutboxInterface`.

Availability commit requires the current stored state to be the exact matching Pending state and the
same finalization to remain unfinished. Repeating the same completed finalization is success only
when the stored Asset is already Available with the exact descriptor. Missing commit accepts only a
domain-approved `Pending -> Missing` or `Available -> Missing` aggregate. Any identity collision,
stale/different state, or different completed finalization returns `IdentityConflict`; no partial
row, finalization, output-key binding, or effect write is visible.

### `AssetManagedContentStoreInterface`

This interface exposes exactly these asynchronous byte-store operations:

| Method | Exact input | Behavior |
| --- | --- | --- |
| `stage_imported_asset_content` | import-source lease, expected `AssetMediaKind`, and `AssetCreatedAt` | consume the imported source once and return digest/length/reference facts |
| `stage_node_output_asset_content` | node-output source lease, expected `AssetMediaKind`, and `AssetCreatedAt` | consume the node-produced source once and return digest/length/reference facts |
| `open_staged_asset_content` | staged reference and caller deadline | return a one-shot import-source lease, or `None` |
| `publish_staged_asset_content` | staged reference, exact descriptor, and caller deadline | verify digest and length while idempotently publishing under the descriptor content ID |
| `open_managed_asset_content` | exact descriptor and caller deadline | return an exact managed-content lease, or `None` |
| `verify_managed_asset_content` | exact descriptor and caller deadline | return whether exact managed bytes match descriptor digest and length |
| `list_stale_asset_staged_content` | exclusive `AssetCreatedAt` cutoff, optional staging cursor, and validated limit | return one ascending bounded page by `(created_at, staged_content_ref)` |
| `remove_asset_staged_content` | exact staged reference and caller deadline | idempotently remove one exact staged object |

`stage_imported_asset_content` rejects an empty stream as `InvalidMedia` and an oversized stream as
`MediaSizeLimitExceeded`; it applies the documented maximum for the supplied expected kind. Source
read or staging write failures map to `ManagedStorageFailed`, and lease expiry maps to
`DeadlineExceeded`. Publishing existing equal bytes is success; existing different bytes return
`ContentDigestMismatch`. An absent staging source returns `ContentMissing`. No method returns a path,
seeks or reopens a supplied source lease, inspects MIME, creates an Asset, or removes managed content.

`stage_node_output_asset_content` has exactly the same digest, length, bound, deadline, error, and
staging-creation semantics as `stage_imported_asset_content`, but accepts only
`AssetNodeOutputSourceLease`. The two methods remain separate so import and node-produced sources
cannot be exchanged accidentally. Implementations may share private copy logic; there is no public
generic staging-source enum, conversion, trait, or unsupported branch.

The stale-staging page contains only `AssetStagedContent` facts and a next cursor. The reconcile use
case checks `is_asset_staged_content_referenced` before each removal; the store never guesses database
reachability.

### `AssetMediaInspectorInterface`

`inspect_asset_media` is its only method. Its exact input is one staged-content stream lease plus the
expected media kind. It consumes the stream and returns `AssetInspectedMedia`. It sniffs MIME and
extracts the frozen facts in one bounded inspection; it ignores caller file extension and MIME hints.
Unsupported bytes, animated images, multiple video programs, unknown duration, container/MIME
mismatch, or fact-bound failure returns `InvalidMedia`. Process/decoder/read failure returns
`InspectionFailed`; cancellation and deadline retain their same-named errors. It never stages,
publishes, transcodes, repairs, or creates a descriptor or Asset.

### Clock And Identity Interfaces

`AssetClockInterface::current_asset_time` is the only clock method and returns a validated
`AssetCreatedAt`; clock failure or out-of-range time returns `IdentityConflict` without fallback.
`AssetIdentityGeneratorInterface` exposes exactly `generate_asset_id`, `generate_asset_import_id`,
`generate_asset_content_finalization_id`, and `generate_asset_preview_lease_id`. Every method returns
its named validated UUIDv4 value. Entropy failure or nil/non-v4 output returns `IdentityConflict`;
transactions independently report persisted collisions as `IdentityConflict`. Generator methods do
not retry or consult persistence.

All six interfaces are `Send + Sync`. They use behavior-specific method names, return
`AssetApplicationError`, and have no default methods or unsupported-operation branch. Contract tests
must run unchanged against every fake and production implementation and cover absence, exact
ordering/cursors, idempotent publication/finalization, collision, partial-write rollback, deadline,
cancellation, and fault mapping.

Production adapter names are frozen as `SqliteAssetRepositoryAdapterImpl`,
`SqliteAssetIngestTransactionAdapterImpl`, `LocalFilesystemAssetManagedContentStoreAdapterImpl`,
`ImageAndFfprobeAssetMediaInspectorAdapterImpl`, `SystemAssetClockAdapterImpl`, and
`UuidV4AssetIdentityGeneratorAdapterImpl`. SQLite owns metadata transactions, the restricted local
filesystem owns staging and managed bytes, the Rust image decoder inspects images, and a bundled
`ffprobe` process inspects Video and Audio. Only `DesktopCompositionRoot` constructs them. Adapter
private paths, process output, rows, and handles never cross an Asset interface.

## Errors And Verification

`AssetDomainError` categories are exactly `InvalidIdentity`, `InvalidDisplayName`,
`InvalidOriginalFileName`, `InvalidDescriptor`, `InvalidMediaFacts`, `InvalidOrigin`,
`InvalidTransition`, and `FinalizationIdentityMismatch`. `AssetApplicationError` adds exactly
`NotFound`, `NotVisible`,
`MediaKindMismatch { expected: AssetMediaKind, observed: AssetMediaKind }`, `ContentPending`, `ContentMissing`, `InvalidMedia`,
`MediaSizeLimitExceeded`, `ContentDigestMismatch`, `NodeOutputConflict`, `ManagedStorageFailed`,
`IdentityConflict`, `InspectionFailed`, `FinalizationFailed`, `PreviewLeaseInvalid`,
`PreviewLeaseExpired`, `PreviewRangeInvalid`, `Cancelled`, and `DeadlineExceeded`. Errors and their
adjacent command or query results use only safe typed identities, never paths, tokens, process
output, or raw content.

Only `MediaKindMismatch` carries the two distinct safe kinds; every other MVP
`AssetApplicationError` variant carries no payload. Query and command values already retain the
other safe typed identities needed by their caller, while adapters record private diagnostics only at
the boundary where the failure is handled. Implementations must not add a catch-all source error,
message string, retry hint, provider detail, path, token, process output, or raw bytes to this public
error. Domain-value construction failures remain `AssetDomainError` and are not duplicated as an
application-error variant.

Value construction maps non-v4 Asset-owned UUIDs to `InvalidIdentity`, negative
`AssetCreatedAt` to `InvalidDescriptor`, invalid display/original names to their same-named
categories, descriptor/content inconsistencies to `InvalidDescriptor`, and technical-fact bound
violations to `InvalidMediaFacts`. No generic validation-message or catch-all category exists.

Verification covers:

- aggregate transitions, Project visibility, immutable facts, and provenance;
- import and node-output success plus every failure boundary;
- output-key same-content replay and different-content conflict;
- behavioral equivalence across fake, SQLite, and filesystem implementations;
- effect consumption, startup reconciliation, and exact-content Missing behavior;
- list ordering/bounds and preview MIME, Range, expiry, and path non-disclosure;
- Desktop bridge translation and no Workflow output before availability.

## Post-MVP

Archive/delete/purge, tags, search, export, remote import, thumbnails, posters, waveforms, content
retention, logical Asset deduplication, garbage collection, cloud sync, multiview, 3D, and scene
Assets require separate product and migration decisions.
