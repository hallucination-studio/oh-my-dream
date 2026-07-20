# Backend Node Capability Architecture

> Status: frozen MVP interface and named roadmap
> Owner: interface in `crates/engine`; exact implementations in `crates/nodes`
> Scope: semantic operations executable by Workflow nodes

A Node Capability is one versioned business operation. It is not a UI shell, provider feature,
native model, Generation Task, or generic job.

## Decision

1. Workflow owns one consumer-facing `WorkflowNodeCapabilityInterface`.
2. Every exact operation has one behavior-revealing implementation such as
   `GenerateVideoCapabilityImpl`.
3. One immutable `WorkflowNodeCapabilityRegistry` holds active implementations by exact
   `NodeCapabilityContractRef`.
4. Each implementation owns its contract, parameter normalization, external readiness, typed
   request mapping, result validation, and execution behavior.
5. Each implementation receives only the focused external interfaces it consumes.
6. Model-powered implementations require a provider-independent `GenerationProfileRef` and create
   durable work through one focused task-start interface.

There is no separate catalog/executor pair, generic options map, optional capability method, or
`Unsupported` implementation.

## Consumer-Owned Interface

```rust
#[async_trait]
pub trait WorkflowNodeCapabilityInterface: Send + Sync {
    fn node_capability_contract(&self) -> &NodeCapabilityContract;

    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError>;

    async fn check_node_external_readiness(
        &self,
        request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue>;

    async fn execute_node_capability(
        &self,
        request: NodeCapabilityExecutionRequest,
    ) -> Result<NodeCapabilityExecutionOutcome, NodeCapabilityExecutionError>;
}
```

`NodeCapabilityExecutionOutcome` is the closed union
`Completed(WorkflowNodeOutputSet)` or `WaitingForGenerationTask`. Only the three active
provider-backed capabilities may return the waiting variant, and only after their task-start call
has durably committed the exact Node Execution origin. It contains no task/provider/route ID.

The names make all four behaviors explicit. Workflow uses the contract for structural graph checks,
normalization for an immutable execution plan, external readiness for Assets and Generation
Profiles, and execution only after durable Run admission.

`NodeCapabilityNormalizedParameters` contains only the complete normalized parameter set. Runtime
input-item references belong to `WorkflowTextValue`, never to node parameters. Exact typed
parameters and the provider-neutral Generation Task request mapping remain private to the
implementation.

`WorkflowNodeCapabilityRegistry` is a concrete immutable collection, not another trait. It rejects
duplicate refs, lists the same contracts used at execution, resolves one exact implementation, and
never reimplements capability rules.

`WorkflowNodeCapabilityRegistry::try_new` consumes capability implementations and returns
`NodeCapabilityRegistryError::DuplicateContractRef` on the first duplicate in input order.
`list_node_capability_contracts` returns borrowed contracts in ascending contract-ref order;
`resolve_node_capability` returns the matching shared implementation or
`NodeCapabilityRegistryError::ContractNotRegistered`. The registry has no mutation, fallback,
version negotiation, provider lookup, roadmap entry, or second catalog.

## Frozen Shared Capability Values

`NodeCapabilityContractId` is 3..=128 lowercase ASCII bytes with two or more dot-separated segments;
each segment matches `[a-z][a-z0-9_]*`. `NodeCapabilityContractVersion { major, minor }` uses a
non-zero `u16` major and `u16` minor. `NodeCapabilityContractRef` canonically displays
`<id>@<major>.<minor>`. Input, output, parameter, and role keys are distinct 1..=64-byte newtypes
matching `[a-z][a-z0-9_]*`. Display labels are never identity.

`NodeCapabilityParameterValue` is a closed union:

| Tag | Variant | Canonical value |
| --- | --- | --- |
| `0` | `UnsignedInteger` | big-endian `u64` |
| `1` | `Text` | UTF-8, bounded by its exact contract |
| `2` | `Choice` | one declared capability-owned key |
| `3` | `GenerationProfile` | canonical D0.3 `GenerationProfileRef` bytes |
| `4` | `ManagedAsset` | canonical D0.4 `AssetId` bytes |
| `5` | `GenerationModel` | exact 16-byte RFC 9562 UUIDv4 `GenerationModelId` |
| `6` | `Boolean` | one byte: `0` false or `1` true |

Engine stores those three cross-context variants as
`NodeCapabilityGenerationProfileRefParameterValue` and
`NodeCapabilityManagedAssetIdParameterValue`, and
`NodeCapabilityGenerationModelIdParameterValue`. They are mechanical boundary representations,
not a second Generation Profile, Generation Model, or Asset identity owner. The first contains the
validated canonical profile ID bytes plus non-zero version; the second contains one
`WorkflowManagedAssetIdBoundaryValue`; the third contains exact UUID bytes. They expose only
canonical bytes, equality, and ordering and cannot answer lifecycle, compatibility, availability,
Project visibility, media kind, or Asset state.

The Generation Profile/Model module implements explicit conversion for `GenerationProfileRef` and
`GenerationModelId` boundary values. The Desktop node-to-Asset bridge converts
between `NodeCapabilityManagedAssetIdParameterValue` and Asset-owned `AssetId` at its boundary; the
same bridge translates the typed Workflow managed-media references and their fingerprints.
Engine performs only canonical shape validation; each semantic owner revalidates and interprets its
domain value. No crate aliases, re-exports, or wraps these boundary values as its authoritative ID.

The parameter set is a `BTreeMap<NodeCapabilityParameterKey, NodeCapabilityParameterValue>` with at
most 64 entries. Its canonical bytes are entry count as big-endian `u32`, then ascending key/value
pairs; keys and variable bytes use big-endian `u32` lengths and the explicit tags above. Unknown
keys, duplicate keys, wrong variants, and values outside the exact contract are rejected. There is
no null, nested map/list, arbitrary JSON, provider option, or untyped string enum.

`NodeCapabilityParameterContract` declares key, exact variant, `Required`, `Optional`, or
`OptionalWithDefault`, and only variant-appropriate bounds/allowed choices. Normalization inserts
only capability-owned static defaults and returns `NodeCapabilityNormalizedParameters`. It never
reads a selected model or vendor default. `Optional` exists for fields whose availability,
requirement, and suggested value come from a selected immutable Generation Model contract.
Construction rejects
duplicate keys, empty/invalid ranges or choices, invalid defaults, duplicate inputs/outputs, no
output, or more than one primary output.

`NodeCapabilityContract::validate_draft_parameters` validates only keys and values present in the
stored parameter map. It rejects unknown keys, wrong variants, and invalid present values, but it
does not require a `Required` key and does not insert defaults. It returns the canonical stored
parameter map used by `WorkflowAggregate` draft construction and restore.
`NodeCapabilityContract::normalize_parameters_for_execution` is the separate completeness boundary:
it requires every `Required` key, inserts only `OptionalWithDefault` static defaults, and returns
`NodeCapabilityNormalizedParameters`. Readiness and Run admission use this operation; mutation and
restore never do. These two methods share the same parameter definitions and value validator, so
there is no second owner of bounds or allowed values.

Its constraint is exactly one closed variant: `UnsignedIntegerRange { minimum, maximum }`,
`UnsignedIntegerAllowedValues(non-empty sorted set)`, `TextUtf8Bytes { minimum, maximum }`,
`ChoiceAllowedKeys(non-empty sorted set)`, `Boolean`, `GenerationProfileRef`, or
`ManagedAssetId { media_kind }`, or `GenerationModelId`. Minimum is not greater than maximum. Presence is exactly `Required`
or `Optional` or `OptionalWithDefault(value)`; a supplied default must satisfy the same constraint. Constraint and value
tags must match, and no independent optional bound, validator callback, or generic metadata exists.

## Versioned Contract

```rust
pub struct NodeCapabilityContract {
    pub contract_ref: NodeCapabilityContractRef,
    pub parameters: Vec<NodeCapabilityParameterContract>,
    pub inputs: Vec<NodeCapabilityInputContract>,
    pub outputs: Vec<NodeCapabilityOutputContract>,
    pub execution_kind: NodeCapabilityExecutionKind,
}
```

Input binding contracts and their role/type rules are defined once in
[`BACKEND_WORKFLOW_GRAPH.md`](BACKEND_WORKFLOW_GRAPH.md#input-contract-model).
`NodeCapabilityOutputContract` contains one output key, one exact `WorkflowDataType`, and
`is_primary`; a contract has 1..=64 outputs and exactly one primary output. No output is optional,
multi-typed, nullable, or role-bearing.

Parameter, input, and output vectors preserve declaration order for presentation while their keys
remain unique identity. Registry order is independent and always ascending by contract ref.

## Frozen Capability Call Values

```rust
pub struct NodeCapabilityReadinessRequest {
    pub project_id: ProjectId,
    pub normalized_parameters: NodeCapabilityNormalizedParameters,
    pub input_binding_snapshot: NodeCapabilityInputBindingSnapshot,
    pub model_selection: NodeCapabilityReadinessModelSelection,
    pub deadline: NodeCapabilityReadinessDeadline,
}

pub struct WorkflowNodeExecutionContext {
    pub project_id: ProjectId,
    pub workflow_run_id: WorkflowRunId,
    pub node_execution_id: WorkflowNodeExecutionId,
    pub deadline: NodeCapabilityExecutionDeadline,
    pub cancellation: NodeCapabilityExecutionCancellation,
}

pub struct WorkflowNodeExecutionOrigin {
    pub workflow_id: WorkflowId,
    pub workflow_revision: WorkflowRevision,
    pub workflow_node_id: WorkflowNodeId,
    pub capability_contract_ref: NodeCapabilityContractRef,
}

pub struct NodeCapabilityExecutionRequest {
    pub context: WorkflowNodeExecutionContext,
    pub origin: WorkflowNodeExecutionOrigin,
    pub normalized_parameters: NodeCapabilityNormalizedParameters,
    pub external_admission: WorkflowNodeExternalAdmission,
    pub inputs: WorkflowNodeInputSet,
}
```

`WorkflowNodeExternalAdmission` is the closed boundary value `None | GenerationModel {
model_id_bytes, revision }`. Run admission supplies `GenerationModel` exactly for a model-powered
capability and `None` for every local capability. `model_id_bytes` are the same exact UUID bytes as
the normalized parameter; revision is non-zero. The capability rejects a mismatched tag, model ID,
or zero revision as `InvalidCapabilityInvocation` before any external call, then explicitly
translates the value to its nodes-owned `GenerationModelRevisionRef`.

`NodeCapabilityReadinessModelSelection` is `Current | Frozen(WorkflowNodeExternalAdmission)`.
Ordinary editing/readiness uses `Current`; a model-powered capability resolves the stable model ID
from normalized parameters against the current Settings snapshot. Run admission first bulk-resolves
all selected IDs from one Settings snapshot, then repeats readiness with each exact `Frozen`
revision before it commits. Local capabilities require `Frozen(None)` during admission. A
capability rejects a mismatched frozen model ID or kind as `InvalidCapabilityInvocation` and never
falls back to `Current`.

`NodeCapabilityInputBindingSnapshot` is a rule-free immutable projection of the candidate graph's
declared input keys, absent/single/ordered shape, stable input-item IDs, roles, and concrete source
output data types. It contains no runtime values, Asset facts, source paths, URLs, or provider data.
Workflow constructs it mechanically from the same revision being evaluated. It lets an exact
capability own cross-field calibration such as Video mode/cardinality rules without teaching
Workflow or React those semantics.

Readiness checks parameter-selected external state and capability-owned relationships among the
selected model contract, parameters, and binding snapshot. It does not resolve upstream runtime
values or media bytes, dispatch providers, mutate state, or write media. An
empty issue vector means ready; otherwise it contains 1..=64 unique issues sorted by category tag,
target kind, then target bytes. Asset/Profile/Model state issues identify one parameter key and its
typed identity; calibration issues identify one parameter, input, or stable input item. At most one
issue is returned for the same category and target; when several non-calibration state observations
could apply to one parameter, the category table order wins. The capability ref comes from the resolved implementation and
is not duplicated in the readiness request. The execution request carries it only inside the
frozen producer origin and the capability validates that copy against its resolved implementation.

`NodeCapabilityReadinessDeadline` is a call-scoped monotonic instant supplied by the Workflow
readiness use case. It exposes `is_reached_at` and `monotonic_instant`, is never serialized or
persisted, and does not include cancellation. Readiness implementations pass that exact instant to
every availability or managed-media boundary and never create or extend their own timeout.

`NodeCapabilityExecutionDeadline` is one call-scoped monotonic instant. It is never serialized or
persisted. `NodeCapabilityExecutionCancellation` is a cloneable, concurrent signal with initially
active and idempotently cancelled states. A capability checks cancellation and deadline before each
external effect and after each await; cancellation wins when both are observed together. Neither
state authorizes rollback, automatic retry, provider resubmission, or a new Run state.

`WorkflowNodeExecutionOrigin` carries the frozen plan's Workflow ID, non-zero revision, Workflow
node ID, and exact capability contract ref. It deliberately omits Project, Run, and node-execution
identity because those already belong to `WorkflowNodeExecutionContext`. A capability requires its
origin capability ref to equal its own contract ref before any external boundary. The value enters
the durable task-start request unchanged and is never sent directly to a provider.

`WorkflowNodeInputSet` is a map with at most 64 exact declared input keys. Each value has the exact
single/ordered shape, stable item identity, role, and runtime type required by the contract; ordered
items preserve vector order. `WorkflowNodeOutputSet` is a map with 1..=64 entries containing every
declared output exactly once, no extras, and values of the exact declared type. A media value must be
an Available managed reference. Input/output set construction rejects partial or invalid sets.

Changing parameter meaning, input cardinality, role meaning, output type, or result guarantee
requires a new contract version. The exact implementation is the semantic owner; Workflow, DTOs,
forms, and provider routes only consume or translate the contract.

`NodeCapabilityExecutionKind` is a closed business classification:
`PureValue`, `ManagedAssetRead`, `ContentGeneration`, `MediaTransformation`, or `ContentAnalysis`.
It never means local versus remote.

## Frozen MVP Implementations

[`BACKEND.md`](BACKEND.md#active-node-capabilities) is the single authority for the seven active
contract refs, implementation names, and external dependencies. `WorkflowNodeCapabilityRegistry`
contains exactly that set; this document does not maintain a second copy.

The active runtime data types are `Text`, `Image`, `Video`, and `Audio`. The UI derives those four
shells from each contract's primary output; shell kind is never persisted as domain identity.

## Frozen Seven Contract Shapes

The refs and implementation names remain single-sourced in `BACKEND.md#active-node-capabilities`.
Their exact semantic shapes are:

| Implementation | Parameters | Inputs | Output | Kind |
| --- | --- | --- | --- | --- |
| `ProvideLiteralTextCapabilityImpl` | required `text`: Text, 1..=65,536 UTF-8 bytes | none | primary `text`: Text | `PureValue` |
| `ReadImageAssetCapabilityImpl` | required `asset_id`: ManagedAsset(Image) | none | primary `image`: Image | `ManagedAssetRead` |
| `ReadVideoAssetCapabilityImpl` | required `asset_id`: ManagedAsset(Video) | none | primary `video`: Video | `ManagedAssetRead` |
| `ReadAudioAssetCapabilityImpl` | required `asset_id`: ManagedAsset(Audio) | none | primary `audio`: Audio | `ManagedAssetRead` |
| `TextToImageCapabilityImpl` | required `generation_profile_ref` and `generation_model_id`; optional `aspect_ratio`, default `square` | required single `prompt`: Text | primary `image`: Image | `ContentGeneration` |
| `GenerateVideoCapabilityImpl` | required `generation_profile_ref`, `generation_model_id`, and `input_mode`; optional dynamic Video fields listed below | optional single `prompt`: Text; optional ordered `images`, `videos`, and `audio` | primary `video`: Video | `ContentGeneration` |
| `TextToSpeechCapabilityImpl` | required `generation_profile_ref` and `generation_model_id` | required single `text`: Text | primary `audio`: Audio | `ContentGeneration` |

`aspect_ratio` choices are `square`, `landscape_4_3`, `portrait_3_4`, `landscape_16_9`, and
`portrait_9_16`. Every model-powered contract requires exactly one compatible active Generation
Profile and one user-selected enabled Generation Model configuration. Voice/model/style behavior
for speech belongs to the selected profile, not an extra provider-native node parameter.

The stable Video parameter superset is exact:

| Key | Static parameter shape |
| --- | --- |
| `input_mode` | required Choice: `text_to_video`, `first_frame`, `first_and_last_frames`, `multimodal_reference` |
| `generate_audio`, `draft`, `camera_fixed`, `watermark` | optional Boolean |
| `resolution` | optional Choice: `p480`, `p720`, `p1080`, `k4` (translated to semantic 480p/720p/1080p/4k values) |
| `ratio` | optional Choice: `landscape_16_9`, `landscape_4_3`, `square_1_1`, `portrait_3_4`, `portrait_9_16`, `cinematic_21_9`, `adaptive` |
| `duration_mode` | optional Choice: `auto`, `seconds`, `frames` |
| `duration_seconds` | optional UnsignedInteger `2..=15` |
| `frame_count` | optional UnsignedInteger `29..=289` |
| `seed_mode` | optional Choice: `random`, `fixed` |
| `seed` | optional UnsignedInteger `0..=4_294_967_295` |

The stable input superset has `prompt` as `OptionalSingleValue(Text)`; `images` as
`OrderedReferences(0..=9)` with Image-only roles `first_frame`, `last_frame`, and
`reference_image`; `videos` as `OrderedReferences(0..=3)` with Video-only role
`reference_video`; and `audio` as `OrderedReferences(0..=3)` with Audio-only role
`reference_audio`. An absent ordered binding means zero items. This broad draft-valid shape lets a
model switch preserve every connection. `VideoGenerationCalibrationPolicy` applies the selected
model revision's narrower mode, availability, value, default, and cross-field contract before Run
admission.

All seven contracts declare exactly one output. Asset-read capabilities resolve and return an
Available exact-kind Asset during execution. Provider-backed capabilities return a durable waiting
handoff; Generation Task later validates the provider result, finalizes the declared Asset, and
notifies Workflow to attach that one output. No batch count, negative prompt, guidance, safety
level, native voice, native aspect token, or provider seed is an MVP node parameter.

## Implementation Shape

```rust
pub struct GenerateVideoCapabilityImpl<R, A, C, T> {
    generation_profile_catalog: Arc<GenerationProfileCatalog>,
    generation_model_availability_reader: A,
    generation_model_contract_reader: C,
    managed_media_reader: R,
    generation_task_starter: T,
    contract: NodeCapabilityContract,
    calibration_policy: VideoGenerationCalibrationPolicy,
}
```

`GenerateVideoCapabilityImpl`:

1. validates and normalizes its stable parameters;
2. checks the parameter-selected `GenerationProfileRef` and stable `GenerationModelId` during
   external readiness;
3. calibrates the stable input/parameter superset against the exact selected model contract;
4. resolves every ordered input media item only during execution while preserving stable item IDs,
   roles, and order;
5. converts Workflow values into `NodeCapabilityGenerationTaskStartRequest`;
6. calls `NodeCapabilityGenerationTaskStarterInterface::start_generation_task`;
7. returns `WaitingForGenerationTask` only after durable task creation succeeds.

It never reads a provider name, route ID, remote task ID, path, URL, Asset repository, or concrete
adapter.

### Generation-Capability Readiness

[`BACKEND.md`](BACKEND.md#active-node-capabilities) owns the complete dependency list for each exact
implementation. Every listed dependency is an independent constructor argument; no generic
generation context or service bundle groups them.

Readiness first requires the complete normalized parameter shape and successful conversion of its
`generation_profile_ref` and `generation_model_id` boundary values. Either failure returns only
`InvalidCapabilityInvocation`. For converted values, it then:

1. requires an Active catalog definition compatible with the capability contract;
2. resolves either the current model revision or the request's exact frozen revision according to
   `NodeCapabilityReadinessModelSelection`;
3. performs one structural availability read and one safe model-capability-contract read for that
   same revision under the request's unchanged monotonic deadline;
4. for `GenerateVideoCapabilityImpl`, runs `VideoGenerationCalibrationPolicy` over that exact
   contract, normalized parameters, and input binding snapshot.

A missing, Retired, or incompatible catalog definition becomes `GenerationProfileIncompatible`.
A missing, disabled, removed, credential-less, incompatible, or unresolvable model becomes
`GenerationModelUnavailable`. The issue targets
`GenerationModel { parameter_key: generation_model_id, generation_model_id }`, contains no
endpoint, native model, provider route, credential detail, or retry metadata, and blocks admission.
Reader failure, deadline expiry, or an invalid observation becomes
`GenerationModelAvailabilityIndeterminate` and also blocks admission. Readiness never resolves
runtime inputs, calls a provider, writes media, retries, caches, or substitutes a profile or model.
It never truncates or replaces the caller's deadline.

Calibration returns zero or more `GenerationModelCalibrationIssue` values. Each contains one
closed rule code, a target of `Parameter(key)`, `Input(key)`, or
`InputItem { input_key, input_item_id }`, and a closed correction of `SetChoice`, `SetBoolean`,
`SetUnsignedInteger`, `RemoveParameter`, `ConnectInput`, `RemoveInputItem`, or
`SetInputItemRole`. A value proposal contains only contract-declared compatible choices, bounds,
or the suggested default. It contains no provider ID, Endpoint, native model ID, credential,
provider error text, or automatic mutation. Missing required model values and still-present
unsupported values are both calibration issues.

### Generation-Capability Execution

Execution accepts only the exact normalized parameters and complete runtime input set declared by
the resolved contract, plus an origin whose capability ref equals that contract ref. A malformed
direct call or mismatched origin returns `InvalidCapabilityInvocation` at `ResolveInputs` with
target `Capability` before an external boundary. It does not repeat catalog or availability reads;
it resolves the exact model capability contract by the selected
`GenerationModelRevisionRef`, repeats the same pure calibration over runtime input identities and
roles, and rejects any disagreement as `InvalidCapabilityInvocation` before media or task work.
After resolving each ordered media input through `NodeCapabilityManagedMediaReaderInterface`,
`GenerateVideoCapabilityImpl` invokes
`VideoGenerationCalibrationPolicy::validate_resolved_video_inputs` with the exact stable input-item
IDs, roles, MIME values, byte lengths, and verified media facts. The method owns every Seedance
input-fact rule and returns the same typed calibration issue vocabulary. It runs before construction
of `NodeCapabilityGenerationTaskStartRequest`; any issue fails at `ResolveInputs` and no Generation
Task or provider call is created. Task and provider adapters validate boundary integrity and wire
bounds only; they never reimplement these capability-owned media eligibility rules.
The selected profile and frozen model revision enter the task request unchanged. The complete
`WorkflowNodeExecutionContext` and `WorkflowNodeExecutionOrigin` enter the task-start request
unchanged; a capability never reconstructs or replaces its Project, Run, node execution, deadline,
cancellation, or frozen origin values.

The exact mappings are:

| Implementation | Generation Task mapping | Asset display name | Asset provenance after finalization |
| --- | --- | --- | --- |
| `TextToImageCapabilityImpl` | `prompt`, selected profile/model revision, normalized `aspect_ratio` | `Generated Image` | `ProviderGenerated { profile_ref }` |
| `GenerateVideoCapabilityImpl` | explicit mode, optional/required `prompt`, ordered readable Images/Videos/Audio with stable IDs and roles, selected profile/model revision, and one calibrated Video parameter set | `Generated Video` | `ProviderDerived { source_media_refs: ordered input media refs, profile_ref }`, or `ProviderGenerated` when no media is supplied |
| `TextToSpeechCapabilityImpl` | `text`, selected profile/model revision | `Synthesized Speech` | `ProviderGenerated { profile_ref }` |

Generation Task finalization uses the same Node Execution coordinates, profile, source Asset
snapshots, declared output key, and ordinal `0` to create exactly one durable Asset. No capability
creates provider metadata, remote handles, raw payloads, or Assets directly.

Cancellation and deadline are checked before input resolution and immediately before task
creation. Once task creation succeeds, the capability always returns
`WaitingForGenerationTask`; it does not turn a durable handoff into a cancellation error after the
fact. If Workflow cancellation concurrently wins before the waiting transition commits, that late
transition is rejected and the task worker observes the cancelled origin and converges the task
without submission or output attachment. Image input resolution failures use stage
`ResolveInputs`; task-start failures use `StartGenerationTask` and target `Capability`. Provider
delivery retry and output finalization happen only inside Generation Task application semantics.

After a valid immediate invocation produces all declared runtime values, a non-provider capability
constructs its complete `WorkflowNodeOutputSet` exactly once. Provider-backed capabilities return
only the waiting outcome after durable task creation.

## MVP External Interfaces

| Interface | Behavior method | Request | Result |
| --- | --- | --- | --- |
| `NodeCapabilityManagedMediaReaderInterface` | `read_managed_media` | `NodeCapabilityManagedMediaReadRequest` | `NodeCapabilityReadableMediaInput` |
| `NodeCapabilityGenerationTaskStarterInterface` | `start_generation_task` | `NodeCapabilityGenerationTaskStartRequest` | `NodeCapabilityGenerationTaskStartResult` |

The task-start request contains the exact semantic generation spec, `GenerationProfileRef`, frozen
`GenerationModelRevisionRef`, `WorkflowNodeExecutionContext`, and
`WorkflowNodeExecutionOrigin`. It contains no provider name, native model ID, credential, endpoint,
URL, path, remote task ID, wire DTO, or generic options map.
Its result proves durable task creation and contains no provider observation or generated output.

The two capability-consumed interfaces and their boundary values are exact. All fields are private with
noun-specific accessors and fallible constructors where invariants exist.

`NodeCapabilityParameterSet::try_from_canonical_bytes` is the only inverse of its canonical byte
encoding. Before reading a length prefix it rejects an input larger than 1 MiB; every key and
variable value is additionally bounded by the global maximum of 64 KiB before allocation. It
consumes the entire slice and rejects unknown value tags, invalid UTF-8, truncation, trailing bytes,
duplicate or non-strictly-sorted keys, more than 64 entries, and violations of global value shape
such as an invalid typed profile/model/Asset boundary value. Every decoded key is constructed through its
authoritative 1..=64-byte grammar-valid `NodeCapabilityParameterKey` constructor. It does not know a capability contract and
therefore does not decide declared keys, Text bounds, Choice members, or numeric constraints. It
re-encodes the decoded shape and requires byte-for-byte equality before returning it. Its concrete
`NodeCapabilityParameterCanonicalDecodeError` reports only bounded
decode categories and offsets; it is not a Workflow readiness category. Capability normalization
still owns declared-key, required-value, and operation-specific validation, so decoding never
becomes a second capability contract or an iterator over parameters.

The exact boundaries are:

### Managed-Media Read Boundary

`NodeCapabilityManagedMediaReference` is the closed union `Image(WorkflowManagedImageRef)`,
`Video(WorkflowManagedVideoRef)`, or `Audio(WorkflowManagedAudioRef)`. Its variant is the expected
kind; there is no second kind field or untyped Asset ID.

`NodeCapabilityManagedMediaReadSelection` is the closed union
`AssetId(NodeCapabilityAssetIdMediaReadSelection)` or
`ExactReference(NodeCapabilityManagedMediaReference)`.
`NodeCapabilityAssetIdMediaReadSelection::new` has private Asset ID and expected-kind fields with
noun-specific accessors.

| Selection | Inputs | Resolution | Stable failure |
| --- | --- | --- | --- |
| `AssetId` | `WorkflowManagedAssetIdBoundaryValue`, expected `NodeCapabilityMediaKind` | Resolve the visible Available exact-kind Asset and return its typed reference | absent/Pending/Missing `Unavailable`; another kind `KindMismatch` |
| `ExactReference` | one typed `NodeCapabilityManagedMediaReference` | Read only that Available reference and fingerprint | absent `Unavailable`; byte disagreement `DigestMismatch` |

The three Asset-read capabilities use `AssetId` because their persisted parameter intentionally has
no content fingerprint. A capability uses `ExactReference` when Workflow already carries an exact
Available reference. There is no path, URL, untyped kind, optional fingerprint, lookup mode flag,
or extra exact-reference wrapper.

`NodeCapabilityManagedMediaReadRequest` contains Project ID, one
`NodeCapabilityManagedMediaReadSelection`, and one exact process-monotonic `Instant`. Readiness passes
`NodeCapabilityReadinessDeadline::monotonic_instant`; execution passes
`NodeCapabilityExecutionDeadline::monotonic_instant`.
`NodeCapabilityManagedMediaReaderInterface::read_managed_media`
returns `Result<NodeCapabilityReadableMediaInput, NodeCapabilityMediaBoundaryError>`. The result is the
matching closed Image/Video/Audio variant containing the resolved exact managed reference, exact MIME, byte
length, declared media facts, and one `NodeCapabilityMediaSourceLease`. A mismatched result is
`KindMismatch`; absent/Pending/Missing content is `Unavailable`; Asset storage and validation errors
map only to the existing exact `NodeCapabilityMediaFailure` categories.

Both selections resolve only within the supplied Project. Deadline is checked before lookup and
before returning the source. The reader applies exactly this precedence:

1. reject an elapsed deadline as `DeadlineExceeded`;
2. resolve the Asset inside the supplied Project;
3. require the selected or referenced media kind;
4. require Available content;
5. for `ExactReference` only, compare the resolved `AssetContentDescriptor.digest` with the reference
   fingerprint without consuming or rehashing the one-shot source;
6. return the typed reference, facts, MIME, descriptor length/digest, and source lease.

`NotFound`, `NotVisible`, `ContentPending`, and `ContentMissing` translate to `Unavailable`;
`MediaKindMismatch` translates to
`KindMismatch { expected: WorkflowDataType, observed: WorkflowDataType }`; an exact-reference descriptor digest difference
translates to `DigestMismatch`. All other already-frozen Asset, media, and deadline translations are
unchanged. These are selection semantics inside the existing reader interface; they do not add an
Asset repository interface, metadata query, readiness cache, stream rehash, or fallback lookup.

For a reader, `expected` is the selection/reference kind and `observed` is the resolved Asset kind;
the Asset-owned distinct kinds translate mechanically to distinct non-Text `WorkflowDataType`
values and are copied unchanged into `ManagedAssetKindMismatch` readiness detail. For a writer,
`expected` is the request payload kind and `observed` is the returned Asset/reference kind; execution
preserves that direction in `NodeCapabilityMediaFailure`. No layer reverses or re-derives them.

`NodeCapabilityMediaMimeType` is exactly `ImagePng`, `ImageJpeg`, `ImageWebp`, `VideoMp4`,
`VideoQuickTime`, `VideoWebm`, `AudioMpeg`, `AudioWav`, or `AudioOgg`. Generated payloads restrict those values to
PNG, MP4, and MPEG respectively. `NodeCapabilityDeclaredMediaFacts` has the same closed Image/Video/Audio fields
and numeric bounds as Asset facts, but is a nodes-owned boundary observation translated by the
Desktop bridge; it is not an Asset-domain alias or a second inspector.

```text
Image { width, height }
Video { width, height, duration_ms, frame_rate_millihertz, has_audio }
Audio { duration_ms, sample_rate_hz, channels }
```

`NodeCapabilityMediaContentDigest` is exactly 32 SHA-256 bytes. Each
`NodeCapabilityReadableImageInput`, `NodeCapabilityReadableVideoInput`, and
`NodeCapabilityReadableAudioInput` contains its matching managed reference, MIME, facts, and source
lease. `NodeCapabilityReadableMediaInput` is the closed union of those three typed values; callers
never downcast an untyped media object.

`NodeCapabilityMediaSourceLease` owns one `Pin<Box<dyn AsyncRead + Send>>`, exact byte length, exact
SHA-256 digest, and the same process-monotonic deadline. It is non-cloneable, non-serializable,
consumed once, rejects handoff at/after deadline as `DeadlineExceeded`, and exposes no path, URL,
seek, reopen, buffer, retry, or provider handle. Its constructor rejects only zero length; the typed
readable-input and payload constructors own kind-specific length, MIME, facts, and fingerprint
agreement before a value crosses an interface.

`NodeCapabilityMediaValueError` is the exact construction/lease error union `InvalidMimeForKind`,
`InvalidMediaFacts`, `InvalidByteLength`, `ContentFingerprintMismatch`, and `DeadlineExceeded`. It
contains no message, Asset error, provider error, path, or adapter detail.

`NodeCapabilityMediaBoundaryError` is exactly `Media(NodeCapabilityMediaFailure)`, `Cancelled`, or
`DeadlineExceeded`. The media reader returns this type. Capabilities map its three variants
mechanically to the existing matching `NodeCapabilityExecutionFailure` variants; cancellation and
deadline are never disguised as storage or finalization failures. A readiness read has no
cancellation signal and therefore never originates `Cancelled`; an execution caller still checks
its cancellation immediately before and after that read.

### Generation Task Start Boundary

`NodeCapabilityGenerationTaskStartRequest` contains the unchanged execution context and origin,
the selected `GenerationProfileRef`, the admission-frozen `GenerationModelRevisionRef`, one
node-owned Text/Image/Video/Voice generation request variant, the declared primary output key, and
ordered exact input Asset snapshots. Construction checks coordinates and closed operation shape but
does not resolve a provider route or create task state. The Desktop adapter translates it
explicitly to `GenerationTaskStartCommand`; `crates/nodes` does not import `crates/tasks` domain
types.

`NodeCapabilityGenerationTaskStarterInterface::start_generation_task` returns
`NodeCapabilityGenerationTaskStartResult` only after task, request hash, and `SubmitTask` effect are
durable. Repeating the same Node Execution and canonical request returns the same task result;
different facts conflict. The result contains the local `GenerationTaskId` for diagnostics inside
the capability boundary, but the capability returns only `WaitingForGenerationTask` to Workflow.

`NodeCapabilityExecutionDeadline::monotonic_instant` returns its exact wrapped `Instant` solely for
consumer-owned boundary translation. It does not extend, replace, serialize, or create a deadline.

### C2 Shared Fake Contracts

Both capability-consumed interfaces have reusable parameterized contract suites run unchanged
against their fakes and production bridges. The reader suite covers Project isolation, exact kind,
fingerprint preservation, one-shot source ownership, deadlines, and media-error mapping. The
task-start suite covers exact origin preservation, canonical request idempotency/conflict, durable
handoff, cancellation/deadline precedence, and absence of provider-native fields. Fakes are
`NodeCapabilityManagedMediaReaderFakeImpl` and `NodeCapabilityGenerationTaskStarterFakeImpl`.

## Named Roadmap Contracts

These operations have approved semantic names but are not part of the MVP registry. Their complete
contracts are introduced only when scheduled. The table classifies dependency direction without
pre-designing future provider methods or request variants: provider-backed operations use the
matching Text/Image/Video/Voice capability, while pure local operations own a focused local
interface.

| Planned contract ref | Reserved implementation name | Boundary class |
| --- | --- | --- |
| `image.generate_from_image@1.0` | `ImageToImageCapabilityImpl` | Image provider capability |
| `image.generate_from_reference_images@1.0` | `ReferenceImagesToImageCapabilityImpl` | Image provider capability |
| `image.crop@1.0` | `ImageCropCapabilityImpl` | local image operation |
| `video.upscale@1.0` | `VideoUpscaleCapabilityImpl` | Video provider capability |
| `video.extract_frames@1.0` | `VideoFrameExtractionCapabilityImpl` | local video operation |
| `video.concatenate@1.0` | `VideoConcatenationCapabilityImpl` | local video operation |
| `video.analyze_storyboard@1.0` | `VideoStoryboardAnalysisCapabilityImpl` | Text provider capability |
| `text.generate_from_text@1.0` | `TextToTextGenerationCapabilityImpl` | Text provider capability |
| `text.generate_from_mixed_media@1.0` | `MixedMediaToTextGenerationCapabilityImpl` | Text provider capability |
| `audio.generate_music_from_text@1.0` | `TextToMusicCapabilityImpl` | future Music provider capability, not Voice |

The active `video.generate@1.0` contract already owns Text-to-Video, FirstFrame,
FirstAndLastFrames, and MultimodalReference as explicit mutually exclusive modes. A roadmap entry
must not reintroduce those modes as separate capabilities. Speech and music remain separate
operations, profiles, results, and failure semantics.

Activating a roadmap operation requires its complete parameter, input-role, result, error, mock,
provider/media interface, Asset provenance, UI contract, and E2E tests in the same change.

## Results And Errors

A capability returns a complete immediate output, one durable waiting outcome, or one structured
error. Required generated media becomes a Workflow output only after Generation Task finalization
has made every corresponding Asset Available; partial Asset creation never becomes a partial
`WorkflowNodeOutputSet`.

`NodeCapabilityParameterError` and `NodeCapabilityReadinessIssue` occur before dispatch.
`NodeCapabilityGenerationTaskStartFailure` uses closed invalid-request, conflict, unavailable,
cancelled, deadline, and persistence categories.
`NodeCapabilityMediaFailure` uses closed categories without authorizing retry.
`NodeCapabilityExecutionError` adds capability, stage, and safe target. Raw provider text, native
IDs, URLs, paths, credentials, and response bodies never cross these errors.

The closed parameter categories are `UnknownParameter`, `RequiredParameterMissing`,
`ParameterValueKindMismatch`, `ParameterValueOutOfBounds`, `ParameterChoiceNotDeclared`, and
`ParameterSetTooLarge`. Readiness categories are `InvalidCapabilityInvocation`, `ManagedAssetUnavailable`,
`ManagedAssetKindMismatch`, `ManagedAssetReadinessIndeterminate`,
`GenerationProfileIncompatible`, `GenerationModelUnavailable`,
`GenerationModelCalibrationRequired`, and
`GenerationModelAvailabilityIndeterminate`. Execution stages are `ResolveInputs`,
`StartGenerationTask`, and `AssembleOutputs`; normalization has its own pre-admission result.
`NodeCapabilityExecutionError` wraps one `NodeCapabilityExecutionFailure` with contract ref, node
execution ID, stage, and a structured safe target.

The error values are closed and field-exact:

- `NodeCapabilityParameterError` contains one parameter category and
  `NodeCapabilityParameterErrorTarget`, either `ParameterSet` or `Parameter(key)`;
- `NodeCapabilityReadinessIssue` contains one readiness category and
  `NodeCapabilityReadinessTarget`. The target is exactly
  `Capability`, `ManagedAsset { parameter_key, asset_id }`, or
  `GenerationProfile { parameter_key, generation_profile_ref }`, or
  `GenerationModel { parameter_key, generation_model_id }`, or a calibration target
  `Parameter(key) | Input(key) | InputItem { input_key, input_item_id }`; kind-mismatch detail additionally
  contains expected and observed `WorkflowDataType` values;
- `NodeCapabilityGenerationTaskStartFailure` contains one safe task-start category and no remote
  handle, route, credential, provider body, or retry instruction;
- `NodeCapabilityMediaFailure` contains exactly one of `Unavailable`,
  `KindMismatch { expected: WorkflowDataType, observed: WorkflowDataType }`, `InvalidMedia`,
  `SizeLimitExceeded`, `DigestMismatch`, `OutputConflict`, `StorageFailed`, `InspectionFailed`,
  or `FinalizationFailed`, plus no adapter text;
- `NodeCapabilityExecutionError` contains contract ref, node execution ID, stage, one
  `NodeCapabilityExecutionFailure`, and `NodeCapabilityExecutionTarget` (`Capability`, parameter key,
  input key, input item, or output key). Its failure is exactly Readiness, GenerationTaskStart, Media, Cancelled, or
  DeadlineExceeded, plus `InvalidCapabilityInvocation` only when a direct execution request does not
  satisfy the already-resolved capability's normalized-parameter/input contract or carries another
  capability contract ref in its origin, and `InvalidCapabilityResult` only when capability-owned
  final output-set validation fails.

Construction rejects an execution target inconsistent with its stage: ResolveInputs targets a
parameter, input, input item, or capability; StartGenerationTask targets the capability; output assembly
targets only an output. Readiness targets
`Capability` only for invalid invocation, a declared parameter for identity state, or a declared
parameter/input/input item for calibration.
Cancellation/deadline use
the operation target active when observed; no absent-key convention carries target meaning.

`InvalidCapabilityInvocation` is non-retryable, has stage `ResolveInputs` and target `Capability`,
and carries no field, message, supplied value, or validation detail. It is never used for invalid raw
parameters before admission, task/provider responses, media failures, or internal adapter failures. Its
only purpose is to let a capability reject a malformed direct trait invocation without panic,
task creation, media read, or misclassifying the failure as another business category.

`InvalidCapabilityResult` is non-retryable and carries no internal error, message, or invalid
value. Final output-set validation uses `AssembleOutputs` and targets the declared output. It never
represents task-start rejection, media boundary failure, cancellation, deadline, or caller input.

Asset-read readiness maps reader outcomes exactly once: `Unavailable` becomes
`ManagedAssetUnavailable`; `KindMismatch { expected, observed }` becomes
`ManagedAssetKindMismatch` with the same distinct kinds; `DeadlineExceeded` and every other
`NodeCapabilityMediaFailure` become `ManagedAssetReadinessIndeterminate`. The indeterminate issue
uses the same `ManagedAsset { parameter_key, asset_id }` target and carries no technical category,
message, retry hint, or adapter detail. It blocks admission only; it does not authorize retry, probe,
cache, fallback, or a second read. Execution still preserves its exact media/deadline failure and
never uses the readiness indeterminate category.

Readiness uses its own no-field `InvalidCapabilityInvocation` issue with target `Capability` when
the supplied `NodeCapabilityNormalizedParameters` do not satisfy the already-resolved capability's
parameter contract. It returns that single issue before any availability or media read. It is not an
external-state observation and carries no parameter value, missing key, validation detail, or error
message. It is the only identity-free `Capability`-target issue and is always returned alone. Raw
parameter normalization still returns `NodeCapabilityParameterError`; valid requests
never produce this issue. This addition does not make readiness renormalize parameters or authorize
repair, defaults, retry, probe, or fallback.

An optional retry instant is present only when retryable and later than error creation. Cancellation,
invalid requests/results, policy rejection, kind mismatch, digest mismatch, and output conflict are
not retryable. Safe identifiers may cross the boundary; message text, remote provider IDs, URLs, paths,
credentials, response bodies, and adapter errors may not.

## Contract Tests

- registry tests reject duplicate refs and assert the exact seven-contract MVP set;
- every active implementation passes the shared `WorkflowNodeCapabilityInterface` behavior suite;
- exact tests cover normalization, stable input order, request mapping, result validation,
  provenance, idempotent media write, and error translation;
- Video calibration tests parameterize every Seedance variant and mode, preserve incompatible
  values across model switches, and cover role/cardinality, defaults, duration/frame exclusivity,
  draft/resolution, generated audio, seed, camera-fixed, and ordered mixed-media mapping;
- the Mock provider composite passes the provider-level suite; every contributed focused capability
  passes its type-specific task-provider suite, which later production adapters must also pass;
- fault-injection tests cover cancellation, timeout, malformed payload, Asset write failure, and
  duplicate output-key conflict;
- architecture tests reject a second catalog/executor, roadmap registration, broad provider
  interface, optional unsupported method, generic options map, and concrete adapter selection
  outside composition.
