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
| `NodeCapability` | executable sub-capability inside the Workflow bounded context | `NodeCapabilityContract`, `NodeCapabilityExecutorPort` |
| `Asset` | Asset bounded context: project-local managed media and availability | `AssetAggregate`, `AssetManagedContentState` |
| exact `*Provider` role | external generation boundary consumed by a node capability | `TextToImageProviderPort` |
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
| `*Payload` | bounded data crossing one named port | `NodeCapabilityGeneratedMediaPayload` |
| `*Kind`, `*Reason` | closed domain classification with no lifecycle | `AssetMediaKind` |
| `*Action` | one member of a closed application command union | `WorkflowMutationAction` |
| `*Input`, `*Stream` | precisely named port data, never a domain aggregate | `NodeCapabilityReadableMediaInput` |
| `*Context` | call-scoped execution identity and controls, never dependencies | `WorkflowNodeExecutionContext` |
| `*ProviderRequest` | semantic input owned by a provider port consumer | `TextToImageProviderRequest` |
| `*Contract` | immutable, versioned domain contract | `NodeCapabilityContract` |
| `*Policy` | pure domain decision with no external I/O | `WorkflowReadinessPolicy` |
| `*Command` | application input requesting a state change | `StartWorkflowRunCommand` |
| `*Query` | application input requesting data without a state change | `ListAssetsQuery` |
| `*Result` | application output from one named use case | `StartWorkflowRunResult` |
| `*UseCase` | application orchestrator for one user intention | `ImportAssetUseCase` |
| `*Port` | consumer-owned trait crossing a substitution or external boundary | `AssetAggregateRepositoryPort` |
| `*Adapter` | concrete infrastructure implementation of a port | `SqliteAssetAggregateRepositoryAdapter` |
| `*Row` | private persistence representation | `SqliteAssetRow` |
| `*Dto` | Tauri or provider wire representation | `WorkflowDto` |
| `*View` | read-only presentation projection assembled for a UI need | `WorkflowNodePresentationView` |
| `*Host` | Desktop process owner for task or protocol lifetime | `DesktopWorkflowRunTaskHost` |
| `*Event` | immutable domain or application fact that already occurred | `WorkflowRunStateChangedEvent` |
| `*Error` | structured failure owned by the named context or boundary | `AssetApplicationError` |
| `*Config` | validated startup configuration | `TextToImageProviderConfig` |

`State`, `Policy`, and `Error` are explicit roles, not permission to use vague prefixes. For
example, `RunState` is invalid because its owning context is unclear; use `WorkflowRunState`.

## Identifier Names

Identifiers include the owning concept. Short generic IDs are not used in public contracts.

| Use | Required name |
| --- | --- |
| Workflow | `WorkflowId` |
| Workflow node | `WorkflowNodeId` |
| Workflow edge | `WorkflowEdgeId` |
| Workflow Run | `WorkflowRunId` |
| node execution inside a Run | `WorkflowNodeExecutionId` |
| Asset | `AssetId` |
| immutable managed bytes | `AssetManagedContentId` |
| idempotent application request | `ApplicationRequestId` |

Local variables may be shorter when their type and scope are obvious. Serialized fields keep the
same semantic name, such as `workflow_run_id`, rather than collapsing it to `run_id` in a mixed
boundary object.

## Authoritative Domain Types

### Workflow Context

| Type | Meaning |
| --- | --- |
| `WorkflowAggregate` | one revisioned editable graph |
| `WorkflowNodeEntity` | one graph node selecting one exact capability contract |
| `WorkflowEdgeEntity` | one typed connection between named ports |
| `WorkflowRunAggregate` | one execution of one frozen Workflow revision |
| `WorkflowNodeExecutionEntity` | state, progress, error, and outputs for one node in one Run |
| `WorkflowRunScope` | whole graph or one node with all ancestors |
| `WorkflowDataType` | exact `Text`, `Image`, `Video`, or `Audio` port type |
| `WorkflowRuntimeValue` | typed runtime text or managed Asset reference |
| `WorkflowMediaPreviewValue` | opaque scoped preview access returned through a Workflow port |
| `WorkflowExecutionPlanValue` | immutable prepared node order and input bindings |
| `WorkflowMutationAction` | one atomic edit inside `ApplyWorkflowMutationCommand` |
| `WorkflowNodeInputSet` | named runtime inputs bound by the execution plan |
| `WorkflowNodeOutputSet` | complete named runtime outputs from an executor |
| `WorkflowNodePresentationView` | application projection for one visible node shell |

The words `Node`, `Edge`, `Run`, `Value`, `Input`, and `Output` are never standalone public type
names.

### NodeCapability Sub-Capability

| Type | Meaning |
| --- | --- |
| `NodeCapabilityContractRef` | immutable capability ID and version |
| `NodeCapabilityContract` | shared Workflow-domain shape for one complete versioned contract |
| `NodeCapabilityEffectKind` | Pure, ManagedRead, or ExternalGeneration classification |
| `NodeCapabilityParameterContract` | one declarative parameter rule |
| `NodeCapabilityInputPortContract` | one named input and its exact Workflow data type |
| `NodeCapabilityOutputPortContract` | one named output and its exact Workflow data type |
| `NodeCapabilityInputPortKey` | stable input key within one capability contract version |
| `NodeCapabilityOutputPortKey` | stable output key within one capability contract version |
| `NodeCapabilityParameterSet` | normalized parameter values for one node capability |
| `NodeCapabilityAssetRefValue` | opaque project Asset reference in an Asset source parameter |
| `NodeCapabilityReadableMediaInput` | bounded readable input prepared for a provider port |
| `NodeCapabilityGeneratedMediaPayload` | bounded pre-storage stream plus declared media facts |
| `NodeCapabilityGeneratedMediaKind` | pre-storage Image, Video, or Audio classification |
| `NodeCapabilityGeneratedMediaMimeTypeValue` | provider-declared MIME before Asset validation |
| `NodeCapabilityGeneratedMediaStream` | bounded asynchronous pre-storage byte stream |
| `WorkflowNodeExecutionContext` | call-scoped identity, cancellation, deadline, and progress |

`WorkflowGeneratedMediaOriginValue` carries the producing Workflow, revision, Run, node, capability,
and output port to `NodeCapabilityGeneratedMediaWriterPort`; the Desktop bridge translates it to
`AssetOriginValue`.

`crates/engine` owns the shared contract shape and validation invariants. Each exact module in
`crates/nodes` owns one contract instance, its parameter semantics, and its executor. There is no
second graph-side capability model.

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
| `AssetOriginValue` | imported or generated provenance |
| `AssetManagedContentLease` | bounded opaque read access to managed bytes |
| `AssetPreviewLease` | short-lived, project-scoped permission to preview an Asset |

`Content` and `Media` are not standalone public types. Use `AssetManagedContent...` for stored bytes,
`AssetMedia...` for verified media semantics, and `NodeCapabilityGeneratedMedia...` for pre-storage
provider output.

## Application And Port Names

Application orchestrators are named for one use case. Do not create `WorkflowService`,
`RunService`, `AssetService`, or `ApplicationService`.

```text
ApplyWorkflowMutationUseCase
StartWorkflowRunUseCase
CancelWorkflowRunUseCase
GetWorkflowNodePresentationUseCase
ImportAssetUseCase
RecordGeneratedAssetUseCase
ResolveAssetContentUseCase
IssueAssetPreviewUseCase
```

Ports name the consumer context and the provided capability:

```text
WorkflowAggregateRepositoryPort
WorkflowRunRepositoryPort
WorkflowNodeCapabilityCatalogPort
NodeCapabilityExecutorPort
WorkflowMediaPreviewIssuerPort
NodeCapabilityManagedMediaReaderPort
NodeCapabilityGeneratedMediaWriterPort
TextToImageProviderPort
ImageToVideoProviderPort
TextToAudioProviderPort
AssetAggregateRepositoryPort
AssetIngestTransactionPort
AssetManagedContentStorePort
AssetMediaInspectorPort
```

`Repository` may appear only inside a `*RepositoryPort` or a concrete `*RepositoryAdapter`. `Store`
may appear only when byte storage is the actual capability, as in `AssetManagedContentStorePort`.

## Representation Boundaries

The same concept may have several representations, but only the domain model owns semantics:

```text
ApplyWorkflowMutationRequestDto
  -> ApplyWorkflowMutationCommand
  -> WorkflowAggregate
  -> SqliteWorkflowRow

WorkflowAggregate
  -> WorkflowDto
  -> WorkflowEditorView
```

Translations are explicit and directional. A `*Dto`, `*Row`, or `*View` must not expose methods that
decide business validity or legal state transitions.

Provider protocol types include the provider and operation in their private adapter name, for
example `FalTextToImageRequestDto`. They never reuse capability request types as wire DTOs.

## Dependency Injection Vocabulary

Dependency injection means explicit constructor injection:

```rust
pub struct StartWorkflowRunUseCase<R, C, E> {
    workflow_run_repository: R,
    node_capability_catalog: C,
    node_capability_executor: E,
}
```

The generic bounds are the corresponding consumer-owned `*Port` traits. Call-scoped values such as
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
