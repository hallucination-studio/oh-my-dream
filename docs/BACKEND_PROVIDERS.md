# Backend Generation Profiles And Providers

> Status: proposed target architecture
> Owner: profile semantics in `crates/nodes`; provider routes in `crates/backends`; wiring in `src-tauri`
> Scope: model selection, availability, routing, and provider translation

Users select a stable generation profile. Provider accounts, native models, and routing remain
replaceable infrastructure.

## Decision

1. Every model-powered capability persists one provider-independent `GenerationProfileRef`.
2. The profile catalog owns identity, lifecycle, and compatibility with exact
   `NodeCapabilityContractRef` values.
3. Each capability owns one focused provider port such as `TextToVideoProviderPort`.
4. One capability-specific router implements that public port and selects a configured route.
5. One concrete route owns profile mapping, native model ID, request translation, provider calls,
   polling, download, and response validation.
6. The composition root constructs routers and routes. Business code never selects a provider.

This is the complete provider stack:

```text
TextToVideoCapability
  -> TextToVideoProviderPort
  -> TextToVideoProviderRouterAdapter
  -> FalTextToVideoProviderRoute
  -> provider API
```

There is no separate binding object, binding registry, binding executor, or provider-wide feature
interface. The router's route map is enough.

## Naming

| Role | Pattern | Example |
| --- | --- | --- |
| public semantic interface | `<Input>To<Output>ProviderPort` | `TextToVideoProviderPort` |
| semantic request | `<Input>To<Output>ProviderRequest` | `TextToVideoProviderRequest` |
| profile router | `<Input>To<Output>ProviderRouterAdapter` | `TextToVideoProviderRouterAdapter` |
| private route interface | `<Input>To<Output>ProviderRoutePort` | `TextToVideoProviderRoutePort` |
| concrete provider route | `<Vendor><Input>To<Output>ProviderRoute` | `FalTextToVideoProviderRoute` |
| deterministic route | `Deterministic<Input>To<Output>ProviderRoute` | `DeterministicTextToVideoProviderRoute` |

The public method states the action, for example `generate_video_from_text`. Type names identify the
input, output, and role without repeating `GenerationFrom`, `Binding`, and `Executor`.

Avoid standalone `Provider`, `Route`, `Model`, `Client`, `Executor`, or `Registry` names. Operation
names remain explicit, including `FirstAndLastFramesToVideoProviderPort`,
`TextToSpeechProviderPort`, and `VideoStoryboardProviderPort`.

## Dependency Direction

```text
crates/engine
  WorkflowNodeCapabilityPort and NodeCapabilityContractRef
             ^
crates/nodes
  exact capability implementations, profile catalog, provider ports
             ^
crates/backends
  provider routers, routes, private DTOs, availability probes
             ^
src-tauri
  configuration, credentials, construction, and registration
```

```text
crates/backends/src/
  provider_routing/<operation>_router.rs
  deterministic_provider/<operation>_route.rs
  <provider_name>/
    shared/{authentication,http,polling,download,error_translation}.rs
    <operation>/{route,request_dto,response_dto}.rs
```

Business code depends only on consumer-owned interfaces. Provider DTOs and native identifiers remain
private to their route.

## Stable Profile

```rust
pub struct GenerationProfileRef {
    pub id: GenerationProfileId,
    pub version: GenerationProfileVersion,
}

pub struct GenerationProfileDefinition {
    pub generation_profile_ref: GenerationProfileRef,
    pub lifecycle_state: GenerationProfileLifecycleState,
    pub compatible_capabilities: BTreeSet<NodeCapabilityContractRef>,
}
```

A profile is an immutable product promise, not an alias for today's native model.

- Compatibility names exact capability versions.
- A compatible route must honor the complete capability contract.
- Different observable semantics require a new profile or capability version.
- Equivalent routes may differ in provider, region, latency, or price, but not behavior.
- Retired profiles remain as tombstones for saved Workflows.
- Display names are never parsed for identity or behavior.

Every model-powered capability declares a required `generation_profile_ref` parameter. A node stores
no provider name, native model, account, endpoint, route, availability snapshot, or provider task.
Editing validates profile identity and compatibility; Run readiness checks current availability.

## Availability

```rust
pub enum GenerationProfileAvailabilityState {
    Available,
    Unavailable {
        reason: GenerationProfileUnavailableReason,
        retry_after: Option<GenerationProfileRetryAfterValue>,
    },
    Indeterminate {
        reason: GenerationProfileAvailabilityIndeterminateReason,
    },
}
```

Unavailable reasons include `NoConfiguredProviderRoute`, `AuthenticationRequired`, `PolicyBlocked`,
`QuotaUnavailable`, `RateLimited`, `ProviderUnavailable`, and `ProviderModelUnavailable`. A timeout
or offline probe is `Indeterminate`, not a false claim that the profile is unavailable.

`GenerationProfileAvailabilityReaderPort` is owned by the profile application layer.
`GenerationProfileAvailabilityReaderAdapter` aggregates bounded, expiring observations from the
configured capability routers. One bulk read accepts an exact capability ref and a bounded profile
set; callers never probe once per profile.

`ListNodeCapabilityGenerationProfilesUseCase` joins catalog definitions with current observations.
It returns stable profile metadata, compatibility, lifecycle, and availability, but never provider,
model, endpoint, credential, or route details.

Availability is advisory and expires. Execution rechecks it. A race returns
`GenerationProfileUnavailable`; the router never substitutes another profile.

## Public Provider Ports

The authoritative port list and method signatures live in
[`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md#exact-external-interfaces). Representative ports
are:

```text
TextToImageProviderPort
ReferenceImagesToImageProviderPort
TextToVideoProviderPort
FirstAndLastFramesToVideoProviderPort
MixedMediaToVideoProviderPort
MultimodalToTextProviderPort
TextToSpeechProviderPort
TextToMusicProviderPort
VideoStoryboardProviderPort
```

Each exact request contains semantic inputs, typed parameters, `GenerationProfileRef`, and
`WorkflowNodeDispatchId`. Each port returns its exact result or `NodeCapabilityProviderFailure`.
There is no broad execute method, `supports_*` probe, provider DTO, or options map.

## Router And Route Interfaces

The public router resolves the profile and delegates to one private route:

```rust
pub struct TextToVideoProviderRouterAdapter {
    routes_by_profile:
        BTreeMap<GenerationProfileRef, Vec<Arc<dyn TextToVideoProviderRoutePort>>>,
    routing_policy: ProviderRoutingPolicy,
}

trait TextToVideoProviderRoutePort: Send + Sync {
    fn route_id(&self) -> ProviderRouteId;
    fn availability(&self) -> ProviderRouteAvailability;

    async fn generate_video_from_text(
        &self,
        request: RoutedTextToVideoRequest,
    ) -> Result<GeneratedVideoPayload, NodeCapabilityProviderFailure>;
}
```

The router removes `generation_profile_ref` only after selecting a matching route. The private routed
request retains every other semantic field. A concrete route cannot branch on another profile and
cannot return `Unsupported`.

```rust
struct FalTextToVideoProviderRoute {
    provider_route_id: ProviderRouteId,
    provider_model_id: FalTextToVideoModelId,
    request_translation_revision: FalTextToVideoTranslationRevision,
    account: FalProviderAccount,
    transport: FalHttpTransport,
}
```

Router construction rejects unknown or incompatible profiles, duplicate route IDs, ambiguous
priority, incomplete configuration, and routes without conformance evidence. Absence is represented
by `NoConfiguredProviderRoute`, not a route that fails at runtime.

Routing may consider configured priority, region, policy, and health. The selected
`ProviderRouteId` is fixed for one `WorkflowNodeDispatchId` before paid submission. The router may
switch equivalent routes only before provider acceptance; it never switches after an accepted or
ambiguous submission.

## Route Responsibilities

Each concrete route:

- maps every semantic field to a private provider DTO;
- uses its typed native model ID instead of parsing profile or display names;
- sends explicit values when native defaults could change observable behavior;
- validates every third-party response and returned model identity;
- submits, polls, downloads when needed, and observes bounds and cancellation;
- maps provider statuses once into `NodeCapabilityProviderFailure`;
- keeps route and translation revisions only in restricted infrastructure audit records.

Remote URLs are validated and downloaded inside the route. No path or URL crosses a public port.
The route never creates an Asset; the capability implementation stores validated media through the
Asset boundary.

## Dispatch And Failure Rules

```text
saved GenerationProfileRef
  -> capability compatibility and availability
  -> durable Workflow Run and node dispatch
  -> router fixes one route
  -> route translates and performs provider work
  -> capability validates and stores the result
  -> node execution completes
```

No database transaction remains open during provider work. Retry creates a new dispatch while
retaining the selected profile. If submission is ambiguous and the provider has no idempotency
lookup, execution fails instead of blindly resubmitting paid work.

Profile errors are `GenerationProfileNotFound`, `GenerationProfileIncompatibleWithNodeCapability`,
`GenerationProfileUnavailable`, and `GenerationProfileAvailabilityIndeterminate`. Provider failures
use the structured categories defined by the capability contract. Raw provider text never controls
Workflow state.

Credentials, response bodies, signed URLs, native model IDs, provider tasks, and route details never
enter public errors or ordinary logs. Endpoints, redirects, requests, responses, polling, downloads,
and artifacts are bounded and validated as untrusted input.

## Configuration And Composition

Only `src-tauri/composition.rs` constructs provider routers and routes. Configuration enables
accounts, regions, routes, and routing policy; it does not select one global model per capability.
Credentials are provided as short-lived `ProviderCredentialSecretValue` values and never stored in
business objects.

Deterministic routes use the same profile catalog, router, availability, request, result, and failure
contracts as production routes.

## Verification

- profile tests cover immutable identity, lifecycle, and exact compatibility;
- router tests reject invalid route maps and prove fixed dispatch selection;
- public port suites run against deterministic and configured routers;
- every concrete route passes its exact capability conformance suite;
- translation and fault tests cover every semantic field, polling, cancellation, idempotency,
  malformed responses, bounded downloads, and availability races;
- architecture tests reject provider-native node fields, broad provider interfaces, removed binding
  layers, and construction outside composition.

## Consequences

The design keeps one public interface per semantic operation but reduces provider infrastructure to
two runtime roles: router and route. Adding a provider means adding concrete routes for the exact
operations it supports. Adding a model makes it selectable only after a stable profile,
compatibility declaration, configured route, and conformance evidence exist.
