# Backend Generation Profiles And Provider Adapters

> Status: proposed target architecture
> Owner: profile semantics in `crates/nodes`; provider infrastructure in `crates/backends`;
> concrete wiring in `src-tauri`
> Scope: model selection, compatibility, availability, routing, and external generation

Naming follows [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md). Users select a stable generation
profile. Provider accounts, native model IDs, and routes remain replaceable infrastructure.

## Decision

1. Every external-generation node persists an exact, provider-independent
   `GenerationProfileRef`.
2. The generation-profile catalog owns profile identity, lifecycle, and compatibility with exact
   `NodeCapabilityContractRef` values.
3. Compatibility means full support for the exact capability contract. Profiles and adapters do
   not redefine or narrow node parameter semantics.
4. Current availability is queried separately and never persisted in a Workflow.
5. The composition root installs a capability router with zero or more equivalent provider
   bindings for each compatible profile. It does not select one global model per capability.
6. Only a capability-specific `*ProviderRouterAdapter` implements the public provider port.
   Concrete `*ProviderBinding` values use a private, already-bound executor and cannot return
   `Unsupported` for profiles.
7. Concrete bindings explicitly translate semantic inputs to private native model IDs and DTOs.
   Provider discovery never publishes an uncurated native model as a product profile.

This separates durable user intent from provider topology without moving provider selection into
business code.

## Semantic Ownership

| Concept | Authoritative owner |
| --- | --- |
| profile identity, version, lifecycle, and compatibility | generation-profile domain in `crates/nodes` |
| node parameter meaning and validation | exact node capability in `crates/nodes` |
| availability states and reason semantics | generation-profile domain in `crates/nodes` |
| current availability observation | `GenerationProfileProviderBindingAvailabilityReaderAdapter` |
| profile picker projection | generation-profile application layer |
| profile-to-native binding and wire translation | concrete binding in `crates/backends` |
| provider settings, route policy, and concrete wiring | `src-tauri` composition root |
| encrypted provider credential persistence | `DesktopProviderCredentialRepositoryPort` implemented by `SqliteEncryptedProviderCredentialRepositoryAdapter` |
| task, polling, transport, and artifact protocol | concrete provider binding |

No DTO, provider response, or React component may become a second source of these semantics.

## Provider Naming Rules

Provider names encode capability, responsibility, and architectural role:

| Term | Exact meaning | Example |
| --- | --- | --- |
| `GenerationProfile` | durable provider-independent user selection | `GenerationProfileRef` |
| `ProviderBinding` | static mapping from one profile to one provider model executor | `FalTextToImageProviderBinding` |
| `ProviderRoute` | one dispatch-time choice of a binding | `TextToImageProviderRouteAssignment` |
| `ProviderRouter` | composite adapter that selects a route and implements a public provider port | `TextToImageProviderRouterAdapter` |
| `ProviderBindingExecutor` | private executor for one already-selected binding | `TextToImageProviderBindingExecutor` |
| `AvailabilityReader` | consumer port that reads expiring availability observations | `GenerationProfileAvailabilityReaderPort` |
| `AvailabilityObservation` | one time-bounded availability result | `GenerationProfileAvailabilityObservation` |

Avoid standalone `Provider`, `Router`, `Binding`, `Model`, `Executor`, `Registry`, or `Availability`
type names. Concrete names include the generation capability; provider-specific names also include
the provider, such as `FalTextToImageProviderBinding`.

## Dependency Direction

```text
crates/engine
  NodeCapabilityContractRef and opaque NodeCapabilityParameterSet
             ^
crates/nodes
  generation-profile domain, catalog, compatible-profile query, availability reader port
  TextToImageProviderPort / ImageToVideoProviderPort / TextToAudioProviderPort
             ^
crates/backends
  provider router adapters, private binding executors, provider DTOs, availability probes
             ^
src-tauri
  configuration, Tauri DTO translation, concrete construction and registration
```

```text
crates/nodes/src/
  generation_profile/{domain,application,ports}
  node_capability/{domain,application,ports}

crates/backends/src/
  generation_provider_routing/{text_to_image,image_to_video,text_to_audio}.rs
  generation_provider_routing/generation_profile_availability_reader.rs
  deterministic_generation_provider/{text_to_image,image_to_video,text_to_audio}.rs
  <provider_name>/
    shared/{account_authentication,http_transport,async_task_polling}.rs
    shared/{generated_media_download,generation_model_availability_probe,provider_error_translation}.rs
    <generation_capability>/{provider_binding,binding_executor,request_dto,response_dto}.rs

src-tauri/src/
  generation_profiles/{commands,dto,translation}.rs
  generation_providers/{configuration,credentials}.rs
  composition.rs
```

Business code depends only on consumer-owned ports. The composition root selects a complete router
as the concrete adapter and injects its eligible bindings and policy. Choosing a binding inside that
router is private infrastructure behavior, not business dependency selection.

## Stable Profile Identity

```rust
pub struct GenerationProfileRef {
    pub id: GenerationProfileId,
    pub version: GenerationProfileVersion,
}

pub struct GenerationProfileDefinition {
    pub generation_profile_ref: GenerationProfileRef,
    pub lifecycle_state: GenerationProfileLifecycleState,
    pub compatible_node_capability_contracts: BTreeSet<NodeCapabilityContractRef>,
}
```

A profile is a product-level generation promise, not an alias for the native model configured
today.

- An exact `(id, version)` is immutable and never rebound.
- Compatibility names exact capability versions, not UI shells or media labels.
- A compatible profile must honor the capability's complete parameter and output contract.
- If a model needs different parameter bounds or behavior, define another capability contract
  version and profile compatibility entry instead of adding profile-owned parameter rules.
- A native model may replace an existing binding only when conformance proves the same promise.
- Retired profiles remain as tombstones so saved Workflows can explain why they cannot run.
- Display names are application metadata and are never parsed for business meaning.

Different native models use different profile refs unless the product deliberately guarantees
their behavioral equivalence. Equivalent bindings may differ in latency, region, or provider, but
not in semantics visible to the Workflow.

## Stable Selection In Nodes

Each external-generation capability declares a required `generation_profile_ref` parameter:

```text
image.text_to_image@<version>  generation_profile_ref, aspect_ratio, optional seed
video.image_to_video@<version> generation_profile_ref, duration, aspect_ratio
audio.text_to_audio@<version>  generation_profile_ref, voice_profile, speed
```

The field belongs in the first released contract of this target architecture. Adding it to an
already-released contract requires a new capability version.

`NodeCapabilityParameterSet` persists only the exact profile ref. It never stores a provider name,
native model ID, account, endpoint, route, availability snapshot, task ID, or provider options map.

Node mutation validates known identity and static compatibility, but not current availability. An
offline Workflow remains editable and portable. Readiness and Run admission separately report
availability. Selecting another profile is always an explicit Workflow mutation; routing never
changes the selected profile.

## Availability And Query

```rust
pub enum GenerationProfileAvailabilityState {
    Available,
    Unavailable {
        reason: GenerationProfileUnavailableReason,
        retry_after: Option<GenerationProfileAvailabilityRetryAfterValue>,
    },
    Indeterminate {
        reason: GenerationProfileAvailabilityIndeterminateReason,
    },
}

pub struct GenerationProfileAvailabilityObservation {
    pub generation_profile_ref: GenerationProfileRef,
    pub node_capability_contract_ref: NodeCapabilityContractRef,
    pub availability: GenerationProfileAvailabilityState,
    pub observed_at: GenerationProfileAvailabilityObservedAtValue,
    pub expires_at: GenerationProfileAvailabilityExpiresAtValue,
}
```

Unavailable reasons include `NoConfiguredProviderBinding`, `AuthenticationRequired`,
`PolicyBlocked`, `QuotaUnavailable`, `RateLimited`, `ProviderUnavailable`, and
`ProviderModelUnavailable`.
`ProbeTimedOut` and `Offline` are indeterminate rather than false claims of unavailability.

`GenerationProfileAvailabilityReaderPort` is owned by the generation-profile application
capability. `GenerationProfileProviderBindingAvailabilityReaderAdapter` implements it over the
composition-built provider binding registries. Its `read_generation_profile_availability` method
accepts an exact capability ref and a bounded set of profile refs, then returns one bulk observation
set. Callers never probe once per profile. Profile lifecycle and compatibility still come from the
authoritative catalog.

```text
list_node_capability_generation_profiles(
  node_capability_contract_ref,
  include_unavailable,
  after?,
  limit
) -> {
  profiles: NodeCapabilityGenerationProfileDto[],
  next_cursor?
}
```

`ListNodeCapabilityGenerationProfilesUseCase` joins catalog definitions with availability
observations and returns `NodeCapabilityGenerationProfileView` values. Each item contains exact
refs, bounded localized metadata, lifecycle, and structured availability with its observation and
expiry. Results are ordered by exact profile ID and version; the opaque cursor contains both, and
`limit` is bounded from 1 to 100. Items contain no provider, native model, endpoint, credential
detail, raw provider result, or route priority.

React and Assistant discovery consume the query. Readiness and execution preparation reuse the
catalog and availability reader port directly; they do not call the UI query. Availability expires
and is advisory, so execution rechecks it. A race returns `GenerationProfileUnavailable` without
silently substituting another profile.

Provider discovery only verifies curated bindings. A discovered native model becomes selectable
only after it has a versioned profile, explicit compatibility, a binding, and conformance tests.

## Public Execution Ports

Capability-owned requests include the stable selection alongside semantic inputs:

```rust
pub struct TextToImageProviderRequest {
    pub generation_profile_ref: GenerationProfileRef,
    pub prompt: WorkflowTextValue,
    pub aspect_ratio: NodeCapabilityImageAspectRatioValue,
    pub seed: Option<NodeCapabilityImageGenerationSeedValue>,
    pub workflow_node_dispatch_id: WorkflowNodeDispatchId,
}
```

`ImageToVideoProviderRequest` additionally carries readable source image, optional prompt,
duration, and aspect ratio. `TextToAudioProviderRequest` carries text, voice profile, and speed. All
three use the explicit `generation_profile_ref` and `workflow_node_dispatch_id` fields and contain
no provider DTO or options map.

The complete public ports remain:

```text
TextToImageProviderPort
ImageToVideoProviderPort
TextToAudioProviderPort
```

There is no broad `Provider`, optional operation, `supports_*` probe, or generic execute interface.
For each exact capability contract, the composition root injects
`TextToImageProviderRouterAdapter`, `ImageToVideoProviderRouterAdapter`, or
`TextToAudioProviderRouterAdapter` behind its matching public port.

## Private Binding Boundary

Each capability router owns a precisely named binding registry, such as
`TextToImageProviderBindingRegistry`, which maps an exact `(capability, profile)` pair to ordered
equivalent bindings:

```text
exact capability + GenerationProfileRef
  -> eligible bound executors
  -> current availability and route policy
  -> one dispatch-scoped route assignment
```

A concrete binding does not implement the public profile-routing port. It implements a crate-private
consumer-owned protocol such as `TextToImageProviderBindingExecutor`, whose input has already been
resolved to that binding and therefore has no profile-selection branch.

```rust
struct FalTextToImageProviderBinding<E> {
    generation_profile_ref: GenerationProfileRef,
    provider_model_id: FalTextToImageModelId,
    request_translation_revision: FalTextToImageRequestTranslationRevision,
    binding_executor: E,
}
```

Registry construction rejects unknown or incompatible refs, incomplete executors, duplicate
binding identity, ambiguous priority, invalid endpoint policy, and bindings that fail the complete
capability conformance suite. Unsupported operations are represented by absence of a binding, not a
method that fails at runtime.

Routing may use configured priority, region, policy, and health. A route is fixed for one
`WorkflowNodeDispatchId` before paid submission. The router may switch equivalent bindings only
before provider acceptance; it never switches after an accepted or ambiguous submission. A user
retry creates a new dispatch but retains the selected profile.

## Provider Translation

Each bound executor owns explicit translation from capability semantics to its private protocol:

- map every semantic field to a private provider DTO;
- use the binding's typed native model ID rather than parsing profile IDs or display names;
- reject incomplete mappings during composition;
- send explicit values when native defaults could change observable semantics;
- validate every third-party response and returned native model identity;
- map private statuses and errors once into stable consumer-owned categories;
- retain binding and translation revisions only in restricted infrastructure audit records.

A native model upgrade keeps the same profile binding only after conformance proves the immutable
profile promise. Otherwise it requires a new `GenerationProfileRef`.

## Durable Dispatch And Result

```text
saved GenerationProfileRef
  -> static compatibility and current readiness
  -> persist frozen Run and node executions
  -> commit
  -> DesktopWorkflowRunTaskHost starts the process-owned task
  -> router rechecks availability and fixes one dispatch-scoped route
  -> bound executor translates, submits, polls, downloads, and validates
  -> persist generated Asset
  -> complete node execution
```

No transaction remains open during network work. The frozen Run contains the semantic profile; the
route and provider task remain active-process state. Restart marks the Run interrupted instead of
reattaching. If a submission is ambiguous and the provider has no idempotency lookup, fail instead
of blindly resubmitting paid work.

All three ports return `NodeCapabilityGeneratedMediaPayload`. Remote URLs are validated and
downloaded within bounds; no URL or path crosses the port. The binding never creates an Asset. The
node executor stores the payload before publishing output, and Asset provenance records an
Asset-owned profile ref rather than provider-native state.

Every router and bound executor preserves the same idempotency, progress, deadline, terminal,
cancellation, retry, download, and structured-error semantics. Provider strings never determine
Workflow state.

## Configuration, Errors, And Security

Adapters receive validated accounts, short-lived `ProviderCredentialSecretValue` values, immutable
bindings, transport, clock, polling, artifact, and probe policies through constructors.
`DesktopBackendConfig` enables
accounts, regions, routing policy, and bounds; it does not choose one global model per capability.
Only `src-tauri/composition.rs` constructs routers and concrete bindings.

Profile and routing errors are `GenerationProfileNotFound`,
`GenerationProfileIncompatibleWithNodeCapability`, `GenerationProfileUnavailable`, and
`GenerationProfileAvailabilityIndeterminate`. Missing configuration is represented by
`GenerationProfileUnavailable { reason: NoConfiguredProviderBinding }`, not a second route error.
Provider errors retain stable categories for authentication, rate limit, invalid request/output,
provider/download failure, timeout, cancellation, and execution failure. Retryability and safe
retry time are structured.

Credentials, bodies, signed URLs, native model IDs, tasks, and route details never enter public
errors or ordinary logs. Endpoints and redirects are validated; requests, responses, deadlines,
polling, probes, and artifacts are bounded; downloaded bytes are sniffed. External responses are
untrusted until validated. Provider-native identifiers appear only in restricted audit records.

Deterministic bindings use the same catalog, availability, router, bound-executor contracts, and
failure semantics as production.

## Verification

- Catalog tests reject rebinding, invalid lifecycle, and ambiguous compatibility.
- Capability tests prove typed profile persistence and full-contract compatibility.
- Query tests cover pagination, expiry, unavailable/indeterminate states, and redaction.
- Registry tests reject unknown, incompatible, incomplete, and ambiguous bindings.
- Routing tests prove equivalent-only candidates and fixed dispatch assignment.
- Public port contract suites run against deterministic and configured routers.
- Bound-executor conformance tests cover every provider/profile/capability binding.
- Translation and fault tests cover semantic fields, native mapping, discovery, polling,
  cancellation, idempotency, downloads, malformed responses, and availability races.
- Architecture tests reject provider-native node fields and construction outside composition.
- Credentialed tests remain behind a separate explicit gate.

## Rejected Alternatives

- **One startup model per capability:** cannot express stable per-node intent.
- **Provider/native ID in nodes:** leaks topology and makes saved Workflows non-portable.
- **Profile-owned parameter constraints:** duplicate capability semantics and split validation.
- **Concrete bindings implementing the routing port:** forces unsupported-profile behavior.
- **Live discovery as catalog:** third-party changes have no stable identity or semantics.
- **Adapter `supports_*` flags:** move compatibility into incomplete implementations.
- **Fallback to another profile:** silently changes user intent. Routing may change only among
  bindings conforming to the same exact profile.

## Consequences And Deferred Work

This requires a curated versioned catalog and conformance suite for every binding. In return, model
selection is stable, availability is queryable, provider replacement is transparent, and each
interface has behaviorally complete implementations.

Remote task recovery, cost accounting, user-defined provider accounts, explicit provider choice,
batch output, multiview, reference generation, text generation, text-to-video, concat, and
plugin-supplied profiles require separate designs. Future provider choice remains a separate policy
and never replaces `GenerationProfileRef` with a native ID. 3D and scenes remain outside scope.
