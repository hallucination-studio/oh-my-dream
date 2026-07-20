# Backend Generation Task Architecture

- Status: frozen production multi-provider target
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
automatic cross-provider routing, standalone task creation, and 3D are Future features. They
must not complicate the MVP model before a concrete use case requires them.

## 2. Delivery scope

### 2.1 MVP

| Capability | MVP decision |
| --- | --- |
| Task model | One `GenerationTaskAggregate` for Text, Image, Video, and Voice generation. |
| Text | Text generation from text inputs; activated when a matching Node Capability/profile route is registered. |
| Image | Text-to-image producing one previewable Image Asset. |
| Voice | Text-to-speech producing one playable Audio Asset. |
| Video | One universal Text/FirstFrame/FirstAndLastFrames/MultimodalReference request with ordered typed media, remote create/query/cancel/delete lifecycle, and one playable Video Asset. |
| Providers | One provider-level interface composing complete Text/Image/Video/Voice interfaces; debug-gated deterministic Mock plus OpenAI Images, Seedream, Seedance, and Agent Plan HTTP TTS production routes. |
| Lifecycle | `Queued`, `Submitting`, `Running`, `CancelRequested`, `Succeeded`, `Failed`, `Cancelled`. |
| Reliability | Idempotency key, canonical request hash, optimistic revision, transactional outbox, bounded delivery retry, restart recovery. |
| Persistence | Tasks with one optional primary result, plus task outbox. |
| API | Get and list. Creation and cancellation occur only through the owning Workflow execution. |
| Task list | Project-scoped cursor pagination with optional status and request-kind filters. |
| Assets | An Image, Audio, or Video result must be an Available durable Asset eligible for its preview representation before task success. |
| Workflow | Terminal task event resumes or fails the owning workflow node through an adapter. |

This delivery is complete only when deterministic Workflow E2E remains green, users can save and
select multiple compatible model configurations, OpenAI Images and Seedream produce Image Assets,
Seedance resumes accepted work after restart only by a handle returned and durably persisted from a
trusted create response. An uncertain create is never repeated or guessed from list results. Its
queued cancellation and terminal record deletion are driven by
durable Task effects, its resulting Video is playable through the Asset preview
protocol, and Agent Plan HTTP TTS produces one playable durable MP3 Audio Asset. ASR is not part of
this Task contract.

### 2.2 Future

| Feature | Add only when |
| --- | --- |
| Automatic generation retry | Product policy requires a new provider submission after a terminal/transient generation failure. |
| `generation_task_attempts` | Multiple submissions under one task require durable attempt history. |
| Manual Retry command and `retry_of_task_id` | Users need task-level reruns outside normal workflow reruns. |
| Archive/retention commands | Task volume makes lifecycle retention controls necessary. |
| Provider-specific options JSON | A shipped model needs a setting that cannot be represented by a stable common field. |
| Advanced image modes | Inpaint, outpaint, masks, control images, or batch variations are scheduled. |
| Advanced audio modes | Music generation, ASR/speech-to-text, or audio-to-audio is scheduled. |
| Advanced video modes | Draft promotion, extend as a first-class operation, lip-sync, or timeline composition is scheduled. |
| Webhook completion | A provider offers reliable callbacks and polling cost matters. |
| Routing and failover | Equivalent provider behavior is defined and contract-tested. |
| Usage, billing, and quotas | Product decisions require cost reporting or limits. |
| Distributed workers | More than one process or host executes tasks. |
| Standalone task creation | Product semantics exist for tasks without a Workflow node origin. |
| 3D media | A 3D asset contract is designed; Meshy and Tripo3D remain references until then. |

Future features require their own reviewed contract and storage decision. They do not create
provider-specific task tables. This version performs a hard storage cut and implements no legacy
reader, importer, or migration.

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
- Provider billing, quota management, shared-account inheritance, or automatic route selection.
- Raw provider payload storage.
- Event sourcing; the task row is authoritative and the outbox carries work/events.

## 4. Lessons taken from DVStudio

The reference implementation keeps separate local mirrors for video, Ark, Gemini, Meshy, and Tripo3D tasks. Their common data includes remote task identity, provider/model, status, progress, prompt, results, error, project/node linkage, and timestamps. Seedance, Meshy, and Tripo3D demonstrate async submit/poll/cancel; Gemini demonstrates an immediate response followed by durable file storage.

The design keeps these behaviors but removes vendor-specific task tables. Provider request/response shapes remain boundary representations and never become the task domain or public DTO.

## 5. Ubiquitous language and boundaries

`GenerationTaskAggregate` owns one durable generation lifecycle. `GenerationTaskOrigin` identifies
its exact Project, Workflow Run, and Node Execution. `GenerationTaskRequest` is an immutable
provider-neutral snapshot, `GenerationTaskTarget` selects the stable Generation Profile, exact
Generation Model revision, and exact non-secret route target, `GenerationProviderTaskHandle` is the opaque remote identity,
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
| `origin` | Required `project_id`, frozen `workflow_id` and revision, `workflow_run_id`, `workflow_node_id`, `workflow_node_execution_id`, and exact capability contract ref. |
| `idempotency` | Caller key plus canonical request hash. |
| `request` | Immutable `GenerationTaskRequest`. |
| `target` | Immutable `GenerationProfileRef`, `GenerationModelRevisionRef`, `GenerationProviderConnectionRevisionRef`, `GenerationProviderCredentialBindingId`, `GenerationModelCapabilityContractRef`, `GenerationProviderId`, `GenerationProviderRouteId`, and closed route target: production service family/normalized Endpoint/native identity or debug built-in identity; no secret bytes, account object, or provider options JSON. |
| `provider_deadline_at` | Persisted UTC-millisecond deadline derived once from task creation time and the frozen route budget. |
| `remote_handle` | Optional opaque `GenerationProviderTaskHandle`, set at most once when remote submission is accepted and retained through terminal state for exact query/cancel/delete recovery. |
| `state` | `GenerationTaskState`, the sole lifecycle semantic owner. |
| `result` | Optional single `GenerationTaskResult`; set atomically with `Succeeded`. |
| `created_at`, `updated_at` | Values supplied by the task-owned clock port. |
| `revision` | Monotonic optimistic-lock version. |

The request hash covers schema version, origin, request, and target. It excludes timestamps and the idempotency key. Reusing `(project_id, idempotency_key)` with the same hash returns the existing task; a different hash returns `IdempotencyConflict`.
Generation kind is owned only by the closed `GenerationTaskRequest` variant; the target's protocol
must resolve to that same kind but does not add another caller-selected kind. The SQLite
`request_kind` column is a derived index discriminator.
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
    Asset { input_item_id: WorkflowInputItemId, role: AssetInputRole, asset: AssetSnapshotRef },
}
pub struct AssetSnapshotRef {
    pub asset_id: AssetId,
    pub media_kind: MediaKind,
    pub content_hash: ContentHash,
    pub mime: AssetMime,
    pub byte_length: NonZeroU64,
    pub media_facts: AssetMediaFacts,
}
pub struct VideoGenerationSpec {
    pub input_mode: VideoGenerationInputMode,
    pub prompt: Option<NonEmptyText>,
    pub images: Vec<VideoGenerationImageInput>,
    pub videos: Vec<VideoGenerationVideoInput>,
    pub audio: Vec<VideoGenerationAudioInput>,
    pub parameters: VideoGenerationParameters,
}
```

| MVP spec | Required | Optional stable fields |
| --- | --- | --- |
| `TextGenerationSpec` | `prompt` | system instruction and bounded output controls only when owned by an active profile contract |
| `ImageGenerationSpec` | `prompt`, `aspect_ratio` | none |
| `VoiceGenerationSpec` | `text` | none; the frozen profile owns voice/output format |
| `VideoGenerationSpec` | explicit mode, mode-valid prompt presence, ordered role-bearing media snapshots, and complete calibrated parameter set | none |

These four structs are operation-specific. `VideoGenerationInputMode` is exactly `TextToVideo`,
`FirstFrame`, `FirstAndLastFrames`, or `MultimodalReference`; it is not inferred from list counts.
The three media vectors retain Workflow input-item identity and order. Image roles are
`FirstFrame | LastFrame | ReferenceImage`; Video and Audio roles are exactly `ReferenceVideo` and
`ReferenceAudio`. Construction applies the capability-owned mode/cardinality rules and rejects an
audio-only multimodal request.

`VideoGenerationParameters` contains the provider-neutral structured values frozen in
`BACKEND_PROVIDERS.md`: generated-audio and draft Booleans when available, resolution, ratio,
`Auto | Seconds(u8) | Frames(u16)`, `Random | Fixed(u32)`, camera-fixed when available, and
watermark. Unsupported fields are absent only after successful calibration. Provider sentinel
`-1`, prompt-suffix flags, generic options JSON, and vendor defaults never enter the Task request.
Input Asset snapshots freeze hash, MIME, length, and verified facts so delivery retry and recovery
cannot silently observe changed media. Canonical hashing includes vector order, input-item IDs,
roles, and every calibrated value.

The current Task protocol has no provider-side input materialization child or effect. Therefore an
activated model contract may select only modes whose frozen local inputs can be consumed directly by
its route. `ReferenceVideo` remains a valid universal capability role for saved drafts, but readiness
returns structured `InputMaterializationUnavailable` and admission is blocked until a separate
durable materialization protocol, storage shape, and cleanup lifecycle are frozen together.

### 6.3 Result

`GenerationTaskResult` is the closed value
`Text { content } | Asset { asset_id, media_kind, content_digest }`.
`GenerationTaskAssetResult` is the media-only value contained by the latter variant.
The request kind mechanically determines the required result variant and media kind. Supporting
multiple primary results requires a later request and storage contract; this version does not add
ordinal, role, output identity, or a child table for a hypothetical batch operation.

The Asset domain remains the semantic owner of MIME type, dimensions, duration, checksum, current
availability, and storage location. Task success proves that the exact Asset ID/kind/digest was
Available when the result was constructed. The immutable result is historical evidence: a later
Asset `Missing` transition does not erase it or invalidate replay of `NotifyWorkflow`. Current
preview or downstream byte access still revalidates Asset availability through the Asset boundary.

## 7. MVP state machine

```rust
pub enum GenerationTaskState {
    Queued,
    Submitting,
    Running { progress_percent: Option<u8> },
    CancelRequested,
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
| `CancelRequested` | `CancelRequested` plus stored remote handle | An in-flight submit returns accepted after local cancellation intent committed. |
| `CancelRequested` | `Cancelled` | Remote cancellation is attempted when available. |
| `Running`/`CancelRequested` | `Cancelled` | Local cancellation wins when no complete remote canceller is registered. |
| `Running` | `Cancelled` | Poll reports that remote work was cancelled outside this process. |
| `Queued`/`Submitting`/`Running` | `Failed` | Permanent failure or exhausted delivery attempts. |

Invariants:

- Request, origin, target, and idempotency data never change.
- `remote_handle` changes only from absent to one validated accepted handle and never changes or
  clears afterward. Immediate or locally completed work has no handle. Running requires one.
  Terminal remote work retains it permanently so remote control/cleanup recovery never depends on
  lifecycle-state payload retention.
- The direct, successfully validated create response is the normal and authoritative source of a
  `remote_handle`: its provider task ID is stored as the generic
  `GenerationProviderTaskHandle`. The only recovery exception is a successful
  `ConfirmRemoteSubmission` outcome whose adapter proves the same local submission through a
  source-fixtured, provider-returned per-task correlation. The repository rejects a handle already
  claimed by another local Task. Timestamps, request fingerprints, model IDs, terminal-user IDs,
  and a merely unique inventory candidate are never ownership proof and cannot attach, cancel,
  delete, download, or publish another remote task.
- Provider deadline never changes. Expiry fails `Queued` or `Running` with `Timeout`.
  An uncertain remote create consumes `SubmitTask`, retains `Submitting`, and enqueues the bounded
  `ConfirmRemoteSubmission` recovery effect. It never repeats create. Only confirmation exhaustion
  or a confirmation result that cannot prove ownership fails the Task as `AmbiguousSubmission`.
  Expiry of `CancelRequested` commits `Cancelled` and records safe
  remote-cancellation-unconfirmed telemetry.
- Progress is optional `0..=100` and monotonic while running.
- `Succeeded` requires exactly one result matching the request's result kind.
- Text or an Available media Asset result commits atomically with `Succeeded` and is immutable.
  Before success, media recovery uses the deterministic Asset node-output key rather than partially
  attaching a result to the Task.
- Terminal states never transition.
- Cancellation is rejected only after a terminal state has committed.
- `CancelRequested` transitions only to `Cancelled`. A permanent failure, exhausted delivery
  budget, or deadline expiry observed while cancellation is pending converges to `Cancelled` with
  safe telemetry; it never becomes `Failed`.
- Optimistic revision serializes cancel/complete races: the first committed transition wins. A
  submit outcome that loses to `CancelRequested` is reconciled against the reloaded state; an
  accepted handle is attached only to drive cancellation and can never return to `Running`.
- Provider status strings and human-readable error text never drive transitions.

Deadline/cancellation precedence is exact:

| Observed durable state | Resolution at or beyond deadline |
| --- | --- |
| `Queued` ready to submit | fail `Timeout` without submission |
| `Submitting` | recover deterministic output if present; otherwise finish `ConfirmRemoteSubmission`: attach only a source-proven handle, or fail `AmbiguousSubmission` once confirmation is exhausted or unconfirmable; never repeat create |
| `Running` | recover/finalize an exact available output first; otherwise fail `Timeout` |
| `CancelRequested` | commit `Cancelled` and record cancellation-unconfirmed telemetry |

An already-committed terminal state always wins. A committed Workflow cancellation observed before
the Task terminal commit wins over ordinary success/failure and converges locally to `Cancelled`.

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
persisted remote handle. An uncertain Immediate call becomes `AmbiguousSubmission`, because it has
no durable remote-confirmation composition. An uncertain remote Submit consumes `SubmitTask` and
enters the bounded confirmation branch; it does not fail merely because the create response was
lost. `ConfirmRemoteSubmission` either binds a source-proven handle, reschedules while the provider
has not yet exposed the proved correlation, or concludes `AmbiguousSubmission` when it cannot prove
ownership. The failure is Task-owned delivery state, not a provider-declared generation result.
Because the current Seedance list response has no uniquely echoed client correlation or idempotency
value, its confirmation implementation can never bind a list candidate. List observations remain
private diagnostics and cannot authorize attachment, cancellation, deletion, result download, or
Asset publication. A later provider may return `Confirmed` only through the reviewed Task-owned
confirmation interface and a source-fixtured ownership proof.
Query-by-ID recovery after `Running` does not require submission idempotency.
This is bounded delivery retry, not a new generation attempt. A non-transient call error or
exhausted delivery budget transitions the task to `Failed`. A terminal provider failure fails
immediately.

Every provider-work reschedule is capped by `provider_deadline_at`, checked `u32` delivery-attempt arithmetic,
and the frozen 500..=5,000 millisecond poll bounds. Provider retry-after may delay within those
bounds but cannot extend the task deadline. Deadline exhaustion transitions active generation once
to `Failed { Timeout }` and enqueues `NotifyWorkflow`; `CancelRequested` instead becomes local
`Cancelled` with the same notification. No task polls forever. The same deadline cap applies to
every effect reschedule, including `SubmitTask` for a `Queued` task still waiting on Workflow
handoff: a reschedule that would land at or past `provider_deadline_at` instead commits the
terminal outcome — `Failed { Timeout }`, or `Cancelled` when cancellation is pending — and
enqueues `NotifyWorkflow`.

## 9. Provider contracts

Generation Task is unified; provider implementations are capability-composed behind one provider-
level `GenerationProviderInterface`. Every provider implementation exposes stable provider identity
and one non-empty `GenerationProviderCapabilities` value. A provider contributes only capabilities
it actually implements.

Examples:

```text
Mock provider under debug gate = Image + Video + Voice
OpenAI provider               = Image
Volcengine Ark provider       = Image + Video
Volcengine Agent Plan provider = Voice
```

Multiple providers may implement the same generation type. One provider object may contribute
several types while sharing account/authentication infrastructure. `GenerationProfileRef` and
the admission-frozen `GenerationModelRevisionRef` select the configured provider/route, and the
immutable task target preserves that exact choice for recovery. Current Settings never participate
after target construction.

Before an Immediate or Submit call whose request contains media, the Task application opens every
frozen `AssetSnapshotRef` through `GenerationTaskInputAssetReaderInterface`. It requires the exact
Project, media kind, digest, MIME, byte length, and facts and constructs one ephemeral
`VideoGenerationInputSourceSet` preserving kind-local vector order, stable input-item ID, and role.
Each source owns a one-shot bounded async lease. Leases are never persisted or passed to Workflow;
the provider route consumes them only to encode or materialize the current authenticated request.
Poll and cancel calls never reopen input media. A source mismatch is `InputAssetUnavailable` before
network I/O and never triggers model fallback.

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
        submission_confirmer: Arc<dyn GenerationRemoteSubmissionConfirmerInterface>,
        poller: Arc<dyn ImageGenerationPollerInterface>,
    },
    CancellableRemote {
        submitter: Arc<dyn ImageGenerationSubmitterInterface>,
        submission_confirmer: Arc<dyn GenerationRemoteSubmissionConfirmerInterface>,
        poller: Arc<dyn ImageGenerationPollerInterface>,
        canceller: Arc<dyn GenerationCancellerInterface>,
    },
    DeletableRemote {
        submitter: Arc<dyn ImageGenerationSubmitterInterface>,
        submission_confirmer: Arc<dyn GenerationRemoteSubmissionConfirmerInterface>,
        poller: Arc<dyn ImageGenerationPollerInterface>,
        deleter: Arc<dyn GenerationRemoteTaskDeleterInterface>,
    },
    CancellableAndDeletableRemote {
        submitter: Arc<dyn ImageGenerationSubmitterInterface>,
        submission_confirmer: Arc<dyn GenerationRemoteSubmissionConfirmerInterface>,
        poller: Arc<dyn ImageGenerationPollerInterface>,
        canceller: Arc<dyn GenerationCancellerInterface>,
        deleter: Arc<dyn GenerationRemoteTaskDeleterInterface>,
    },
}

pub struct GenerationProviderCallContext {
    pub task_id: GenerationTaskId,
    pub target: GenerationTaskTarget,
    pub task_created_at: Timestamp,
    pub provider_deadline_at: Timestamp,
    pub remote_submission_fence: Option<GenerationRemoteSubmissionFence>,
}

pub struct GenerationRemoteSubmissionFence {
    pub scope_id: GenerationRemoteSubmissionScopeId,
    pub correlation_id: GenerationRemoteSubmissionCorrelationId,
    pub submitted_at: Timestamp,
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
pub trait GenerationRemoteSubmissionConfirmerInterface: Send + Sync {
    async fn confirm_remote_submission(
        &self,
        context: &GenerationProviderCallContext,
    ) -> Result<GenerationRemoteSubmissionConfirmation, GenerationProviderCallError>;
}

pub enum GenerationRemoteSubmissionConfirmation {
    Confirmed(GenerationProviderTaskHandle),
    NotObservedYet,
    Unconfirmable,
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

#[async_trait]
pub trait GenerationRemoteTaskDeleterInterface: Send + Sync {
    async fn delete_remote_generation_task(
        &self,
        context: &GenerationProviderCallContext,
        handle: &GenerationProviderTaskHandle,
    ) -> Result<GenerationRemoteTaskDeletionOutcome, GenerationProviderCallError>;
}
```

`GenerationProviderCapabilities::try_new` rejects four absent fields. `Option` represents the
provider's immutable composition, not an optional operation: callers select only a present complete
interface, and no focused interface may return `Unsupported`.

Each provider contributes at most one focused interface per kind. Its focused contract owns a
non-empty set of shipped route contracts for that kind. The immutable model revision resolves to
one of those route IDs; task admission persists it, and the matching
`resolve_*_generation_route` method returns the exact execution composition during both initial
dispatch and restart recovery.

The Text, Video, and Voice execution values follow the same closed shape. Each returned execution
value is one closed `Immediate`, `Remote`, `CancellableRemote`, `DeletableRemote`, or
`CancellableAndDeletableRemote` composition. `Immediate` contains one complete executor. Every
remote variant contains one submitter, one complete submission confirmer, and one poller. The
remaining variants add only a complete canceller, a complete remote-record deleter, or both; no
method returns `Unsupported` and no `supports_*` probe exists. Confirmation is not an optional
provider convenience: every remote route must state the only safe recovery outcome for a lost create
response. A route without a source-proven correlation returns `Unconfirmable`; it never guesses from
an inventory match.
The Text, Video, and Voice executor/submitter/poller methods have the same call context and
type-specific spec/result outcome. Video Immediate/Submit additionally consume the complete
non-cloneable `VideoGenerationInputSourceSet`; Text, Image-from-Text, Voice, poll, and cancel accept
no meaningless media argument. `GenerationProviderCallContext` is immutable task-owned data;
adapters may not reinterpret it as vendor configuration. Immediate outcomes are exactly
`Completed(result) | Rejected(failure)`. Submit outcomes are exactly
`Accepted(handle) | Completed(result) | Rejected(failure)`; confirmation outcomes are exactly
`Confirmed(handle) | NotObservedYet | Unconfirmable`; poll outcomes are exactly
`Pending(progress) | Completed(result) | Failed(failure) | Cancelled`, and cancellation outcomes are
exactly `Cancelled | AlreadyCancelled | TooLateRunning | RemoteAbsent`. `TooLateRunning` is the
normal race outcome of a provider whose documented cancellation operation applies only while
queued; it is not `Unsupported` and local cancellation still converges. Remote deletion outcomes are exactly
`Deleted | RemoteAbsent`. Deletion proves only that the provider task record is deleted or absent;
it is not interpreted as cancellation unless an accepted provider fixture separately proves that
the same operation stops execution. A remote poller must return an equivalent terminal outcome for
the same persisted handle through the task deadline.

Every completed media outcome contains exactly one non-cloneable
`GenerationTaskOutputSourceLease`, owned by the Task interface and backed by one already-open
`Pin<Box<dyn AsyncRead + Send>>`. The lease has one consuming `try_take_stream` operation and the
remaining Task deadline; it is process-local, non-serializable, non-rewindable, and contains no URL,
path, provider response, or adapter handle. The worker moves it directly into the Asset sink's
`AssetNodeOutputSourceLease`; neither layer buffers a complete media output in business memory.

There is no optional execution method, `supports_*` probe, or `Unsupported` result. Absence of Voice
means `capabilities.voice` is `None`, so Voice Settings choices cannot be derived for that provider.
The production OpenAI Images and Agent Plan HTTP TTS routes exercise and freeze the `Immediate`
execution composition. The Text focused contract remains reserved because no active Node
Capability/route exercises it; its semantics bind only when the first Text route ships with
implementation and contract tests in the same change.

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
admission resolves the admission-frozen model revision and copies its exact
profile/protocol/provider/route/endpoint/native-identity tuple into the immutable target before any
external call; row restoration enforces that closed shape.

Each submit outcome is `Accepted`, type-specific `Completed`, or
`Rejected(GenerationProviderFailure)`. Each remote poll outcome is `Pending`, type-specific
`Completed`, `Failed(GenerationProviderFailure)`, or `Cancelled`. Only provider capability
implementations see vendor statuses.

Production adapters validate third-party responses as untrusted, use the stable task-derived
submission correlation only where their reviewed protocol proves both request carriage and response
echo, return normalized outputs/handles, and keep credentials, signed URLs, raw payloads, and vendor
error bodies out of DTOs and logs. A route that cannot return a durable handle may complete
immediately but cannot claim restart-safe remote polling. Mock and Seedance Remote contracts
guarantee that polling the same accepted handle after completion returns an equivalent terminal
outcome through the task deadline. Credentials are
loaded by the exact non-secret credential-binding ID persisted in the Task target immediately before
each authenticated call. Current model or connection Settings are never consulted. Token-only
rotation may repair that same Endpoint-compatible binding; missing retained bytes fail structurally
before a call or as `Authentication` when removed concurrently.

Production provider composites may contribute any non-empty subset without changing Generation
Task semantics or these interfaces.

## 10. Durable MVP execution

1. A generation Node Capability resolves its semantic inputs, calls the task-start bridge, and receives the durable task identity.
2. `GenerationTaskAggregate::create` validates invariants and emits `SubmitTask`.
3. One transaction inserts the task and outbox message.
4. The worker claims the message, requires the exact origin Node Execution to be
   `WaitingForExternalCompletion`, and atomically persists `Submitting`, a stable diagnostic client
   request ID, and one `GenerationRemoteSubmissionFence` before calling create outside the
   transaction. The fence contains an opaque local correlation and a submission scope. The
   repository allows at most one unresolved submission in that scope; a scope is derived from the
   frozen remote target plus the stable local terminal identity. This serializes local uncertainty;
   it is never evidence that an inventory task is ours. A still-`Running` origin reschedules without
   changing state or calling the provider. The diagnostic ID is not an idempotency or ownership claim
   unless the provider's reviewed create contract explicitly guarantees those semantics.
5. Direct `Accepted` atomically saves the validated handle and `Running`, consumes `SubmitTask`, and
   enqueues `PollTask`. This is the normal Seedance path: create returns its task `id`, the generic
   Task stores it as `GenerationProviderTaskHandle`, and all later query/cancel/delete work uses that
   exact handle. An uncertain remote Submit instead consumes `SubmitTask`, retains `Submitting`, and
   enqueues `ConfirmRemoteSubmission`; create is never repeated. A direct `Completed` or `Rejected`
   settles the Task normally.
6. `ConfirmRemoteSubmission` is a Task-owned recovery branch, not a callback or a vendor-visible
   workflow. `Confirmed(handle)` atomically attaches the handle and enqueues `PollTask`, or
   `CancelRemoteTask` when local cancellation already won. `NotObservedYet` reschedules only within
   the original Task deadline. `Unconfirmable`, a permanent confirmation error, or deadline
   exhaustion commits `Failed(AmbiguousSubmission)` and enqueues `NotifyWorkflow`. Raw list records
   never leave the adapter, and only a source-proven per-task correlation may produce `Confirmed`.
7. `Pending` atomically saves progress, consumes the current poll, and enqueues the next delayed poll.
8. Completed Text is validated inline. For completed Image, Video, or Voice media,
   `store_generation_task_asset` records/replays the deterministic Asset node-output key, durably
   drives its existing Pending finalization protocol, and returns only an Available Asset. The Task
   remains `Submitting` or `Running` during this external Asset operation.
9. Success atomically stores the Text or Asset result, saves `Succeeded`, consumes the work message,
   and enqueues `NotifyWorkflow`. The sink returns an exact replay for the same digest and a conflict
   for different bytes.
10. Terminal failure atomically saves `Failed`, consumes the work message, and enqueues `NotifyWorkflow`.
11. A transient origin read, Poll, Confirm, Cancel, Asset finalization, or notification
    error reschedules the same safe message. An uncertain Immediate call fails
    `AmbiguousSubmission`; an uncertain remote Submit follows steps 5-6. A live process
    never reclaims its worker's `Claimed` message; startup resets prior-process claims only after
    acquiring the exclusive database lock.

When a remote route composes `GenerationRemoteTaskDeleterInterface`, has a persisted handle, and has
observed provider-terminal evidence, the corresponding terminal transition also enqueues one
deduplicated `DeleteRemoteTask` effect in the same transaction. Local cancellation without confirmed
remote termination does not enqueue deletion. The effect runs only after the Task result/failure and
terminal state are durable.
`Deleted` and `RemoteAbsent` complete it idempotently. Transient failures reschedule within a
deletion-specific bounded delivery budget; exhaustion completes the effect with safe telemetry and
never changes the Task result, Workflow outcome, or managed Asset. A provider record is therefore
cleanup state, not Task identity or history.

If the process crashes during finalization, the unconsumed message is reclaimed and the Asset sink
is queried first by deterministic task/output key. `Available` completes without a provider call;
`Pending` reschedules behind its durable Asset effect; `Missing` fails once with
`OutputAssetImport` and enqueues `NotifyWorkflow`; only `SourceRequired` permits an accepted
asynchronous provider to be queried again by its persisted handle. An Immediate call without a
durable result and a remote create without a persisted accepted handle are never repeated after a
crash. For a remote `Submitting` Task with a persisted submission fence, startup atomically replaces
the stale `SubmitTask` with `ConfirmRemoteSubmission`. The confirmation path can bind only a
source-proven handle; otherwise it eventually commits `AmbiguousSubmission` and notifies Workflow.
Restart-safe polling begins after a direct create response or confirmed recovery handle is committed
atomically with `Running`.

Queued cancellation atomically commits `Cancelled`, consumes stale `SubmitTask`, and enqueues
`NotifyWorkflow`. Submitting cancellation commits `CancelRequested`; only the validated direct
response from that already in-flight create call or a source-proven confirmation may still attach the
accepted handle solely to drive remote control/cleanup. An unconfirmable response converges to local
`Cancelled` with `NotifyWorkflow` and leaves any unowned remote work untouched.
Running cancellation either commits
`CancelRequested` while retaining the handle, consumes `PollTask`, and enqueues `CancelRemoteTask` when a
complete `GenerationCancellerInterface` is registered, or atomically commits local `Cancelled`,
consumes `PollTask`, and enqueues `NotifyWorkflow` when none is registered. A deletable-only route
does not delete a possibly active remote task; safe telemetry states that external work and charges
may continue. Application wiring selects the separate complete canceller and deleter boundaries;
the aggregate has no support probe.

Cancellation enters this state machine when the Task worker's mandatory origin read observes that
the canonical `WorkflowCancelRunUseCase` has committed the owning Run cancellation. Desktop may wake
the relevant worker after that commit for responsiveness, but the durable Submit/Poll effect and
origin read are the recovery authority, so a crash cannot lose cancellation convergence. There is
no independently authorized Task-cancel command in MVP.

When an in-flight submit returns after `CancelRequested`, `Rejected` or `Completed` converges to
local `Cancelled` with `NotifyWorkflow`. `Accepted(handle)` or `Confirmed(handle)` atomically
attaches the handle and then enqueues `CancelRemoteTask`, or converges directly to local `Cancelled`
when no canceller is registered. `NotObservedYet` keeps only `ConfirmRemoteSubmission` alive through
the bounded deadline; `Unconfirmable` converges locally to `Cancelled`. A deletable-only route
retains the handle but does not delete without provider-terminal evidence. Every path consumes the
claimed submit or confirmation effect. No cancellation path leaves stale recovery work or a waiting
Workflow node without a terminal notification.

A remote canceller returning `TooLateRunning` commits local `Cancelled` and `NotifyWorkflow`, then
atomically sets `remote_cleanup_deadline_at` from the route's fixed cleanup policy and enqueues
`PollTask` for control-only observation by the already persisted handle. That effect never attaches
a late result; it continues until provider-terminal evidence permits `DeleteRemoteTask` or the
durable cleanup deadline expires. Cleanup-deadline exhaustion consumes the poll with safe telemetry
and cannot rewrite Task, Workflow, or Asset state. A provider-confirmed queued cancellation needs no
deletion when the provider contract guarantees automatic cancelled-record expiry.

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
`DesktopPostCommitEffect`. `GenerationTaskEffectWorkerImpl` consumes only the six closed task
effect kinds. It neither schedules Workflow graphs nor accepts arbitrary handlers or payloads.
Desktop owns one database-wide process lock and runs exactly one Generation Task worker. Startup
resets every prior-process `Claimed` task effect to `Ready` before accepting commands; the running
process never reclaims work from its live worker. It does not
interrupt a Run whose waiting Node Executions have either authoritative non-terminal Generation
Tasks or terminal Generation Tasks with unprocessed `NotifyWorkflow`. A terminal task whose
notification is already processed requires the node to be terminal; any other combination is
corruption and fails startup rather than guessing. A Running node with an exact Queued task,
unconsumed SubmitTask, and no handle replays Workflow to finish handoff. A waiting node with a
Submitting remote Task but no directly persisted handle restores `ConfirmRemoteSubmission` from its
durable fence before prior-process claims are reset. It becomes `Failed(AmbiguousSubmission)` only
when that bounded confirmation finishes without a proved handle. Other Running nodes without durable
handoff still use `InterruptedByRestart`.

The completion interface returns exactly `Applied`, `AlreadyApplied`, or `OriginTerminal`; all three
consume `NotifyWorkflow`, and consumption commits strictly after the Workflow completion commit has
returned, so a terminal task with a consumed notification proves its node outcome was durably
committed first. A transient storage error reschedules it, and so does a Workflow completion
revision conflict: bounded in-flight execution lets `NotifyWorkflow` effects for different tasks in
the same Run race on one `WorkflowRunAggregate`, the first commit wins, and the rescheduled loser
converges to `Applied`, `AlreadyApplied`, or `OriginTerminal` on replay. A terminal notification has
no delivery-attempt exhaustion path: it remains durable until one of those three outcomes commits.
Its diagnostic attempt counter saturates at `u32::MAX` and never controls delivery. Corrupt identity
or result data fails startup and is never silently abandoned.

`GenerationTaskOriginStateReaderInterface` and `GenerationTaskWorkflowCompletionInterface` are the
only Workflow-to-Task crossing points. Every future generation-adjacent feature extends one of
these two contracts; introducing a third crossing point requires a new reviewed architecture
decision.

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
result, consumed outbox work, and new outbox work. The Desktop task-start bridge resolves the exact
`GenerationModelRevisionRef` frozen by Run admission and copies its complete non-secret target into
the Task; later Settings changes cannot change that Run. Consuming, completing, or rescheduling work
requires the effect ID to remain `Claimed` and the aggregate revision to match. The process lock,
single claiming worker, bounded in-flight executions, and startup-only claim reset ensure that no
live worker is reclaimed concurrently. `GenerationTaskOutboxReaderInterface` claims at most one due effect and
reschedules safe transient delivery errors. `GenerationTaskRepositoryFakeImpl` and
`SqliteGenerationTaskRepositoryAdapterImpl` run the same idempotency, concurrency, transaction,
ordering, and pagination contract tests.
ID-only loading is reserved for trusted effect recovery after the Task ID is obtained from the
outbox. Public get uses only `load_generation_task_for_project`; absence and cross-Project identity
return the same `None` result.

Other focused ports are `GenerationTaskAssetSinkInterface`,
`GenerationTaskInputAssetReaderInterface`,
`GenerationTaskWorkflowCompletionInterface`, `GenerationTaskOriginStateReaderInterface`, and
`GenerationTaskClockInterface`. Application use
cases use generic trait bounds; only the dynamic provider registry requires trait objects.
`GenerationTaskInputAssetReaderInterface::open_generation_task_input_asset` accepts Project ID,
one exact `AssetSnapshotRef`, and the current provider-call deadline. It returns a matching
`GenerationTaskInputAssetSourceLease` only after revalidating visibility, availability, kind,
digest, MIME, length, and facts. The lease is non-cloneable, one-shot, deadline-bound, and exposes
no path, URL, seek, reopen, or provider identity. A shared contract suite covers Project isolation,
snapshot mismatch, ordering, deadline, and source failure. The Task application checks aggregate
cancellation before and after each open.
`GenerationTaskAssetSinkInterface` has exactly
`recover_generation_task_asset(key) -> Available | Pending | Missing | SourceRequired` and
`store_generation_task_asset(command) -> GenerationTaskAvailableAsset`. Recovery performs no
provider call: `Available` returns the exact Asset, `Pending` proves that its durable staging and
finalization effect still exists so the Task reschedules without polling, `Missing` proves the
output key is terminally bound to unavailable content and fails the Task once as
`OutputAssetImport`, and `SourceRequired` reports that no Asset is bound and provider result bytes
are still required. `Missing` enqueues `NotifyWorkflow` with the terminal Task failure and never
polls, downloads, stages, or attempts to replace that Asset identity.
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
| Workflow revision | `INTEGER` | Frozen non-zero Workflow revision. |
| Capability contract | `TEXT`, `INTEGER`, `INTEGER` | Exact capability ID plus non-zero major and minor version. |
| `idempotency_key`, `request_hash` | `BLOB` | Unique `(project_id, idempotency_key)` and canonical SHA-256 request hash. |
| `request_schema_version`, `request_kind`, `request_json` | `INTEGER`, `TEXT`, `TEXT` | Immutable domain request snapshot. |
| profile, model ID/revision, connection revision, credential-binding ID, model-contract ref, protocol/variant, provider, route | bounded `TEXT`/`BLOB`/`INTEGER` | Immutable non-secret target and recovery identity. |
| route target | tagged bounded `TEXT` | Production Endpoint/native identity or debug built-in identity with mutually exclusive row constraints; never returned by public Task DTOs. |
| `status` | `TEXT` with `CHECK` | Normalized task status. |
| `progress_percent` | nullable `INTEGER` | Enforce `0..=100`. |
| `remote_task_id` | nullable `TEXT` | Opaque query/cancellation/deletion handle retained independently of lifecycle state. |
| `client_request_id` | nullable bounded `TEXT` | Stable diagnostic correlation for a Submit attempt; it does not prove provider idempotency or remote-task ownership. |
| `submission_scope_id`, `submission_correlation_id`, `submission_fence_at` | nullable bounded `BLOB`/`TEXT`/`INTEGER` | Opaque Task-owned fence committed before one remote create. Present together for unresolved remote submission; never exposed by DTOs or interpreted as provider proof without a source-fixtured echo. |
| Result fields | nullable `TEXT`/`BLOB` | One tagged inline Text or Asset reference, present exactly for `Succeeded`. |
| Failure fields | nullable `TEXT` | Kind, code, and safe message only. |
| `provider_deadline_at`, `remote_cleanup_deadline_at`, `completed_at` | `INTEGER`, nullable `INTEGER` | Aggregate-owned UTC epoch milliseconds. Cleanup deadline is present only after `TooLateRunning` on a route with fixed control-only cleanup. |
| `created_at`, `updated_at`, `revision` | `INTEGER` | Audit, ordering, optimistic lock. |

### `generation_task_outbox`

Fields: `id` PK, `task_id` FK, `kind`, `payload_json`, `deduplication_key`, `available_at`, `state`,
`delivery_attempts`, `processed_at`, `last_error`, and `created_at`. MVP kinds are `SubmitTask`,
`ConfirmRemoteSubmission`, `PollTask`, `CancelRemoteTask`, `DeleteRemoteTask`, and
`NotifyWorkflow`; states are `Ready`, `Claimed`, and `Completed`.

Desktop holds an OS-level exclusive lock for the database lifetime and starts exactly one task
worker. That worker is the only claimer and claims due `Ready` rows one at a time, but it executes
claimed effects on a bounded in-flight pool (`generation_task_effect_concurrency`, default `4`,
bounds `1..=8`) so one slow provider call or media download never blocks polling, submission, or
cancellation of other tasks. At most one effect per task is in flight; a due row whose task already
has an active execution is skipped until that execution commits. Each execution is bounded by the
smaller of its route operation deadline and the remaining task budget, so a hung call cannot leak
its slot, and each execution commits its own atomic aggregate/outbox transition. Graceful shutdown
joins every in-flight execution before releasing the lock. After an abnormal exit, acquiring the
lock proves that no prior process worker can still commit, so startup resets every `Claimed` row to
`Ready`. There is no lease, renewal, fencing token, active-worker registry, or same-process reclaim
path in the MVP.

Outbox payloads contain task/message identifiers only. Workers load the aggregate from the repository; prompts, asset URLs, Provider requests, and signed output URLs are never copied into outbox rows.

Indexes: task list `(project_id, created_at DESC, id DESC)`, Workflow lookup
`(workflow_run_id, workflow_node_execution_id)`, unique origin `(project_id,
workflow_node_execution_id)`, and due outbox work `(processed_at, available_at, id)`. Recovery loads
by Task ID and its complete immutable target. The same opaque text may exist in different
provider/connection scopes, but `(connection_revision_ref, route_id, remote_handle)` is unique when
non-null so one remote task cannot be claimed by two local Tasks. A partial unique constraint on
`submission_scope_id` permits at most one unbound `Submitting`/`CancelRequested` remote Task per
scope. This prevents local overlap during confirmation but does not turn timing or inventory
uniqueness into remote ownership evidence.

Rows and DTOs use named translators. Invalid row combinations return corruption errors and never bypass aggregate constructors.

## 13. MVP task-list and command contracts

`GenerationTaskSummaryDto` contains `id`, origin IDs, `requestKind`, `status`,
`progressPercent`, `generationProfileRef`, stable Generation Model ID/revision, the admitted model
display-name snapshot, and creator-facing protocol name,
prompt preview, optional preview Asset ID, result presence, structured failure summary, and exact
`createdAt`, `updatedAt`, and optional `completedAt` timestamps. It never exposes a credential,
endpoint, route ID, native model ID, signed URL, or raw provider response. Removed model
configurations remain readable through the admitted display-name/protocol snapshot.

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

- Task rows contain the exact non-secret credential-binding ID required for recovery but never
  credential bytes; outbox rows contain neither.
- Do not log prompts, input content, signed URLs, response bodies, or API tokens.
- Remote media results enforce scheme/host policy, redirect/time/byte limits, MIME sniffing, and checksum validation.
- Raw provider requests/responses are not persisted.
- Structured tracing includes task ID, Workflow Run/Node Execution ID, provider ID, Generation
  Profile ref, Generation Model ID/revision, protocol, normalized status, latency, and failure code.
- Minimum counters cover queued, running, succeeded, failed, cancelled, delivery retry, startup
  claim reset, submission confirmation outcomes, ambiguous submission, `TooLateRunning`, and remote
  deletion success/absence/exhaustion.

Focused task/application tests cover all four request/output branches, including inline Text
persistence and restart restore. Remote tests cover create, persisted-handle query, cancellation,
deletion cleanup, delete exhaustion,
cancellation/delete races, and restart recovery. Workflow E2E
covers the three currently active provider-backed Node Capabilities (Image, Video, and Voice), and
proves each media result is Available and previewable; Text remains a provider/task contract ready
for a separately reviewed Text Node Capability rather than a hidden UI feature.

Future observability may add cost metrics, provider dashboards, long-term event analytics, and configurable retention.

## 15. MVP verification

1. Domain tests cover every legal/illegal transition, terminal immutability, progress monotonicity, output-kind validation, and cancel/complete races.
2. Request tests cover all four variants, canonical hashing, invalid modes, and Asset
   snapshot mismatch.
3. Repository contract tests cover atomic outbox consume/enqueue, optimistic conflicts, idempotency, cursor ordering, single-worker claiming, and startup claim reset.
4. Debug-gated Mock and production provider contract tests cover immediate, async, authentication,
   malformed responses, idempotency where proved, repeatable terminal polling, and each complete
   submitter/confirmer/poller/canceller/deleter composition.
5. Remote ownership tests prove the direct validated create response is the normal handle source;
   a source-proven confirmation may bind the same submission after response loss; no confirmation
   retries create; `NotObservedYet` remains bounded; `Unconfirmable` becomes
   `AmbiguousSubmission`; restart restores confirmation from the durable fence; remote-handle and
   unresolved-scope uniqueness are enforced; and an externally-created identical task can never be
   attached, queried, cancelled, deleted, downloaded, or published by this client.
6. Seedance control tests prove queued cancellation, the queued-to-running `TooLateRunning` race,
   local cancellation without late result attachment, later succeeded/failed/expired deletion,
   cancelled-record auto-expiry without delete, and control-only polling of an already-bound handle
   through the independent cleanup deadline.
7. Backend E2E covers Workflow node -> task -> deterministic provider -> Asset -> Workflow outcome
   for image, audio, and video, including restart after accepted submission. One exact chained case
   proves Text-to-Image output becomes the FirstFrame input of universal Video generation and the Workflow finishes with one
   image Task/Asset and one video Task/Asset without duplicate submission.
8. Tauri DTO fixtures and frontend contract tests are updated with command/DTO changes.

## 16. Implementation sequence

### Current production extension

1. Hard-cut the Video request to the universal ordered multimodal shape and add exact request tests.
2. Add immutable model-revision and model-contract target resolution without changing the Task
   state machine.
3. Extend persistence/translation and recovery tests for the exact non-secret target snapshot.
4. Register debug-gated Mock plus OpenAI Images, both frozen Seedream identities, Seedance create/
   query/cancel/delete, and the exact Agent Plan HTTP TTS operation behind the focused
   interfaces and shared contract suites.
5. Extend task-list projections, DTO fixtures, and production-gated E2E coverage.
6. Run formatting, clippy, focused Rust/frontend suites, and the CI E2E gate.

### Future delivery order

1. Add user Retry plus `retry_of_task_id` if workflow rerun is insufficient.
2. Add `generation_task_attempts` and `RetryWaiting` only with automatic generation retry.
3. Add archive/retention when task volume requires it.
4. Add advanced modes, including ASR only through its own capability contract, before considering
   versioned provider options.
5. Add automatic routing/failover, billing, webhooks, distributed workers, or 3D only behind
   separate design decisions.

## 17. Architectural consequences

- MVP has one aggregate, one lifecycle, four Text/Image/Video/Voice request variants, two tables, and two commands.
- Reliability comes from idempotency, optimistic revision, atomic outbox transitions, and content-addressed assets rather than an early general job framework.
- Adding an implementation of an existing protocol changes an adapter and composition wiring, not
  task lifecycle semantics.
- Adding a new media kind or mode is intentionally a domain/API/schema change.
- Attempt history and generation retry remain clean extensions instead of mandatory MVP concepts.
- Local cancellation is deterministic. Remote cancellation is route-specific: Seedance confirms
  queued cancellation, reports running work as `TooLateRunning`, and polls the already-bound handle
  in control-only mode for later cleanup.

Do not generalize this bounded context into a platform-wide job system until another business capability demonstrates the same semantics and contract requirements.
