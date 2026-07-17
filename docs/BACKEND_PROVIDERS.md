# Backend Generation Profiles And Providers

> Status: frozen MVP provider interfaces; Mock implementation only
> Owner: profile semantics in `crates/nodes`; provider interfaces in `crates/tasks`; routes in
> `crates/backends`; wiring in `src-tauri`
> Scope: provider-independent model selection, availability, routing, and translation

Users select a stable Generation Profile. Provider accounts, native models, endpoints, and routing
remain replaceable infrastructure. MVP freezes the provider-facing interfaces and proves them with
one deterministic Mock provider. Every production adapter, native wire, and vendor-specific route
configuration is explicitly deferred and does not constrain this design.

## Decision

1. Every model-powered node persists one provider-independent `GenerationProfileRef`.
2. `GenerationProfileDefinition` owns identity, lifecycle, and exact capability compatibility.
3. Provider-backed capabilities create one provider-neutral Generation Task rather than owning
   remote polling state.
4. Generation Task owns one provider-level `GenerationProviderInterface` and the four focused
   `TextGenerationProviderInterface`, `ImageGenerationProviderInterface`,
   `VideoGenerationProviderInterface`, and `VoiceGenerationProviderInterface` capabilities.
5. One cloud-provider implementation composes any non-empty subset of those complete capabilities
   while sharing its private transport, account, and authentication infrastructure.
6. Multiple providers may contribute the same capability. Provider Settings choose which provider/
   route serves each Generation Profile; missing capabilities are not selectable.
7. Only `DesktopCompositionRoot` constructs provider composites and the immutable provider registry.

The complete runtime stack is intentionally short:

```text
ImageToVideoCapabilityImpl
  -> NodeCapabilityGenerationTaskStarterInterface
  -> GenerationTaskStartUseCase
  -> GenerationProviderInterface resolves configured Mock provider
  -> VideoGenerationProviderInterface contribution
  -> deterministic Remote { submitter, poller }
  -> stateless Mock route
```

There is one provider-wide composition interface, but no provider-wide generic execution method,
optional capability method, or node-level provider/model field. `GenerationTaskAggregate` is the
sole durable provider-work owner.

## Source-And-Behavior Names

| Role | Required pattern | Example |
| --- | --- | --- |
| provider composite interface | `GenerationProviderInterface` | `MockGenerationProviderAdapterImpl` implements it |
| focused provider capability | `<Type>GenerationProviderInterface` | `VideoGenerationProviderInterface` |
| semantic request | `<Output>GenerationSpec` | `VideoGenerationSpec` |
| concrete provider composite | `<Provider>GenerationProviderAdapterImpl` | `MockGenerationProviderAdapterImpl` |
| private route interface | `<Input>To<Output>ProviderRouteInterface` | `ImageToVideoProviderRouteInterface` |
| concrete provider route | `<Provider><Input>To<Output>ProviderRouteImpl` | `MockImageToVideoProviderRouteImpl` |

The provider-level interface exposes identity and one non-empty typed capability product. The safe
UI contract is mechanically projected from that product. Focused capability interfaces expose
complete Immediate, Remote, or CancellableRemote execution compositions selected by an exact route
ID. One provider contributes at most one focused interface per kind, and that interface owns all
shipped routes for the provider/kind pair.
Standalone `Provider`, `Client`, `Model`, `Route`, `Binding`,
`Executor`, `Task`, and `Registry` names remain prohibited.

Behavioral equivalence for `GenerationProviderInterface` means stable provider identity,
deterministic mechanically-derived projection ordering, and no side effect during discovery. It
does not mean every provider supports every generation type. Behavioral equivalence for execution
is enforced separately on each focused
capability interface through its shared contract suite.

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
  exact capabilities, Generation Profile catalog, task-start interface
             ^
crates/tasks
  Generation Task aggregate, application, provider interfaces
             ^
crates/backends
  provider capability adapters, vendor routes, private protocol DTOs
             ^
src-tauri
  SQLite configuration, Assistant credentials, and construction
```

Recommended adapter layout:

```text
crates/backends/src/
  provider_capabilities/<type>/{submission,polling,cancellation}.rs
  deterministic_provider/<operation>_route.rs
  <vendor>/
    shared/{authentication,http,polling,download,error_translation}.rs
    <operation>/{route,request_dto,response_dto}.rs
```

Vendor DTOs, status values, native model IDs, and signed URLs remain private to their concrete
capability adapter. Only normalized provider outcomes and the validated opaque remote handle cross
into Generation Task; the handle never enters Workflow, Asset, Assistant, or Generation Profile.

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
`GenerationProviderRegistryProfileAvailabilityReaderAdapterImpl` performs one bounded bulk
observation for one exact capability and profile set. It reads the canonical
`(profile, generation kind)`-to-provider/route Settings map plus the matched provider contract and
never maintains another capability mapping. It does not probe once per UI row or persist availability. An exact configured
Mock route is always `Available`; Mock performs no synthetic network or credential probe.

`GenerationProfileListForCapabilityUseCase` joins definitions with current observations and returns
only provider-independent metadata. In the MVP, both `Unavailable` and `Indeterminate` prevent Run
admission. Task admission resolves only the configured provider/route binding structurally through
the immutable registry and persists that exact target; it performs no second availability probe,
and kind remains authoritative in the closed task request variant. Recovery of an accepted task
does not repeat the availability probe: it resolves the persisted provider + request kind + route
contract and lets the poll call return its normalized outcome. Recovery never consults or
substitutes the active Settings mapping. No route may silently substitute a different profile.

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

Provider Settings derive their selectable provider/route list from `GenerationProviderContract`.
Each focused capability contract contains a non-empty set of safe route contracts; each route
contract has its stable route ID, display name, and exact compatible Generation Profile refs. For a
Text profile the projection includes only compatible Text routes; Image includes only compatible
Image routes; Video only compatible Video routes; speech/voice only compatible Voice routes.
Applying a `(profile_ref, generation_kind, provider_id, route_id)` mapping absent from that exact
projection is rejected before persistence. If configuration is missing or becomes
invalid after a configuration change, availability is `NoConfiguredRoute`, so Workflow readiness blocks it and
the UI cannot select it. The UI never guesses capability support from provider name or a failed
generation call.

## Frozen MVP Generation Task Provider Contracts

[`BACKEND_TASK.md`](BACKEND_TASK.md#9-provider-contracts) owns the public submit, poll, and cancel
interfaces. This document owns their provider translation. The closed task request variants carry
provider-neutral Text/Image/Video/Voice semantics. The current three active Node Capabilities use
the last three rows; Text activates through the same registry when its capability/profile contract
is admitted:

| Task request | Semantic fields after profile/origin | Required output |
| --- | --- | --- |
| `Text` | bounded prompt and profile-owned text controls | one Text value |
| `Image(TextToImage)` | `prompt: WorkflowTextValue`, `aspect_ratio: ImageAspectRatio` | one image |
| `Video(ImageToVideo)` | exact input Asset snapshot, optional prompt, `duration_seconds: 5 | 10` | one video |
| `Voice(TextToSpeech)` | `text: WorkflowTextValue` | one Audio Asset |

The immutable task target supplies the exact profile, provider, and route reference.
The stable `GenerationTaskId` becomes the native submission idempotency key where supported.
Media outputs retain the frozen MIME and size limits: PNG up to 32 MiB, MP4 up to 512 MiB, and
MPEG up to 64 MiB. Zero bytes, a second primary output, unknown length, mismatched facts, trailing
bytes, or digest mismatch is `InvalidResponse`.

## Provider Capability Composition And Private Route

One provider-level adapter implements `GenerationProviderInterface` and assembles its complete
focused capabilities. MVP registers one Mock provider that contributes Image, Video, and Voice
without pretending to implement Text:

```rust
pub struct MockGenerationProviderAdapterImpl {
    provider_id: GenerationProviderId,
    display_name: GenerationProviderDisplayName,
    capabilities: GenerationProviderCapabilities,
}

fn mock_image_to_video_execution(
    route: Arc<MockImageToVideoProviderRouteImpl>,
) -> VideoGenerationProviderExecution {
    VideoGenerationProviderExecution::Remote {
        submitter: route.clone(),
        poller: route,
    }
}
```

This is one adapter-private route composition, not a second public contract. The Mock routes
implement the same complete task-owned submitter and poller interfaces that a future asynchronous
production adapter must implement. They perform no network, filesystem, credential, or vendor DTO
work.

The composed provider registry resolves `GenerationProfileRef` only through its exact
provider/type/route entry. The
routed request retains every other semantic field. One route maps exactly one profile semantic
contract and cannot branch on another profile or return `Unsupported`.

Provider and route IDs use immutable lower-case dot/hyphen segments and are never repurposed. The
frozen MVP composition map is exact:

| Profile ref | Provider ID | Route ID | Implementation |
| --- | --- | --- | --- |
| `image.high_quality_general@1` | `mock` | `mock.image.high-quality-general.v1` | `MockTextToImageProviderRouteImpl` |
| `video.cinematic_image_animation@1` | `mock` | `mock.video.cinematic-image-animation.v1` | `MockImageToVideoProviderRouteImpl` |
| `speech.multilingual_narration@1` | `mock` | `mock.voice.multilingual-narration.v1` | `MockTextToSpeechProviderRouteImpl` |

The Mock route derives a stable opaque handle from `GenerationTaskId` and the persisted task
creation timestamp supplied in `GenerationProviderCallContext`; repeating submit for the same task
therefore returns the same handle and completion instant.
Before a fixed completion instant encoded by that validated handle it returns deterministic
`Pending` progress; at and after that instant it returns the same terminal result forever. Its media
bytes are fixed valid test fixtures within the frozen MIME and size limits. It needs no mutable
process state, so restart tests reconstruct a fresh Mock adapter and prove recovery solely from the
persisted handle. Mock routes intentionally omit remote cancellation; local cancellation remains
deterministic and the separate canceller interface is frozen by its focused contract tests.
All three Mock routes use a 30-second task deadline, a one-second deterministic completion offset,
and a 500-millisecond poll delay; fault-injection tests override outcomes through a separately
constructed fake, never through runtime Mock configuration.

Production provider work starts only after a separate review freezes that adapter's typed route
configuration, native request/response DTOs, authentication, untrusted-response validation,
idempotency behavior, polling guarantees, and cancellation behavior. In particular, MVP defines no
vendor route configuration, endpoint, native model, API request, or vendor credential field. Adding
one implements these interfaces; it does not change Generation Task,
Workflow, Asset, Settings projection, or focused provider contracts.

Provider construction rejects an empty capability product and any route ID reused
across its focused capabilities. Settings validation rejects unknown or incompatible profiles,
and duplicate `(profile, generation kind)` mappings. A missing mapping is represented by
`NoConfiguredRoute`, not a placeholder implementation. Contract conformance is proved in tests,
not represented as runtime configuration.

Composition maintains two distinct structures: the committed active
`(profile, generation kind)`-to-provider/route map
read transactionally for new task admission, and the immutable shipped provider registry used for
existing task bindings. Disabling or rebinding a profile affects future tasks but does not remove
its shipped adapter from recovery. Recovery first resolves persisted provider ID, then request kind,
then route ID through the matching focused interface; it never consults the current profile
selection. A software version may remove a shipped recovery adapter only through an explicit
retirement decision that first proves no non-terminal task references it. This is a release-safety
check, not a data migration or compatibility path.

## Dispatch Rules

```text
saved GenerationProfileRef
  -> compatibility and availability
  -> durable Workflow Run and WorkflowNodeExecutionId
  -> Generation Task persists immutable request and exact route binding
  -> provider adapter translates and submits
  -> task persists remote ID and polls by that ID
  -> task finalizes media through the Asset boundary
  -> task notification lets Workflow commit node output and state
```

The selected `GenerationProviderRouteId` is fixed inside one active node execution before paid
submission. The MVP never switches routes during that execution. An ambiguous submission fails
with a structured category and is never automatically repeated.
Multi-route selection and automatic failover are post-MVP concerns.

Accepted remote handles are persisted only inside `GenerationTaskAggregate` and its SQLite row.
They never enter Workflow, Asset, Assistant, Generation Profile, ordinary logs, or Asset
provenance or public DTOs. On restart, the task worker queries the same provider operation by that
ID and the waiting Workflow node remains non-terminal.

Remote cancellation uses the separate complete `GenerationCancellerInterface` when the adapter
implements it. Lack of remote cancellation never becomes an optional method or `Unsupported`
result; local task and Workflow cancellation remain deterministic while external work or charges
may continue.

## Route Responsibilities

Each concrete route:

- maps every semantic field to one private vendor DTO;
- uses a typed native model ID rather than parsing profile/display names;
- sends explicit values when native defaults could change promised behavior;
- validates the frozen response shape, media kind, size, and MIME, plus reported model or checksum
  only when that exact vendor response contract supplies the field;
- bounds submission, polling, redirects, downloads, deadlines, and response sizes;
- maps provider status exactly once into task-owned normalized outcomes;
- keeps credentials, raw bodies, signed URLs, and route details private;
- returns only a validated opaque handle for durable task persistence.

Each registered route supplies one bounded operation deadline and poll policy satisfying the Task
contract. A future production route additionally freezes request/response byte limits, redirect and
download policy, HTTPS allowlist, and SSRF protection in its own reviewed adapter design. A route
never resubmits after ambiguous acceptance. Recovery polls only a handle that was durably committed
with `Running`.

Remote media is downloaded and validated inside the route. The route returns a semantic payload but
never creates an Asset; Generation Task finalization owns the call to its Asset sink boundary.

## Credentials And Configuration

Non-secret MVP configuration declares only the exact `(profile, generation kind)`-to-Mock-route
selection. The Mock provider requires no account, credential, endpoint, native model, or
route-specific configuration.

Each production adapter must introduce its non-secret route configuration as a reviewed typed value
owned by that adapter. No generic provider-options JSON exists, and no vendor configuration,
account, credential mutation, credential revision, or authenticated recovery contract is defined
before its production adapter is scheduled. Existing plaintext provider credential rows are
retained as inactive legacy data and never exposed by Mock Settings or loaded by the Mock registry.

## Failure Semantics

Profile failures are `GenerationProfileNotFound`, `GenerationProfileIncompatible`,
`GenerationProfileUnavailable`, and `GenerationProfileAvailabilityIndeterminate`.

`GenerationProviderFailure` categories are exactly `InvalidSemanticRequest`,
`AuthenticationFailed`, `PermissionDenied`, `ContentPolicyRejected`, `RateLimited`,
`ProviderUnavailable`, `DeadlineExceeded`, `ProviderRejected`, `InvalidResponse`,
`DownloadRejected`, and `AmbiguousSubmission`. Every `GenerationProviderFailure` is a terminal
generation outcome, including `RateLimited` and `ProviderUnavailable`; a retryable rate limit or
outage on a poll or cancellation call is instead a transient `GenerationProviderCallError`, which
reschedules the same accepted handle. No category permits automatic Immediate or Submit
re-execution. `AmbiguousSubmission` is normally synthesized by the Generation Task application from
an uncertain Immediate or Submit call error; a provider adapter declares it only when its own
validated response proves acceptance cannot be known. Optional retry time belongs only to a
transient `GenerationProviderCallError` and must be in the future when returned. Provider strings
never determine Workflow state.

The Generation Task application normalizes each provider failure category exactly once into the
`GenerationTaskFailure` kind owned by `BACKEND_TASK.md`:

| `GenerationProviderFailure` category | `GenerationTaskFailure` kind |
| --- | --- |
| `InvalidSemanticRequest` | `InvalidRequest` |
| `AuthenticationFailed` | `Authentication` |
| `PermissionDenied` | `PermissionDenied` |
| `ContentPolicyRejected` | `ContentPolicy` |
| `RateLimited` | `RateLimited` |
| `ProviderUnavailable` | `ProviderUnavailable` |
| `DeadlineExceeded` | `Timeout` |
| `ProviderRejected` | `ProviderRejected` |
| `InvalidResponse` | `InvalidProviderResponse` |
| `DownloadRejected` | `InvalidProviderResponse` |
| `AmbiguousSubmission` | `AmbiguousSubmission` |

A permanent `GenerationProviderCallError` on a poll or cancellation call maps to
`ProviderUnavailable`; an uncertain Immediate or Submit call error maps to `AmbiguousSubmission`;
worker-observed deadline expiry maps to `Timeout`. `InputAssetUnavailable`, `OutputAssetImport`,
and `Internal` are task-internal kinds with no provider origin.

No database transaction remains open during any provider call. The Generation Task application
translates one provider outcome into an aggregate transition; its Workflow notification bridge then
lets Workflow own the node/Run transition.

## Verification

- profile tests cover immutable identity, tombstones, and exact compatibility;
- availability tests cover bulk bounds, expiry, unavailable/indeterminate distinction, and detail
  redaction;
- provider-composite tests reject empty capability products and duplicate route IDs and prove
  deterministic mechanically-derived safe contracts;
- focused capability tests prove one fixed configured provider/route per task and exact interface behavior;
- Mock routes pass the shared private route contract suite; future production routes must pass the
  same suite before registration;
- every registered route passes translation, malformed-response, deadline, and response-bound tests;
- Immediate routes additionally pass equivalent-result rules, Remote routes pass submit/poll and
  ambiguous-submission rules, and only CancellableRemote routes run the cancellation contract suite;
- architecture tests reject node provider/model fields, broad provider interfaces, provider-owned
  task semantics, roadmap runtime interfaces, and construction outside composition.
