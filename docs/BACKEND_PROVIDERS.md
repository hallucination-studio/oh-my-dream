# Backend Generation Profiles And Providers

> Status: frozen MVP provider architecture
> Owner: profile semantics in `crates/nodes`; routes in `crates/backends`; wiring in `src-tauri`
> Scope: provider-independent model selection, availability, routing, and translation

Users select a stable Generation Profile. Provider accounts, native models, endpoints, and routing
remain replaceable infrastructure.

## Decision

1. Every model-powered node persists one provider-independent `GenerationProfileRef`.
2. `GenerationProfileDefinition` owns identity, lifecycle, and exact capability compatibility.
3. Each active exact capability owns one focused provider interface.
4. One capability-specific router implements that interface and resolves the profile to its single
   configured route.
5. One concrete vendor route owns native model mapping, request translation, submission, polling,
   download, response validation, and provider error translation.
6. Only `DesktopCompositionRoot` constructs routes and routers.

The complete runtime stack is intentionally short:

```text
ImageToVideoCapabilityImpl
  -> ImageToVideoProviderInterface
  -> ImageToVideoProviderRouterImpl
  -> FalImageToVideoProviderRouteImpl
  -> provider API
```

There is no provider-wide feature interface, binding object, binding registry, generation task,
provider executor, or node-level provider/model field.

## Source-And-Behavior Names

| Role | Required pattern | Example |
| --- | --- | --- |
| public semantic interface | `<Input>To<Output>ProviderInterface` | `ImageToVideoProviderInterface` |
| semantic request | `<Input>To<Output>ProviderRequest` | `ImageToVideoProviderRequest` |
| profile router | `<Input>To<Output>ProviderRouterImpl` | `ImageToVideoProviderRouterImpl` |
| private route interface | `<Input>To<Output>ProviderRouteInterface` | `ImageToVideoProviderRouteInterface` |
| concrete vendor route | `<Vendor><Input>To<Output>ProviderRouteImpl` | `FalImageToVideoProviderRouteImpl` |
| deterministic route | `Deterministic<Input>To<Output>ProviderRouteImpl` | `DeterministicImageToVideoProviderRouteImpl` |

Methods state the complete behavior: `generate_image_from_text`, `generate_video_from_image`, and
`synthesize_speech_from_text`. Standalone `Provider`, `Client`, `Model`, `Route`, `Binding`,
`Executor`, `Task`, and `Registry` names are prohibited.

Provider-shared infrastructure uses the `GenerationProvider` prefix, such as
`GenerationProviderAccountId`, `GenerationProviderRouteId`, and
`GenerationProviderCredentialRepositoryInterface`. It cannot be confused with Assistant model
configuration.

## Dependency Direction

```text
crates/engine
  WorkflowNodeCapabilityInterface and WorkflowNodeExecutionId
             ^
crates/nodes
  exact capabilities, Generation Profile catalog, exact provider interfaces
             ^
crates/backends
  provider routers, vendor routes, private protocol DTOs
             ^
src-tauri
  SQLite configuration/plaintext credential repositories, construction
```

Recommended adapter layout:

```text
crates/backends/src/
  provider_routing/<operation>_router.rs
  deterministic_provider/<operation>_route.rs
  <vendor>/
    shared/{authentication,http,polling,download,error_translation}.rs
    <operation>/{route,request_dto,response_dto}.rs
```

Vendor DTOs, status values, native model IDs, signed URLs, and remote task handles remain private to
their concrete route.

## Stable Generation Profile

```rust
pub struct GenerationProfileRef {
    pub id: GenerationProfileId,
    pub version: GenerationProfileVersion,
}

pub struct GenerationProfileDefinition {
    pub profile_ref: GenerationProfileRef,
    pub display_name: GenerationProfileDisplayName,
    pub lifecycle_state: GenerationProfileLifecycleState,
    pub compatible_capabilities: BTreeSet<NodeCapabilityContractRef>,
}
```

`GenerationProfileId` is 3..=128 lowercase ASCII bytes with two or more dot-separated segments;
each segment matches `[a-z][a-z0-9_]*`. `GenerationProfileVersion` is a non-zero `u32`, and the
canonical ref is `<id>@<version>`. `GenerationProfileDisplayName` is trimmed, contains 1..=80
Unicode scalar values, and contains no control character. Lifecycle is the closed union `Active |
Retired`.

All fields are private with noun-specific accessors. `GenerationProfileDefinition::try_new`
requires a non-empty compatible-capability set and otherwise returns `InvalidDefinition`; it does
not validate registration or availability. Identity and compatibility are immutable after
construction. `find_generation_profile` returns `ProfileNotFound` rather than an optional or
fallback definition when the exact ref is absent.

The frozen MVP catalog contains exactly these definitions:

| Profile ref | Display name | Compatible capability |
| --- | --- | --- |
| `image.high_quality_general@1` | High Quality Image | `image.generate_from_text@1.0` |
| `video.cinematic_image_animation@1` | Cinematic Image Animation | `video.generate_from_image@1.0` |
| `speech.multilingual_narration@1` | Multilingual Narration | `audio.synthesize_speech_from_text@1.0` |

The capability refs above must belong to the exact set in `BACKEND.md#active-node-capabilities`;
display text is not identity. Adding a profile, compatibility, or lifecycle state changes this
frozen catalog and is not configuration.

A profile is an immutable product promise, not an alias for today's native model.

For `video.cinematic_image_animation@1`, an absent semantic prompt means the profile-owned exact
default `"Animate the source image with coherent natural motion."`. This is part of profile version
1 behavior, not vendor configuration; changing it requires a new profile version.

- compatibility names exact capability versions;
- a route must satisfy the complete capability contract;
- a changed observable semantic requires a new profile or capability version;
- every route bound to the profile must preserve the same observable behavior;
- retired profiles remain as tombstones for saved Workflows;
- display names and native model strings are never parsed as identity.

Every active model-powered capability requires `generation_profile_ref` in its normalized parameter
contract. A Workflow node stores no provider, native model, account, endpoint, credential, route,
availability snapshot, or provider task.

`GenerationProfileCatalog` is one concrete immutable collection owned by `crates/nodes`. Its
`frozen_mvp` constructor creates exactly the three definitions above and no caller-supplied
definitions. `find_generation_profile` resolves Active and Retired definitions by exact ref so a
saved Workflow can explain a tombstone. `list_active_generation_profiles_for_capability` returns
only Active compatible definitions in ascending `GenerationProfileRef` order. A capability with no
compatible profile returns an empty list. The catalog has no trait, mutation, configuration,
provider route, native model, availability cache, default selection, fallback, or version
negotiation.

`GenerationProfileRef::try_from_node_capability_parameter_value` and
`GenerationProfileRef::to_node_capability_parameter_value` are the only semantic-owner conversion
methods for `NodeCapabilityGenerationProfileRefParameterValue`. Conversion validates the same ID
grammar and non-zero version; it does not consult lifecycle, compatibility, availability, or a
provider. Invalid identity bytes return `GenerationProfileError::InvalidProfileRef`.

## Compatibility And Availability

Compatibility is immutable catalog data. Availability is an expiring operational observation:

```rust
pub enum GenerationProfileAvailabilityState {
    Available,
    Unavailable {
        reason: GenerationProfileUnavailableReason,
        retry_after: Option<GenerationProfileRetryAfter>,
    },
    Indeterminate {
        reason: GenerationProfileAvailabilityIndeterminateReason,
    },
}
```

Unavailable reasons include `NoConfiguredRoute`, `AuthenticationRequired`, `PolicyBlocked`,
`QuotaUnavailable`, `RateLimited`, `ProviderUnavailable`, and `NativeModelUnavailable`. Probe
timeout, offline state, and an untrustworthy response are `Indeterminate` rather than a false claim.

`GenerationProfileAvailabilityObservation` contains the requested profile ref, state,
`observed_at_epoch_ms`, and `expires_at_epoch_ms`. Both times are non-negative; expiry is later than
observation and no more than 30 seconds later. A bulk request contains one exact capability ref and
1..=100 unique compatible profile refs and has a five-second deadline. It returns exactly one
observation per requested ref in request order. `retry_after_epoch_ms`, when present, is later than
observation. Indeterminate reasons are exactly `ProbeTimedOut`, `NetworkOffline`, and
`UntrustedResponse`.

`GenerationProfileAvailabilityReaderInterface` is consumer-owned by the profile application module.
`ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl` performs one bounded bulk
observation for one exact capability and profile set. It reads the same three router
implementations' profile-to-route maps and never maintains another mapping. It does not probe once
per UI row or persist availability.

`GenerationProfileListForCapabilityUseCase` joins definitions with current observations and returns
only provider-independent metadata. In the MVP, both `Unavailable` and `Indeterminate` prevent Run
admission. The router checks again at execution because availability can change after admission.
No route may silently substitute a different profile.

The C1 values and application contracts are exact:

- `GenerationProfileError` is the closed union `InvalidProfileRef`, `InvalidDisplayName`,
  `InvalidDefinition`, `CapabilityNotFound`, `ProfileNotFound`, `ProfileIncompatible`,
  `InvalidAvailabilityObservation`, `AvailabilityRequestInvalid`, `AvailabilityReadFailed`, and
  `DeadlineExceeded`. It contains no provider text or generic validation message.
- `GenerationProfileAvailabilityRequest` contains one exact capability ref, `1..=100` unique
  compatible profile refs in ascending order, and one process-monotonic deadline later than the
  construction instant and at most five seconds after it. Construction rejects an empty, duplicate,
  unsorted, expired, or over-five-second request as `AvailabilityRequestInvalid`. Compatibility is already
  guaranteed by the catalog-derived list and is not reimplemented by this request value.
- `GenerationProfileAvailabilityReaderInterface::read_generation_profile_availability` returns a
  vector in the same order with exactly one observation for every requested ref. Technical reader
  failure is `AvailabilityReadFailed`; elapsed deadline is `DeadlineExceeded`. Missing, duplicate,
  reordered, mismatched, or invalid observations are rejected by the use case as
  `InvalidAvailabilityObservation`.
- `NodeCapabilityListUseCase` contains only a shared `WorkflowNodeCapabilityRegistry` and
  `list_node_capabilities` returns its exact ascending borrowed contracts as owned contract values.
  It performs no profile join, filtering, projection, provider read, registration, or fallback.
- `GenerationProfileListForCapabilityQuery` contains one exact `NodeCapabilityContractRef` and the
  caller's process-monotonic deadline. `GenerationProfileListForCapabilityUseCase` first requires
  that exact capability to be registered, returning `CapabilityNotFound` when it is not. It obtains the
  catalog's ascending Active compatible definitions. When empty, it returns an empty result without
  calling the availability reader. Otherwise it performs exactly one bulk read and returns
  `GenerationProfileForCapabilityListItem { definition, availability }` values in profile-ref order.
  A Retired compatible definition remains resolvable from the catalog but never appears in this
  selectable list.

`GenerationProfileForCapabilityListItem` contains only the complete provider-independent
definition and its matching current observation. The list result has no selected/default flag,
provider, native model, route, credential, price, pagination, refresh token, stale cache, or UI
metadata. The use case does not persist observations or substitute a profile.

## Frozen MVP Provider Interfaces

[`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md#mvp-external-interfaces) owns public interface,
request, and result names. This section freezes their field semantics. Provider infrastructure
implements exactly its three MVP interfaces: `TextToImageProviderInterface`,
`ImageToVideoProviderInterface`, and `TextToSpeechProviderInterface`.

Their requests carry `GenerationProfileRef` and `WorkflowNodeExecutionContext`; the context contains
`WorkflowNodeExecutionId`, one deadline, and cancellation, but no credential or provider value. The
execution ID becomes the native submission idempotency key where supported. Exact semantic fields
and results are:

| Interface method | Request after profile/context | Success result |
| --- | --- | --- |
| `generate_image_from_text` | `prompt: WorkflowTextValue`, `aspect_ratio: ImageAspectRatio` | one `GeneratedImagePayload` |
| `generate_video_from_image` | readable Image, optional `prompt: WorkflowTextValue`, `duration_seconds: 5 | 10` | one `GeneratedVideoPayload` |
| `synthesize_speech_from_text` | `text: WorkflowTextValue` | one `SynthesizedSpeechPayload` |

Each payload has its fixed media kind, MIME, byte length, SHA-256 digest, declared media facts, and
one bounded asynchronous byte stream. Frozen profile outputs are respectively `image/png` up to 32
MiB, `video/mp4` up to 512 MiB, and `audio/mpeg` up to 64 MiB. Zero bytes, a second output, unknown
length, mismatched facts, trailing bytes, or digest mismatch is `InvalidResponse`. Roadmap interface
names are reserved by the capability document but do not exist in the MVP runtime.

## Router And Private Route

The public router resolves one stable profile and delegates to one private exact route:

```rust
pub struct ImageToVideoProviderRouterImpl {
    routes_by_profile:
        BTreeMap<GenerationProfileRef, Arc<dyn ImageToVideoProviderRouteInterface>>,
}

#[async_trait]
trait ImageToVideoProviderRouteInterface: Send + Sync {
    fn generation_provider_route_id(&self) -> GenerationProviderRouteId;
    async fn observe_provider_route_availability(
        &self,
    ) -> GenerationProviderRouteAvailability;

    async fn generate_video_from_image(
        &self,
        request: ImageToVideoProviderRouteRequest,
    ) -> Result<GeneratedVideoPayload, NodeCapabilityProviderFailure>;
}
```

The router removes `GenerationProfileRef` only after resolving its exact configured route. The
routed request retains every other semantic field. One route maps exactly one profile semantic
contract and cannot branch on another profile or return `Unsupported`.

```rust
struct FalImageToVideoProviderRouteImpl {
    route_id: GenerationProviderRouteId,
    native_model_id: FalImageToVideoModelId,
    account: FalGenerationProviderAccount,
    transport: FalHttpTransport,
}
```

The frozen composition map is exact. Endpoint IDs are product configuration constants, not
user-editable values:

| Profile ref | Production route implementation | Native operation |
| --- | --- | --- |
| `image.high_quality_general@1` | `FalTextToImageProviderRouteImpl` | `fal-ai/flux-pro/kontext/text-to-image` |
| `video.cinematic_image_animation@1` | `FalImageToVideoProviderRouteImpl` | `fal-ai/kling-video/v3/standard/image-to-video` |
| `speech.multilingual_narration@1` | `ElevenLabsTextToSpeechProviderRouteImpl` | `POST /v1/text-to-speech/{voice_id}?output_format=mp3_44100_128`, model `eleven_multilingual_v2` |

Native endpoint and model identifiers are private typed constants owned by each concrete route. Tests replace each production
route with its matching `Deterministic<Input>To<Output>ProviderRouteImpl`; they do not add catalog
profiles or routes.

### Frozen Native Wires

Both fal routes use the authenticated fal queue protocol for their exact endpoint ID: submit one
JSON request, poll only the returned request ID until `COMPLETED` or a terminal failure, then fetch
that request's result. `IN_QUEUE` and `IN_PROGRESS` are the only non-terminal states. The
`Authorization: Key <secret>` header is attached by the transport. The request ID and returned media
URL remain private route values and are never persisted. The image-to-video route encodes the exact
readable input bytes as a `data:<verified MIME>;base64,<canonical base64>` URL accepted by the fal
file-input contract; it performs no public upload and creates no separately recoverable remote file.

For endpoint ID `<endpoint>`, the native HTTP operations are exactly
`POST https://queue.fal.run/<endpoint>`,
`GET https://queue.fal.run/<endpoint>/requests/<request_id>/status`, and
`GET https://queue.fal.run/<endpoint>/requests/<request_id>`. Submission sends the route request as
the JSON body and requires one non-empty `request_id`; status and result requests have no body.
Redirects are rejected for queue operations. Non-success HTTP status or an unknown/malformed queue
state is translated once through the frozen provider-failure categories, never retried as a new
submission.

The text-to-image request is exactly `prompt`, the semantic `aspect_ratio`, `num_images: 1`,
`output_format: "png"`, `safety_tolerance: "2"`, and `enhance_prompt: false`. It omits `seed` and
`guidance_scale` because neither is part of profile semantics. The result must contain exactly one
image with `content_type: "image/png"`, a non-empty HTTPS URL, and no positive
`has_nsfw_concepts` entry. Timing, seed, and echoed prompt are observational and discarded.

The image-to-video request is exactly `start_image_url`, optional non-empty `prompt`, semantic
`duration` encoded as `"5"` or `"10"`, and `generate_audio: false`. An absent semantic prompt sends
the profile-owned version-1 default above. It never sends `multi_prompt`,
elements, voices, or an end image. The result must contain exactly one `video` with
`content_type: "video/mp4"`, a non-empty HTTPS URL, and a positive bounded file size when supplied.
The profile promises silent image animation; adding native audio requires a new profile version.

The ElevenLabs route sends `xi-api-key`, `Content-Type: application/json`, and body
`{ "text": <semantic text>, "model_id": "eleven_multilingual_v2" }` to exactly
`POST https://api.elevenlabs.io/v1/text-to-speech/<voice_id>?output_format=mp3_44100_128`.
`voice_id` is one required,
validated `ElevenLabsVoiceId` in non-secret route configuration; it is not a node parameter or
model-selected value. A successful response is raw `audio/mpeg` bytes bounded by the route download
limit. Redirects, JSON success bodies, empty bodies, and other content types are invalid responses.
The route sends no voice-setting override, pronunciation dictionary, continuity request ID, or
latency option.

These wires are supported by the vendor contracts published at
<https://fal.ai/models/fal-ai/flux-pro/kontext/text-to-image/api>,
<https://fal.ai/models/fal-ai/kling-video/v3/standard/image-to-video/api>, and
<https://elevenlabs.io/docs/api-reference/text-to-speech/convert/>. A vendor wire change that cannot
preserve these exact semantics makes the route unavailable until a reviewed profile/route update;
it never silently changes the profile promise.

Router construction rejects unknown or incompatible profiles, duplicate profile mappings,
duplicate route IDs, and incomplete credentials. A missing mapping is represented by
`NoConfiguredRoute`, not a placeholder implementation. Contract conformance is proved in tests,
not represented as runtime configuration.

## Dispatch Rules

```text
saved GenerationProfileRef
  -> compatibility and availability
  -> durable Workflow Run and WorkflowNodeExecutionId
  -> router resolves the one configured route
  -> route translates, submits, polls, and validates
  -> capability stores media through the Asset boundary
  -> Workflow commits node output and state
```

The selected `GenerationProviderRouteId` is fixed inside one active node execution before paid
submission. The MVP never switches routes during that execution. An ambiguous submission fails
with a structured category unless the same route and vendor idempotency key can safely recover it.
Multi-route selection and automatic failover are post-MVP concerns.

MVP provider polling and remote handles are process-local. No remote task ID enters Workflow,
Asset, Assistant, SQLite, DTOs, or ordinary logs. On process restart, the owning Run becomes
`InterruptedByRestart`; the user starts a new Run.

Cancellation is supplied through `WorkflowNodeExecutionContext`. A route stops local polling and
requests remote cancellation when its exact vendor operation supports it. Lack of remote
cancellation never becomes an optional public interface method; the public observable result is still a
cancelled local execution, while external work or charges may continue.

## Route Responsibilities

Each concrete route:

- maps every semantic field to one private vendor DTO;
- uses a typed native model ID rather than parsing profile/display names;
- sends explicit values when native defaults could change promised behavior;
- validates the frozen response shape, media kind, size, and MIME, plus reported model or checksum
  only when that exact vendor response contract supplies the field;
- bounds submission, polling, redirects, downloads, deadlines, and response sizes;
- maps provider status exactly once into `NodeCapabilityProviderFailure`;
- keeps credentials, raw bodies, signed URLs, remote handles, and route details private.

Submission/status response bodies are at most 1 MiB. Text-to-image, image-to-video, and
text-to-speech deadlines are respectively 180, 900, and 120 seconds. Poll delay stays within
500..=5,000 milliseconds and attempt count is derived from the operation deadline. Downloads allow
at most three redirects, require HTTPS and an allowlisted host, and reject loopback, private,
link-local, and changed resolved addresses. A route never resubmits after an ambiguous acceptance;
it may only recover the same submission with the same route and idempotency key.

Remote media is downloaded and validated inside the route. The route returns a semantic payload but
never creates an Asset; the capability owns the call to the Asset-write boundary.

## Credentials And Configuration

Non-secret MVP configuration declares only enabled accounts, exact profile-to-shipped-route
selection, `GenerationProviderCredentialId` references, and the required `ElevenLabsVoiceId`.
Endpoint IDs/hosts, operation deadlines, polling bounds, response limits, and native model IDs are
constants owned by each shipped concrete route; configuration cannot override them or create a new
route/profile mapping.

`GenerationProviderCredentialRepositoryInterface` is owned by the Desktop
provider-configuration consumer. Its production adapter stores the credential as plaintext in the
dedicated generation-provider credential table in `metadata.sqlite`. The MVP provides no
encryption at rest: an actor able to read that database can read the credential. The value is
materialized as one short-lived `GenerationProviderCredentialSecret` for authenticated calls and
never enters the config payload, public DTOs, domain objects, errors, or logs.

Only `DesktopCompositionRoot` supplies the focused credential repository and constructs routes;
only an authenticated route request loads the credential ID's secret. A missing or inaccessible
credential makes affected profiles unavailable without preventing application startup.
Each constructed provider account retains only the repository handle and typed credential ID.
Immediately before an authenticated HTTP request the repository loads one
`GenerationProviderCredentialSecret`, the transport attaches its vendor header, and the temporary
buffer is zeroized where supported when that request is built. Route structs, clients, availability
observations, and retry/poll state never retain or clone plaintext credentials.

## Failure Semantics

Profile failures are `GenerationProfileNotFound`, `GenerationProfileIncompatible`,
`GenerationProfileUnavailable`, and `GenerationProfileAvailabilityIndeterminate`.

`NodeCapabilityProviderFailure` categories are exactly `InvalidSemanticRequest`,
`AuthenticationFailed`, `PermissionDenied`, `ContentPolicyRejected`, `RateLimited`,
`ProviderUnavailable`, `DeadlineExceeded`, `ProviderRejected`, `InvalidResponse`,
`DownloadRejected`, and `AmbiguousSubmission`. Only `RateLimited` and `ProviderUnavailable` are
retryable; `DeadlineExceeded` is retryable only before accepted submission. `AmbiguousSubmission`
is never automatically retried. Optional retry time is valid only for a retryable category and must
be in the future when returned. Provider strings never determine Workflow state.

No database transaction remains open during any provider call. A capability translates one
provider failure into `NodeCapabilityExecutionError`; Workflow then owns the node/Run transition.

## Verification

- profile tests cover immutable identity, tombstones, and exact compatibility;
- availability tests cover bulk bounds, expiry, unavailable/indeterminate distinction, and detail
  redaction;
- router tests reject invalid route maps and prove one fixed route per profile and node execution;
- every router passes its exact public-interface suite;
- deterministic and vendor routes pass the same private route contract suite;
- each vendor route passes translation, idempotency, polling, cancellation, malformed-response,
  bounded-download, and ambiguous-submission tests;
- credential tests prove SQLite plaintext round trip, namespace isolation, redaction from
  DTOs/errors/logs, and missing-credential availability behavior;
- architecture tests reject node provider/model fields, broad provider interfaces, roadmap runtime
  interfaces, removed binding/task layers, and construction outside composition.
