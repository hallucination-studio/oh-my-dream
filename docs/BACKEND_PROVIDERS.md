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
`GenerationProviderCredentialVaultInterface`. It cannot be confused with Assistant model configuration.

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
  non-secret configuration, credential-vault adapters, construction
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

A profile is an immutable product promise, not an alias for today's native model.

- compatibility names exact capability versions;
- a route must satisfy the complete capability contract;
- a changed observable semantic requires a new profile or capability version;
- every route bound to the profile must preserve the same observable behavior;
- retired profiles remain as tombstones for saved Workflows;
- display names and native model strings are never parsed as identity.

Every active model-powered capability requires `generation_profile_ref` in its normalized parameter
contract. A Workflow node stores no provider, native model, account, endpoint, credential, route,
availability snapshot, or provider task.

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

`GenerationProfileAvailabilityReaderInterface` is consumer-owned by the profile application module.
`ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl` performs one bounded bulk
observation for one exact capability and profile set. It reads the same three router
implementations' profile-to-route maps and never maintains another mapping. It does not probe once
per UI row or persist availability.

`GenerationProfileListForCapabilityUseCase` joins definitions with current observations and returns
only provider-independent metadata. In the MVP, both `Unavailable` and `Indeterminate` prevent Run
admission. The router checks again at execution because availability can change after admission.
No route may silently substitute a different profile.

## Frozen MVP Provider Interfaces

[`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md#mvp-external-interfaces) is the only authority
for public interface signatures, requests, and results. Provider infrastructure implements exactly its
three MVP interfaces: `TextToImageProviderInterface`, `ImageToVideoProviderInterface`, and
`TextToSpeechProviderInterface`.

Their requests carry `GenerationProfileRef` and `WorkflowNodeExecutionId`; the execution ID becomes
the native submission idempotency key where supported. Roadmap interface names are reserved by the
capability document but do not exist in the MVP runtime.

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
- validates response shape, reported model, media kind, size, MIME, and checksums;
- bounds submission, polling, redirects, downloads, deadlines, and response sizes;
- maps provider status exactly once into `NodeCapabilityProviderFailure`;
- keeps credentials, raw bodies, signed URLs, remote handles, and route details private.

Remote media is downloaded and validated inside the route. The route returns a semantic payload but
never creates an Asset; the capability owns the call to the Asset-write boundary.

## Credentials And Configuration

Non-secret configuration declares enabled accounts, profile-to-route entries, route endpoints,
timeouts, and polling bounds. It contains only `GenerationProviderCredentialId` references.

`GenerationProviderCredentialVaultInterface` is owned by the Desktop provider-configuration consumer.
Production adapters use the operating-system credential facility. Plaintext exists only in one
short-lived `GenerationProviderCredentialSecret` and never enters SQLite, config files, DTOs,
domain objects, errors, or logs. There is no plaintext or embedded-key fallback.

Only `DesktopCompositionRoot` loads credentials and constructs routes. A missing or inaccessible
credential makes affected profiles unavailable without preventing application startup.

## Failure Semantics

Profile failures are `GenerationProfileNotFound`, `GenerationProfileIncompatible`,
`GenerationProfileUnavailable`, and `GenerationProfileAvailabilityIndeterminate`.

`NodeCapabilityProviderFailure` uses closed categories for invalid semantic request, authentication,
permission, content policy, rate limit, provider unavailable, timeout, provider rejection, invalid
response, download rejection, and ambiguous submission. It carries safe retryability and optional
retry time. Provider strings never determine Workflow state.

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
- credential tests prove OS-vault round trip, denial handling, no persisted plaintext, and missing-
  credential availability behavior;
- architecture tests reject node provider/model fields, broad provider interfaces, roadmap runtime
  interfaces, removed binding/task layers, and construction outside composition.
