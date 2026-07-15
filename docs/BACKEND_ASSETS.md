# Backend MVP Asset Architecture

> Status: proposed MVP design
> Owner: `crates/assets`
> Scope: project-local Image, Video, and Audio required for execution and preview

Naming follows [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md). `AssetAggregate` is the aggregate root
of the Asset bounded context.

## MVP Goal

The Asset context provides one durable media path:

```text
user import or generated stream
  -> validate managed content
  -> create stable Asset identity
  -> return Workflow managed-media reference
  -> preview image, video, or audio
```

It owns Asset identity, Project visibility, media kind, content availability, verified technical
facts, origin, and legal content-state transitions. It does not own Workflow edges, node parameters,
provider tasks, canvas state, or playback state.

Text is not an Asset. Paths, storage keys, SQLite rows, object URLs, and preview URLs are boundary
representations, never Asset identity.

## DDD Layers

```text
crates/assets/src/asset/
  domain/          aggregate, content state, media facts, origin, errors
  application/     import, generated write, get, list, resolve, preview, recovery
  ports/           persistence, transaction, managed bytes, inspection, clock, IDs
  infrastructure/  SQLite, managed filesystem, and media-probe adapters
```

`AssetAggregate` owns state transitions. Use cases coordinate the aggregate and ports. Infrastructure
implements consumer-owned ports and is constructed only by the Desktop composition root.

## Asset Aggregate

`AssetId` identifies one logical media item. `AssetManagedContentId` identifies one immutable
managed byte object. Assets remain distinct even when their verified bytes match.

```rust
pub struct AssetAggregate {
    pub id: AssetId,
    pub project_id: ProjectId,
    pub media_kind: AssetMediaKind,
    pub managed_content_state: AssetManagedContentState,
    pub origin: AssetOriginValue,
    pub media_facts: AssetMediaFactsValue,
    pub display_name: AssetDisplayNameValue,
    pub created_at: AssetCreatedAtValue,
}

pub enum AssetMediaKind {
    Image,
    Video,
    Audio,
}
```

MVP Assets are Project-local. An operation in another Project cannot list, resolve, or preview them.
Neither identity is a path, filename, URL, object URL, or provider task ID.

## Managed Content State

```rust
pub struct AssetManagedContentDescriptorValue {
    pub id: AssetManagedContentId,
    pub digest: AssetContentDigestValue,
    pub byte_length: u64,
    pub mime_type: AssetMediaMimeTypeValue,
    pub media_kind: AssetMediaKind,
}

pub enum AssetManagedContentState {
    Pending {
        descriptor: AssetManagedContentDescriptorValue,
        finalization_id: AssetManagedContentFinalizationId,
    },
    Available {
        descriptor: AssetManagedContentDescriptorValue,
    },
    Missing {
        expected: AssetManagedContentDescriptorValue,
        reason: AssetContentMissingReason,
    },
}
```

`AssetManagedContentId` is derived from a versioned SHA-256 digest scheme. The filesystem adapter
maps it to a private managed location. MIME is sniffed from bytes and checked against media kind;
caller MIME and extension are hints only.

The MVP never replaces bytes in place. Every import or generation creates a new `AssetAggregate`,
so an earlier Run keeps the exact output it recorded.

## Verified Media Facts

`AssetMediaFactsValue` is a closed enum:

```text
Image { width, height }
Video { width, height, duration_ms, has_audio }
Audio { duration_ms, sample_rate_hz, channels }
```

Facts are immutable observations extracted before availability. Invalid, unreadable, or unsupported
media is rejected rather than stored with guessed metadata.

## Origin

```rust
pub enum AssetOriginValue {
    Imported {
        asset_import_id: AssetImportId,
        original_file_name: AssetOriginalFileNameValue,
    },
    GeneratedByWorkflowNode {
        workflow_id: AssetOriginWorkflowId,
        workflow_revision: AssetOriginWorkflowRevision,
        workflow_run_id: AssetOriginWorkflowRunId,
        workflow_node_id: AssetOriginWorkflowNodeId,
        node_capability_ref: AssetOriginNodeCapabilityRef,
        generation_profile_ref: AssetOriginGenerationProfileRef,
        node_output_port: AssetOriginNodeOutputPortKey,
    },
    DerivedByWorkflowNode {
        workflow_id: AssetOriginWorkflowId,
        workflow_revision: AssetOriginWorkflowRevision,
        workflow_run_id: AssetOriginWorkflowRunId,
        workflow_node_id: AssetOriginWorkflowNodeId,
        node_capability_ref: AssetOriginNodeCapabilityRef,
        source_asset_ids: NonEmptyVec<AssetOriginSourceAssetId>,
        node_output_port: AssetOriginNodeOutputPortKey,
    },
    ModelDerivedByWorkflowNode {
        workflow_id: AssetOriginWorkflowId,
        workflow_revision: AssetOriginWorkflowRevision,
        workflow_run_id: AssetOriginWorkflowRunId,
        workflow_node_id: AssetOriginWorkflowNodeId,
        node_capability_ref: AssetOriginNodeCapabilityRef,
        source_asset_ids: NonEmptyVec<AssetOriginSourceAssetId>,
        generation_profile_ref: AssetOriginGenerationProfileRef,
        node_output_port: AssetOriginNodeOutputPortKey,
    },
}
```

Asset-owned origin wrappers prevent a dependency on Workflow persistence types. The Desktop bridge
performs explicit translation. Imported origin does not retain an absolute source path. Generated
origin records the provider-independent profile; deterministic derived origin records exact source
Asset identities; model-derived origin records both. None duplicates prompts, provider/native model
state, progress, or cost.

## Aggregate Invariants

- identity, Project, media kind, origin, and content descriptor are immutable;
- descriptor digest, length, MIME, kind, and verified bytes agree;
- only `Pending -> Available`, `Pending -> Missing`, or `Available -> Missing` is legal in the MVP;
- `Missing -> Available` requires the exact expected digest and length;
- only the owning Project may list, resolve, or preview an Asset;
- paths and preview URLs never enter the aggregate;
- finalization transitions are idempotent by `AssetManagedContentFinalizationId`.

Repositories persist transitions already approved by `AssetAggregate`; they expose no arbitrary
state setter.

## MVP Use Cases

| Use case | Responsibility |
| --- | --- |
| `ImportAssetUseCase` | validate a user-selected file and create an Asset |
| `RecordNodeProducedAssetUseCase` | validate one generated-or-derived node stream and create an Asset |
| `GetAssetUseCase` | return one visible Asset |
| `ListAssetsUseCase` | return one bounded Project page |
| `ResolveAssetContentUseCase` | return opaque managed-byte access for execution |
| `IssueAssetPreviewUseCase` | grant short-lived preview access |
| `ReconcileAssetContentUseCase` | finish bounded Pending work after interruption |

Archive, delete, purge, tagging, collections, content-retention policy, remote import, and export are
not MVP use cases.

## Import Flow

`ImportAssetCommand` contains trusted Project scope plus a Tauri-owned file handle. It never accepts
a reusable path from Workflow JSON.

```text
open bounded file stream
  -> write restricted staging content
  -> calculate digest and sniff MIME
  -> extract and validate AssetMediaFactsValue
  -> commit Pending AssetAggregate + managed-content finalization
  -> finalize managed bytes after commit
  -> transition AssetAggregate to Available
  -> return ImportAssetResult
```

Validation failure creates no aggregate and removes staging. Database failure removes staging. A
failure after the Pending commit remains recoverable and is never reported as available.

The local Desktop MVP does not persist import replay receipts. An interrupted import is reconciled
only after its Pending Asset and managed-content finalization have committed.

## Node-Produced Media Flow

```text
node capability executor
  -> NodeCapabilityProducedMediaWriterPort
  -> RecordNodeProducedAssetUseCase
  -> Pending/finalization/Available flow
  -> Workflow managed-media reference
```

A node cannot succeed until every produced Asset needed by its result is available. Provider bytes
and URLs never enter Workflow state. Storage failure publishes no node output.

## Resolve Use Case

```text
ResolveAssetContentQuery {
  project_id,
  asset_id,
  expected_asset_media_kind
} -> ResolveAssetContentResult {
  lease: AssetManagedContentLease,
  descriptor: AssetManagedContentDescriptorValue
}
```

Resolution checks Project ownership, exact media kind, `Available` state, and current content
existence. `AssetManagedContentLease` is bounded and opaque, never a path. A node adapter can stream
it to a provider; Desktop preview code can stream it to React.

Pending, Missing, wrong-kind, and wrong-Project cases return distinct structured errors.

## List Use Case

```text
ListAssetsQuery { project_id, media_kind?, after?, limit }
ListAssetsResult { assets, next_cursor }
```

Ordering is `(created_at DESC, asset_id DESC)`. The opaque cursor contains both values, and `limit`
is bounded from 1 to 100. This supports the MVP picker without loading the full library.

## Preview Use Case

`IssueAssetPreviewUseCase` creates `AssetPreviewLease`, a short-lived Project-scoped permission. The
Desktop protocol translates the lease into a `desktop-asset://` URL:

```text
Image -> verified image MIME
Video -> MIME, Content-Length, ETag, and one valid byte Range
Audio -> MIME, Content-Length, ETag, and one valid byte Range
```

Every protocol request rechecks Project scope and expiry. It supports bounded `GET` and `HEAD`, sets
`nosniff`, and never discloses a managed path. React owns zoom, seek, volume, playback, and object URL
lifetime.

## Recovery

`ReconcileAssetContentUseCase` performs bounded startup recovery:

- retry idempotent finalization while expected staging content exists;
- mark an Available aggregate Missing when its exact managed content is absent;
- remove stale unreferenced staging content after a configured retention window.

Recovery never searches by basename, crosses Project scope, or rewrites an Asset binding.

## Consumer-Owned Ports

| Port | Required behavior |
| --- | --- |
| `AssetAggregateRepositoryPort` | load aggregates and perform stable cursor queries |
| `AssetIngestTransactionPort` | atomically persist Pending, finalization, and approved transitions |
| `AssetManagedContentStorePort` | stage, finalize, lease, inspect existence, clean staging |
| `AssetMediaInspectorPort` | sniff MIME and extract verified media facts |
| `AssetClockPort` | provide deterministic timestamps |
| `AssetIdentityGeneratorPort` | create Asset, import, and managed-content finalization identities |

Each use case receives only its required ports through its constructor. `crates/nodes` separately
owns `NodeCapabilityManagedMediaReaderPort` and `NodeCapabilityProducedMediaWriterPort`; Desktop
implements those ports by calling Asset use cases.

Concrete implementations are named by technology and role, for example
`SqliteAssetAggregateRepositoryAdapter`, `FileSystemAssetManagedContentStoreAdapter`, and
`FfprobeAssetMediaInspectorAdapter`.

Logical metadata records, managed-content publication, and crash recovery are defined in
[`BACKEND_STORAGE.md`](BACKEND_STORAGE.md).

## Errors

`AssetDomainError` covers invariant and transition failures. `AssetApplicationError` adds not found,
not visible, kind mismatch, Pending, Missing, invalid media, import limit, digest mismatch, managed
storage failure, and finalization failure. Errors include safe typed IDs and structured details,
never paths or raw untrusted content.

## Verification

- aggregate tests cover Project scope, kind, origin, and legal transitions;
- import and generated-media tests cover validation, Pending finalization, and storage failure;
- port contract suites run against fake and production adapters;
- fault-injection tests stop at each write boundary and prove recovery;
- query tests prove cursor ordering, bounds, and Project isolation;
- preview tests prove MIME, ETag, Range, expiry, and path non-disclosure;
- bridge tests prove exact kind, provenance, and no output before availability.

## Post-MVP

Global Assets, archive/delete/purge, tags, search, preview derivatives, remote import, packages,
garbage collection, and cloud sync require separate product requirements. Multiview, 3D, and scene
Assets are not product scope.
