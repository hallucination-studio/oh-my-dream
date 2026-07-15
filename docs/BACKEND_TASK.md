# Backend Generation Task Architecture

- Status: Proposed
- Date: 2026-07-15
- Scope: provider-backed text, image, audio, and video generation used by workflows

## 1. Decision summary

Introduce a `Generation Tasks` bounded context with one vendor-neutral `GenerationTask` aggregate. A task is one durable intent to produce a primary text, image, audio, or video result. It is not a workflow run, provider request, download record, or generic background job.

The MVP uses one lifecycle and three durable tables: `generation_tasks`, `generation_task_outputs`, and `generation_task_outbox`. This is sufficient for a unified task list, idempotent creation, remote polling, cancellation, restart recovery, optimistic concurrency, and side effects after commit.

Automatic generation retries, attempt history, arbitrary provider options, archiving, cross-provider routing, and 3D are Future features. They must not complicate the MVP model before a concrete use case requires them.

## 2. Delivery scope

### 2.1 MVP

| Capability | MVP decision |
| --- | --- |
| Task model | One `GenerationTask` aggregate for all four media kinds. |
| Text | Text generation from text inputs. |
| Image | Text-to-image and image-to-image. |
| Audio | Text-to-speech. |
| Video | Text-to-video and image-to-video. |
| Providers | Runtime registry, deterministic mock, and at least one real adapter for each of the four modalities. |
| Lifecycle | `Queued`, `Submitting`, `Running`, `Finalizing`, `CancelRequested`, `Succeeded`, `Failed`, `Cancelled`. |
| Reliability | Idempotency key, canonical request hash, optimistic revision, transactional outbox, bounded delivery retry, restart recovery. |
| Persistence | Tasks, outputs, and outbox only. |
| API | Create, get, list, and cancel. No generic status update or PATCH. |
| Task list | Project-scoped cursor pagination with optional status and media-kind filters. |
| Assets | Image, audio, and video outputs must be durable assets before task success. |
| Workflow | Terminal task event resumes or fails the owning workflow node through an adapter. |

MVP is complete only when the mock-backed workflow E2E path works for all four request variants, each modality has a real adapter, and immediate/asynchronous provider shapes pass the shared contracts.

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
| 3D media | A 3D asset contract is designed; Meshy and Tripo3D remain references until then. |

Future features extend the same aggregate through explicit migrations. They do not create provider-specific task tables.

## 3. Goals and non-goals

### Goals

- Show text, image, audio, and video work in one stable task list.
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

`GenerationTask` owns one durable generation lifecycle. `TaskOrigin` identifies its project/workflow/node attempt. `GenerationRequest` is an immutable provider-neutral snapshot, `ProviderTarget` selects provider/model, `ProviderTaskHandle` is the opaque remote identity, `TaskOutput` is inline text or an asset reference, and `TaskSummary` is a rule-free list projection.

Delivery retry repeats the same outbox action and submission key after an uncertain call. Generation retry creates another provider submission after a failed generation and is Future only.

Use `GenerationTask`, not a generic `Job` or `Task`, in public code.

```text
ui
  -> src-tauri commands and DTO translators
       -> composition root
            -> crates/tasks (domain, application, consumer-owned ports)
            -> crates/backends (provider adapters -> task provider ports)
            -> crates/assets (authoritative assets)
            -> SQLite/clock/asset adapters (-> task ports)

crates/engine (pure workflow semantics)
  <- workflow/task integration adapter in the composition boundary
```

Recommended layout:

```text
crates/tasks/src/
  domain/{task, request, state, failure, event}.rs
  application/{commands, queries, execution, ports}.rs
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
| `id` | UUIDv7 `GenerationTaskId`. |
| `origin` | Required `project_id`, `workflow_id`, `workflow_run_id`, `node_id`, and `node_attempt`. |
| `idempotency` | Caller key plus canonical request hash. |
| `request` | Immutable `GenerationRequest`. |
| `target` | Immutable `ProviderId` and `ModelId`; no provider options JSON in MVP. |
| `state` | `TaskState`, the sole lifecycle semantic owner. |
| `outputs` | Ordered task outputs, empty until finalization. |
| `created_at`, `updated_at` | Values supplied by the task-owned clock port. |
| `revision` | Monotonic optimistic-lock version. |

The request hash covers schema version, origin, request, and target. It excludes timestamps and the idempotency key. Reusing `(project_id, idempotency_key)` with the same hash returns the existing task; a different hash returns `IdempotencyConflict`.

### 6.2 Requests and inputs

```rust
pub enum GenerationRequest {
    Text(TextGenerationSpec), Image(ImageGenerationSpec),
    Audio(AudioGenerationSpec), Video(VideoGenerationSpec),
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
| `TextGenerationSpec` | `prompt` | `system_instruction`, `max_output_tokens`, `temperature` |
| `ImageGenerationSpec` | `mode`, `prompt`, `inputs`, `count` | `negative_prompt`, `aspect_ratio`, `width`, `height`, `seed` |
| `AudioGenerationSpec` | `text` | `voice_id`, `sample_rate_hz` |
| `VideoGenerationSpec` | `mode`, `prompt`, `inputs` | `negative_prompt`, `duration_ms`, `aspect_ratio`, `resolution`, `include_audio`, `seed` |

MVP modes are closed enums: image is `TextToImage | ImageToImage`; audio is `TextToSpeech`; video is `TextToVideo | ImageToVideo`. Input asset snapshots include a content hash so retries/recovery cannot silently observe changed media.

### 6.3 Outputs

`TaskOutput` contains `id`, `ordinal`, `role`, and a tagged value: `Text { content }` or `Asset { asset_id, media_kind }`.

The asset domain remains the semantic owner of MIME type, dimensions, duration, checksum, and storage location. A task only references the durable asset.

## 7. MVP state machine

```rust
pub enum TaskState {
    Queued,
    Submitting,
    Running { handle: ProviderTaskHandle, progress_percent: Option<u8> },
    Finalizing { handle: Option<ProviderTaskHandle> },
    CancelRequested { handle: Option<ProviderTaskHandle> },
    Succeeded { completed_at: Timestamp },
    Failed { completed_at: Timestamp, failure: TaskFailure },
    Cancelled { completed_at: Timestamp },
}
```

| From | To | Cause |
| --- | --- | --- |
| `Queued` | `Submitting` | Worker claims `SubmitTask`. |
| `Submitting` | `Running` | Provider returns a remote handle. |
| `Submitting`/`Running` | `Finalizing` | Provider returns normalized outputs. |
| `Running` | `Running` | Normalized progress update. |
| `Queued` | `Cancelled` | Cancellation occurs before submission. |
| `Submitting`/`Running` | `CancelRequested` | Cancellation races with external work. |
| `CancelRequested` | `Cancelled` | Remote cancellation is attempted when available. |
| `Finalizing` | `Succeeded` | All outputs are durable and valid. |
| Any active state | `Failed` | Permanent failure or exhausted delivery attempts. |

Invariants:

- Request, origin, target, and idempotency data never change.
- Progress is optional `0..=100` and monotonic while running.
- `Succeeded` requires at least one output matching the request's primary media kind.
- Outputs can be attached only during `Finalizing` and are immutable after success.
- Terminal states never transition.
- Cancellation is rejected once `Finalizing` has committed.
- Optimistic revision serializes cancel/complete races: the first committed transition wins.
- Provider status strings and human-readable error text never drive transitions.

## 8. Failure semantics

`TaskFailure` contains `kind`, machine-readable `code`, safe `message`, and optional `provider_request_id`. Kinds are `InvalidRequest`, `Authentication`, `PermissionDenied`, `ContentPolicy`, `RateLimited`, `ProviderUnavailable`, `Timeout`, `ProviderRejected`, `InvalidProviderResponse`, `InputAssetUnavailable`, `OutputAssetImport`, and `Internal`.

A `ProviderFailure` is an explicit terminal result reported by a provider. A `ProviderCallError` means the current HTTP/transport/protocol call could not produce a trustworthy result. Keeping these types separate prevents a network timeout from being confused with a provider-declared failed generation.

A transient `ProviderCallError` reschedules the same outbox message with the submission key derived from `GenerationTaskId`. This is bounded delivery retry, not a new generation attempt. A non-transient call error or exhausted delivery budget transitions the task to `Failed`. A terminal `ProviderFailure` fails immediately in MVP.

## 9. Provider contracts

The runtime registry needs trait objects. Provider ports therefore use object-safe boxed futures rather than relying on `async fn` object safety:

```rust
pub type ProviderFuture<'a, T> =
    Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait GenerationSubmitter: Send + Sync {
    fn submit<'a>(&'a self, command: SubmitGeneration) ->
        ProviderFuture<'a, Result<SubmitOutcome, ProviderCallError>>;
}
pub trait GenerationPoller: Send + Sync {
    fn poll<'a>(&'a self, handle: &'a ProviderTaskHandle) ->
        ProviderFuture<'a, Result<PollOutcome, ProviderCallError>>;
}
pub trait GenerationCanceller: Send + Sync {
    fn cancel<'a>(&'a self, handle: &'a ProviderTaskHandle) ->
        ProviderFuture<'a, Result<CancelOutcome, ProviderCallError>>;
}
```

`SubmitOutcome` is `Accepted`, `Completed`, or `Rejected(ProviderFailure)`. `PollOutcome` is `Pending`, `Completed`, `Failed(ProviderFailure)`, or `Cancelled`. This is the only layer that sees vendor statuses.

An adapter that can return `Accepted` must be registered with a `GenerationPoller`. Remote cancellation is a separate capability; there is no `supports_cancel` method and no adapter implements an unsupported operation.

Provider adapters must validate third-party responses as untrusted, use the stable task-derived submission key where the provider supports idempotency, return normalized outputs/handles, and keep credentials, signed URLs, raw payloads, and vendor error bodies out of DTOs and logs.

DVStudio-inspired adapter examples are ByteDance Ark/Seedream for images, Google Gemini for text/images, and ByteDance Seedance for video. Audio uses the same contracts; the vendor is a composition decision.

## 10. Durable MVP execution

1. The boundary validates the DTO, resolves a registered provider/model, and verifies input asset snapshots.
2. `GenerationTask::create` validates invariants and emits `SubmitTask`.
3. One transaction inserts the task and outbox message.
4. The worker claims the message with a lease, commits `Submitting`, then calls the provider outside the transaction.
5. `Accepted` atomically saves `Running`, consumes `SubmitTask`, and enqueues `PollTask`.
6. `Pending` atomically saves progress, consumes the current poll, and enqueues the next delayed poll.
7. `Completed` first commits `Finalizing` without consuming the current message. Outputs are then imported through `TaskAssetSink`.
8. Success atomically stores outputs, saves `Succeeded`, consumes the work message, and enqueues `NotifyWorkflow`.
9. Terminal failure atomically saves `Failed`, consumes the work message, and enqueues `NotifyWorkflow`.
10. A transient call error reschedules the same message. Expired leases are reclaimable.

If the process crashes during finalization, the unconsumed message is reclaimed. Async providers are polled again; immediate providers are submitted again with the same submission key. Providers without native idempotency must document the unavoidable duplicate-submission risk.

Queued cancellation commits `Cancelled`; any stale submit becomes a no-op. Submitting/running cancellation commits `CancelRequested` and enqueues `CancelRemoteTask`. Provider cancellation is best effort, but the local task deterministically reaches `Cancelled`; provider charges may still occur.

## 11. Consumer-owned persistence ports

```rust
pub struct OutboxChanges {
    pub consume: Option<OutboxId>,
    pub enqueue: Vec<OutboxMessage>,
}

pub trait GenerationTaskRepository {
    async fn create(&self, task: &GenerationTask, message: OutboxMessage)
        -> Result<CreateResult, TaskRepositoryError>;
    async fn get(&self, id: GenerationTaskId)
        -> Result<Option<GenerationTask>, TaskRepositoryError>;
    async fn save(&self, task: &GenerationTask, expected_revision: u64,
        outbox: OutboxChanges) -> Result<(), TaskRepositoryError>;
    async fn list(&self, query: TaskListQuery)
        -> Result<CursorPage<TaskSummary>, TaskRepositoryError>;
}
```

`create` and `save` atomically persist aggregate state, outputs, consumed outbox work, and new outbox work. `TaskOutboxReader` only claims due work and reschedules transient delivery errors. The in-memory fake and SQLite adapter run the same idempotency, concurrency, transaction, ordering, and pagination contract tests.

Other focused ports are `TaskAssetSink` and `TaskClock`. Repository services use generic trait bounds; only the dynamic Provider registry requires boxed object-safe futures.

## 12. MVP persistence schema

### `generation_tasks`

| Fields | SQLite type | Purpose |
| --- | --- | --- |
| `id` PK | `TEXT` | UUIDv7 task identity. |
| Origin IDs; `node_attempt` | `TEXT`; `INTEGER` | Required workflow origin. |
| `idempotency_key`, `request_hash` | `TEXT` | Unique `(project_id, idempotency_key)`. |
| `request_schema_version`, `request_kind`, `request_json` | `INTEGER`, `TEXT`, `TEXT` | Immutable domain request snapshot. |
| `provider_id`, `model_id` | `TEXT` | Immutable target; no credentials/options JSON. |
| `status` | `TEXT` with `CHECK` | Normalized task status. |
| `progress_percent` | nullable `INTEGER` | Enforce `0..=100`. |
| `remote_task_id` | nullable `TEXT` | Opaque polling/cancellation handle. |
| Failure fields | nullable `TEXT` | Kind, code, safe message, provider request ID. |
| `cancel_requested_at`, `started_at`, `completed_at` | nullable `INTEGER` | UTC epoch milliseconds. |
| `created_at`, `updated_at`, `revision` | `INTEGER` | Audit, ordering, optimistic lock. |

### `generation_task_outputs`

Fields: `id` PK, `task_id` FK, `ordinal`, `role`, `media_kind`, `text_content` nullable, `asset_id` nullable, and `created_at`. Enforce unique `(task_id, ordinal)`, exactly one of text/asset, and `media_kind = text` for inline text.

### `generation_task_outbox`

Fields: `id` PK, `task_id` FK, `kind`, `payload_json`, `deduplication_key`, `available_at`, `delivery_attempts`, `locked_by`, `locked_until`, `processed_at`, `last_error`, and `created_at`. MVP kinds are `SubmitTask`, `PollTask`, `CancelRemoteTask`, and `NotifyWorkflow`.

Outbox payloads contain task/message identifiers only. Workers load the aggregate from the repository; prompts, asset URLs, Provider requests, and signed output URLs are never copied into outbox rows.

Indexes: task list `(project_id, updated_at DESC, id DESC)`, workflow lookup `(workflow_run_id, node_id)`, remote lookup `(provider_id, remote_task_id)`, and due outbox work `(processed_at, available_at, id)`.

Rows and DTOs use named translators. Invalid row combinations return corruption errors and never bypass aggregate constructors.

## 13. MVP task-list and command contracts

`TaskSummaryDto` contains `id`, origin IDs, `requestKind`, `status`, `progressPercent`, `providerId`, `modelId`, `promptPreview`, `previewAssetId`, `outputCount`, structured failure summary, and lifecycle timestamps.

`ListGenerationTasksInput` requires `projectId` and accepts `status`, `requestKind`, `cursor`, and `limit` (`1..=100`). Ordering is always `(updated_at DESC, id DESC)`. The opaque seek cursor contains the last ordering pair.

MVP Tauri commands:

- `create_generation_task(input) -> GenerationTaskDto`
- `get_generation_task(task_id) -> GenerationTaskDto`
- `list_generation_tasks(input) -> CursorPage<TaskSummaryDto>`
- `cancel_generation_task(task_id) -> GenerationTaskDto`

There is no public status setter, delete, retry, archive, or generic update command. DTOs are separate tagged unions with named domain translators. Stable command error codes include `INVALID_ARGUMENT`, `NOT_FOUND`, `CONFLICT`, `IDEMPOTENCY_CONFLICT`, `ILLEGAL_TRANSITION`, `PROVIDER_NOT_CONFIGURED`, and `STORAGE_FAILURE`. Provider execution failures normally become task state, not command transport errors.

## 14. Security and observability

MVP requirements:

- Provider credentials are resolved only inside adapters and never persisted in tasks/outbox.
- Do not log prompts, input content, signed URLs, response bodies, or API keys.
- Remote outputs enforce scheme/host policy, redirect/time/byte limits, MIME sniffing, and checksum validation.
- Raw provider requests/responses are not persisted.
- Structured tracing includes task ID, workflow run/node ID, provider/model, normalized status, latency, and failure code.
- Minimum counters cover queued, running, succeeded, failed, cancelled, delivery retry, and expired lease counts.

Future observability may add cost metrics, provider dashboards, long-term event analytics, and configurable retention.

## 15. MVP verification

1. Domain tests cover every legal/illegal transition, terminal immutability, progress monotonicity, output-kind validation, and cancel/complete races.
2. Request tests cover all four variants, canonical hashing, invalid modes, and asset snapshot mismatch.
3. Repository contract tests cover atomic outbox consume/enqueue, optimistic conflicts, idempotency, cursor ordering, and expired lease recovery.
4. Provider contract tests cover immediate, async, failure, untrusted response, idempotency, polling, and registered cancellation behavior.
5. Application tests cover immediate success, async success, terminal failure, bounded delivery retry, crash after submit, crash during asset import, duplicate create, and cancellation races.
6. Backend E2E covers workflow node -> task -> mock provider -> asset -> workflow outcome for text, image, audio, and video.
7. Tauri DTO fixtures and frontend contract tests are updated with command/DTO changes.

## 16. Implementation sequence

### MVP

1. Add task domain types, state transitions, failures, events, and unit tests.
2. Add application ports/use cases plus in-memory repository and mock provider.
3. Add the three SQLite tables, translators, outbox leasing, and repository contract tests.
4. Add worker/composition wiring and crash-recovery tests.
5. Add the four Tauri commands, task-list projection, DTO fixtures, and frontend API tests.
6. Add real adapters incrementally; every adapter must pass the shared contracts.
7. Run formatting, clippy, Rust tests, and the full E2E gate.

### Future delivery order

1. Add user Retry plus `retry_of_task_id` if workflow rerun is insufficient.
2. Add `generation_task_attempts` and `RetryWaiting` only with automatic generation retry.
3. Add archive/retention when task volume requires it.
4. Add advanced modes and typed fields before considering versioned provider options.
5. Add routing, billing, webhooks, distributed workers, or 3D only behind separate design decisions.

## 17. Architectural consequences

- MVP has one aggregate, one lifecycle, four media request variants, three tables, and four commands.
- Reliability comes from idempotency, optimistic revision, atomic outbox transitions, and content-addressed assets rather than an early general job framework.
- Adding a provider changes an adapter and composition registration, not task semantics or storage.
- Adding a new media kind or mode is intentionally a domain/API/schema change.
- Attempt history and generation retry remain clean extensions instead of mandatory MVP concepts.
- Local cancellation is deterministic; remote cancellation and provider charges remain best effort.

Do not generalize this bounded context into a platform-wide job system until another business capability demonstrates the same semantics and contract requirements.
