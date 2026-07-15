# Backend Terminology And Naming

> Status: authoritative for the frozen backend MVP
> Scope: exported types, functions, commands, stable IDs, and adapter roles

## Naming Goal

A public name must reveal three facts without opening its module:

```text
<OwningModule><BusinessObjectOrBehavior><ArchitecturalRole>
```

For example:

```text
WorkflowStartRunUseCase
AssetRecordNodeOutputUseCase
AssistantWorkflowChangeAggregate
ImageToVideoProviderInterface
SqliteWorkflowRunRepositoryAdapterImpl
```

The owner comes first. The middle words describe a business object or an explicit behavior. The
suffix describes the architectural role. Module paths improve navigation but do not excuse vague
exported names.

## Owner Prefixes

| Prefix | Semantic source | Examples |
| --- | --- | --- |
| `Project` | workspace identity, metadata, revision, listing, and opening | `ProjectAggregate`, `ProjectOpenUseCase` |
| `Workflow` | graph, revision, readiness, plan, Run, node execution, and output association | `WorkflowAggregate`, `WorkflowStartRunUseCase` |
| `NodeCapability` | shared exact-operation contracts and node execution boundary | `NodeCapabilityContract`, `NodeCapabilityExecutionRequest` |
| exact operation name | one capability implementation or provider operation | `TextToImageCapabilityImpl`, `TextToImageProviderInterface` |
| `GenerationProfile` | provider-independent model selection, compatibility, and availability | `GenerationProfileRef`, `GenerationProfileListForCapabilityUseCase` |
| `GenerationProvider` | provider account, route, credential, or policy shared by generation adapters | `GenerationProviderAccountId`, `GenerationProviderRouteId` |
| `Asset` | managed media identity, availability, facts, provenance, and preview permission | `AssetAggregate`, `AssetImportUseCase` |
| `Assistant` | production plan, conversation request, Workflow change, review, human decision, and repair activation | `AssistantProductionPlanAggregate`, `AssistantWorkflowChangeAggregate` |
| `Desktop` | Tauri host, post-commit effects, protocol, configuration, and composition | `DesktopCompositionRoot`, `DesktopPostCommitEffectWorker` |

`ProjectId` is the only Project type shared with other business contexts. A provider vendor or
storage technology is a concrete adapter prefix, not a semantic owner:
`FalImageToVideoProviderRouteImpl`, `SqliteWorkflowRunRepositoryAdapterImpl`, and
`MacOsKeychainGenerationProviderCredentialVaultAdapterImpl`.

## Role Suffixes

| Suffix | Meaning | Example |
| --- | --- | --- |
| `*Aggregate` | domain aggregate root and transition authority | `WorkflowRunAggregate` |
| `*Entity` | mutable identity owned by an aggregate | `WorkflowNodeExecutionEntity` |
| `*State` | closed lifecycle state | `AssetManagedContentState` |
| `*Id`, `*Key`, `*Ref` | typed identity, key, or immutable reference | `WorkflowNodeId`, `GenerationProfileRef` |
| `*Contract` | immutable, versioned semantic contract | `NodeCapabilityContract` |
| `*Impl` | concrete implementation of an interface | `ImageToVideoCapabilityImpl` |
| `*Policy` | pure domain decision without external I/O | `WorkflowReadinessPolicy` |
| `*Command` | application request that may change state | `WorkflowApplyMutationCommand` |
| `*Query` | bounded application read request | `AssetListQuery` |
| `*Result` | result of one named application behavior | `WorkflowStartRunResult` |
| `*UseCase` | application orchestrator for one user intention | `AssistantDecideWorkflowChangeUseCase` |
| `*Interface` | consumer-owned trait at a real substitution or external boundary | `AssetManagedContentStoreInterface` |
| `*AdapterImpl` | infrastructure implementation of a consumer-owned interface | `SqliteAssetRepositoryAdapterImpl` |
| `*RouterImpl` | stable-profile dispatcher implementing one exact provider interface | `ImageToVideoProviderRouterImpl` |
| `*RouteImpl` | concrete vendor-operation implementation | `FalImageToVideoProviderRouteImpl` |
| `*Row` | private persistence representation | `SqliteWorkflowRunRow` |
| `*Dto` | Tauri or provider wire representation | `WorkflowStartRunRequestDto` |
| `*View` | read-only UI projection with no business transitions | `WorkflowNodePresentationView` |
| `*Event` | immutable fact that already occurred | `WorkflowRunStateChangedEvent` |
| `*Effect` | typed intent for one post-commit side effect | `WorkflowExecuteRunEffect` |
| `*Error` | structured failure owned by its prefix | `AssetApplicationError` |
| `*Lease` | bounded permission to use an external resource | `AssetPreviewLease` |
| `*Receipt` | durable idempotency or accepted-operation evidence | `WorkflowMutationReceipt` |
| `*Config` | validated startup configuration | `DesktopBackendConfig` |
| `*Worker` | Desktop consumer of the closed post-commit effect outbox | `DesktopPostCommitEffectWorker` |

Use `Value`, `Payload`, `Context`, `Registry`, or `Store` only when the precise compound name needs
that role:

- `WorkflowRuntimeValue` is a tagged runtime value;
- `GeneratedVideoPayload` is bounded data returned by one interface;
- `WorkflowNodeExecutionContext` contains call-scoped deadline and cancellation values;
- `WorkflowNodeCapabilityRegistry` is the one immutable collection of implementations;
- `AssetManagedContentStoreInterface` stores byte content, not business aggregates.

Do not add `Value` to every immutable type or `Service` to every application object.

## Behavior Names

### Use-Case Types

Use-case types are source-first and then verb-first:

| Module | Representative valid names |
| --- | --- |
| Project | `ProjectCreateUseCase`, `ProjectRenameUseCase`, `ProjectOpenUseCase` |
| Workflow | `WorkflowApplyMutationUseCase`, `WorkflowStartRunUseCase`, `WorkflowGetNodePresentationUseCase` |
| Node Capability | `NodeCapabilityListUseCase` |
| Generation Profile | `GenerationProfileListForCapabilityUseCase` |
| Asset | `AssetImportUseCase`, `AssetRecordNodeOutputUseCase`, `AssetIssuePreviewUseCase` |
| Assistant | `AssistantSendMessageUseCase`, `AssistantDecideWorkflowChangeUseCase`, `AssistantActivateRepairUseCase` |

The associated public method repeats the behavior clearly. It is not named only `execute`, `run`,
`handle`, or `process`:

```rust
WorkflowStartRunUseCase::start_workflow_run
AssetImportUseCase::import_asset
AssistantDecideWorkflowChangeUseCase::decide_assistant_workflow_change
```

### Interface Methods

Interface methods describe the exact observable operation:

```rust
WorkflowAggregateRepositoryInterface::load_workflow
WorkflowAggregateRepositoryInterface::commit_workflow_mutation
WorkflowRunRepositoryInterface::admit_workflow_run
WorkflowRunRepositoryInterface::commit_workflow_run_transition

WorkflowNodeCapabilityInterface::node_capability_contract
WorkflowNodeCapabilityInterface::normalize_node_parameters
WorkflowNodeCapabilityInterface::check_node_external_readiness
WorkflowNodeCapabilityInterface::execute_node_capability

NodeCapabilityManagedMediaReaderInterface::read_managed_media
NodeCapabilityProducedMediaWriterInterface::write_node_output_media
WorkflowMediaPreviewIssuerInterface::issue_workflow_media_preview

TextToImageProviderInterface::generate_image_from_text
ImageToVideoProviderInterface::generate_video_from_image
TextToSpeechProviderInterface::synthesize_speech_from_text

AssetManagedContentStoreInterface::stage_asset_content
AssetManagedContentStoreInterface::publish_asset_content
AssetManagedContentStoreInterface::open_asset_content
```

Do not use generic `get`, `save`, `list`, `update`, `execute`, or `create` methods on a public trait.
The noun must remain visible even when the trait already implies it.

### Tauri Commands

[`BACKEND_APPLICATION.md`](BACKEND_APPLICATION.md#frozen-tauri-surface) owns the complete command
surface. Command names use source-first snake case; representative examples are:

```text
project_open
workflow_apply_mutation
workflow_start_run
node_capability_list
generation_profile_list_for_capability
asset_import
assistant_send_message
assistant_decide_workflow_change
```

Request and response types mirror the same phrase in UpperCamelCase, for example
`WorkflowStartRunRequestDto` and `AssistantDecideWorkflowChangeResponseDto`.

### Stable Contract And Tool IDs

Capability contract IDs use the produced domain followed by an exact behavior:

```text
image.generate_from_text@1.0
video.generate_from_image@1.0
audio.synthesize_speech_from_text@1.0
```

Assistant tool IDs are owner-first and versioned:

```text
assistant.workspace.get_snapshot@1
assistant.node_capability.list@1
assistant.workflow.evaluate_mutation@1
assistant.workflow.propose_change@1
```

Display labels, filenames, provider model names, and human-readable descriptions are never parsed
as identity or behavior.

## Authoritative Business Terms

### Project

| Name | Meaning |
| --- | --- |
| `ProjectAggregate` | one durable creative workspace identity, name, and revision |
| `ProjectId` | authoritative scope shared with Workflow, Asset, and Assistant |
| `ProjectMutationRequestId` | stable idempotency identity for one create or rename request |
| `ProjectMutationReceipt` | exact committed Project outcome and integrity evidence for replay |
| `ProjectListCursor` | Project-owned keyset position containing update time and Project identity |
| `ProjectWorkflowSummary` | minimal translated Workflow identity, revision, and Ready/Blocked state |
| `ProjectWorkspaceView` | opened Project plus optional translated current Workflow summary |

### Workflow

| Name | Meaning |
| --- | --- |
| `WorkflowAggregate` | one revisioned editable graph |
| `WorkflowNodeEntity` | one graph node selecting one exact capability contract |
| `WorkflowInputBinding` | one named single or ordered-reference input binding |
| `WorkflowInputItemEntity` | one stable directed reference item inside a binding |
| `WorkflowExecutionPlan` | immutable normalized plan for one admitted Run |
| `WorkflowRunAggregate` | one execution of one frozen Workflow revision |
| `WorkflowNodeExecutionEntity` | one planned node's state, progress, failure, and outputs |
| `WorkflowRuntimeValue` | exact Text, Image, Video, or Audio runtime value in the MVP |
| `WorkflowNodeOutputSet` | complete named output values from one node execution |
| `WorkflowCreateRequestId` | stable idempotency identity for one Workflow creation request |
| `WorkflowMutationRequestId` | stable idempotency identity for one mutation request |
| `WorkflowMutationReceipt` | exact committed Workflow snapshot and integrity evidence for replay |
| `WorkflowReadinessResult` | Ready or a non-empty sorted structured issue set |
| `WorkflowRunRequestReceipt` | stable Run admission identity mapped to one admitted Run |
| `WorkflowRunEventPage` | bounded ascending durable Run events after one sequence cursor |
| `WorkflowExecuteRunEffect` | committed intent to execute one admitted Run |

The words `WorkflowRun` and `WorkflowNodeExecution` replace generic `Task`, `Job`, and `Execution`
when referring to creative Workflow work.

### Node Capability And Generation Profile

| Name | Meaning |
| --- | --- |
| `NodeCapabilityContractRef` | immutable capability ID and version |
| `NodeCapabilityContract` | declared parameters, inputs, outputs, and execution kind |
| `NodeCapabilityInputKey` | stable key for one declared capability input |
| `NodeCapabilityOutputKey` | stable key for one declared capability output |
| `WorkflowNodeCapabilityInterface` | Workflow-owned interface implemented by every exact capability |
| `WorkflowNodeCapabilityRegistry` | immutable active implementation collection |
| `NodeCapabilityNormalizedParameters` | validated normalized parameters plus stable input-item references |
| `NodeCapabilityExecutionRequest` | exact frozen inputs and execution context supplied by Workflow |
| `NodeCapabilityProducedMediaOutputKey` | node-owned idempotency value translated to `AssetNodeOutputKey` |
| `NodeCapabilityGenerationProfileRefParameterValue` | engine boundary shape translated to nodes-owned `GenerationProfileRef` |
| `NodeCapabilityManagedAssetIdParameterValue` | engine boundary UUID bytes translated to Asset-owned `AssetId` |
| `GenerationProfileRef` | stable provider-independent user selection persisted on a model-powered node |
| `GenerationProfileDefinition` | profile identity, lifecycle, and compatible capability refs |
| `GenerationProfileAvailabilityObservation` | expiring operational observation, never persisted on a node |

A Generation Profile is not a provider model alias. A Node Capability is not a UI shell. A provider
route is not a capability.

### Asset

| Name | Meaning |
| --- | --- |
| `AssetAggregate` | one Project-local logical Image, Video, or Audio item |
| `AssetManagedContentState` | Pending, Available, or Missing byte availability |
| `AssetContentDescriptor` | content ID, digest, length, verified MIME, and media kind |
| `AssetMediaFacts` | verified dimensions, duration, sample rate, and related technical facts |
| `AssetOrigin` | imported or exact Workflow-node provenance |
| `AssetNodeOutputKey` | idempotency key for node execution, output key, and ordinal |
| `AssetManagedContentLease` | opaque bounded access to managed bytes |
| `AssetPreviewLease` | short-lived Project-scoped preview permission |
| `AssetFinalizeContentEffect` | safely replayable intent to publish one Pending content finalization |

An Asset ID is never a path, URL, provider task ID, or content digest. Text is not an Asset.

### Assistant

| Name | Meaning |
| --- | --- |
| `AssistantProductionPlanAggregate` | durable model-authored working memory; never a scheduler queue or Workflow |
| `AssistantPlanItemEntity` | one user-meaningful plan item with Rust-owned transitions |
| `AssistantWorkflowChangeAggregate` | one immutable candidate plus review, decision, apply, and Run-link lifecycle |
| `AssistantWorkflowChangeState` | Proposed, ReviewRejected, AwaitingApproval, Rejected, Applying, Applied, ApplyFailed, or Expired |
| `AssistantReviewVerdict` | structured reviewer decision over an exact candidate digest |
| `AssistantWorkflowChangeDecision` | human Approve or Reject decision |
| `AssistantWorkspaceSnapshot` | bounded read-only projection assembled from authoritative modules |
| `AssistantModelProfileRef` | stable provider-independent Assistant model-route selection |
| `AssistantModelRunnerInterface` | Assistant-owned boundary to the external model runtime |
| `AssistantRepairActivation` | factual failed-Run activation; never a selected repair step |
| `AssistantApplyWorkflowChangeEffect` | idempotent post-approval apply intent |

Assistant messages, Production Plans, and model state never become Workflow authority. Approved
apply and Run start use canonical Workflow use cases; a repair is another reviewed and approved
Workflow Change.

## Interface And Implementation Endings

An interface name starts with the consumer or exact consumed behavior:

```text
ProjectRepositoryInterface
ProjectWorkflowSummaryReaderInterface
WorkflowAggregateRepositoryInterface
WorkflowRunRepositoryInterface
WorkflowNodeCapabilityInterface
WorkflowMediaPreviewIssuerInterface
NodeCapabilityManagedMediaReaderInterface
NodeCapabilityProducedMediaWriterInterface
GenerationProfileAvailabilityReaderInterface
AssetRepositoryInterface
AssetIngestTransactionInterface
AssetManagedContentStoreInterface
AssistantWorkspaceSnapshotReaderInterface
AssistantWorkflowMutationEvaluatorInterface
AssistantWorkflowMutationApplierInterface
AssistantWorkflowRunStarterInterface
AssistantWorkflowRunReaderInterface
AssistantModelRunnerInterface
AssistantProductionPlanRepositoryInterface
AssistantWorkflowChangeRepositoryInterface
AssistantRepairActivationRepositoryInterface
GenerationProviderCredentialVaultInterface
DesktopPostCommitEffectOutboxInterface
```

Concrete names add the technology, vendor, or host before the consumer phrase:

```text
SqliteProjectRepositoryAdapterImpl
DesktopProjectWorkflowBridgeAdapterImpl
SqliteWorkflowRunRepositoryAdapterImpl
LocalFilesystemAssetManagedContentStoreAdapterImpl
DesktopNodeCapabilityAssetBridgeAdapterImpl
DesktopAssistantWorkflowBridgeAdapterImpl
FalImageToVideoProviderRouteImpl
MacOsKeychainGenerationProviderCredentialVaultAdapterImpl
SqliteDesktopPostCommitEffectOutboxAdapterImpl
DesktopPostCommitEffectWorker
```

Provider roles remain exact:

- `*ProviderInterface` is the public capability-owned semantic operation;
- `*ProviderRouterImpl` implements that interface and resolves a stable profile to its one configured
  route;
- `*ProviderRouteInterface` is the router-owned private substitution interface;
- `<Vendor>*ProviderRouteImpl` translates and performs one configured native operation.

Every substitution interface ends in `Interface`. Every implementation of an interface ends in
`Impl`; the preceding role remains visible as `CapabilityImpl`, `AdapterImpl`, `RouterImpl`, or
`RouteImpl`. Do not use legacy boundary suffixes, `Trait`, bare `Implementation`, or a concrete type
ending in `Interface`.

Aggregates, policies, and use cases are not interface implementations. They retain their precise
role suffixes and must not be named `*AggregateImpl`, `*PolicyImpl`, or `*UseCaseImpl`.

There is no public provider-wide interface or optional unsupported provider method.

Contract tests target the `*Interface`; every production, deterministic, fake, or fault-injecting
implementation ends in `Impl`.

## Representation Names

Representations keep owner and role visible:

```text
ProjectAggregate <-> SqliteProjectRow
WorkflowApplyMutationRequestDto -> WorkflowApplyMutationCommand -> WorkflowAggregate
WorkflowAggregate <-> SqliteWorkflowRow
AssetAggregate <-> SqliteAssetRow
AssistantWorkflowChangeAggregate <-> SqliteAssistantWorkflowChangeRow
ImageToVideoProviderRequest <-> FalImageToVideoRequestDto
```

A `*Dto`, `*Row`, or `*View` performs shape conversion only. Business validation and transitions
remain with the authoritative aggregate, capability, or policy.

## Prohibited Ambiguous Names

Do not export these standalone or weakly qualified names:

```text
Task        Job          Item         Data          Value
Service     Manager      Handler      Processor     Executor
Store       Repository   Provider     Router        Client
Model       Config       Context      State         Result
Operation   Resource     Content      Media         Registry
```

The word may appear only inside a name that reveals its owner and role, such as
`WorkflowNodeExecutionState`, `GenerationProviderRouteId`, or
`WorkflowNodeCapabilityRegistry`. `TextToAudio` is also prohibited because speech and music have
different semantics.

## Naming Review Checklist

Before accepting a public name, verify that:

- the first words identify its semantic source or concrete technology/vendor;
- its behavior uses a precise business verb and object;
- its suffix identifies its architectural role;
- every substitution trait ends in `Interface` and every implementing type ends in `Impl`;
- the same phrase is mirrored consistently across use case, command/query, DTO, method, and event;
- it cannot be confused with a domain object, boundary representation, or another module's state;
- it does not expose provider, storage, UI, or Assistant implementation details inward;
- it names frozen behavior, not an unregistered roadmap abstraction.
