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
| `AssetFinalizeContentUseCase::finalize_asset_content` | consume one committed finalization effect and publish exact managed bytes |
| `AssetGetUseCase::get_asset` | return one Project-visible Asset |
| `AssetListUseCase::list_assets` | return one stable bounded Project page |
| `AssetResolveContentUseCase::resolve_asset_content` | return opaque managed-byte access for execution |
| `AssetIssuePreviewUseCase::issue_asset_preview` | grant short-lived preview permission |
| `AssetReconcileContentUseCase::reconcile_asset_content` | finish a bounded batch of interrupted local publication |

Delete, archive, purge, tags, collections, search, export, remote import, derivatives, and garbage
collection are not MVP use cases.

## Import Flow

`AssetImportCommand` contains trusted Project scope and a Tauri-owned file handle. It never accepts a
reusable path from Workflow JSON.

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

## Node-Produced Media Flow

```text
exact capability
  -> NodeCapabilityProducedMediaWriterInterface::write_node_output_media
  -> DesktopNodeCapabilityAssetBridgeAdapterImpl
  -> AssetRecordNodeOutputUseCase::record_asset_node_output
  -> Pending + AssetFinalizeContentEffect
  -> AssetFinalizeContentUseCase::finalize_asset_content after commit
  -> Available
  -> typed Workflow managed-media reference
```

The bridge translates Project scope, media kind, node provenance, source Asset IDs, profile ref,
and `NodeCapabilityProducedMediaOutputKey` into Asset-owned values including
`AssetNodeOutputKey`. It never gives node code an Asset repository, SQLite connection, path, or
preview URL.

A capability publishes its `WorkflowNodeOutputSet` only after every required output Asset is
Available. If one output fails, already-created Assets retain provenance but no partial Workflow
output set is committed. The node-output writer waits within the node deadline for its exact
finalization. The first finalization attempt happens inline after the outbox commit, so a Run never
waits for the worker executing that same Run. Failure leaves the effect available for the worker or
startup reconciliation; no code publishes bytes before the effect is durable.

## Resolve And List

```rust
pub struct AssetResolveContentQuery {
    pub project_id: ProjectId,
    pub asset_id: AssetId,
    pub expected_media_kind: AssetMediaKind,
}
```

Resolution verifies Project visibility, exact kind, `Available` state, and current content
existence. It returns `AssetManagedContentLease` plus `AssetContentDescriptor`. The lease is bounded
and opaque; provider upload and Desktop preview can stream it but cannot discover a path.

`AssetManagedContentLease` is call-scoped to one exact content ID and byte length, supports one
forward asynchronous stream, and expires at its caller-supplied deadline. `AssetImportSourceLease`
is the equivalent one-shot opaque stream over the already-open trusted file handle. Neither lease
is cloneable, serializable, persisted, or convertible to a path.

The two stream leases are application values. Each owns one
`Pin<Box<dyn AsyncRead + Send>>` and a process-monotonic `Instant` deadline. Construction accepts an
already-open stream and validates no bytes. `AssetManagedContentLease` additionally carries its
exact `AssetContentId` and byte length; `AssetImportSourceLease` carries no file identity or path.
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

Recovery compares content ID, digest, and length. It never searches by filename, crosses Project
scope, substitutes similar bytes, or changes an output key.

## Consumer-Owned Interfaces

| Interface | Explicit behavior |
| --- | --- |
| `AssetRepositoryInterface` | load Assets, resolve an output key, and perform stable bounded queries |
| `AssetIngestTransactionInterface` | atomically commit Pending/finalization and approved availability transitions |
| `AssetManagedContentStoreInterface` | stage, publish, open, verify, and remove managed bytes |
| `AssetMediaInspectorInterface` | sniff MIME and extract verified media facts |
| `AssetClockInterface` | provide deterministic Asset timestamps |
| `AssetIdentityGeneratorInterface` | create Asset, import, finalization, and preview-lease identities |

Fake and production implementations run the same ordering, idempotency, Project-isolation,
transaction, and failure contract suites.

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
`NotFound`, `NotVisible`, `MediaKindMismatch`, `ContentPending`, `ContentMissing`, `InvalidMedia`,
`MediaSizeLimitExceeded`, `ContentDigestMismatch`, `NodeOutputConflict`, `ManagedStorageFailed`,
`IdentityConflict`, `InspectionFailed`, `FinalizationFailed`, `PreviewLeaseInvalid`,
`PreviewLeaseExpired`, `PreviewRangeInvalid`, `Cancelled`, and `DeadlineExceeded`. Errors and their
adjacent command or query results use only safe typed identities, never paths, tokens, process
output, or raw content.

The MVP `AssetApplicationError` variants carry no payload. Query and command values already retain
the safe typed identities needed by their caller, while adapters record private diagnostics only at
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
