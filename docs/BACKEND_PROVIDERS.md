# Backend MVP Provider Adapters

> Status: proposed MVP design
> Owner: `crates/backends`
> Scope: text-to-image, image-to-video, and text-to-audio infrastructure

Naming follows [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md). Providers are infrastructure adapters,
not a DDD bounded context and not owners of node semantics.

## Responsibility

Provider adapters translate three node-owned generation contracts into external protocols. They own
authentication, private wire DTOs, transport, submission, polling, bounded retry, remote cancellation
attempts, artifact download, and provider error mapping.

They do not own Workflow nodes, capability parameters, Run state, Asset identity, preview URLs, or
adapter selection.

## Dependency Direction

```text
crates/nodes ports
  TextToImageProviderPort       <- concrete and deterministic adapters
  ImageToVideoProviderPort      <- concrete and deterministic adapters
  TextToAudioProviderPort       <- concrete and deterministic adapters
```

Each trait is owned by the exact node capability that consumes it. An adapter implements only a
complete trait it supports. Business code has no broad `Provider`, optional operation, capability
probe, unsupported branch, downcast, or implementation-name switch.

## Adapter Structure

```text
crates/backends/src/
  mock/
    text_to_image.rs
    image_to_video.rs
    text_to_audio.rs
  <provider>/
    transport.rs
    polling.rs
    artifact_download.rs
    text_to_image.rs
    image_to_video.rs
    text_to_audio.rs
    dto.rs
    error.rs
  lib.rs
```

Provider DTOs remain private to their adapter. Their names include provider and protocol role, such
as `FalTextToImageRequestDto` and `FalTaskStatusResponseDto`. Shared transport helpers may remove
mechanical duplication but cannot define creative parameters or capability results.

## Consumer-Owned Requests

```rust
pub struct TextToImageProviderRequest {
    pub prompt: WorkflowTextValue,
    pub aspect_ratio: NodeCapabilityImageAspectRatioValue,
    pub seed: Option<NodeCapabilityImageGenerationSeedValue>,
    pub dispatch_id: WorkflowNodeDispatchId,
}

pub struct ImageToVideoProviderRequest {
    pub source_image: NodeCapabilityReadableMediaInput,
    pub prompt: Option<WorkflowTextValue>,
    pub duration: NodeCapabilityVideoDurationValue,
    pub aspect_ratio: NodeCapabilityVideoAspectRatioValue,
    pub dispatch_id: WorkflowNodeDispatchId,
}

pub struct TextToAudioProviderRequest {
    pub text: WorkflowTextValue,
    pub voice_profile: NodeCapabilityAudioVoiceProfileValue,
    pub speed: NodeCapabilityAudioSpeechSpeedValue,
    pub dispatch_id: WorkflowNodeDispatchId,
}
```

These semantic request types are owned in `crates/nodes`. They contain no Workflow aggregate, UI
node, Asset repository, SQLite row, path, preview URL, provider model ID, or provider options map.
Startup configuration selects one adapter and model per capability.

## Consumer-Owned Result

All three ports return the capability-owned `NodeCapabilityGeneratedMediaPayload` defined in
[`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md#generated-media-payload).

The stream is bounded and asynchronous. When a provider returns a remote URL, its adapter validates
the URL, downloads within configured limits, verifies the declared media kind, and returns a stream.
No remote URL or local path crosses the provider port.

The adapter never creates an Asset. The node executor passes the payload to
`NodeCapabilityGeneratedMediaWriterPort` and publishes output only after storage succeeds.

## Constructor Injection

A concrete adapter receives long-lived dependencies in its constructor:

```text
validated provider config and redacted credential
HTTP transport
clock and bounded polling sleeper
artifact download policy
```

Execution receives only the semantic request plus call-scoped deadline, cancellation, and progress
sink required by the port. An adapter never reads global configuration or locates another adapter at
runtime. `src-tauri/composition.rs` is the only concrete construction point.

## Execution Semantics

Every adapter preserves the same port behavior:

- at most one submission for one dispatch identity while the process is active;
- monotonic progress in `[0.0, 1.0]`;
- bounded poll interval, attempts, and absolute deadline;
- exactly one terminal success, failure, cancellation, or timeout;
- complete bounded artifact download before success;
- one normalized structured error contract;
- no open database transaction during network work.

Behavioral equivalence is verified by running one parameterized port contract suite against the mock
and every configured implementation.

## Submission Identity

The application persists queued Run intent before provider dispatch. The node capability supplies a
stable `WorkflowNodeDispatchId`. An adapter forwards it to provider idempotency support when
available and prevents duplicate submission in the active process.

If submission receives an ambiguous response and the provider offers no idempotency lookup, the
adapter fails rather than blindly resubmitting paid work. Cross-process remote-task resume is not an
MVP requirement; an interrupted Run fails and the user starts a new Run.

## Polling And Progress

Provider-native statuses map privately to waiting, running, success, or failure. Unknown status is
`InvalidProviderOutput`, never an infinite poll. A provider retry hint may delay the next bounded
poll but cannot extend the absolute deadline. Native progress is clamped and made monotonic.

Provider status strings and human-readable messages never determine Workflow state.

## Cancellation

Cancellation is checked before submission, between polls, before download, and before returning
success. When supported, the adapter requests remote cancellation idempotently. Otherwise it stops
local waiting and discards a late result without claiming remote billed work stopped.

## Retry

Transport retry is allowed only when safe under the same dispatch identity, such as polling and
bounded artifact reads. Authentication failure, invalid request, invalid output, and explicit
provider failure are not retried. User retry creates a new Workflow Run; the adapter does not own
that policy.

## Error Translation

Each adapter maps its private `*ProviderAdapterError` once into
`NodeCapabilityProviderError`. Stable categories are:

```text
AuthenticationRequired
RateLimited
InvalidProviderRequest
ProviderUnavailable
InvalidProviderOutput
ArtifactDownloadFailed
TimedOut
Cancelled
ProviderExecutionFailed
```

Retryability and optional safe retry time are structured. Secrets, raw response bodies, signed
URLs, and provider task IDs do not enter business errors or ordinary logs.

## Security And Bounds

- credentials use redacted values and never enter DTOs;
- endpoints are validated at startup and cannot be overridden by node parameters;
- prompts, text, duration, request/response size, timeouts, polls, redirects, and artifacts are
  bounded;
- redirect targets are revalidated;
- downloaded bytes are sniffed and checked against the expected media kind;
- logs contain stable capability, Workflow Run, node, duration, and terminal category only.

## Deterministic Adapters

`MockTextToImageProviderAdapter`, `MockImageToVideoProviderAdapter`, and
`MockTextToAudioProviderAdapter` return deterministic viewable or playable media. They expose stable
progress, cancellation points, and injectable structured failures.

Production capability code has no mock branch. The deterministic adapters are wired through the
same ports used by configured providers.

## Composition

`src-tauri/composition.rs` constructs one implementation for each generation port. A node capability
is reported runnable only when its required adapter and configuration are complete. Business code
receives `*ProviderPort`, never a concrete provider type.

## Verification

- one port contract suite runs against deterministic and configured adapters;
- mapping tests cover every semantic request field and bound;
- response tests cover terminal states, malformed DTOs, and wrong media;
- polling tests cover progress, timeout, unknown state, and bounded attempts;
- cancellation tests cover pre-submit, poll, download, and late-result races;
- submission tests prove one active submission per dispatch identity;
- transport tests cover auth, limits, redirects, retry, and redaction;
- credentialed tests remain behind a separate explicit gate.

## Post-MVP

Per-node provider selection, multiple profiles, remote task recovery, cost accounting, fallback
routing, multiview, reference generation, text generation, text-to-video, concat, and batch output
are deferred. 3D and scene providers are not product scope.
