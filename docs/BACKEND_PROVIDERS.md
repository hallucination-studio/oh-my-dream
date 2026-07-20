# Backend Generation Profiles And Providers

> Status: frozen production multi-provider target; implementation status lives in `ROADMAP.md`
> Owner: profile, Provider Connection, Generation Model, and Generation Settings application
> semantics in `crates/nodes`; provider interfaces in `crates/tasks`; routes in `crates/backends`;
> adapters and wiring in `src-tauri`
> Scope: provider-independent profile semantics, saved Generation Models, production endpoint and
> credential configuration, availability, routing, and translation

Users save reusable Provider Connections, attach several Generation Models to each compatible
connection, and select one stable model on each model-powered node. A connection owns the service
family, normalized Endpoint root, and write-only credential binding. A model owns native identity,
code-owned model family, capability contract, Generation Profile, and lifecycle. OpenAI Images,
Volcengine Ark Standard visual, Volcengine Agent Plan visual, and Volcengine Agent Plan speech are
different service families even when one vendor account can access more than one. The deterministic
Mock remains test infrastructure and enters composition only under the explicit debug gate below.

## Decision

1. Every model-powered node persists one provider-independent `GenerationProfileRef` and one
   `GenerationModelId` selected from compatible enabled saved models.
2. `GenerationProfileDefinition` owns identity, lifecycle, and exact capability compatibility.
3. `GenerationProviderConnectionConfiguration` owns one reusable service/Endpoint/credential
   boundary; `GenerationModelConfiguration` references one exact connection revision and owns the
   model identity, family, profile, lifecycle, and revision.
4. Provider-backed capabilities create one provider-neutral Generation Task rather than owning
   remote polling state.
5. Generation Task owns one provider-level `GenerationProviderInterface` and the four focused
   `TextGenerationProviderInterface`, `ImageGenerationProviderInterface`,
   `VideoGenerationProviderInterface`, and `VoiceGenerationProviderInterface` capabilities.
6. One cloud-provider implementation composes any non-empty subset of those complete capabilities
   while sharing its private transport, account, and authentication infrastructure.
7. Multiple enabled Generation Models may contribute the same capability and profile, and several
   models may share one connection without duplicating its Endpoint or token.
8. Every immutable model revision references one immutable connection revision. Run admission
   freezes the model revision and therefore its exact connection revision; Task target construction
   additionally copies the connection and credential-binding refs. Later Settings changes affect
   only later Runs; there is no mid-Run switch or automatic fallback.
9. Only `DesktopCompositionRoot` constructs provider composites and the immutable protocol registry.

The complete runtime stack is intentionally short:

```text
GenerateVideoCapabilityImpl
  -> NodeCapabilityGenerationTaskStarterInterface
  -> selected GenerationModelId resolves one enabled model revision
  -> model revision resolves one immutable Provider Connection revision
  -> GenerationTaskStartUseCase freezes model/connection/profile/provider/route target
  -> GenerationProviderInterface resolves the shipped Seedance protocol route
  -> VideoGenerationProviderInterface contribution
  -> Remote { submitter, poller }
  -> typed Volcengine request/response translation
```

There is one provider-wide composition interface, but no provider-wide generic execution method or
optional capability method. A node stores only the stable application-owned `GenerationModelId`;
it never stores provider IDs, native model IDs, endpoints, credentials, or route IDs.
`GenerationTaskAggregate` is the sole durable provider-work owner.

## Source-And-Behavior Names

| Role | Required pattern | Example |
| --- | --- | --- |
| provider composite interface | `GenerationProviderInterface` | `MockGenerationProviderAdapterImpl` implements it |
| focused provider capability | `<Type>GenerationProviderInterface` | `VideoGenerationProviderInterface` |
| semantic request | `<Output>GenerationSpec` | `VideoGenerationSpec` |
| concrete provider composite | `<Provider>GenerationProviderAdapterImpl` | `MockGenerationProviderAdapterImpl` |
| private route interface | `<Behavior>ProviderRouteInterface` | `GenerateVideoProviderRouteInterface` |
| concrete provider route | `<Provider><Behavior>ProviderRouteImpl` | `MockGenerateVideoProviderRouteImpl` |

The provider-level interface exposes identity and one non-empty typed capability product. The safe
UI contract is mechanically projected from that product. Focused capability interfaces expose
complete Immediate, Remote, or CancellableRemote execution compositions selected by an exact route
ID. One provider contributes at most one focused interface per kind, and that interface owns all
shipped routes for the provider/kind pair.
Standalone `Provider`, `Client`, `Model`, `Route`, `Binding`, `Executor`, `Task`, and `Registry`
names remain prohibited. The fully-qualified business term `GenerationModelConfiguration` is not a
provider-native `Model` alias.

Behavioral equivalence for `GenerationProviderInterface` means stable provider identity,
deterministic mechanically-derived projection ordering, and no side effect during discovery. It
does not mean every provider supports every generation type. Behavioral equivalence for execution
is enforced separately on each focused
capability interface through its shared contract suite.

Provider-shared infrastructure uses the `GenerationProvider` prefix, such as
`GenerationProviderConnectionId`, `GenerationProviderCredentialBindingId`,
`GenerationProviderRouteId`, and
`GenerationProviderCredentialRepositoryInterface`. It cannot be confused with Assistant model
configuration.

## Dependency Direction

```text
crates/engine
  WorkflowNodeCapabilityInterface and WorkflowNodeExecutionId
             ^
crates/nodes
  exact capabilities, Generation Profile catalog, Provider Connection and Generation Model
  aggregates, Generation Settings use cases/interfaces, task-start interface
             ^
crates/tasks
  Generation Task aggregate, application, provider interfaces
             ^
crates/backends
  provider capability adapters, vendor routes, private protocol DTOs
             ^
src-tauri
  SQLite Generation Settings and credential adapters, DTOs, commands, and construction
```

`crates/nodes::generation_settings` is the only business owner of
`GenerationProviderConnectionConfiguration`, `GenerationModelConfiguration`, their lifecycle and
revision transitions, the closed Settings mutations, `GenerationSettingsGetUseCase`,
`GenerationSettingsApplyUseCase`, model-selection use cases, and
`GenerationSettingsRepositoryInterface`. It contains no SQLite, HTTP, filesystem, Tauri, or
credential bytes. `src-tauri` only translates DTOs and implements the repository/credential
interfaces; it must not revalidate or reinterpret a Settings transition.

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

## Provider Connections, Saved Models, And Production Protocols

`GenerationProviderConnectionConfiguration` is the authoritative user-managed aggregate for one
reusable authentication boundary. It contains stable `GenerationProviderConnectionId`, non-zero
`GenerationProviderConnectionRevision`, display name, immutable
`GenerationProviderServiceFamily`, normalized `GenerationProviderEndpointRoot`, one
`GenerationProviderCredentialBindingId`, and `Enabled | Disabled | Removed` lifecycle. The ID is an
RFC 9562 UUIDv4. Display name follows the model display-name bounds below and is unique among
non-Removed connections. At most 16 non-Removed connections exist.

The service family is immutable after connection creation. Changing from Ark Standard to Agent Plan
visual or speech therefore creates another connection. Updating only the Endpoint root creates a new
connection revision and a new credential binding with an explicitly supplied token. In the same
atomic Settings mutation, every current non-Removed model attached to the prior connection revision
receives a new model revision pointing at the new connection revision. Token-only rotation keeps the
same connection revision and credential binding, so it can repair authenticated calls for admitted
work only within the exact same service family and Endpoint compatibility epoch.

`GenerationModelConfiguration` is the authoritative aggregate for one selectable model. It contains:

```text
GenerationModelId
GenerationModelRevision
GenerationModelDisplayName
GenerationProviderConnectionRevisionRef
GenerationModelFamilyRef
GenerationModelIdentity
GenerationModelIdentityEvidence
GenerationProfileRef
GenerationTaskRequestKind
GenerationModelCapabilityContractRef
GenerationModelLifecycleState = Enabled | Disabled | Removed
```

`GenerationModelId` is an RFC 9562 UUIDv4 and remains stable across edits.
`GenerationModelRevision` is a non-zero monotonic `u64`. Display name is trimmed, contains 1..=80
Unicode scalar values, contains no control character, and is unique under Unicode case folding
among non-Removed models. At most 64 non-Removed models exist. Generic provider option JSON,
arbitrary headers, caller-authored paths, and a token on a model are prohibited.

The first service-family roots are exact and never interchangeable:

| Service family | Normalized root | Models and adapter-owned operation |
| --- | --- | --- |
| `OpenAiImageApi` | user root ending at the API version, default `https://api.openai.com/v1` | code-owned OpenAI image families; `POST {root}/images/generations` |
| `VolcengineArkStandardVisual` | user root ending in `/api/v3`; default `https://ark.cn-beijing.volces.com/api/v3` | source-verified Seedream image and Seedance task operations |
| `VolcengineArkAgentPlanVisual` | user root ending in `/api/plan/v3` | Agent Plan visual families registered only with exact source fixtures; never routed through standard Ark |
| `VolcengineAgentPlanSpeech` | `https://openspeech.bytedance.com/api/v3/plan` or an equivalent validated loopback test root | fixed Agent Plan TTS 2.0 HTTP operation at `/tts/unidirectional` |

The supplied official source set is recorded without reinterpreting it: Agent Plan embedding
[`2375464`](https://www.volcengine.com/docs/82379/2375464), Agent Plan visual
[`2375486`](https://www.volcengine.com/docs/82379/2375486), Agent Plan speech
[`2516286`](https://www.volcengine.com/docs/82379/2516286), Seedance create
[`1520757`](https://www.volcengine.com/docs/82379/1520757), Seedance query
[`1521309`](https://www.volcengine.com/docs/82379/1521309), Seedance list
[`1521675`](https://www.volcengine.com/docs/82379/1521675), Seedance cancel/delete
[`1521720`](https://www.volcengine.com/docs/82379/1521720), Seedream image
[`1541523`](https://www.volcengine.com/docs/82379/1541523), and the official OpenAI
[Image generation guide](https://developers.openai.com/api/docs/guides/image-generation). Embedding
is not a Text/Image/Video/Voice generation operation and is excluded from this MVP. A production
route becomes `Selectable` only when its exact request, response, authentication, operation path,
limits, and model-family contract exist as source fixtures.

The exact first-release Volcengine production calls are:

| Creator route | Native model identity | Complete HTTP operation | Authentication/correlation |
| --- | --- | --- | --- |
| Seedream 5.0 Lite Image | `doubao-seedream-5-0-260128` | `POST https://ark.cn-beijing.volces.com/api/v3/images/generations` | `Authorization: Bearer <ARK_API_KEY>` |
| Seedream 5.0 Pro Image | `doubao-seedream-5-0-pro-260628` | `POST https://ark.cn-beijing.volces.com/api/v3/images/generations` | `Authorization: Bearer <ARK_API_KEY>` |
| Seedance 2.0 Video create | `doubao-seedance-2-0-260128` | `POST https://ark.cn-beijing.volces.com/api/v3/contents/generations/tasks` | `Authorization: Bearer <ARK_API_KEY>` plus diagnostic `X-Client-Request-Id` |
| Seedance 2.0 Video query | same persisted model/handle scope | `GET https://ark.cn-beijing.volces.com/api/v3/contents/generations/tasks/{task_id}` | same frozen Ark credential binding |
| Seedance 2.0 Video list (private diagnostics only) | connection-scoped inventory; never Task ownership evidence | `GET https://ark.cn-beijing.volces.com/api/v3/contents/generations/tasks` | same frozen Ark credential binding |
| Seedance 2.0 Video cancel/delete | same persisted handle | `DELETE https://ark.cn-beijing.volces.com/api/v3/contents/generations/tasks/{task_id}` | same frozen Ark credential binding |
| Doubao Seed TTS 2.0 Speech | `doubao-seed-tts-2.0` | `POST https://openspeech.bytedance.com/api/v3/plan/tts/unidirectional` | `X-Api-Key`, `X-Api-Resource-Id: seed-tts-2.0`, and unique request ID |

The official TTS source is [Agent Plan speech](https://www.volcengine.com/docs/82379/2516286#a4e97d10),
which selects the HTTP unidirectional route documented at
[`1598757`](https://www.volcengine.com/docs/6561/1598757?lang=zh). WebSocket TTS and ASR endpoints
listed by that source are not first-release routes. A configurable connection may replace only the
validated protocol root; every operation suffix above remains code-owned.

The Agent Plan speech credential is its dedicated API key. Ark Standard, Agent Plan visual, and
Agent Plan speech use separate connections and credential bindings even when the user believes one
account can serve all three. No adapter probes one family by sending another family's credential.

Agent Plan TTS uses the HTTP endpoint because the current Workflow operation produces one durable
Audio Asset. Bidirectional and unidirectional WebSocket playback, incremental audio presentation,
and ASR are outside this contract. TTS sends `X-Api-Key`, fixed
`X-Api-Resource-Id: seed-tts-2.0`, and a unique request/connect identifier; the token and provider
headers never enter a model-neutral request or public DTO. The route records the response
`X-Tt-Logid` as a bounded structured tracing field for support correlation without exposing it to
Workflow or UI.

For `speech.multilingual_narration@1`, the Agent Plan route freezes native speaker
`zh_female_vv_uranus_bigtts`, MP3, and 24,000 Hz. These are adapter translations of the versioned
profile promise, not Settings fields or node parameters. Changing them requires a reviewed profile
version and route fixture update.

The OpenAI protocol is specifically the Image API Generations operation, not the Responses API
image tool. It requests one output and accepts only the documented single-image result shape. The
TTS route appends the adapter-owned `/tts/unidirectional` HTTP operation, parses the bounded chunked JSON
stream, concatenates only successful base64 audio chunks, and stops only at the documented terminal
code. It never starts a WebSocket session.

`GenerationProviderEndpointRoot` is a normalized URL of at most 2,048 bytes. HTTPS is required for a
non-loopback host; HTTP is accepted only for an explicit loopback development endpoint. Userinfo,
query, fragment, control characters, and a path that escapes the protocol-owned root are rejected.
The protocol adapter owns the operation path and disables redirects for authenticated calls.
Provider-returned media URLs are untrusted and require an HTTPS provider-specific host allowlist,
redirect rejection, deadline, size bound, MIME inspection, and digest verification before bytes
cross the provider boundary.

Connection/model mutations atomically compare-and-swap the one Generation Settings revision. A new
connection requires a new credential binding and token. A null or blank token on an Endpoint-stable
connection update retains the same binding; a non-empty token rotates that binding in place. An
Endpoint-root change requires a new binding and token and atomically writes the connection revision
plus all affected model revisions. A failed validation, conflict, revision write, cascade, or
credential write changes none of them. Public reads expose only `has_api_token`; secrets never enter
configuration JSON, revision rows, DTOs, errors, logs, tracing fields, Workflow, or Task rows.

A Workflow node persists the stable `GenerationModelId`, not a configuration revision. Every model
mutation and every attached-connection Endpoint change advances `GenerationModelRevision`;
token-only rotation does not. Credential bytes are excluded from every revision snapshot and
fingerprint. Run admission resolves the current enabled model and its exact enabled connection
revision from one snapshot, then freezes `GenerationModelRevisionRef { model_id, revision }` in the
execution plan. Generation Task resolves that immutable model revision, follows its immutable
connection-revision ref, and copies the exact non-secret service family, Endpoint,
credential-binding ref, native identity, provider, and route target before any external call. A Settings mutation
therefore affects only Runs admitted afterward. There is no mid-Run switch, automatic fallback, or
silent use of the latest revision during recovery.

Disabled or Removed models or connections cannot admit new Runs, but every model revision,
connection revision, route, and credential binding referenced by admitted non-terminal work remains
resolvable. Removal is a tombstone. Token rotation intentionally applies to later calls using the
same binding; a new Endpoint can never reuse that binding, which prevents a new secret from being
sent to an old frozen Endpoint during recovery.

### Debug-Gated Deterministic Models

`OH_MY_DREAM_ENABLE_MOCK_MODELS=true` is the only switch that registers creator-visible Mock
models. Desktop reads it from the process environment loaded by the local `.env` convention at
composition time. Missing, blank, malformed, or any value other than the exact lower-case `true`
means disabled. Release packaging forces it disabled; deterministic unit tests may construct Mock
routes directly, and Desktop E2E launches with it enabled.

When enabled, composition contributes one immutable built-in Mock model for each active generated
media capability. Each has an application-owned fixed UUIDv4, revision `1`, fixed profile and
capability contract, `Debug mock` presentation, and no endpoint, native model, Settings row, or
credential. Built-ins appear in `generation_model_list_for_capability` and model-capability-contract
queries but never in `generation_settings_get`. They cannot be created, edited, disabled, or
removed. A Workflow may persist their stable IDs; reopening it with the gate off preserves the node
and reports `GenerationModelUnavailable(DebugModelDisabled)` until the gate is enabled or the user
explicitly selects a production model. Nothing silently substitutes a Mock model.

This gate exists for deterministic browser/Desktop E2E and local demonstrations. It is not a
credential bypass, production fallback, persisted preference, or generic provider option.
Task admission translates a selected built-in into the closed debug target variant containing only
its fixed model definition, provider, and route identities. It never fabricates a production
protocol, Endpoint, native identity, or credential reference.

## Model Capability Contracts And Video Calibration

`GenerationModelFamilyDefinition` is a code-owned catalog entry. It binds one stable
`GenerationModelFamilyRef` to compatible service families, exact identity-evidence policy, one safe
`GenerationModelCapabilityContract`, provider route, and lifecycle `Selectable | Deprecated |
RecoveryOnly`. `Selectable` permits new models, selection, and Run admission. `Deprecated` prohibits
new model creation and new node selection but permits an already-selected saved model to admit a
Run. `RecoveryOnly` prohibits every new Run and remains registered solely for admitted work. A route
or family can be removed from a software release only after storage proves no non-terminal Run or
Task references it.

`GenerationModelIdentityEvidence` is exactly `KnownAlias` or `ProviderVerifiedEndpoint`.
`KnownAlias` requires an exact native identity listed by the family definition.
`ProviderVerifiedEndpoint` requires an official metadata operation that returns the endpoint's
model family and an immutable provider resource revision; Settings apply persists only the bounded
identity, resource revision, evidence digest, and observation time. A user-selected family label is
never evidence. An opaque Endpoint ID without that operation is saved only as an explainable draft
model with `IdentityUnverified` availability and cannot be selected or admitted.

The immutable family registry maps a proven `(service family, model family)` pair to one safe
`GenerationModelCapabilityContract`. A model revision freezes that contract ref and one exact
connection revision. The same contract drives node controls, backend readiness/calibration, Run
admission, and provider translation. React never parses a native ID or maintains an OpenAI,
Seedream, Seedance, or Agent Plan matrix.

`GenerationModelCapabilityContractRef` is a code-owned stable ID plus non-zero version. Its
observable input, parameter, default, or cross-field semantics are immutable. A changed matrix gets
a new ref and new model configuration revision; the old contract and recovery route remain
registered while any non-terminal Run or Task references them.

Every Image family contains a closed `ImageGenerationModelContract` with supported semantic aspect
ratios, one exact canonical provider size/resolution for each ratio, suggested ratio, output count,
output MIME, response representation, and adapter request defaults. The MVP output count is exactly
one. An active OpenAI family must map each exposed ratio to a documented `size` token and freeze
non-streaming plus its exact base64 or URL response path. An active Seedream or Agent Plan visual
family must map each exposed ratio to its exact documented `size`/resolution value and response
shape. There is no generic five-ratio contract and no implementation-time size calculation.

The first-release Image families are GPT Image 2, Seedream 5.0 Lite, and Seedream 5.0 Pro. The two
Seedream identities and their shared operation are frozen in the production-call table above. A
family remains absent or `RecoveryOnly` until its exact ratio-to-size table, default,
PNG normalization rule, response fixture, and byte bounds are populated from the accepted source
packet. `TextToImageCapabilityImpl` consumes the same model-contract reader as Video, calibrates
`aspect_ratio`, and blocks a missing or unsupported value before Task creation.

The Seedream MVP subset is deliberately complete and narrow for both frozen identities:
Text-to-Image only, exactly one output, semantic ratio `1:1` translated to explicit
`size = "2048x2048"`, `output_format = "png"`, `response_format = "url"`, and
`watermark = false`. The route validates exactly one returned PNG URL and imports its bytes through
the Task output-source lease. Image editing, reference images, sequential/group output, additional
ratios, and resolution presets remain absent until separate model-specific fixtures extend the
contract; the adapter never calculates a size from a ratio at runtime.

The required first-release Seedance family is `Seedance2_0` with native model
`doubao-seedance-2-0-260128`. `Seedance2_0Fast`, `Seedance2_0Mini`, `Seedance1_5Pro`,
`Seedance1_0Pro`, and `Seedance1_0ProFast` remain unavailable until their exact source fixtures and
contract suites pass; they are not necessary to run the first production Video flow. Each active contract
contains a closed `VideoGenerationModelContract`:

```text
supported_input_modes and mode-specific prompt requirements
image/video/audio role and cardinality contracts
supported/default resolutions and mode-specific ratios
duration modes, bounds, and defaults
closed parameter availability, value sets, and defaults
closed cross-field calibration rules
```

The universal Video capability has stable inputs: one optional Text `prompt`, ordered `images`,
ordered `videos`, and ordered `audio`. Every media item has a capability-owned role. A selected
model contract admits exactly one input mode:

| Input mode | Exact shape |
| --- | --- |
| `TextToVideo` | required Text; no media |
| `FirstFrame` | optional Text plus exactly one Image role `FirstFrame` |
| `FirstAndLastFrames` | optional Text plus exactly two Images ordered as `FirstFrame`, `LastFrame` |
| `MultimodalReference` | optional Text; 0..=9 Images role `ReferenceImage`, 0..=3 Videos role `ReferenceVideo`, 0..=3 Audio items role `ReferenceAudio`; at least one Image or Video, so Audio is never the only media |

The modes are mutually exclusive. First/last-frame roles cannot mix with any reference role.
Seedance 2.0 variants support all four modes; 1.5 Pro and 1.0 Pro support the first three;
1.0 Pro Fast supports only Text-to-Video and FirstFrame. Reference Video and Audio are therefore
2.0-only in this protocol set. `TextToVideo` is the suggested initial mode for a new unconnected
Video node; connecting media never changes the mode automatically.

The provider-neutral spec keeps separate semantic order for Images, Videos, and Audio. Seedance
translation emits optional Text first, then Images, Videos, and Audio, preserving order within each
kind and every role. Prompt references such as the first reference Video therefore use that stable
kind-local order; provider response order or UI sorting never changes it.

Video parameter keys are stable even when a model omits them: `input_mode`, `generate_audio`,
`draft`, `resolution`, `ratio`, `duration_mode`, `duration_seconds`,
`frame_count`, `seed_mode`, `seed`, `camera_fixed`, and `watermark`. The selected contract decides
which controls are available, their allowed values, and their suggested defaults. The stable node
contract permits absent dynamic values; model calibration, not generic parameter normalization,
decides whether a value is required for the selected revision.

| Variant family | Input modes | Resolution and default | Duration and default | Frames | Other gated fields |
| --- | --- | --- | --- | --- | --- |
| Seedance 2.0 | all four | base: `480p/720p/1080p/4k`; Fast/Mini: `480p/720p`; default `720p` | `4..=15` seconds or `Auto`; suggested `Seconds(5)` | unavailable | `generate_audio` default `true`; no draft, seed, or camera-fixed |
| Seedance 1.5 Pro | Text, FirstFrame, FirstAndLastFrames | `480p/720p/1080p`; default `720p` | `4..=12` seconds or `Auto`; suggested `Seconds(5)` | unavailable | `generate_audio` default `true`, `draft` default `false`, seed and camera-fixed |
| Seedance 1.0 Pro | Text, FirstFrame, FirstAndLastFrames | `480p/720p/1080p`; default `1080p` | `2..=12` seconds; suggested `Seconds(5)` | `29..=289` and exactly `25 + 4n` | seed and camera-fixed; no generated audio or draft |
| Seedance 1.0 Pro Fast | Text, FirstFrame | `480p/720p/1080p`; default `1080p` | `2..=12` seconds; suggested `Seconds(5)` | `29..=289` and exactly `25 + 4n` | seed and camera-fixed; no generated audio or draft |

All families expose `16:9`, `4:3`, `1:1`, `3:4`, `9:16`, and `21:9`. `Adaptive` is available for
every 2.0/1.5 mode and only FirstFrame/FirstAndLastFrames on 1.0. The suggested ratio is `Adaptive`
for 2.0/1.5, `16:9` for 1.0 TextToVideo, and `Adaptive` for 1.0 image modes.
`duration_mode` is the structured union `Auto | Seconds | Frames`; `seed_mode` is `Random | Fixed`,
so provider sentinel `-1` never becomes a magic domain integer. `Seconds` requires
`duration_seconds`, `Frames` requires `frame_count`, and every other timing field must be absent.
`Fixed` requires `seed`; `Random` requires it absent. Seed bounds are `0..=2^32-1`.
The suggested seed mode for every seed-capable variant is `Random` with no stored seed value.
Only the Seedance wire translator maps `Auto` duration or `Random` seed to provider sentinel `-1`;
that integer never enters Workflow, the calibrated Task spec, Settings, or UI.
`draft = true` is 1.5-only, requires `resolution = 480p`, forces `return_last_frame` absent, and
cannot select an offline service tier because neither field is exposed by this node.
`camera_fixed` defaults to `false`, is unavailable on 2.0, and cannot be `true` when any image has
role `ReferenceImage`. `watermark` is available to every listed variant and defaults to `false`.

`GenerateVideoCapabilityImpl` owns one `VideoGenerationCalibrationPolicy` that validates these
contracts and input/parameter combinations exactly once. It returns typed
`GenerationModelCalibrationIssue` values identifying an input item or parameter, the violated
rule, and a closed correction proposal containing compatible values or the selected contract's
suggested default. It never relies on provider error prose. Changing models changes only the model
selection, preserves every connection and parameter, and never silently applies a proposal. The
node stays blocked until the user explicitly commits calibration changes through the canonical
Workflow mutation.

The policy also validates facts available before submission. Seedance images must be a supported
managed Image MIME, under 30 MiB, and within the documented dimension and ratio bounds; reference
videos must be MP4 or QuickTime, 2..=15 seconds each, at most 15 seconds total, under 200 MiB each,
and satisfy the documented dimension and FPS bounds; reference audio must be MPEG or WAV, 2..=15
seconds each, at most 15 seconds total, and under 15 MiB each. A provider may still reject content
or facts unavailable locally. That failure identifies the exact stable input-item ID and safe
contract rule, never the provider's prose.

The Seedance adapter also measures the fully serialized request and rejects it before network I/O
when it would exceed the documented 64 MiB body limit, including data-URL expansion. It does not
estimate that bound from raw Asset sizes or allow the server to become the semantic validator.

Images and Audio can use the documented bounded data-URL forms. The Seedance create API accepts
reference Video only by provider-reachable URL or `asset://` identity, not by local bytes. The
current Task protocol has no durable materialization effect or child record, so every first-release
model contract omits `ReferenceVideo`. A persisted universal-contract draft using it remains valid
but readiness returns structured `InputMaterializationUnavailable`; admission and provider dispatch
are blocked. A later route may expose it only after one separate design freezes official upload or
query-by-identity evidence, durable identity/state/effects, expiry, restart recovery, and terminal
cleanup together. An adapter must never invent a data URL, local HTTP server, or unpersisted provider
reference to bypass that gate.

Draft-task promotion, callback URLs, provider priority, service tiers, web search, return-last-frame,
and provider task IDs are not node parameters in this iteration. `draft` means creation of a sample
video only; using that remote task as a later final-generation input requires a separate durable
provider-neutral contract.

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
| `video.general_generation@1` | General Video Generation | `video.generate@1.0` |
| `speech.multilingual_narration@1` | Multilingual Narration | `audio.synthesize_speech_from_text@1.0` |

The capability refs above must belong to the exact set in `BACKEND.md#active-node-capabilities`;
display text is not identity. Adding a profile, compatibility, or lifecycle state changes this
frozen catalog and is not configuration.

A profile is an immutable product promise, not an alias for today's native model.

- compatibility names exact capability versions;
- a route must satisfy the complete capability contract;
- a changed observable semantic requires a new profile or capability version;
- every route bound to the profile must preserve the same observable behavior;
- retired profiles remain as tombstones for saved Workflows;
- display names and native model strings are never parsed as identity.

Every active model-powered capability requires `generation_profile_ref` and `generation_model_id`
in its normalized parameter contract. A Workflow node stores no provider, native model, account,
endpoint, credential, route, configuration revision, availability snapshot, or provider task.

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

Profile compatibility is immutable catalog data. Model compatibility is an exact join over one
model revision's profile, request kind, family definition, capability contract, and immutable
connection revision. Model availability is a current structural observation over that exact pair:

```rust
pub enum GenerationModelAvailabilityState {
    Available,
    Unavailable {
        reason: GenerationModelUnavailableReason,
    },
}
```

Unavailable reasons are exactly `ModelDisabled`, `ModelRemoved`, `ConnectionDisabled`,
`ConnectionRemoved`, `CredentialMissing`, `IdentityUnverified`, `DebugModelDisabled`,
`ProfileIncompatible`, `KindIncompatible`, `FamilyRecoveryOnly`, `ProtocolUnavailable`,
`InputMaterializationUnavailable`, and `ConfigurationInvalid`.
`Available` means a saved model revision and its connection are enabled, structurally valid,
credential-backed, identity-proven, and resolve to one Selectable or permitted Deprecated family
and its shipped route, or an immutable Mock built-in is enabled by
the debug gate and resolves to its exact route. It is not a network-health, authentication,
quota, billing, or content-policy claim. Readiness and Settings never issue a paid generation or
invent capability support from a provider name. Those remote outcomes belong to the Generation
Task provider call and its structured failure semantics.

`GenerationModelAvailabilityObservation` contains the requested `GenerationModelRevisionRef`, its
exact connection revision ref, capability and profile refs, model state, and
`observed_at_epoch_ms`. It is a bounded
application projection and is never persisted in Workflow or Settings. A bulk request contains one
exact capability ref, one exact profile ref, 1..=64 unique model revision refs in ascending model-ID
order, and a process-monotonic deadline no more than five seconds away. It returns exactly one
observation per requested ref in request order.

`GenerationModelAvailabilityReaderInterface` is consumer-owned by the model-selection application
module. `GenerationProviderRegistryModelAvailabilityReaderAdapterImpl` joins immutable model and
connection revisions, exact credential-binding presence, identity evidence, debug built-ins when
enabled, and the shipped family/route registry in
one bounded read. It performs no network call, writes no cache, and never falls back to another
model. Mock models require no credential, exist only in debug/test composition, and are never
user-created production configurations.

`GenerationProfileListForCapabilityUseCase` returns only Active provider-independent profile
definitions. `GenerationModelListForCapabilityUseCase` accepts one exact registered capability and
one compatible Active profile plus an optional currently selected model ID. It returns every
compatible non-Removed saved choice, debug built-ins only when gated on, and the selected saved
record even when disabled, removed, or now incompatible so an existing node remains explainable.
Items are ordered by Unicode-case-folded display
name and then model ID.
Each item exposes only stable model ID, revision, display name, creator-facing connection/service name,
profile, kind, and availability. It exposes no endpoint, native model ID, credential identifier,
token, route ID, implementation type, price, default flag, or fallback order.

Workflow readiness checks only the node-selected stable model ID against its selected profile and
capability. Missing, disabled, removed, incompatible, credential-less, or unresolvable selection
blocks admission with a structured model issue. Run admission repeats the authoritative current
resolution under the admitted Workflow revision and freezes the exact model revision in the
execution plan. Task creation resolves only that frozen revision; recovery uses only the Task's
persisted target and exact credential binding. No current Settings row or alternative model or connection may
replace it.

## Frozen MVP Generation Task Provider Contracts

[`BACKEND_TASK.md`](BACKEND_TASK.md#9-provider-contracts) owns the public submit, poll, and cancel
interfaces. This document owns their provider translation. The closed task request variants carry
provider-neutral Text/Image/Video/Voice semantics. The current three active Node Capabilities use
the last three rows; Text activates through the same registry when its capability/profile contract
is admitted:

| Task request | Semantic fields after profile/origin | Required output |
| --- | --- | --- |
| `Text` | bounded prompt and profile-owned text controls | one Text value |
| `Image(TextToImage)` | `prompt: WorkflowTextValue`, calibrated `aspect_ratio`, and exact image-contract ref | one image |
| `Video(GenerateVideo)` | explicit input mode, optional/required prompt, ordered role-bearing Image/Video/Audio snapshots, and calibrated Video parameters | one video |
| `Voice(TextToSpeech)` | `text: WorkflowTextValue` | one Audio Asset |

The immutable task target supplies the exact profile, model revision, connection revision,
credential-binding ref, provider, Endpoint/native-identity snapshot, and route reference while
keeping every secret outside the Task.
The stable `GenerationTaskId` becomes the native submission idempotency key where supported.
Media outputs retain the frozen MIME and size limits: PNG up to 32 MiB, MP4 up to 512 MiB, and
MPEG up to 64 MiB. Zero bytes, a second primary output, unknown length, mismatched facts, trailing
bytes, or digest mismatch is `InvalidResponse`.

## Provider Capability Composition And Private Route

One provider-level adapter implements `GenerationProviderInterface` and assembles its complete
focused capabilities. The debug-gated deterministic composition registers one Mock provider that
contributes Image, Video, and Voice without pretending to implement Text:

```rust
pub struct MockGenerationProviderAdapterImpl {
    provider_id: GenerationProviderId,
    display_name: GenerationProviderDisplayName,
    capabilities: GenerationProviderCapabilities,
}

fn mock_generate_video_execution(
    route: Arc<MockGenerateVideoProviderRouteImpl>,
) -> VideoGenerationProviderExecution {
    VideoGenerationProviderExecution::Remote {
        submitter: route.clone(),
        poller: route,
    }
}
```

This is one adapter-private route composition, not a second public contract. Mock routes implement
the same complete task-owned contracts as production routes while performing no network,
filesystem, credential, or vendor DTO work.

The composed provider registry resolves `GenerationProfileRef` only through its exact
provider/type/route entry. The
routed request retains every other semantic field. One route maps exactly one profile semantic
contract and cannot branch on another profile or return `Unsupported`.

Provider and route IDs use immutable lower-case dot/hyphen segments and are never repurposed. When
the debug gate is enabled, the deterministic composition map is exact:

| Profile ref | Provider ID | Route ID | Implementation |
| --- | --- | --- | --- |
| `image.high_quality_general@1` | `mock` | `mock.image.high-quality-general.v1` | `MockTextToImageProviderRouteImpl` |
| `video.general_generation@1` | `mock` | `mock.video.general-generation.v1` | `MockGenerateVideoProviderRouteImpl` |
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

The production target defines these protocol routes. Composition registers a row only after its
exact source fixture, implementation, and route contract suite pass:

| Protocol | Provider ID | Route ID | Execution composition |
| --- | --- | --- | --- |
| `OpenAiImageGeneration` | `openai` | `openai.image.generations.v1` | Image `Immediate` |
| `VolcengineArkSeedream` | `volcengine.ark` | `volcengine.ark.seedream.v1` | Image `Immediate` through the exact Seedream MVP contract |
| `VolcengineArkSeedance` | `volcengine.ark` | `volcengine.ark.seedance.v1` | Video `CancellableAndDeletableRemote` create/query/queued-cancel/terminal-delete; list is private diagnostics only |
| `VolcengineAgentPlanVisual` | `volcengine.agent-plan` | `volcengine.agent-plan.visual.v1` | Exact Image or Video composition declared by each source-fixtured family; never the standard Ark route |
| `VolcengineAgentPlanTts` | `volcengine.agent-plan` | `volcengine.agent-plan.tts-http.v1` | Voice `Immediate` over HTTP chunked response |

Each registered production route has typed request/response DTOs, authentication,
untrusted-response validation, the production policy below, and fixture-backed contract tests. A family without an
accepted exact execution fixture remains unavailable rather than letting implementation choose
Immediate versus Remote. Adding a source-fixtured family changes no Workflow, Asset, or focused
provider interface.

Provider construction rejects an empty capability product and any route ID reused across its
focused capabilities. Model-configuration validation rejects unknown protocols, incompatible
profile/kind pairs, and protocols whose shipped route does not advertise that exact compatibility.
Contract conformance is proved in tests, not represented as runtime configuration. An unregistered
route cannot produce a selectable model.

Composition maintains two distinct structures: the mutable revisioned connection/model catalog
used for new Run admission and the immutable shipped family/provider registry used for task
dispatch and recovery. Disabling, removing, or revising a model affects only later Run admission.
Recovery first resolves the provider ID, request kind, and route ID copied into the Task target; it
never consults a current model/connection revision or substitutes another configuration. A software version
may remove a shipped recovery route only through an explicit retirement decision that first proves
no non-terminal Run or Task references it.

## Dispatch Rules

```text
saved GenerationProfileRef + GenerationModelId
  -> current compatible enabled model revision at Run admission
  -> durable Workflow Run and WorkflowNodeExecutionId
  -> frozen GenerationModelRevisionRef enters the execution plan
  -> Generation Task persists immutable request and exact non-secret route target
  -> task records Submitting and calls create once
  -> task persists only the remote ID returned directly by that validated create response
  -> task polls by the persisted ID
  -> task finalizes media through the Asset boundary
  -> task notification lets Workflow commit node output and state
  -> task outbox deletes the remote record after any terminal outcome when the route declares deletion
```

The selected `GenerationProviderRouteId` is fixed inside one active node execution before paid
submission. Runtime never switches routes during that execution. An ambiguous submission fails
with a structured category and is never automatically repeated. Selecting among multiple saved
models is current scope; automatic routing and failover are not.

Before every Seedance create, the Task worker durably commits `Submitting` and a stable diagnostic
client request ID, then calls create once. The remote handle may be attached only from that Task's
direct validated create response. A lost or otherwise uncertain response consumes `SubmitTask`,
commits `AmbiguousSubmission`, and is never repeated.

Seedance list output does not echo a proved idempotency or local ownership key. Timestamps, request
fingerprints, model IDs, and pseudonymous terminal-user IDs therefore cannot distinguish this
client's work from an externally-created identical task. The list operation may remain private to
the adapter for support diagnostics, but it is not a Task interface and can never attach a handle or
authorize query, cancellation, deletion, download, or result publication. A future reconciliation
protocol requires separately reviewed provider ownership proof and a new Task-owned interface.

Accepted remote handles are persisted only inside `GenerationTaskAggregate` and its SQLite row.
They never enter Workflow, Asset, Assistant, Generation Profile, ordinary logs, or Asset
provenance or public DTOs. On restart, the task worker queries the same provider operation by that
ID and the waiting Workflow node remains non-terminal.

Remote cancellation uses the separate complete `GenerationCancellerInterface` when the adapter
implements it. Lack of remote cancellation never becomes an optional method or `Unsupported`
result; local task and Workflow cancellation remain deterministic while external work or charges
may continue. Seedance adapts DELETE of a `queued` task as cancellation. A queued-to-running race
returns `TooLateRunning`; it is a normal documented outcome, never `Unsupported`.

Remote task-record deletion uses the separate complete
`GenerationRemoteTaskDeleterInterface`. It is a post-terminal cleanup operation driven by the Task
outbox and never deletes local Task history, Workflow output, or the managed Asset. Seedance uses
`DELETE /api/v3/contents/generations/tasks/{task_id}` with these official state semantics:

| Observed Seedance state | DELETE meaning |
| --- | --- |
| `queued` | cancel queueing; the task becomes `cancelled` |
| `running` | deletion/cancellation is too late and unsupported for that state |
| `succeeded`, `failed`, or `expired` | delete the remote task record |
| `cancelled` | do not DELETE; the provider removes the record automatically after 24 hours |

The route translates those facts into the complete canceller and deleter interfaces. If local
cancellation sees `TooLateRunning`, the local Task still converges to `Cancelled`, query by the
already persisted handle continues in control-only mode without attaching a late result, and
terminal observation later enqueues remote deletion. Provider-confirmed queued cancellation needs
no deletion.

## Route Responsibilities

Each concrete route:

- maps every semantic field to one private vendor DTO;
- uses a typed native model ID rather than parsing profile/display names;
- sends explicit values when native defaults could change promised behavior;
- validates the frozen response shape, media kind, size, and MIME, plus reported model or checksum
  only when that exact vendor response contract supplies the field;
- freezes one preview-compatible output container/codec contract for each creator-visible media
  route and rejects inspected output that cannot issue the required Asset preview representation;
- bounds submission, polling, redirects, downloads, deadlines, and response sizes;
- maps provider status exactly once into task-owned normalized outcomes;
- keeps credentials, raw bodies, signed URLs, and route details private;
- returns only a validated opaque handle for durable task persistence.

Production policy is business-visible through terminal timing and is therefore fixed, not an adapter
choice:

| Policy profile | Task deadline | Per-call deadline | Poll schedule | Bounded response/output |
| --- | --- | --- | --- | --- |
| `ImageImmediate180` | 180 s | 120 s including media retrieval | none | JSON 2 MiB; decoded Image 32 MiB |
| `VideoRemote1800` | 30 min | submit 30 s; query 15 s; media retrieval 120 s | 1 s through 30 s, 2 s through 5 min, then 5 s | create body 64 MiB; JSON 2 MiB; decoded Video 512 MiB |
| `SpeechImmediate120` | 120 s | 90 s for the complete chunked call | none | one JSON chunk 2 MiB; total wire 96 MiB; decoded Audio 64 MiB |

OpenAI Images and every active Seedream Image family use `ImageImmediate180`. Seedance uses
`VideoRemote1800`. Agent Plan speech uses `SpeechImmediate120`. Each Agent Plan visual family must
select the matching Image or Video profile in its accepted route fixture before becoming
Selectable. Retry-After may delay only within the active schedule and remaining Task deadline.

After Seedance returns `TooLateRunning`, the Task sets `remote_cleanup_deadline_at` to exactly
`task_created_at + 7 days`, rejecting timestamp overflow as corruption. Control-only query uses a
fixed 30-second cadence for the first 30 minutes after cancellation and a 5-minute cadence
thereafter. It stops at provider-terminal evidence or that durable deadline, never downloads or
attaches a result, and survives restart without consulting current Settings or provider inventory.

Seedance list is `GET /api/v3/contents/generations/tasks`. A private diagnostic client may page `page_num` and
`page_size` within `1..=500`, uses exact Endpoint-ID `filter.model`, repeated
`filter.task_ids`, status and service-tier filters only as documented, and treats seven days as the
provider inventory retention horizon. Its observations never enter Task business code or authorize
state transitions. List does not return prompt or a proved client ownership token.

Seedance request `safety_identifier` is one fixed privacy-safe pseudonymous identifier per terminal
user, not a per-task idempotency value. `X-Client-Request-Id` is persisted and sent for diagnostic
correlation only; current evidence does not prove it is idempotent or returned by list. Query/list
records are provider-retained for seven days and returned output URLs expire after 24 hours. The
30-minute Task deadline is intentionally shorter; a completed URL is downloaded immediately under
the remaining budget and never persisted. Expiry after the Task deadline cannot reopen or extend a
terminal Task. Control-only cleanup may continue only for a handle already persisted from a direct
create response.

Every production route fixture freezes an exact HTTPS media-host allowlist. Wildcard public suffixes,
credentials in URLs, redirects, non-public DNS results, loopback/link-local/private targets, host
changes after DNS resolution, and hosts absent from that route's fixture are rejected. A route
never resubmits after ambiguous acceptance. Recovery polls only a handle durably committed with
`Running`; an uncertain create fails `AmbiguousSubmission` and never causes another create or list
attachment. The first-release Task protocol has no provider-side input materialization path.

Remote media is downloaded and validated inside the route. The route returns only the Task-owned,
non-cloneable `GenerationTaskOutputSourceLease` over an already-open bounded stream; it never returns
a URL/path, buffers the complete output in business code, or creates an Asset. Generation Task
finalization moves that lease into its Asset sink boundary.

## Credentials And Configuration

Generation Settings owns reusable Provider Connections plus independently named Generation Models;
it never owns a single active binding per profile/kind. Multiple models may share one connection,
while OpenAI, Ark Standard, Agent Plan visual, and Agent Plan speech remain distinct connection
families. The node chooses only a stable model ID.

Each service family and model family has a closed typed configuration variant. There is no generic
provider-options JSON, arbitrary header map, caller-authored operation path, global default model,
priority, weight, or fallback chain. API tokens use
`GenerationProviderCredentialRepositoryInterface`, keyed by
`GenerationProviderCredentialBindingId` and loaded only immediately before an authenticated call.
Task targets store the binding ID required for exact recovery but no secret bytes. Exact atomic
storage and retention rules are owned by `BACKEND_STORAGE.md`.

## Failure Semantics

Profile failures are `GenerationProfileNotFound` and `GenerationProfileIncompatible`. Model
selection failures are `GenerationModelNotFound`, `GenerationModelRevisionNotFound`,
`GenerationModelUnavailable`, `GenerationModelConfigurationConflict`, and
`GenerationSettingsRevisionConflict`. None contains provider response text or credential
material.

`GenerationProviderFailure` categories are exactly `InvalidSemanticRequest`,
`AuthenticationFailed`, `PermissionDenied`, `ContentPolicyRejected`, `RateLimited`,
`ProviderUnavailable`, `DeadlineExceeded`, `ProviderRejected`, `InvalidResponse`,
`DownloadRejected`, and `AmbiguousSubmission`. Every `GenerationProviderFailure` is a terminal
generation outcome, including `RateLimited` and `ProviderUnavailable`; a retryable rate limit or
outage on a poll or cancellation call is instead a transient `GenerationProviderCallError`, which
reschedules the same accepted handle. No category permits automatic Immediate or Submit
re-execution. `AmbiguousSubmission` is synthesized immediately for an uncertain Immediate call, or
an uncertain Submit call. A provider adapter declares it only when its own
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

A permanent `GenerationProviderCallError` on a poll, cancellation, or deletion call maps to
`ProviderUnavailable`; an uncertain Immediate call maps to `AmbiguousSubmission`, while an
uncertain Submit call maps immediately to `AmbiguousSubmission` without repeating create;
worker-observed deadline expiry maps to `Timeout`. `InputAssetUnavailable`, `OutputAssetImport`,
and `Internal` are task-internal kinds with no provider origin.

No database transaction remains open during any provider call. The Generation Task application
translates one provider outcome into an aggregate transition; its Workflow notification bridge then
lets Workflow own the node/Run transition.

## Verification

- profile tests cover immutable identity, tombstones, and exact compatibility;
- model-selection tests cover stable ID resolution, immutable revision freeze, lifecycle,
  credential presence, exact profile/kind compatibility, ordering, and redaction;
- provider-composite tests reject empty capability products and duplicate route IDs and prove
  deterministic mechanically-derived safe contracts;
- focused capability tests prove one fixed selected model revision and provider/route per task and
  exact interface behavior;
- debug-gated Mock and production routes pass the same shared private route contract suite before registration;
- every registered route passes translation, malformed-response, deadline, and response-bound tests;
- Immediate routes additionally pass equivalent-result rules; Remote routes pass submit/poll,
  direct-handle ownership, response-loss ambiguity, restart, and repeatable terminal observation
  rules; cancellable/deletable routes run their exact state-race contract suites;
- architecture tests allow only node `GenerationModelId` and reject node provider/native-model,
  endpoint, credential, route, or revision fields; they also reject broad provider interfaces,
  provider-owned task semantics, roadmap runtime interfaces, and construction outside composition.
