# Backend Terminology And Naming

> Status: authoritative for the backend MVP architecture
> Scope: English business terms, Rust type names, layers, and dependency boundaries

## Purpose

This document gives every backend concept one meaning and every architectural type one readable
role. A type name must answer, without opening its module:

1. which business context owns the concept;
2. which architectural layer the type belongs to;
3. what responsibility or capability the type represents.

All other `BACKEND*.md` documents use this vocabulary. New synonyms require changing this document
first. Assistant terminology remains unchanged and outside this naming pass.

## Ownership Prefixes

| Prefix | Ownership level | Examples |
| --- | --- | --- |
| `Workflow` | Workflow bounded context: graph, revision, plan, and Run lifecycle | `WorkflowAggregate`, `WorkflowRunAggregate` |
| `NodeCapability` | executable sub-capability inside the Workflow bounded context | `WorkflowNodeCapabilityPort`, `TextToVideoCapability` |
| `GenerationProfile` | provider-independent generation choice inside the NodeCapability sub-capability | `GenerationProfileRef`, `GenerationProfileAvailabilityReaderPort` |
| `Asset` | Asset bounded context: project-local managed media and availability | `AssetAggregate`, `AssetManagedContentState` |
| `ProviderCredential` | Desktop-owned provider secret persisted only in encrypted form | `ProviderCredentialId`, `ProviderCredentialSecretValue` |
| exact `*Provider` role | model-powered boundary consumed by one exact node capability | `TextToVideoProviderPort` |
| `Desktop` | Tauri host: transport, projection, task hosting, configuration, composition | `DesktopErrorDto`, `DesktopCompositionRoot` |

The MVP has two bounded contexts: Workflow and Asset. `Project` is shared application scope, not a
third context. Provider adapters and the Desktop host are boundary code, not semantic owners.

## Layers Inside A Context

Each business context is organized by capability first, then by DDD layer:

```text
<context>/
  domain/          aggregates, entities, value objects, policies, domain errors
  application/     use cases, commands, queries, results
  ports/           consumer-owned traits needed by application or domain code
  infrastructure/  concrete adapters when the crate owns them
```

The repository must not create global `domain`, `services`, `repositories`, or `dto` buckets. A
layer name without its business context hides ownership.

## Role Suffixes

| Suffix | Layer and meaning | Example |
| --- | --- | --- |
| `*Aggregate` | domain aggregate root; owns invariants and legal transitions | `AssetAggregate` |
| `*Entity` | mutable domain identity owned by an aggregate | `WorkflowNodeEntity` |
| `*State` | closed domain lifecycle state | `WorkflowRunState` |
| `*Value` | immutable domain value object | `WorkflowRuntimeValue` |
| `*Id`, `*Key`, `*Ref`, `*Set` | precise immutable domain value or collection | `WorkflowNodeId`, `WorkflowNodeOutputSet` |
| `*Version`, `*Revision`, `*Scope` | precise immutable domain coordinate | `WorkflowSchemaVersion`, `WorkflowRunScope` |
| `*Lease` | bounded permission to use an external resource | `AssetPreviewLease` |
| `*Payload` | bounded data crossing one named port | `GeneratedVideoPayload` |
| `*Kind`, `*Reason` | closed domain classification with no lifecycle | `AssetMediaKind` |
| `*Action` | one member of a closed application command union | `WorkflowMutationAction` |
| `*Input`, `*Stream` | precisely named port data, never a domain aggregate | `NodeCapabilityReadableMediaInput` |
| `*Context` | call-scoped execution identity and controls, never dependencies | `WorkflowNodeExecutionContext` |
| `*ProviderRequest` | semantic input owned by a provider port consumer | `TextToVideoProviderRequest` |
| `*Contract` | immutable, versioned domain contract | `NodeCapabilityContract` |
| `*Capability` | exact implementation of `WorkflowNodeCapabilityPort` | `TextToVideoCapability` |
| `*Policy` | pure domain decision with no external I/O | `WorkflowReadinessPolicy` |
| `*Command` | application input requesting a state change | `StartWorkflowRunCommand` |
| `*Query` | application input requesting data without a state change | `ListAssetsQuery` |
| `*Result` | application output from one named use case | `StartWorkflowRunResult` |
| `*UseCase` | application orchestrator for one user intention | `ImportAssetUseCase` |
| `*Port` | consumer-owned trait crossing a substitution or external boundary | `AssetAggregateRepositoryPort` |
| `*Adapter` | concrete infrastructure implementation of a port | `SqliteAssetAggregateRepositoryAdapter` |
| `*Row` | private persistence representation | `SqliteAssetAggregateRow` |
| `*Dto` | Tauri or provider wire representation | `WorkflowDto` |
| `*View` | read-only presentation projection assembled for a UI need | `WorkflowNodePresentationView` |
| `*Host` | Desktop process owner for task or protocol lifetime | `DesktopWorkflowRunTaskHost` |
| `*Event` | immutable domain or application fact that already occurred | `WorkflowRunStateChangedEvent` |
| `*Error` | structured failure owned by the named context or boundary | `AssetApplicationError` |
| `*Config` | validated startup configuration | `FalProviderAccountConfig` |

`State`, `Policy`, and `Error` are explicit roles, not permission to use vague prefixes. For
example, `RunState` is invalid because its owning context is unclear; use `WorkflowRunState`.

## Identifier Names

Identifiers include the owning concept. Short generic IDs are not used in public contracts.

| Use | Required name |
| --- | --- |
| Workflow | `WorkflowId` |
| Workflow node | `WorkflowNodeId` |
| Workflow input item | `WorkflowInputItemId` |
| Workflow Run | `WorkflowRunId` |
| node execution inside a Run | `WorkflowNodeExecutionId` |
| Asset | `AssetId` |
| immutable managed bytes | `AssetManagedContentId` |
| Asset managed-content finalization | `AssetManagedContentFinalizationId` |
| configured provider account | `ProviderAccountId` |
| encrypted provider credential | `ProviderCredentialId` |

Local variables may be shorter when their type and scope are obvious. Serialized fields keep the
same semantic name, such as `workflow_run_id`, rather than collapsing it to `run_id` in a mixed
boundary object.

## Authoritative Domain Types

### Workflow Context

| Type | Meaning |
| --- | --- |
| `WorkflowAggregate` | one revisioned editable graph |
| `WorkflowNodeEntity` | one graph node selecting one exact capability contract |
| `WorkflowInputTargetValue` | one target node and named input port |
| `WorkflowInputBindingValue` | one explicit single-value or ordered-reference target binding |
| `WorkflowInputItemEntity` | one stable, optionally role-bearing directed graph edge inside a binding |
| `NodeCapabilityInputRoleKey` | stable role key interpreted only by its exact capability module |
| `WorkflowAcceptedDataTypeSet` | non-empty concrete type set accepted by one reference role |
| `WorkflowRunAggregate` | one execution of one frozen Workflow revision |
| `WorkflowNodeExecutionEntity` | state, progress, error, and outputs for one node in one Run |
| `WorkflowRunScope` | whole graph or one node with all ancestors |
| `WorkflowDataType` | exact Text, Image, Video, Audio, ImageSequence, or VideoStoryboard port type |
| `WorkflowTextValue` | bounded structured literal text and stable input-item references |
| `WorkflowTextPartValue` | one literal segment or one stable input-item reference |
| `WorkflowRuntimeValue` | typed runtime text, managed Asset reference, image sequence, or storyboard |
| `WorkflowImageSequenceValue` | ordered timestamped managed Image references |
| `WorkflowVideoStoryboardValue` | ordered validated scenes and overall summary |
| `WorkflowRuntimeInputItemValue` | stable item identity, optional role, and concrete runtime value |
| `WorkflowNodeInputValue` | explicit single value or semantically ordered reference sequence |
| `WorkflowMediaPreviewValue` | opaque scoped preview access returned through a Workflow port |
| `WorkflowExecutionPlanValue` | immutable prepared node order and input bindings |
| `WorkflowPlannedNodeIdentityValue` | canonical identity of one frozen node and structural bindings |
| `WorkflowNodeDispatchIdentityValue` | planned identity plus fully resolved concrete input identities |
| `WorkflowMutationAction` | one atomic edit inside `ApplyWorkflowMutationCommand` |
| `WorkflowNodeInputSet` | named runtime inputs bound by the execution plan |
| `WorkflowNodeOutputSet` | complete named runtime outputs from one capability implementation |
| `WorkflowNodePresentationView` | application projection for one visible node shell |

The words `Node`, `Edge`, `Run`, `Value`, `Input`, and `Output` are never standalone public type
names.

### NodeCapability Sub-Capability

| Type | Meaning |
| --- | --- |
| `NodeCapabilityContractRef` | immutable capability ID and version |
| `NodeCapabilityContract` | shared Workflow-domain shape for one complete versioned contract |
| `WorkflowNodeCapabilityPort` | Workflow-owned interface implemented by every exact node capability |
| `WorkflowNodeCapabilityRegistry` | concrete immutable collection of exact capability implementations |
| `TextToVideoCapability` | exact implementation of `video.generate_from_text` |
| `NodeCapabilityExecutionKind` | PureValue, ManagedAssetRead, ContentGeneration, MediaTransformation, or ContentAnalysis |
| `NodeCapabilityParameterContract` | one declarative parameter rule |
| `NodeCapabilityInputPortContract` | one named input and its binding rule |
| `NodeCapabilityInputBindingContract` | optional value, required value, or ordered-reference rule |
| `NodeCapabilityOutputPortContract` | one named output and its exact Workflow data type |
| `NodeCapabilityInputPortKey` | stable input key within one capability contract version |
| `NodeCapabilityOutputPortKey` | stable output key within one capability contract version |
| `NodeCapabilityInputRoleKey` | exact capability-owned role identity persisted mechanically by Workflow |
| `NodeCapabilityParameterSet` | normalized parameter values for one node capability |
| `NormalizedNodeCapabilityParameters` | normalized parameter set plus mechanically projected input-item references |
| `GenerationProfileRef` | immutable provider-independent profile ID and version persisted by model-powered nodes |
| `GenerationProfileDefinition` | immutable profile identity, lifecycle, and compatibility entries |
| `GenerationProfileLifecycleState` | Active, Deprecated, or Retired lifecycle of one exact profile |
| `GenerationProfileAvailabilityState` | current Available, Unavailable, or Indeterminate operational observation |
| `GenerationProfileAvailabilityObservation` | expiring availability observation for one exact profile/capability pair |
| `NodeCapabilityGenerationProfileView` | application projection of one generation profile selectable by an exact node capability |
| `NodeCapabilityAssetRefValue` | opaque project Asset reference in an Asset source parameter |
| `NodeCapabilityReadableMediaInput` | bounded readable input prepared for a provider port |
| `GeneratedImagePayload` | kind-safe pre-storage Image result from an image-generation provider port |
| `GeneratedVideoPayload` | kind-safe pre-storage Video result from a video-generation provider port |
| `SynthesizedSpeechPayload` | kind-safe pre-storage Audio result from speech synthesis |
| `GeneratedMusicPayload` | kind-safe pre-storage Audio result from music generation |
| `NodeCapabilityProducedMediaPayload` | validated generated-or-derived media translated for Asset storage |
| `NodeCapabilityProducedMediaStream` | bounded asynchronous pre-storage byte stream |
| `WorkflowNodeExecutionContext` | call-scoped identity, cancellation, deadline, and progress |

`WorkflowNodeProducedMediaOriginValue` carries the producing Workflow, revision, Run, node,
capability, production kind, and output port to `NodeCapabilityProducedMediaWriterPort`. Its
generated variant carries the selected profile, its deterministic-derived variant carries source
Asset identities, and its model-derived variant carries both. The Desktop bridge translates it to
`AssetOriginValue`.

`crates/engine` owns the shared contract shape, validation invariants, capability interface, and
registry. Each exact type in `crates/nodes` implements that interface and owns one contract plus its
parameter, readiness, and execution semantics. There is no separate catalog/executor pair.

The UI shell is not a domain node kind. `WorkflowNodeShellKindDto` is a Desktop DTO derived from the
primary output's `WorkflowDataType`; it is never persisted in `WorkflowAggregate`.

### Asset Context

| Type | Meaning |
| --- | --- |
| `AssetAggregate` | one project-local logical media item |
| `AssetMediaKind` | exact `Image`, `Video`, or `Audio` kind |
| `AssetMediaMimeTypeValue` | MIME verified from managed Asset bytes |
| `AssetManagedContentState` | `Pending`, `Available`, or `Missing` managed-byte state |
| `AssetManagedContentDescriptorValue` | content identity, digest, length, MIME, and kind |
| `AssetMediaFactsValue` | verified image, video, or audio technical facts |
| `AssetOriginValue` | imported, generated-by-node, deterministic-derived, or model-derived provenance |
| `AssetManagedContentLease` | bounded opaque read access to managed bytes |
| `AssetPreviewLease` | short-lived, project-scoped permission to preview an Asset |

### Provider Credential Boundary

| Type | Meaning |
| --- | --- |
| `ProviderAccountId` | stable local identity of one configured provider account |
| `ProviderCredentialId` | stable local identity of one encrypted provider credential |
| `ProviderCredentialSecretValue` | plaintext provider secret held only during one application call |

`ProviderCredentialSecretValue` is never a domain field, persistence Row, DTO, log value, or error
detail.

`Content` and `Media` are not standalone public types. Use `AssetManagedContent...` for stored bytes,
`AssetMedia...` for verified media semantics, behavior-named payloads for exact provider results,
and `NodeCapabilityProducedMedia...` for the Asset-write boundary.

## Application And Port Names

Application orchestrators are named for one use case. Do not create `WorkflowService`,
`RunService`, `AssetService`, or `ApplicationService`.

```text
ApplyWorkflowMutationUseCase
StartWorkflowRunUseCase
CancelWorkflowRunUseCase
GetWorkflowNodePresentationUseCase
ListNodeCapabilityGenerationProfilesUseCase
ImportAssetUseCase
RecordNodeProducedAssetUseCase
ResolveAssetContentUseCase
IssueAssetPreviewUseCase
```

Ports name the consumer context and the provided capability:

```text
WorkflowAggregateRepositoryPort
WorkflowRunRepositoryPort
WorkflowNodeCapabilityPort
WorkflowMediaPreviewIssuerPort
NodeCapabilityManagedMediaReaderPort
NodeCapabilityProducedMediaWriterPort
GenerationProfileAvailabilityReaderPort
TextToImageProviderPort
FirstAndLastFramesToVideoProviderPort
MultimodalToTextProviderPort
TextToSpeechProviderPort
TextToMusicProviderPort
VideoStoryboardProviderPort
ImageCropPort
VideoFrameExtractionPort
VideoConcatenationPort
AssetAggregateRepositoryPort
AssetIngestTransactionPort
AssetManagedContentStorePort
AssetMediaInspectorPort
DesktopBackendConfigReaderPort
DesktopProviderCredentialRepositoryPort
```

`Repository` may appear only inside a `*RepositoryPort` or a concrete `*RepositoryAdapter`. `Store`
may appear only when byte storage is the actual capability, as in `AssetManagedContentStorePort`.

Provider adapter role names are explicit:

- `*ProviderPort` is a public capability-owned model-operation contract;
- `*ProviderRouterAdapter` selects one profile-compatible route and implements the public port;
- `*ProviderRoutePort` is the router-owned private substitution interface;
- `<Vendor>*ProviderRoute` translates and executes one configured native provider route;
- standalone `Provider`, `Router`, `Route`, `Model`, or `Executor` type names are prohibited.

## Representation Boundaries

The same concept may have several representations, but only the domain model owns semantics:

```text
ApplyWorkflowMutationRequestDto
  -> ApplyWorkflowMutationCommand
  -> WorkflowAggregate
  -> SqliteWorkflowAggregateRow

WorkflowAggregate
  -> WorkflowDto
  -> WorkflowEditorView
```

Translations are explicit and directional. A `*Dto`, `*Row`, or `*View` must not expose methods that
decide business validity or legal state transitions.

Provider protocol types include the provider and operation in their private adapter name, for
example `FalTextToVideoRequestDto`. They never reuse capability request types as wire
DTOs.

Storage representations include technology, business ownership, and persistence role:

```text
SqliteWorkflowAggregateRow
SqliteWorkflowRunAggregateRow
SqliteWorkflowNodeExecutionRow
SqliteWorkflowNodeExecutionOutputRow
SqliteWorkflowRunEventRow
SqliteAssetAggregateRow
SqliteAssetManagedContentFinalizationRow
SqliteProviderCredentialRow
```

These are private infrastructure types, not table names and not domain models. Storage naming and
encoding are defined in [`BACKEND_STORAGE.md`](BACKEND_STORAGE.md).

Concrete encrypted credential persistence is named
`SqliteEncryptedProviderCredentialRepositoryAdapter`: the name identifies technology, protection,
business concept, and adapter role without exposing that mechanism to business code.

## Dependency Injection Vocabulary

Dependency injection means explicit constructor injection:

```rust
pub struct StartWorkflowRunUseCase<R> {
    workflow_run_repository: R,
    node_capabilities: WorkflowNodeCapabilityRegistry,
}
```

Generic bounds are the corresponding consumer-owned `*Port` traits; the capability registry is a
stable concrete collection of `WorkflowNodeCapabilityPort` implementations. Call-scoped values such as
Project identity, request identity, deadlines, and cancellation belong in commands or execution
contexts, not constructors.

Only `src-tauri/composition.rs` selects and constructs concrete adapters. Business code must not use
a service locator, mutable global, adapter registry lookup, downcast, implementation-name switch, or
concrete adapter type in a constructor.

## Prohibited Ambiguous Names

Do not introduce these standalone public type names:

```text
Data             Item             Manager          Operation
Value            Result           Service          Store
Repository       Context          Config           State
Model            Resource         Content          MediaType
Node             Edge             Run              Asset
Provider         Router           Binding          Executor
Registry         Availability
```

The words may appear inside a precise compound name with an architectural role, such as
`WorkflowRuntimeValue`, `StartWorkflowRunResult`, or `AssetManagedContentStorePort`.

Avoid abbreviations other than established protocol terms such as `Id`, `Dto`, `MIME`, and `URL`.
Rust type names use `Dto`, while prose may use DTO.

## Naming Review Checklist

Before adding or renaming a public type, verify:

- its prefix identifies the bounded context or exact cross-context capability;
- its suffix identifies domain, application, port, adapter, persistence, or transport role;
- it has one semantic owner and does not repeat business rules in a DTO, Row, or View;
- it cannot be confused with another representation of the same concept;
- it describes current MVP behavior rather than a speculative platform abstraction;
- it is listed here when it introduces a new authoritative term.
