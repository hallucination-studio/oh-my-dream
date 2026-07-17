# Backend Generation Task Architecture

- Status: frozen MVP design
- Date: 2026-07-17
- Owner: `crates/tasks`
- Scope: durable provider-backed generation initiated by Workflow node executions

## 1. Decision summary

Introduce a Generation Task bounded context with one vendor-neutral `GenerationTaskAggregate`. A
task is one durable intent to produce the primary result of one provider-backed Workflow node
execution. It is not a Workflow Run, provider wire request, download record, or generic background
job.

The MVP uses one lifecycle and two durable tables: `generation_tasks` and
`generation_task_outbox`. Each admitted operation produces exactly one primary result, stored on
the task as inline Text or one Asset reference. This is sufficient for a unified task list,
idempotent creation, remote polling, cancellation, restart recovery, optimistic concurrency, and
side effects after commit.

Automatic generation retries, attempt history, arbitrary provider options, archiving,
cross-provider routing, standalone task creation, and 3D are Future features. They
must not complicate the MVP model before a concrete use case requires them.

## 2. Delivery scope

### 2.1 MVP

| Capability | MVP decision |
| --- | --- |
| Task model | One `GenerationTaskAggregate` for Text, Image, Video, and Voice generation. |
| Text | Text generation from text inputs; activated when a matching Node Capability/profile route is registered. |
| Image | Text-to-image only. |
| Voice | Text-to-speech producing an Audio Asset. |
| Video | Image-to-video only. |
| Providers | One provider-level interface composing complete Text/Image/Video/Voice interfaces; MVP implements only one deterministic Mock provider. |
| Lifecycle | `Queued`, `Submitting`, `Running`, `CancelRequested`, `Succeeded`, `Failed`, `Cancelled`. |
| Reliability | Idempotency key, canonical request hash, optimistic revision, transactional outbox, bounded delivery retry, restart recovery. |
| Persistence | Tasks with one optional primary result, plus task outbox. |
| API | Get and list. Creation and cancellation occur only through the owning Workflow execution. |
| Task list | Project-scoped cursor pagination with optional status and request-kind filters. |
| Assets | An Image, Audio, or Video result must be a durable Asset before task success. |
| Workflow | Terminal task event resumes or fails the owning workflow node through an adapter. |

MVP is complete only when the Mock Workflow E2E path works for text-to-image, image-to-video, and
text-to-speech, and an accepted Mock asynchronous task resumes after process restart without
another submission. Production provider routes and their vendor-specific configuration are a later,
separately reviewed delivery phase.

### 2.2 Future

| Feature | Add only when |
| --- | --- |
| Automatic generation retry | Product policy requires a new provider submission after a terminal/transient generation failure. |
| `generation_task_attempts` | Multiple submissions under one task require durable attempt history. |
| Manual Retry command and `retry_of_task_id` | Users need task-level reruns outside normal workflow reruns. |
| Archive/retention commands | Task volume makes lifecycle retention controls necessary. |
| Provider-specific options JSON | A shipped model needs a setting that cannot be represented by a stable common field. |
| Advanced image modes | Inpaint, outpaint, masks, control images, or batch variations are scheduled. |
| Advanced audio modes | Music generation, speech-to-text, or audio-to-audio is scheduled. |
| Advanced video modes | Extend, interpolate, lip-sync, or timeline composition is scheduled. |
| Webhook completion | A provider offers reliable callbacks and polling cost matters. |
| Routing and failover | Equivalent provider behavior is defined and contract-tested. |
| Usage, billing, and quotas | Product decisions require cost reporting or limits. |
| Distributed workers | More than one process or host executes tasks. |
| Standalone task creation | Product semantics exist for tasks without a Workflow node origin. |
| 3D media | A 3D asset contract is designed; Meshy and Tripo3D remain references until then. |

Future features extend the same aggregate through explicit migrations. They do not create provider-specific task tables.

## 3. Goals and non-goals

### Goals

- Show Text, Image, Video, and Voice generation work in one stable task list.
- Preserve tasks and remote handles across application restarts.
- Normalize provider statuses, failures, progress, and cancellation exactly once.
- Prevent duplicate local tasks when workflow execution is retried.
- Persist successful media through the asset capability before declaring success.
- Keep workflow logic independent of network, filesystem, database, UI, and vendors.

### MVP non-goals

- A generic job platform or arbitrary task graph.
- Automatic resubmission after a provider has terminally rejected or failed a generation.
- Automatic cross-provider failover.
- Provider billing, account, secret, or quota management.
- Raw provider payload storage.
- Event sourcing; the task row is authoritative and the outbox carries work/events.

## 4. Lessons taken from DVStudio

The reference implementation keeps separate local mirrors for video, Ark, Gemini, Meshy, and Tripo3D tasks. Their common data includes remote task identity, provider/model, status, progress, prompt, results, error, project/node linkage, and timestamps. Seedance, Meshy, and Tripo3D demonstrate async submit/poll/cancel; Gemini demonstrates an immediate response followed by durable file storage.

The design keeps these behaviors but removes vendor-specific task tables. Provider request/response shapes remain boundary representations and never become the task domain or public DTO.

## 5. Ubiquitous language and boundaries

`GenerationTaskAggregate` owns one durable generation lifecycle. `GenerationTaskOrigin` identifies
its exact Project, Workflow Run, and Node Execution. `GenerationTaskRequest` is an immutable
provider-neutral snapshot, `GenerationTaskTarget` selects the stable Generation Profile and exact
route binding, `GenerationProviderTaskHandle` is the opaque remote identity,
`GenerationTaskResult` is inline Text or an Asset reference, and `GenerationTaskSummaryView` is a
rule-free list projection.

Delivery retry repeats safe reads, polling, cancellation, finalization, and notification. It never
repeats an Immediate execution or Submit call after an uncertain outcome. Generation retry creates
another provider submission after a failed generation and is Future only.

Use `GenerationTask`, not a generic `Job` or `Task`, in public code.

```text
ui
  -> src-tauri commands and DTO translators
       -> composition root
            -> crates/tasks (domain, application, consumer-owned ports)
            -> crates/backends (provider adapters -> task provider ports)
            -> crates/assets (authoritative assets)
            -> SQLite/clock/asset adapters (-> task ports)

crates/engine (pure Workflow semantics)
  <-> task/Workflow bridge adapters in the Desktop composition boundary
```

Recommended layout:

```text
crates/tasks/src/
  generation_task/domain/{aggregate, request, state, failure, result}.rs
  generation_task/application/{commands, queries, execution}.rs
  generation_task/interfaces/
crates/backends/src/generation/{mock, bytedance, gemini, ...}.rs
src-tauri/src/task_adapters/{sqlite, assets, workflow_events}.rs
src-tauri/src/commands/generation_tasks.rs
src-tauri/src/composition.rs
```

Rules:

1. `crates/tasks` imports no Tauri, SQL, HTTP, filesystem API, or concrete provider.
2. Consumer-owned ports are defined in `crates/tasks`; adapters depend inward and implement them.
3. Concrete providers and storage are selected only in the composition root.
4. Task input/output references the authoritative `AssetId`; local paths are never business data.
5. Workflow/run/node IDs use engine-owned value types rather than copied string semantics.
6. Domain methods are the only legal way to change task state.

## 6. MVP domain model

### 6.1 Aggregate fields

| Field | Type and meaning |
| --- | --- |
| `id` | RFC 9562 UUIDv4 `GenerationTaskId`. |
| `origin` | Required `project_id`, `workflow_id`, `workflow_run_id`, `workflow_node_id`, and `workflow_node_execution_id`. |
| `idempotency` | Caller key plus canonical request hash. |
| `request` | Immutable `GenerationRequest`. |
| `target` | Immutable `GenerationProfileRef`, `GenerationProviderId`, and `GenerationProviderRouteId`; no secret, account, credential, or provider options JSON. |
| `provider_deadline_at` | Persisted UTC-millisecond deadline derived once from task creation time and the frozen route budget. |
| `state` | `GenerationTaskState`, the sole lifecycle semantic owner. |
| `result` | Optional single `GenerationTaskResult`; set atomically with `Succeeded`. |
| `created_at`, `updated_at` | Values supplied by the task-owned clock port. |
| `revision` | Monotonic optimistic-lock version. |

The request hash covers schema version, origin, request, and target. It excludes timestamps and the idempotency key. Reusing `(project_id, idempotency_key)` with the same hash returns the existing task; a different hash returns `IdempotencyConflict`.
Generation kind is owned only by the closed `GenerationTaskRequest` variant; it is not duplicated
inside `GenerationTaskTarget`. The SQLite `request_kind` column is a derived index discriminator.
Row restoration verifies it equals the decoded request variant before aggregate construction.
Exactly one Generation Task may exist for one `WorkflowNodeExecutionId`. Repeating another
idempotency key for the same Node Execution returns the existing task only when the canonical hash
matches; otherwise it is `GenerationTaskOriginConflict`. This invariant is enforced by the
aggregate repository transaction and a unique Project/Node Execution index.

### 6.2 Requests and inputs

```rust
pub enum GenerationTaskRequest {
    Text(TextGenerationSpec),
    Image(ImageGenerationSpec),
    Voice(VoiceGenerationSpec),
    Video(VideoGenerationSpec),
}
pub enum GenerationInput {
    Text { role: TextInputRole, content: NonEmptyText },
    Asset { role: AssetInputRole, asset: AssetSnapshotRef },
}
pub struct AssetSnapshotRef {
    pub asset_id: AssetId, pub media_kind: MediaKind, pub content_hash: ContentHash,
}
```

| MVP spec | Required | Optional stable fields |
| --- | --- | --- |
| `TextGenerationSpec` | `prompt` | system instruction and bounded output controls only when owned by an active profile contract |
| `ImageGenerationSpec` | `prompt`, `aspect_ratio` | none |
| `VoiceGenerationSpec` | `text` | none; the frozen profile owns voice/output format |
| `VideoGenerationSpec` | exact input Image snapshot, `duration_seconds` | non-empty prompt |

These four structs are operation-specific, so the MVP adds no redundant mode field.
`aspect_ratio` and `duration_seconds` are the closed values in `BACKEND_CAPABILITIES.md`. Input Asset
snapshots include a content hash so delivery retry and recovery cannot silently observe changed
media. Image-to-image, text-to-video, negative prompts, counts, dimensions, seeds, resolution, and
audio-generation switches remain Future.

### 6.3 Result

`GenerationTaskResult` is the closed value `Text { content } | Asset { asset_id, media_kind }`.
`GenerationTaskAssetResult` is the media-only value contained by the latter variant.
The request kind mechanically determines the required result variant and media kind. Supporting
multiple primary results requires a later request contract and schema migration; MVP does not add
ordinal, role, output identity, or a child table for a hypothetical batch operation.

The asset domain remains the semantic owner of MIME type, dimensions, duration, checksum, and storage location. A task only references the durable asset.

## 7. MVP state machine

```rust
pub enum GenerationTaskState {
    Queued,
    Submitting,
    Running { handle: GenerationProviderTaskHandle, progress_percent: Option<u8> },
    CancelRequested { handle: Option<GenerationProviderTaskHandle> },
    Succeeded { completed_at: Timestamp },
    Failed { completed_at: Timestamp, failure: GenerationTaskFailure },
    Cancelled { completed_at: Timestamp },
}
```

| From | To | Cause |
| --- | --- | --- |
| `Queued` | `Submitting` | Worker claims `SubmitTask`. |
| `Submitting` | `Running` | Provider returns a remote handle. |
| `Submitting`/`Running` | `Succeeded` | Provider returns a valid Text result or its media result is an Available Asset. |
| `Running` | `Running` | Normalized progress update. |
| `Queued` | `Cancelled` | Cancellation occurs before submission. |
| `Submitting`/`Running` | `CancelRequested` | Cancellation races with external work. |
| `CancelRequested { handle: None }` | `CancelRequested { handle: Some }` | An in-flight submit returns accepted after local cancellation intent committed. |
| `CancelRequested` | `Cancelled` | Remote cancellation is attempted when available. |
| `Running`/`CancelRequested` | `Cancelled` | Local cancellation wins when no complete remote canceller is registered. |
| `Running` | `Cancelled` | Poll reports that remote work was cancelled outside this process. |
| `Queued`/`Submitting`/`Running` | `Failed` | Permanent failure or exhausted delivery attempts. |

Invariants:

- Request, origin, target, and idempotency data never change.
- Provider deadline never changes. Expiry fails `Queued` or `Running` with `Timeout`.
  `Submitting` is deliberately conservative: once that state commits, a crash, call error, or
  deadline cannot prove that the provider did not accept work, so it fails
  `AmbiguousSubmission`. Expiry of `CancelRequested` commits `Cancelled` and records safe
  remote-cancellation-unconfirmed telemetry.
- Progress is optional `0..=100` and monotonic while running.
- `Succeeded` requires exactly one result matching the request's result kind.
- Text or an Available media Asset result commits atomically with `Succeeded` and is immutable.
  Before success, media recovery uses the deterministic Asset node-output key rather than partially
  attaching a result to the Task.
- Terminal states never transition.
- Cancellation is rejected only after a terminal state has committed.
- Optimistic revision serializes cancel/complete races: the first committed transition wins. A
  submit outcome that loses to `CancelRequested` is reconciled against the reloaded state; an
  accepted handle is attached only to drive cancellation and can never return to `Running`.
- Provider status strings and human-readable error text never drive transitions.

## 8. Failure semantics

`GenerationTaskFailure` contains `kind`, machine-readable `code`, and safe `message`. Kinds are
`InvalidRequest`, `Authentication`, `PermissionDenied`,
`ContentPolicy`, `RateLimited`, `ProviderUnavailable`, `Timeout`, `ProviderRejected`,
`InvalidProviderResponse`, `AmbiguousSubmission`, `InputAssetUnavailable`, `OutputAssetImport`, and
`Internal`.

A `GenerationProviderFailure` is an explicit terminal result reported by a provider. A
`GenerationProviderCallError` means the current HTTP/transport/protocol call could not produce a
trustworthy result. Keeping these types separate prevents a network timeout from being confused
with a provider-declared failed generation.

A transient `GenerationProviderCallError` reschedules polling or result retrieval for the same
persisted remote handle. An uncertain Immediate or Submit call always becomes
`AmbiguousSubmission`; the generic worker never repeats it, regardless of a vendor idempotency
feature. Query-by-ID recovery after `Running` does not require submission idempotency.
This is bounded delivery retry, not a new generation attempt. A non-transient call error or
exhausted delivery budget transitions the task to `Failed`. A terminal provider failure fails
immediately.

Every provider-work reschedule is capped by `provider_deadline_at`, checked `u32` delivery-attempt arithmetic,
and the frozen 500..=5,000 millisecond poll bounds. Provider retry-after may delay within those
bounds but cannot extend the task deadline. Deadline exhaustion transitions active generation once
to `Failed { Timeout }` and enqueues `NotifyWorkflow`; `CancelRequested` instead becomes local
`Cancelled` with the same notification. No task polls forever.

## 9. Provider contracts

Generation Task is unified; provider implementations are capability-composed behind one provider-
level `GenerationProviderInterface`. Every provider implementation exposes stable provider identity
and one non-empty `GenerationProviderCapabilities` value. A provider contributes only capabilities
it actually implements.

Examples:

```text
Mock provider       = Image + Video + Voice
Future provider A   = Text + Image
Future provider B   = Video
Future provider C   = Text + Voice
```

Multiple providers may implement the same generation type. One provider object may contribute
several types while sharing account/authentication infrastructure. `GenerationProfileRef` and
provider Settings select the configured provider/route at task admission, and the immutable task
target preserves that exact choice for recovery.

The runtime registry needs trait objects. The provider and focused capability discovery methods are
synchronous and side-effect free; the complete executor/submitter/poller/canceller interfaces use
the workspace's established `async_trait` boundary for their external calls:

```rust
pub trait GenerationProviderInterface: Send + Sync {
    fn generation_provider_id(&self) -> &GenerationProviderId;
    fn generation_provider_display_name(&self) -> &GenerationProviderDisplayName;
    fn generation_provider_capabilities(&self) -> &GenerationProviderCapabilities;
}

pub struct GenerationProviderCapabilities {
    pub text: Option<Arc<dyn TextGenerationProviderInterface>>,
    pub image: Option<Arc<dyn ImageGenerationProviderInterface>>,
    pub video: Option<Arc<dyn VideoGenerationProviderInterface>>,
    pub voice: Option<Arc<dyn VoiceGenerationProviderInterface>>,
}

pub trait ImageGenerationProviderInterface: Send + Sync {
    fn image_generation_contract(&self) -> &ImageGenerationProviderContract;
    fn resolve_image_generation_route(
        &self,
        route_id: &GenerationProviderRouteId,
    ) -> Result<ImageGenerationProviderExecution, GenerationProviderRouteResolutionError>;
}

pub trait TextGenerationProviderInterface: Send + Sync {
    fn text_generation_contract(&self) -> &TextGenerationProviderContract;
    fn resolve_text_generation_route(
        &self,
        route_id: &GenerationProviderRouteId,
    ) -> Result<TextGenerationProviderExecution, GenerationProviderRouteResolutionError>;
}

pub trait VideoGenerationProviderInterface: Send + Sync {
    fn video_generation_contract(&self) -> &VideoGenerationProviderContract;
    fn resolve_video_generation_route(
        &self,
        route_id: &GenerationProviderRouteId,
    ) -> Result<VideoGenerationProviderExecution, GenerationProviderRouteResolutionError>;
}

pub trait VoiceGenerationProviderInterface: Send + Sync {
    fn voice_generation_contract(&self) -> &VoiceGenerationProviderContract;
    fn resolve_voice_generation_route(
        &self,
        route_id: &GenerationProviderRouteId,
    ) -> Result<VoiceGenerationProviderExecution, GenerationProviderRouteResolutionError>;
}

pub enum ImageGenerationProviderExecution {
    Immediate(Arc<dyn ImageGenerationImmediateExecutorInterface>),
    Remote {
        submitter: Arc<dyn ImageGenerationSubmitterInterface>,
        poller: Arc<dyn ImageGenerationPollerInterface>,
    },
    CancellableRemote {
        submitter: Arc<dyn ImageGenerationSubmitterInterface>,
        poller: Arc<dyn ImageGenerationPollerInterface>,
        canceller: Arc<dyn GenerationCancellerInterface>,
    },
}

pub struct GenerationProviderCallContext {
    pub task_id: GenerationTaskId,
    pub target: GenerationTaskTarget,
    pub task_created_at: Timestamp,
    pub provider_deadline_at: Timestamp,
}

#[async_trait]
pub trait ImageGenerationImmediateExecutorInterface: Send + Sync {
    async fn execute_image_generation(
        &self,
        context: &GenerationProviderCallContext,
        spec: &ImageGenerationSpec,
    ) -> Result<ImageGenerationImmediateOutcome, GenerationProviderCallError>;
}

#[async_trait]
pub trait ImageGenerationSubmitterInterface: Send + Sync {
    async fn submit_image_generation(
        &self,
        context: &GenerationProviderCallContext,
        spec: &ImageGenerationSpec,
    ) -> Result<ImageGenerationSubmitOutcome, GenerationProviderCallError>;
}

#[async_trait]
pub trait ImageGenerationPollerInterface: Send + Sync {
    async fn poll_image_generation(
        &self,
        context: &GenerationProviderCallContext,
        handle: &GenerationProviderTaskHandle,
    ) -> Result<ImageGenerationPollOutcome, GenerationProviderCallError>;
}

#[async_trait]
pub trait GenerationCancellerInterface: Send + Sync {
    async fn cancel_generation(
        &self,
        context: &GenerationProviderCallContext,
        handle: &GenerationProviderTaskHandle,
    ) -> Result<GenerationCancellationOutcome, GenerationProviderCallError>;
}
```

`GenerationProviderCapabilities::try_new` rejects four absent fields. `Option` represents the
provider's immutable composition, not an optional operation: callers select only a present complete
interface, and no focused interface may return `Unsupported`.

Each provider contributes at most one focused interface per kind. Its focused contract owns a
non-empty set of shipped route contracts for that kind. Configuration selects one of those route
IDs; task admission persists it, and the matching `resolve_*_generation_route` method returns the
exact execution composition during both initial dispatch and restart recovery.

The Text, Video, and Voice execution values follow the same closed shape. Each returned execution value is a closed
`Immediate`, `Remote`, or `CancellableRemote` composition. `Immediate` contains one complete
immediate executor. `Remote` contains one submitter and one poller. `CancellableRemote` additionally
contains one complete canceller.
The Text, Video, and Voice executor/submitter/poller methods have the same call context and
type-specific spec/result outcome. `GenerationProviderCallContext` is immutable task-owned data;
adapters may not reinterpret it as vendor configuration. Immediate outcomes are exactly
`Completed(result) | Rejected(failure)`. Submit outcomes are exactly
`Accepted(handle) | Completed(result) | Rejected(failure)`, poll outcomes are exactly
`Pending(progress) | Completed(result) | Failed(failure) | Cancelled`, and cancellation outcomes are
exactly `Accepted | AlreadyCancelled | RemoteAbsent`. A remote poller must return an equivalent
terminal outcome for the same persisted handle through the task deadline.
There is no optional execution method, `supports_*` probe, or `Unsupported` result. Absence of Voice
means `capabilities.voice` is `None`, so Voice Settings choices cannot be derived for that provider.

`GenerationProviderContract` is a safe projection mechanically derived by the registry from provider
identity and the focused contracts inside `GenerationProviderCapabilities`; a provider never
supplies a second independently-authored contract tree. The projection contains no implementation
objects or secrets. Duplicate route IDs anywhere inside one provider, an empty capability product,
or an invalid focused contract is rejected when the immutable registry is constructed.

Each `TextGenerationProviderContract`, `ImageGenerationProviderContract`,
`VideoGenerationProviderContract`, and `VoiceGenerationProviderContract` contains its fixed kind
and a non-empty set of `GenerationProviderRouteContract` values. A route contract exposes only its
stable route ID, display name, and exact compatible Generation Profile refs. It never exposes an
endpoint, native model ID, credential, implementation name, vendor configuration, or vendor DTO.

`GenerationProviderRegistry` is a concrete immutable collection of
`Arc<dyn GenerationProviderInterface>`. It validates unique provider IDs and builds typed lookup
indexes for task dispatch and UI projection. Generic Task routing depends on this provider-level
interface/registry. Code whose business reason is only Image generation may instead depend directly
on `ImageGenerationProviderInterface`; it never needs Mock or another concrete type.

Generic dispatch is exact and has no vendor branch:

```text
persisted GenerationTaskTarget.provider_id
  -> GenerationProviderRegistry resolves GenerationProviderInterface
  -> GenerationTaskRequest kind selects exactly one typed capability contribution
  -> persisted route_id resolves Immediate, Remote, or CancellableRemote
  -> typed request/handle enters that execution only
```

A missing provider, missing kind contribution, incompatible route, or contract mismatch is a
structured configuration/recovery error before an external call. UI and Settings receive only the
safe contracts; they never receive the contribution trait objects or execution compositions. Task
admission copies the selected profile/provider/route tuple into the immutable target before any
external call; row restoration enforces that closed shape.

Each submit outcome is `Accepted`, type-specific `Completed`, or
`Rejected(GenerationProviderFailure)`. Each remote poll outcome is `Pending`, type-specific
`Completed`, `Failed(GenerationProviderFailure)`, or `Cancelled`. Only provider capability
implementations see vendor statuses.

Future production adapters must validate third-party responses as untrusted, use the stable
task-derived submission key where supported, return normalized outputs/handles, and keep
credentials, signed URLs, raw payloads, and vendor error bodies out of DTOs and logs. A route that
cannot return a durable handle may complete immediately but cannot claim restart-safe remote
polling. MVP's Mock Remote contract guarantees that polling the same handle after completion returns
the same terminal outcome until the task deadline. Account selection, credential replacement, and
credential-aware recovery for a production provider require a separately reviewed target-schema
extension; the Mock MVP does not freeze speculative fields or lifecycle rules for them.

MVP constructs only `MockGenerationProviderAdapterImpl`. A future production provider may
contribute any non-empty subset without changing Generation Task semantics or these interfaces.

## 10. Durable MVP execution

1. A generation Node Capability resolves its semantic inputs, calls the task-start bridge, and receives the durable task identity.
2. `GenerationTaskAggregate::create` validates invariants and emits `SubmitTask`.
3. One transaction inserts the task and outbox message.
4. The worker claims the message, requires the exact origin Node Execution to be
   `WaitingForExternalCompletion`, then commits `Submitting` and calls the provider outside the
   transaction. A still-`Running` origin means Workflow has not completed durable handoff; the
   worker reschedules without changing task state or calling the provider.
5. `Accepted` atomically saves `Running`, consumes `SubmitTask`, and enqueues `PollTask`.
6. `Pending` atomically saves progress, consumes the current poll, and enqueues the next delayed poll.
7. Completed Text is validated inline. For completed Image, Video, or Voice media,
   `store_generation_task_asset` records/replays the deterministic Asset node-output key, durably
   drives its existing Pending finalization protocol, and returns only an Available Asset. The Task
   remains `Submitting` or `Running` during this external Asset operation.
8. Success atomically stores the Text or Asset result, saves `Succeeded`, consumes the work message,
   and enqueues `NotifyWorkflow`. The sink returns an exact replay for the same digest and a conflict
   for different bytes.
9. Terminal failure atomically saves `Failed`, consumes the work message, and enqueues `NotifyWorkflow`.
10. A transient origin read, Poll, Cancel, Asset finalization, or notification error reschedules the
    same safe message. An uncertain Immediate or Submit call atomically fails
    `AmbiguousSubmission`, consumes `SubmitTask`, and enqueues `NotifyWorkflow`. A live process
    never reclaims its worker's `Claimed` message; startup resets prior-process claims only after
    acquiring the exclusive database lock.

If the process crashes during finalization, the unconsumed message is reclaimed and the Asset sink
is queried first by deterministic task/output key. `Available` completes without a provider call;
`Pending` reschedules behind its durable Asset effect; only `SourceRequired` permits an accepted
asynchronous provider to be queried again by its persisted handle. An Immediate or not-yet-accepted
`Submitting` call is never repeated after crash;
the task fails as `AmbiguousSubmission` rather than risk duplicate paid work. Therefore restart-safe
remote recovery begins only after `Accepted(handle)` and the `Running` state commit atomically.

Queued cancellation atomically commits `Cancelled`, consumes stale `SubmitTask`, and enqueues
`NotifyWorkflow`. Submitting cancellation commits `CancelRequested { handle: None }` because the
in-flight call may still return a handle. Running cancellation either commits
`CancelRequested { handle: Some }`, consumes `PollTask`, and enqueues `CancelRemoteTask` when a
complete `GenerationCancellerInterface` is registered, or atomically commits local `Cancelled`,
consumes `PollTask`, and enqueues `NotifyWorkflow` when none is registered. Application wiring
selects the separate complete canceller boundary; the aggregate has no support probe.

Cancellation enters this state machine when the Task worker's mandatory origin read observes that
the canonical `WorkflowCancelRunUseCase` has committed the owning Run cancellation. Desktop may wake
the relevant worker after that commit for responsiveness, but the durable Submit/Poll effect and
origin read are the recovery authority, so a crash cannot lose cancellation convergence. There is
no independently authorized Task-cancel command in MVP.

When an in-flight submit returns after `CancelRequested`, `Rejected` or `Completed` converges to
local `Cancelled` with `NotifyWorkflow`. `Accepted(handle)` atomically attaches the handle and then
enqueues `CancelRemoteTask`, or converges directly to local `Cancelled` when no canceller is
registered. Every path consumes the claimed submit effect. No cancellation path leaves a stale
submit/poll effect or a waiting Workflow node without a terminal notification.

Before submit and before every poll/cancel/Asset-finalization call, the worker reads the exact Workflow origin
through `GenerationTaskOriginStateReaderInterface`. Submission is permitted only for the matching
`WaitingForExternalCompletion` origin. A still-`Running` origin reschedules because the Workflow
effect must finish the handoff first. When the read observes a cancelled or terminal origin, it
prevents submission/result attachment and makes remaining task work converge to local cancellation;
a transient origin read failure reschedules without calling the provider. Poll, cancel, and Asset
finalization otherwise require the matching waiting origin. A Workflow cancellation may commit
after the final origin read and race the external call. The first Task-state commit owns the
outcome. If cancellation commits first, a later Asset remains available but unattached. If success
commits first, the cancelled Workflow rejects its late completion notification. Avoiding the external call itself would require the
cross-context transaction or lock intentionally excluded from this design.

For a registered remote canceller, `Cancelled`, accepted cancellation, and already-absent remote
work converge to local `Cancelled`. Transient cancellation-call failure is retried within the
bounded delivery budget; exhaustion still commits local `Cancelled` and emits a safe structured
trace that remote cancellation was unconfirmed. Once `CancelRequested` commits, a later poll result
cannot attach a result. Optimistic revision makes cancel versus success first-commit-wins.
When an ordinary `PollTask` reports `Cancelled` without local cancellation intent, the Task commits
`Cancelled`, consumes the poll effect, and enqueues `NotifyWorkflow`; the completion bridge maps it
to the structured Workflow node failure described below.

### Workflow integration

Generation Task and Workflow have different semantic ownership:

- `GenerationTaskAggregate` owns submission, the remote handle, polling, progress, cancellation,
  result import, and its terminal provider outcome.
- `WorkflowRunAggregate` owns graph scheduling, Node Execution state, downstream blocking, complete
  Workflow outputs, and Run termination.
- Workflow persists no provider, route, credential, remote handle, signed URL, or vendor status.
- Generation Task does not decide that a Workflow node or Run succeeded.

`WorkflowNodeCapabilityInterface` returns a closed execution outcome: immediate
`Completed(WorkflowNodeOutputSet)` or `WaitingForGenerationTask`. The waiting variant contains no
provider identity; the exact `WorkflowNodeExecutionId` is the correlation key. The Run aggregate
transitions that node from `Running` to `WaitingForExternalCompletion`, commits its event, and lets
the current `WorkflowExecuteRunEffect` complete when no other node is ready.

`NotifyWorkflow` is consumed through `GenerationTaskWorkflowCompletionInterface`, owned by the task
application capability and implemented by a Desktop bridge over the canonical Workflow completion
use case. That Workflow use case verifies the exact origin and waiting Node Execution, translates
`Succeeded` mechanically to Workflow runtime values, translates `Failed` or an unexpected
provider-originated `Cancelled` to a structured node failure, commits the node outcome and events,
and enqueues a new `WorkflowExecuteRunEffect` when downstream work may now run. A Task cancelled
because its Workflow was already cancelled returns `OriginTerminal` instead of reopening that node. Repeated
notification is idempotent. A cancelled or already-terminal Workflow node rejects late attachment
without changing the terminal task or deleting its durable Assets.

The `generation_task_outbox` is capability-specific delayed work, not a fourth
`DesktopPostCommitEffect`. `GenerationTaskEffectWorkerImpl` consumes only the four closed task
effect kinds. It neither schedules Workflow graphs nor accepts arbitrary handlers or payloads.
Desktop owns one database-wide process lock and runs exactly one Generation Task worker. Startup
resets every prior-process `Claimed` task effect to `Ready` before accepting commands; the running
process never reclaims work from its live worker. It does not
interrupt a Run whose waiting Node Executions have either authoritative non-terminal Generation
Tasks or terminal Generation Tasks with unprocessed `NotifyWorkflow`. A terminal task whose
notification is already processed requires the node to be terminal; any other combination is
corruption and fails startup rather than guessing. A Running node with an exact Queued task,
unconsumed SubmitTask, and no handle replays Workflow to finish handoff; other Running nodes without
durable handoff still use `InterruptedByRestart`.

The completion interface returns exactly `Applied`, `AlreadyApplied`, or `OriginTerminal`; all three
consume `NotifyWorkflow`. Only a transient storage error reschedules it. A terminal notification has
no delivery-attempt exhaustion path: it remains durable until one of those three outcomes commits.
Its diagnostic attempt counter saturates at `u32::MAX` and never controls delivery. Corrupt identity
or result data fails startup and is never silently abandoned.

## 11. Consumer-owned persistence ports

```rust
pub struct GenerationTaskOutboxChanges {
    pub consume: Option<GenerationTaskEffectClaim>,
    pub enqueue: Vec<GenerationTaskEffect>,
}

pub struct GenerationTaskEffectClaim {
    pub effect_id: GenerationTaskEffectId,
}

pub trait GenerationTaskRepositoryInterface {
    async fn create_generation_task(&self, task: &GenerationTaskAggregate,
        message: GenerationTaskEffect)
        -> Result<GenerationTaskCreateResult, GenerationTaskRepositoryError>;
    async fn load_generation_task(&self, id: GenerationTaskId)
        -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError>;
    async fn load_generation_task_for_project(&self, project_id: ProjectId, id: GenerationTaskId)
        -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError>;
    async fn save_generation_task(&self, task: &GenerationTaskAggregate, expected_revision: u64,
        outbox: GenerationTaskOutboxChanges) -> Result<(), GenerationTaskRepositoryError>;
    async fn list_generation_tasks(&self, query: GenerationTaskListQuery)
        -> Result<GenerationTaskCursorPage<GenerationTaskSummaryView>, GenerationTaskRepositoryError>;
}
```

`create_generation_task` and `save_generation_task` atomically persist aggregate state, the single
result, consumed outbox work, and new outbox work. The Desktop task-start bridge resolves one
currently selected Mock binding and copies its exact profile/provider/route tuple into the Task;
later Settings changes affect only later resolutions. Consuming, completing, or rescheduling work
requires the effect ID to remain `Claimed` and the aggregate revision to match. The process lock,
single worker, bounded provider calls, and startup-only claim reset ensure that no live worker is
reclaimed concurrently. `GenerationTaskOutboxReaderInterface` claims at most one due effect and
reschedules safe transient delivery errors. `GenerationTaskRepositoryFakeImpl` and
`SqliteGenerationTaskRepositoryAdapterImpl` run the same idempotency, concurrency, transaction,
ordering, and pagination contract tests.
ID-only loading is reserved for trusted effect recovery after the Task ID is obtained from the
outbox. Public get uses only `load_generation_task_for_project`; absence and cross-Project identity
return the same `None` result.

Other focused ports are `GenerationTaskAssetSinkInterface`,
`GenerationTaskWorkflowCompletionInterface`, `GenerationTaskOriginStateReaderInterface`, and
`GenerationTaskClockInterface`. Application use
cases use generic trait bounds; only the dynamic provider registry requires trait objects.
`GenerationTaskAssetSinkInterface` has exactly
`recover_generation_task_asset(key) -> Available | Pending | SourceRequired` and
`store_generation_task_asset(command) -> GenerationTaskAvailableAsset`. Recovery performs no
provider call: `Available` returns the exact Asset, `Pending` proves that its durable staging and
finalization effect still exist so the Task reschedules without polling, and `SourceRequired`
reports that no recoverable exact Asset/source exists.
Store durably records/replays the deterministic node-output key, uses the Asset-owned
Pending/finalization protocol, and returns only after the exact Asset is Available. It changes
neither Task nor Workflow state. A crash is recovered by the Asset effect and retained Task effect;
replay calls recover by key first and polls the persisted remote handle only on `SourceRequired`.
It never repeats submission.

## 12. MVP persistence schema

### `generation_tasks`

| Fields | SQLite type | Purpose |
| --- | --- | --- |
| `id` PK | `BLOB` | Exact 16-byte RFC 9562 UUIDv4 task identity. |
| Origin IDs | `BLOB` | Exact 16-byte Project, Workflow, Run, node, and Node Execution identities. |
| `idempotency_key`, `request_hash` | `BLOB` | Unique `(project_id, idempotency_key)` and canonical SHA-256 request hash. |
| `request_schema_version`, `request_kind`, `request_json` | `INTEGER`, `TEXT`, `TEXT` | Immutable domain request snapshot. |
| profile, provider, route | bounded `TEXT` | Immutable non-secret target binding. |
| `status` | `TEXT` with `CHECK` | Normalized task status. |
| `progress_percent` | nullable `INTEGER` | Enforce `0..=100`. |
| `remote_task_id` | nullable `TEXT` | Opaque polling/cancellation handle. |
| Result fields | nullable `TEXT`/`BLOB` | One tagged inline Text or Asset reference, present exactly for `Succeeded`. |
| Failure fields | nullable `TEXT` | Kind, code, and safe message only. |
| `provider_deadline_at`, `completed_at` | `INTEGER`, nullable `INTEGER` | Aggregate-owned UTC epoch milliseconds. |
| `created_at`, `updated_at`, `revision` | `INTEGER` | Audit, ordering, optimistic lock. |

### `generation_task_outbox`

Fields: `id` PK, `task_id` FK, `kind`, `payload_json`, `deduplication_key`, `available_at`, `state`,
`delivery_attempts`, `processed_at`, `last_error`, and `created_at`. MVP kinds are `SubmitTask`,
`PollTask`, `CancelRemoteTask`, and `NotifyWorkflow`; states are `Ready`, `Claimed`, and `Completed`.

Desktop holds an OS-level exclusive lock for the database lifetime and starts exactly one task
worker. The worker claims at most one due `Ready` row. Graceful shutdown joins that worker before
releasing the lock. After an abnormal exit, acquiring the lock proves that no prior process worker
can still commit, so startup resets every `Claimed` row to `Ready`. There is no lease, renewal,
fencing token, active-worker registry, or same-process reclaim path in the MVP.

Outbox payloads contain task/message identifiers only. Workers load the aggregate from the repository; prompts, asset URLs, Provider requests, and signed output URLs are never copied into outbox rows.

Indexes: task list `(project_id, created_at DESC, id DESC)`, Workflow lookup
`(workflow_run_id, workflow_node_execution_id)`, unique origin `(project_id,
workflow_node_execution_id)`, and due outbox work `(processed_at, available_at, id)`. Recovery loads
by Task ID and its complete immutable target; no uniqueness is assumed for a provider's opaque
remote handles.

Rows and DTOs use named translators. Invalid row combinations return corruption errors and never bypass aggregate constructors.

## 13. MVP task-list and command contracts

`GenerationTaskSummaryDto` contains `id`, origin IDs, `requestKind`, `status`,
`progressPercent`, `generationProfileRef`, provider ID, optional current provider display name,
prompt preview, optional preview Asset ID, result presence, structured failure summary, and exact
`createdAt`, `updatedAt`, and optional `completedAt` timestamps. It never exposes a credential, route ID, native model ID, signed URL, or raw
provider response. Removed providers remain readable through provider ID without requiring a stale
display-name snapshot in the aggregate.

`GenerationTaskDto` additionally contains the optional tagged result. The opaque provider task handle remains
internal recovery state and is not exposed by either Task DTO; it has no useful desktop interaction
and must not become Workflow, Asset, or UI identity.

`GenerationTaskListRequestDto` requires `projectId` and accepts `status`, `requestKind`, `cursor`,
and `limit` (`1..=100`). Ordering is always immutable `(created_at DESC, id DESC)` so progress
updates cannot move rows across pages. The opaque seek cursor contains the last ordering pair.
Rows are current projections at read time; this is not a historical snapshot, but stable ordering
prevents duplicates and omissions caused solely by task updates.

Get requests require `projectId` plus `generationTaskId`. A task outside that
Project returns the same `NOT_FOUND` shape as an absent task. Repository application methods never
load or mutate a task by unscoped public identity.

MVP Tauri commands:

- `generation_task_get(request) -> GenerationTaskDto`
- `generation_task_list(request) -> CursorPage<GenerationTaskSummaryDto>`

There is no public create, cancel, status setter, delete, retry, archive, or generic update command.
Workflow cancellation drives the correlated Task through the internal cancellation use case, so a
single Task cannot create an undefined partial-Workflow outcome. DTOs
are separate tagged unions with named domain translators. Stable command error codes include
`INVALID_ARGUMENT`, `NOT_FOUND`, `CONFLICT`, `IDEMPOTENCY_CONFLICT`, `ORIGIN_CONFLICT`, `ILLEGAL_TRANSITION`,
`PROVIDER_NOT_CONFIGURED`, and `STORAGE_FAILURE`. Provider execution failures normally become task
state, not command transport errors.

## 14. Security and observability

MVP requirements:

- Task and outbox rows never contain provider credentials or credential references.
- Do not log prompts, input content, signed URLs, response bodies, or API keys.
- Remote media results enforce scheme/host policy, redirect/time/byte limits, MIME sniffing, and checksum validation.
- Raw provider requests/responses are not persisted.
- Structured tracing includes task ID, Workflow Run/Node Execution ID, provider ID, Generation
  Profile ref, normalized status, latency, and failure code.
- Minimum counters cover queued, running, succeeded, failed, cancelled, delivery retry, and startup claim-reset counts.

Focused task/application tests cover all four request/output branches, including inline Text
persistence and restart restore. Workflow E2E covers the three currently active provider-backed
Node Capabilities (Image, Video, and Voice); Text remains a provider/task contract ready for a
separately reviewed Text Node Capability rather than a hidden UI feature.

Future observability may add cost metrics, provider dashboards, long-term event analytics, and configurable retention.

## 15. MVP verification

1. Domain tests cover every legal/illegal transition, terminal immutability, progress monotonicity, output-kind validation, and cancel/complete races.
2. Request tests cover all four variants, canonical hashing, invalid modes, and Asset
   snapshot mismatch.
3. Repository contract tests cover atomic outbox consume/enqueue, optimistic conflicts, idempotency, cursor ordering, single-worker claiming, and startup claim reset.
4. Mock provider contract tests cover immediate, async, failure, idempotency, and repeatable terminal polling; a focused canceller fake separately proves the registered cancellation contract.
5. Application tests cover immediate success, async success, terminal failure, bounded safe delivery retry, ambiguous crash during submit, crash after accepted-handle commit, crash during asset import, duplicate create, and cancellation races.
6. Backend E2E covers Workflow node -> task -> deterministic provider -> Asset -> Workflow outcome
   for image, audio, and video, including restart after accepted submission. One exact chained case
   proves Text-to-Image output becomes the Image-to-Video input and the Workflow finishes with one
   image Task/Asset and one video Task/Asset without duplicate submission.
7. Tauri DTO fixtures and frontend contract tests are updated with command/DTO changes.

## 16. Implementation sequence

### MVP

1. Add task domain types, state transitions, failures, events, and unit tests.
2. Add application ports/use cases plus in-memory repository and Mock provider.
3. Add the two SQLite tables, translators, single-worker claim protocol, and repository contract tests.
4. Add worker/composition wiring and crash-recovery tests.
5. Add the two Tauri commands, task-list projection, DTO fixtures, and frontend API tests.
6. Run formatting, clippy, Rust tests, and the full E2E gate.

### Future delivery order

1. Design and add one production provider adapter at a time, including its typed route
   configuration and private native wire; each must pass the shared focused contracts.
2. Add user Retry plus `retry_of_task_id` if workflow rerun is insufficient.
3. Add `generation_task_attempts` and `RetryWaiting` only with automatic generation retry.
4. Add archive/retention when task volume requires it.
5. Add advanced modes and typed fields before considering versioned provider options.
6. Add routing, billing, webhooks, distributed workers, or 3D only behind separate design decisions.

## 17. Architectural consequences

- MVP has one aggregate, one lifecycle, four Text/Image/Video/Voice request variants, two tables, and two commands.
- Reliability comes from idempotency, optimistic revision, atomic outbox transitions, and content-addressed assets rather than an early general job framework.
- Adding a provider changes an adapter and composition wiring, not task semantics or storage.
- Adding a new media kind or mode is intentionally a domain/API/schema change.
- Attempt history and generation retry remain clean extensions instead of mandatory MVP concepts.
- Local cancellation is deterministic; remote cancellation and provider charges remain best effort.

Do not generalize this bounded context into a platform-wide job system until another business capability demonstrates the same semantics and contract requirements.
