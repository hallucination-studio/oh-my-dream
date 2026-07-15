# Backend Node Capability Architecture

> Status: frozen MVP interface and named roadmap
> Owner: interface in `crates/engine`; exact implementations in `crates/nodes`
> Scope: semantic operations executable by Workflow nodes

A Node Capability is one versioned business operation. It is not a UI shell, provider feature,
native model, or generic task.

## Decision

1. Workflow owns one consumer-facing `WorkflowNodeCapabilityInterface`.
2. Every exact operation has one behavior-revealing implementation such as
   `ImageToVideoCapabilityImpl`.
3. One immutable `WorkflowNodeCapabilityRegistry` holds active implementations by exact
   `NodeCapabilityContractRef`.
4. Each implementation owns its contract, parameter normalization, external readiness, typed
   request mapping, result validation, and execution behavior.
5. Each implementation receives only the focused external interfaces it consumes.
6. Model-powered implementations require a provider-independent `GenerationProfileRef`.

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
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError>;
}
```

The names make all four behaviors explicit. Workflow uses the contract for structural graph checks,
normalization for an immutable execution plan, external readiness for Assets and Generation
Profiles, and execution only after durable Run admission.

`NodeCapabilityNormalizedParameters` contains only the complete normalized parameter set. Runtime
input-item references belong to `WorkflowTextValue`, never to node parameters. Exact typed
parameters and provider requests remain private to the implementation.

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

Engine stores those two cross-context variants as
`NodeCapabilityGenerationProfileRefParameterValue` and
`NodeCapabilityManagedAssetIdParameterValue`. They are mechanical boundary representations, not a
second Generation Profile or Asset identity owner. The first contains the validated canonical
profile ID bytes plus non-zero version; the second contains one
`WorkflowManagedAssetIdBoundaryValue`. They expose
only canonical bytes, equality, and ordering and cannot answer lifecycle, compatibility,
availability, Project visibility, media kind, or Asset state.

The Generation Profile module implements explicit conversion between `GenerationProfileRef` and
`NodeCapabilityGenerationProfileRefParameterValue`. The Desktop node-to-Asset bridge converts
between `NodeCapabilityManagedAssetIdParameterValue` and Asset-owned `AssetId` at its boundary; the
same bridge translates the typed Workflow managed-media references and their fingerprints.
Engine performs only canonical shape validation; each semantic owner revalidates and interprets its
domain value. No crate aliases, re-exports, or wraps these boundary values as its authoritative ID.

The parameter set is a `BTreeMap<NodeCapabilityParameterKey, NodeCapabilityParameterValue>` with at
most 64 entries. Its canonical bytes are entry count as big-endian `u32`, then ascending key/value
pairs; keys and variable bytes use big-endian `u32` lengths and the explicit tags above. Unknown
keys, duplicate keys, wrong variants, and values outside the exact contract are rejected. There is
no null, nested map/list, arbitrary JSON, provider option, or untyped string enum.

`NodeCapabilityParameterContract` declares key, exact variant, Required or Optional-with-default,
and only variant-appropriate bounds/allowed choices. Normalization inserts declared defaults and
returns `NodeCapabilityNormalizedParameters`. It never reads vendor defaults. Construction rejects
duplicate keys, empty/invalid ranges or choices, invalid defaults, duplicate inputs/outputs, no
output, or more than one primary output.

Its constraint is exactly one closed variant: `UnsignedIntegerRange { minimum, maximum }`,
`UnsignedIntegerAllowedValues(non-empty sorted set)`, `TextUtf8Bytes { minimum, maximum }`,
`ChoiceAllowedKeys(non-empty sorted set)`, `GenerationProfileRef`, or
`ManagedAssetId { media_kind }`. Minimum is not greater than maximum. Presence is exactly `Required`
or `OptionalWithDefault(value)`; the default must satisfy the same constraint. Constraint and value
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
    pub inputs: WorkflowNodeInputSet,
}
```

Readiness checks only parameter-selected external state: managed Assets and Generation Profiles.
They do not resolve upstream runtime inputs, dispatch providers, mutate state, or write media. An
empty issue vector means ready; otherwise it contains 1..=64 unique issues sorted by category tag,
target kind, then target bytes. Every external-state issue identifies one parameter key and its typed
Asset ID or Generation Profile ref. At most one issue is returned per parameter; when several observations could
apply, the category table order wins. The capability ref comes from the resolved implementation and
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
origin capability ref to equal its own contract ref before any external boundary. The value is not
sent to a provider request and accompanies every produced-media write unchanged.

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
| `TextToImageCapabilityImpl` | required `generation_profile_ref`; optional `aspect_ratio`, default `square` | required single `prompt`: Text | primary `image`: Image | `ContentGeneration` |
| `ImageToVideoCapabilityImpl` | required `generation_profile_ref`; optional `duration_seconds`, default `5` | required single `image`: Image; optional single `prompt`: Text | primary `video`: Video | `MediaTransformation` |
| `TextToSpeechCapabilityImpl` | required `generation_profile_ref` | required single `text`: Text | primary `audio`: Audio | `ContentGeneration` |

`aspect_ratio` choices are `square`, `landscape_4_3`, `portrait_3_4`, `landscape_16_9`, and
`portrait_9_16`. `duration_seconds` is UnsignedInteger restricted to `{5, 10}`. Every model-powered
contract requires exactly one compatible active Generation Profile. Voice/model/style behavior for
speech belongs to the selected profile, not an extra provider-native node parameter.

All seven return exactly one output. Asset-read capabilities resolve an Available exact-kind Asset
on every readiness check and execution. Generation capabilities never return provider bytes
directly: they validate one provider payload, write it through the media writer, and return one
Available managed reference. No batch count, negative prompt, guidance, safety level, native voice,
native aspect token, or provider seed is an MVP node parameter.

## Implementation Shape

```rust
pub struct ImageToVideoCapabilityImpl<R, A, P, W> {
    generation_profile_catalog: Arc<GenerationProfileCatalog>,
    generation_profile_availability_reader: A,
    managed_media_reader: R,
    image_to_video_provider: P,
    produced_media_writer: W,
    contract: NodeCapabilityContract,
}
```

`ImageToVideoCapabilityImpl`:

1. validates and normalizes its stable parameters;
2. checks only the parameter-selected `GenerationProfileRef` during external readiness;
3. resolves the exact input Image only during execution;
4. converts Workflow values into `ImageToVideoProviderRequest`;
5. calls `ImageToVideoProviderInterface::generate_video_from_image`;
6. stores the already-validated `GeneratedVideoPayload` through
   `NodeCapabilityProducedMediaWriterInterface::write_node_output_media`;
7. returns one managed Video in a complete `WorkflowNodeOutputSet`.

It never reads a provider name, route ID, path, URL, Asset repository, or concrete adapter.

### Generation-Capability Readiness

[`BACKEND.md`](BACKEND.md#active-node-capabilities) owns the complete dependency list for each exact
implementation. Every listed dependency is an independent constructor argument; no generic
generation context or service bundle groups them.

Readiness first requires the complete normalized parameter shape and successful conversion of its
`generation_profile_ref` boundary value. Either failure returns only
`InvalidCapabilityInvocation`. For a converted `GenerationProfileRef`, it then:

1. requires an Active catalog definition compatible with the capability contract;
2. constructs one `GenerationProfileAvailabilityRequest` with the implementation's exact
   capability ref, only that selected profile ref, and the readiness request's unchanged monotonic
   deadline;
3. performs exactly one availability read and requires exactly one matching observation.

A missing, Retired, or incompatible catalog definition becomes
`GenerationProfileIncompatible`. Availability state `Available` returns no issue, `Unavailable`
becomes `GenerationProfileUnavailable`, and `Indeterminate` becomes
`GenerationProfileAvailabilityIndeterminate`.
`AvailabilityRequestInvalid`, `AvailabilityReadFailed`, `DeadlineExceeded`, or an observation set
that would be `InvalidAvailabilityObservation` also becomes
`GenerationProfileAvailabilityIndeterminate`. Every Generation Profile issue above targets
`GenerationProfile { parameter_key: generation_profile_ref, generation_profile_ref }`, contains no
provider detail or retry metadata, and blocks admission. Readiness never resolves runtime inputs,
calls a provider, writes media, retries, caches, or substitutes a profile. It never truncates or
replaces the caller's deadline.

### Generation-Capability Execution

Execution accepts only the exact normalized parameters and complete runtime input set declared by
the resolved contract, plus an origin whose capability ref equals that contract ref. A malformed
direct call or mismatched origin returns `InvalidCapabilityInvocation` at `ResolveInputs` with
target `Capability` before an external boundary. It does not repeat catalog or availability reads;
the selected profile enters the exact provider request unchanged. The complete
`WorkflowNodeExecutionContext` from the
execution request also enters that provider request unchanged; a capability never reconstructs or
replaces its Project, Run, node execution, deadline, or cancellation values.

The exact mappings are:

| Implementation | Provider mapping | Write display name | Write provenance |
| --- | --- | --- | --- |
| `TextToImageCapabilityImpl` | `prompt`, selected profile, normalized `aspect_ratio` | `Generated Image` | `ProviderGenerated { profile_ref }` |
| `ImageToVideoCapabilityImpl` | exact readable Image, optional `prompt`, selected profile, normalized `duration_seconds` | `Generated Video` | `ProviderDerived { source_media_refs: [input image ref], profile_ref }` |
| `TextToSpeechCapabilityImpl` | `text`, selected profile | `Synthesized Speech` | `ProviderGenerated { profile_ref }` |

Every write uses `NodeCapabilityProducedMediaOutputKey` with the request's Workflow Run ID, node
execution ID, declared primary output key, and ordinal `0`. The write request carries both the
execution request's unchanged `WorkflowNodeExecutionContext` and unchanged
`WorkflowNodeExecutionOrigin`. The payload variant must be
`GeneratedImage`, `GeneratedVideo`, or `SynthesizedSpeech` respectively. The writer result must be
the matching Image, Video, or Audio reference and must preserve the payload fingerprint; any writer
kind mismatch preserves `expected` then `observed` in `NodeCapabilityMediaFailure::KindMismatch`.
No capability creates another output, display-name parameter, provider metadata, or Asset directly.

Cancellation and deadline are checked before every external call and after every await;
cancellation wins when both are observed. Image input resolution failures use stage `ResolveInputs`
and target `Input(image)`. Provider failures use `CallProvider` and target `Capability`. Produced
media writer boundary failures use `WriteManagedMedia` and the declared output target.
Provider and media boundary categories are preserved exactly once; no failure triggers retry,
provider resubmission, fallback, cleanup, or a second write.

For the fixed C4 values, `InvalidDisplayName`, `InvalidOutputCoordinates`, or
`InvalidProvenance` while constructing the write request becomes
`NodeCapabilityExecutionFailure::InvalidCapabilityResult` at
`ValidateProviderResult` with the declared output target. A returned reference of another media
kind becomes `KindMismatch { expected, observed }`; a matching-kind reference with another
fingerprint becomes `DigestMismatch`. Both are `WriteManagedMedia` failures with the declared
output target. The capability retains the payload digest solely for this comparison. These mappings
surface a broken boundary without panic and do not turn an implementation defect into
`InvalidCapabilityInvocation`.

After a valid invocation produces all declared runtime values, every capability constructs its
complete `WorkflowNodeOutputSet` exactly once. C3 and C4 implementations use the output-assembly
failure rule defined below.

## MVP External Interfaces

| Interface | Behavior method | Request | Result |
| --- | --- | --- | --- |
| `NodeCapabilityManagedMediaReaderInterface` | `read_managed_media` | `NodeCapabilityManagedMediaReadRequest` | `NodeCapabilityReadableMediaInput` |
| `NodeCapabilityProducedMediaWriterInterface` | `write_node_output_media` | `NodeCapabilityProducedMediaWriteRequest` | typed managed Workflow media ref |
| `TextToImageProviderInterface` | `generate_image_from_text` | `TextToImageProviderRequest` | `GeneratedImagePayload` |
| `ImageToVideoProviderInterface` | `generate_video_from_image` | `ImageToVideoProviderRequest` | `GeneratedVideoPayload` |
| `TextToSpeechProviderInterface` | `synthesize_speech_from_text` | `TextToSpeechProviderRequest` | `SynthesizedSpeechPayload` |

Every exact provider request contains semantic inputs, typed parameters, `GenerationProfileRef`,
and `WorkflowNodeExecutionContext`; that context contains `WorkflowNodeExecutionId`, deadline, and
cancellation. It contains no provider name, native model ID, credential, endpoint, URL, path,
provider task, wire DTO, or generic options map.

The five C2 interfaces and their boundary values are exact. All fields are private with
noun-specific accessors and fallible constructors where invariants exist:

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
`VideoWebm`, `AudioMpeg`, `AudioWav`, or `AudioOgg`. Generated MVP payloads restrict those values to
PNG, MP4, and MPEG respectively. `NodeCapabilityDeclaredMediaFacts` has the same closed Image/Video/Audio fields
and numeric bounds as Asset facts, but is a nodes-owned boundary observation translated by the
Desktop bridge; it is not an Asset-domain alias or a second inspector.

```text
Image { width, height }
Video { width, height, duration_ms, has_audio }
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
`InvalidMediaFacts`, `InvalidByteLength`, `ContentFingerprintMismatch`, `InvalidDisplayName`,
`InvalidOutputCoordinates`, `InvalidProvenance`, and `DeadlineExceeded`. It contains no message,
Asset error, provider error, path, or adapter detail. Boundary implementations translate it once
into the already-frozen `NodeCapabilityMediaFailure` or `NodeCapabilityProviderFailure`; public
interface methods return only those interface-owned failure types.

`NodeCapabilityMediaBoundaryError` is exactly `Media(NodeCapabilityMediaFailure)`, `Cancelled`, or
`DeadlineExceeded`. Both media interfaces return this type. Capabilities map its three variants
mechanically to the existing matching `NodeCapabilityExecutionFailure` variants; cancellation and
deadline are never disguised as storage or finalization failures. A readiness read has no
cancellation signal and therefore never originates `Cancelled`; an execution caller still checks
its cancellation immediately before and after that read. The writer receives the execution context
and may return either call-scoped category.

### Produced-Media Write Boundary

`NodeCapabilityProducedMediaOutputKey` contains Workflow Run ID, node execution ID, declared
`NodeCapabilityOutputKey`, and zero-based `u32` ordinal. Its constructor is infallible from typed
fields. `NodeCapabilityProducedMediaProvenance` is exactly `ProviderGenerated { profile_ref }` or
`ProviderDerived { source_media_refs, profile_ref }`; the latter requires `1..=64` ordered typed
source references. There is no deterministic-derived variant in the active generated capabilities.

`NodeCapabilityProducedMediaWriteRequest` contains `WorkflowNodeExecutionContext`, the exact output
key, the unchanged `WorkflowNodeExecutionOrigin`, a validated 1..=80-scalar
`NodeCapabilityProducedMediaDisplayName`, one provenance value, and exactly one
`GeneratedImagePayload`, `GeneratedVideoPayload`, or `SynthesizedSpeechPayload`. Output-key Run and
node-execution coordinates must match the request's `WorkflowNodeExecutionContext`, which is also
carried for deadline/cancellation. Construction rejects mismatch as
`NodeCapabilityMediaValueError::InvalidOutputCoordinates` before the writer is called.
Empty/oversized provider-derived sources return `InvalidProvenance`.
The display name is trimmed, contains no control scalar, and contains 1..=80 Unicode scalars.

`NodeCapabilityProducedMediaWriterInterface::write_node_output_media` returns the matching typed
`WorkflowManagedImageRef`, `WorkflowManagedVideoRef`, or `WorkflowManagedAudioRef` inside
`NodeCapabilityProducedMediaReference`, or one `NodeCapabilityMediaBoundaryError`. It returns
only an Available reference. It never returns an Asset type, Pending state, path, preview URL,
provider value, partial output set, retry instruction, or second output.

`NodeCapabilityProducedMediaPayload` is the closed union `GeneratedImage`, `GeneratedVideo`, or
`SynthesizedSpeech` over the three named payload types. `NodeCapabilityProducedMediaReference` is
the matching closed `Image`, `Video`, or `Audio` union over engine-owned managed references. Writer
implementations reject a returned kind or fingerprint that differs from the consumed payload.

### Provider Requests And Payloads

`ImageAspectRatio` is the closed union `Square`, `LandscapeFourByThree`, `PortraitThreeByFour`,
`LandscapeSixteenByNine`, and `PortraitNineBySixteen`. `ImageToVideoDurationSeconds` is exactly
`Five` or `Ten`; neither type accepts a provider-native string or number.

- `TextToImageProviderRequest` contains `GenerationProfileRef`, `WorkflowNodeExecutionContext`,
  `WorkflowTextValue`, and `ImageAspectRatio`.
- `ImageToVideoProviderRequest` contains `GenerationProfileRef`,
  `WorkflowNodeExecutionContext`, one `NodeCapabilityReadableImageInput`, optional
  `WorkflowTextValue`, and `ImageToVideoDurationSeconds`.
- `TextToSpeechProviderRequest` contains `GenerationProfileRef`, `WorkflowNodeExecutionContext`, and
  `WorkflowTextValue`.

Each request constructor checks only its typed shape; profile compatibility/availability is owned by
readiness and routers. The three provider methods return their named payload or
`NodeCapabilityProviderFailure` unchanged.

`GeneratedImagePayload`, `GeneratedVideoPayload`, and `SynthesizedSpeechPayload` are distinct
non-convertible wrappers around one matching `NodeCapabilityMediaSourceLease` and matching declared
facts. Their MIME and maximum byte lengths are exactly PNG/32 MiB, MP4/512 MiB, and MPEG/64 MiB.
Construction rejects wrong MIME/facts and zero/oversized/unknown length as
`NodeCapabilityMediaValueError`; every provider implementation maps that failure to a
`NodeCapabilityProviderFailure` whose category is `InvalidResponse`. Every provider implementation
must verify exact stream length and digest before returning; the shared contract suite proves the
returned stream matches those facts. Payloads contain no second output, URL, provider
task, native model, response DTO, or arbitrary metadata.

`NodeCapabilityExecutionDeadline::monotonic_instant` returns its exact wrapped `Instant` solely for
consumer-owned boundary translation. It does not extend, replace, serialize, or create a deadline.

The media-write request includes `NodeCapabilityProducedMediaOutputKey`, derived from Workflow Run,
node execution, output key, and ordinal. The Desktop bridge translates it to the Asset-owned
`AssetNodeOutputKey`. Repeating the same key and digest returns the same Asset; a different digest
is a conflict.

### C2 Shared Fake Contracts

Each of the five interfaces has one reusable parameterized contract suite run unchanged against its
deterministic fake and later production adapter/router. Reader and writer suites cover exact typed
kind, Project isolation input, fingerprint preservation, one-shot stream ownership, deadline,
every media-error mapping, output-key replay/conflict, and Available-only
write results. Each provider suite covers every semantic request field, fixed output MIME/kind/size,
one-shot exact-length/digest bytes, provider failure propagation, execution-context preservation,
and absence of provider-native fields. Fakes are named
`NodeCapabilityManagedMediaReaderFakeImpl`, `NodeCapabilityProducedMediaWriterFakeImpl`,
`TextToImageProviderFakeImpl`, `ImageToVideoProviderFakeImpl`, and
`TextToSpeechProviderFakeImpl`; they have no unsupported branch or behavior not present in the
interface.

## Named Roadmap Contracts

These operations have approved semantic names but are not part of the MVP registry. Their traits
and implementations are introduced only when scheduled; this table prevents future provider-shaped
or ambiguous names.

| Planned contract ref | Reserved implementation name | Reserved external interface and method |
| --- | --- | --- |
| `image.generate_from_image@1.0` | `ImageToImageCapabilityImpl` | `ImageToImageProviderInterface::generate_image_from_image` |
| `image.generate_from_reference_images@1.0` | `ReferenceImagesToImageCapabilityImpl` | `ReferenceImagesToImageProviderInterface::generate_image_from_reference_images` |
| `image.crop@1.0` | `ImageCropCapabilityImpl` | `ImageCropInterface::crop_image` |
| `video.generate_from_text@1.0` | `TextToVideoCapabilityImpl` | `TextToVideoProviderInterface::generate_video_from_text` |
| `video.generate_from_reference_images@1.0` | `ReferenceImagesToVideoCapabilityImpl` | `ReferenceImagesToVideoProviderInterface::generate_video_from_reference_images` |
| `video.generate_from_first_frame@1.0` | `FirstFrameToVideoCapabilityImpl` | `FirstFrameToVideoProviderInterface::generate_video_from_first_frame` |
| `video.generate_from_first_and_last_frames@1.0` | `FirstAndLastFramesToVideoCapabilityImpl` | `FirstAndLastFramesToVideoProviderInterface::generate_video_from_first_and_last_frames` |
| `video.generate_from_mixed_media@1.0` | `MixedMediaToVideoCapabilityImpl` | `MixedMediaToVideoProviderInterface::generate_video_from_mixed_media` |
| `video.upscale@1.0` | `VideoUpscaleCapabilityImpl` | `VideoUpscaleProviderInterface::upscale_video` |
| `video.extract_frames@1.0` | `VideoFrameExtractionCapabilityImpl` | `VideoFrameExtractionInterface::extract_video_frames` |
| `video.concatenate@1.0` | `VideoConcatenationCapabilityImpl` | `VideoConcatenationInterface::concatenate_videos` |
| `video.analyze_storyboard@1.0` | `VideoStoryboardAnalysisCapabilityImpl` | `VideoStoryboardAnalysisProviderInterface::analyze_storyboard_from_video` |
| `text.generate_from_text@1.0` | `TextToTextGenerationCapabilityImpl` | `TextToTextGenerationProviderInterface::generate_text_from_text` |
| `text.generate_from_mixed_media@1.0` | `MixedMediaToTextGenerationCapabilityImpl` | `MixedMediaToTextGenerationProviderInterface::generate_text_from_mixed_media` |
| `audio.generate_music_from_text@1.0` | `TextToMusicCapabilityImpl` | `TextToMusicProviderInterface::generate_music_from_text` |

The distinctions are contractual:

- `ImageToVideo` uses an image as conditioning input but does not promise exact first-frame pixels;
- `FirstFrameToVideo` guarantees the supplied image is the first frame;
- `FirstAndLastFramesToVideo` guarantees both endpoint frames;
- reference-image operations use ordered role-bearing subject/style/material references;
- mixed-media operations accept typed Image and Video items with explicit roles;
- speech and music remain separate operations, profiles, results, and failure semantics.

Activating a roadmap operation requires its complete parameter, input-role, result, error, mock,
provider/media interface, Asset provenance, UI contract, and E2E tests in the same change.

## Results And Errors

Provider media payloads contain one fixed media kind, declared facts, and a bounded asynchronous
byte stream. Text and Storyboard results are validated semantic values, never provider JSON hidden
inside a string.

A capability returns all declared outputs or one structured error. Required media becomes a
Workflow output only after every corresponding Asset is Available; partial Asset creation never
becomes a partial `WorkflowNodeOutputSet`.

`NodeCapabilityParameterError` and `NodeCapabilityReadinessIssue` occur before dispatch.
`NodeCapabilityProviderFailure` uses closed categories, retryability, and optional safe retry time.
`NodeCapabilityMediaFailure` uses closed categories without authorizing retry.
`NodeCapabilityExecutionError` adds capability, stage, and safe target. Raw provider text, native
IDs, URLs, paths, credentials, and response bodies never cross these errors.

The closed parameter categories are `UnknownParameter`, `RequiredParameterMissing`,
`ParameterValueKindMismatch`, `ParameterValueOutOfBounds`, `ParameterChoiceNotDeclared`, and
`ParameterSetTooLarge`. Readiness categories are `InvalidCapabilityInvocation`, `ManagedAssetUnavailable`,
`ManagedAssetKindMismatch`, `ManagedAssetReadinessIndeterminate`,
`GenerationProfileIncompatible`, `GenerationProfileUnavailable`, and
`GenerationProfileAvailabilityIndeterminate`. Execution stages are `ResolveInputs`, `CallProvider`,
`ValidateProviderResult`, `WriteManagedMedia`, and `AssembleOutputs`; normalization has its own
pre-admission result.
`NodeCapabilityExecutionError` wraps one `NodeCapabilityExecutionFailure` with contract ref, node
execution ID, stage, and a structured safe target.

The error values are closed and field-exact:

- `NodeCapabilityParameterError` contains one parameter category and
  `NodeCapabilityParameterErrorTarget`, either `ParameterSet` or `Parameter(key)`;
- `NodeCapabilityReadinessIssue` contains one readiness category and
  `NodeCapabilityReadinessTarget`. The target is exactly
  `Capability`, `ManagedAsset { parameter_key, asset_id }`, or
  `GenerationProfile { parameter_key, generation_profile_ref }`; kind-mismatch detail additionally
  contains expected and observed `WorkflowDataType` values;
- `NodeCapabilityProviderFailure` contains its provider category, retryable flag, and optional safe
  retry instant; the category-specific retry rules remain owned by `BACKEND_PROVIDERS.md`;
- `NodeCapabilityMediaFailure` contains exactly one of `Unavailable`,
  `KindMismatch { expected: WorkflowDataType, observed: WorkflowDataType }`, `InvalidMedia`,
  `SizeLimitExceeded`, `DigestMismatch`, `OutputConflict`, `StorageFailed`, `InspectionFailed`,
  or `FinalizationFailed`, plus no adapter text;
- `NodeCapabilityExecutionError` contains contract ref, node execution ID, stage, one
  `NodeCapabilityExecutionFailure`, and `NodeCapabilityExecutionTarget` (`Capability`, parameter key,
  input key, or output key). Its failure is exactly Readiness, Provider, Media, Cancelled, or
  DeadlineExceeded, plus `InvalidCapabilityInvocation` only when a direct execution request does not
  satisfy the already-resolved capability's normalized-parameter/input contract or carries another
  capability contract ref in its origin, and `InvalidCapabilityResult` only when capability-owned
  fixed result construction or final output-set validation fails.

Construction rejects an execution target inconsistent with its stage: ResolveInputs targets a
parameter, input, or capability; CallProvider targets the capability; result validation and media
write target an output or capability; output assembly targets only an output. Readiness targets
`Capability` only for invalid invocation and a declared parameter for every external-state issue.
Cancellation/deadline use
the operation target active when observed; no absent-key convention carries target meaning.

`InvalidCapabilityInvocation` is non-retryable, has stage `ResolveInputs` and target `Capability`,
and carries no field, message, supplied value, or validation detail. It is never used for invalid raw
parameters before admission, provider responses, media failures, or internal adapter failures. Its
only purpose is to let a capability reject a malformed direct trait invocation without panic,
provider dispatch, media read/write, or misclassifying the failure as another business category.

`InvalidCapabilityResult` is non-retryable and carries no internal error, message, or invalid
value. Fixed write-request construction uses `ValidateProviderResult`; final output-set validation
uses `AssembleOutputs`. Both target the declared output. It never represents provider rejection,
media boundary failure, cancellation, deadline, or caller input.

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
not retryable. Safe identifiers may cross the boundary; message text, provider IDs, URLs, paths,
credentials, response bodies, and adapter errors may not.

## Contract Tests

- registry tests reject duplicate refs and assert the exact seven-contract MVP set;
- every active implementation passes the shared `WorkflowNodeCapabilityInterface` behavior suite;
- exact tests cover normalization, stable input order, request mapping, result validation,
  provenance, idempotent media write, and error translation;
- deterministic and production routers pass the same exact provider-interface suites;
- fault-injection tests cover cancellation, timeout, malformed payload, Asset write failure, and
  duplicate output-key conflict;
- architecture tests reject a second catalog/executor, roadmap registration, broad provider
  interface, optional unsupported method, generic options map, and concrete adapter selection
  outside composition.
