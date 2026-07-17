# Backend MVP Architecture

> Status: frozen MVP architecture
> Owner: backend architecture as a whole
> Scope: one local, provider-independent creative Workflow loop

## Purpose

The backend has one closed business loop:

```text
author or import inputs
  -> create or open a Project
  -> edit its one typed Workflow
  -> select a stable Generation Profile on each model-powered node
  -> persist a Run before external work
  -> execute exact Node Capabilities
  -> persist media as Assets
  -> attach outputs to the Run
  -> publish durable progress
  -> reopen and preview the result
```

The MVP is intentionally smaller than the target capability roadmap. Only the capabilities and
commands named in this document are registered. A roadmap name is not a runtime contract, public
API, database record, or promise of provider support.

## Document Map

| Document | Single authority |
| --- | --- |
| [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md) | source-first names, behavior verbs, and role suffixes |
| [`BACKEND_PROJECT.md`](BACKEND_PROJECT.md) | Project identity, metadata, listing, opening, and Workflow discovery |
| [`BACKEND_WORKFLOW_GRAPH.md`](BACKEND_WORKFLOW_GRAPH.md) | editable graph, typed bindings, and graph invariants |
| [`BACKEND_WORKFLOW.md`](BACKEND_WORKFLOW.md) | readiness, execution plan, Run lifecycle, and output association |
| [`BACKEND_CAPABILITIES.md`](BACKEND_CAPABILITIES.md) | capability interface, implementation behavior, and operation contracts |
| [`BACKEND_PROVIDERS.md`](BACKEND_PROVIDERS.md) | stable profiles, provider composites, focused capabilities, routes, and translation |
| [`BACKEND_TASK.md`](BACKEND_TASK.md) | durable Generation Task lifecycle, remote recovery, progress, cancellation, and task list |
| [`BACKEND_ASSETS.md`](BACKEND_ASSETS.md) | Asset identity, managed content, provenance, and preview permission |
| [`BACKEND_ASSISTANT.md`](BACKEND_ASSISTANT.md) | Assistant plan, proposal, review, approval, canonical Run, and repair |
| [`BACKEND_APPLICATION.md`](BACKEND_APPLICATION.md) | Tauri commands, DTO translation, post-commit effects, events, and composition |
| [`BACKEND_STORAGE.md`](BACKEND_STORAGE.md) | metadata transactions, managed media, credentials, and restart recovery |

This document owns cross-module rules and the MVP freeze. Detailed documents may refine their own
semantics but must not widen this surface or redefine another module's state.

## Context Map

```text
Project bounded context (`crates/projects`)
  identity, name, revision, list, open
       |
       | ProjectId scopes
       v
Workflow bounded context (`crates/engine`)
  graph, revision, readiness, execution plan, Run, node execution, events
       ^                           ^
       | implements               | reads/writes through interfaces
Node Capability support           |
(`crates/nodes`)                   +------ Asset bounded context (`crates/assets`)
  exact capabilities                     managed media, provenance, preview permission
  Generation Profile catalog
       ^
       | task-start interface
Generation Task (`crates/tasks`)
       ^
       | provider-level and focused interfaces
Provider adapters (`crates/backends`)
  provider composites -> focused capabilities -> vendor routes -> external APIs

Generation Task bounded context (`crates/tasks`)
  durable generation intent -> submit/poll/finalize outbox -> Asset and Workflow bridges

Assistant bounded context (`crates/assistant`)
  proposal -> review -> human decision -> Workflow mutation interface

Desktop boundary (`src-tauri`)
  commands, DTOs, bridges, closed Desktop/task workers, preview protocol, composition
```

Project, Workflow, Generation Task, Asset, and Assistant are business contexts. Node Capability
and Generation Profile support Workflow. Provider and Desktop modules are boundaries, not owners
of creative business state.

## Semantic Owners

| Business fact | Only owner | Authoritative type |
| --- | --- | --- |
| Project identity, name, revision, and existence | Project | `ProjectAggregate` |
| editable nodes, bindings, position, and revision | Workflow | `WorkflowAggregate` |
| draft validity and Run readiness | Workflow using capability contracts | `WorkflowReadinessPolicy` |
| one frozen execution and output association | Workflow | `WorkflowRunAggregate` |
| one node's graph-execution state, failure, and terminal output | Workflow Run | `WorkflowNodeExecutionEntity` |
| one provider-backed generation lifecycle, remote handle, and progress | Generation Task | `GenerationTaskAggregate` |
| exact operation parameters, inputs, outputs, and execution behavior | exact capability implementation | `TextToImageCapabilityImpl`, and peers |
| stable user-selectable model identity and compatibility | Generation Profile catalog | `GenerationProfileDefinition` |
| current profile availability and route health | provider availability adapter | `GenerationProfileAvailabilityObservation` |
| provider-native request/status translation | one concrete provider route | private provider types |
| media identity, bytes availability, facts, and provenance | Asset | `AssetAggregate` |
| Assistant working plan | Assistant | `AssistantProductionPlanAggregate` |
| Assistant candidate, review result, and human decision | Assistant | `AssistantWorkflowChangeAggregate` |
| persistence representation | storage adapter | private `*Row` types |
| wire representation and preview URL | Desktop | `*Dto` and protocol adapters |
| selection, viewport, playback, and object URLs | React | UI session state |

No DTO, Row, View, provider response, or Assistant model message may decide a transition owned by
one of these types.

## Dependency Direction

Business code depends on consumer-owned interfaces:

```text
React / Python model adapter
          |
          v
Desktop commands and composition
   |               |                |                 |
   v               v                v                 v
Project interfaces   Workflow interfaces   Generation Task interfaces   Asset interfaces   Assistant interfaces
                   ^                ^
                   |                |
Node capabilities ----> task-start and media interfaces
                              ^
                              |
                  Generation Task application
                              ^
                              |
                   provider task adapters
```

Rules:

1. `crates/projects` owns `ProjectId` and imports no other business context.
2. `crates/engine` remains pure and imports only `ProjectId`, never UI, network, filesystem, SQL,
   vendor, or Assistant code.
3. `crates/nodes` implements `WorkflowNodeCapabilityInterface` and owns only the focused interfaces
   its exact capabilities consume, including the task-start boundary for provider-backed work.
4. `crates/tasks` owns the Generation Task aggregate, application use cases, and provider/storage/
   Asset/Workflow completion interfaces it consumes.
5. `crates/backends` implements task-owned provider interfaces; native DTOs never cross inward.
6. `crates/assets` imports only `ProjectId`, never Workflow or provider types. Desktop translates provenance and
   managed-media references explicitly.
7. Assistant consumes bounded Project, Workflow, capability, and workspace interfaces. It imports no repository
   adapter and owns no alternative Workflow rules.
8. Only `DesktopCompositionRoot` constructs or selects concrete adapters.

## MVP Freeze

### Active Node Capabilities

The composition root registers exactly these contracts:

| Contract ref | Implementation | Exact consumed dependencies |
| --- | --- | --- |
| `text.provide_literal@1.0` | `ProvideLiteralTextCapabilityImpl` | none |
| `image.read_asset@1.0` | `ReadImageAssetCapabilityImpl` | `NodeCapabilityManagedMediaReaderInterface` |
| `video.read_asset@1.0` | `ReadVideoAssetCapabilityImpl` | `NodeCapabilityManagedMediaReaderInterface` |
| `audio.read_asset@1.0` | `ReadAudioAssetCapabilityImpl` | `NodeCapabilityManagedMediaReaderInterface` |
| `image.generate_from_text@1.0` | `TextToImageCapabilityImpl` | profile catalog/availability and `NodeCapabilityGenerationTaskStarterInterface` |
| `video.generate_from_image@1.0` | `ImageToVideoCapabilityImpl` | profile catalog/availability, managed-media reader, and `NodeCapabilityGenerationTaskStarterInterface` |
| `audio.synthesize_speech_from_text@1.0` | `TextToSpeechCapabilityImpl` | profile catalog/availability and `NodeCapabilityGenerationTaskStarterInterface` |

This supports the complete `Text -> Image -> Video` path and an independent `Text -> Speech` path,
with imported Image, Video, and Audio inputs. Every registered model-powered capability has a
Mock route before MVP release. Production provider adapters are a separately reviewed later phase.

### Module Surface

| Module | Included in MVP | Explicitly deferred |
| --- | --- | --- |
| Project | create, rename, get, stable list, open with current Workflow summary | archive, delete, duplicate, templates, search, collaboration |
| Workflow | create/get-current, atomic mutation, readiness, whole/through-node Run, cancel, Run/event query, node presentation | history, backend undo, retry-in-place, cache, batches, groups, subgraphs, conditions |
| Node Capability | one interface and seven implementations above | registration of the roadmap operations |
| Generation Profile | stable per-node profile selection, compatibility, availability query, Settings-owned provider/route binding | node-level vendor selection, arbitrary model IDs/options, cross-profile fallback |
| Generation Task | one Text/Image/Video/Voice task lifecycle, provider-level composition over focused capabilities, submit/poll/finalize outbox, progress, Workflow-owned cancellation, restart recovery, get/list | standalone creation, independent cancellation, retry attempts, archive, retention, webhooks |
| Provider | one Mock composite with exact routes for three active model operations and frozen task contracts | production adapters, failover after acceptance, billing, arbitrary vendor options |
| Asset | import, get/list, node-output write, resolve, preview, Pending reconciliation | delete, archive, tags, search, export, derivatives, garbage collection |
| Assistant (design intent, reserved) | durable non-executable plan, candidate, review, human decision, exact apply, canonical Run, reviewed repair | plan-as-queue scheduler, unreviewed apply/repair, parallel approvals, distributed Sessions |
| Desktop | commands, DTOs, closed post-commit and Generation Task workers, durable event repair, preview protocol, composition | generic job host, server mode, plugins, distributed workers |
| Storage | SQLite metadata/config/plaintext credentials, managed files, staging | cloud sync, multi-writer coordination, credential/media encryption, backup/restore UI |

Roadmap capability names remain in `BACKEND_CAPABILITIES.md` so their semantics and names are not
invented ad hoc later. They enter the active registry only through a new reviewed MVP or release
decision with implementation and contract tests.

Freeze discipline: a public surface is frozen only when the same change exercises it with
implementation and contract tests. An unexercised surface is at most reserved — its name is
recorded so later work does not invent a second one, but its semantics bind nothing. Currently
reserved, not frozen: the provider `Immediate` execution composition, the Text focused provider
contract, and the complete Assistant context (`BACKEND_ASSISTANT.md`). Feature expansion beyond
the registered MVP surface requires a new reviewed decision.

## Public Boundary Naming

Exported names follow the grammar in `BACKEND_GLOSSARY.md`:

```text
<OwningModule><BehaviorOrBusinessObject><ArchitecturalRole>
```

Examples are `WorkflowStartRunUseCase`, `AssetRecordNodeOutputUseCase`,
`AssistantDecideWorkflowChangeUseCase`, `GenerationProviderInterface`, and
`SqliteWorkflowRunRepositoryAdapterImpl`. Tauri command names are source-first, such as
`workflow_start_run`, `asset_import`, and `assistant_decide_workflow_change`.

Public methods state their action. Vague methods such as `execute`, `process`, `handle`, `update`, or
`run` alone are prohibited. Examples are `apply_workflow_mutation`, `execute_node_capability`,
`generate_video_from_image`, and `issue_asset_preview`.

## Module Interaction Rules

| Caller | Allowed interface | Callee/result | Prohibited shortcut |
| --- | --- | --- | --- |
| Desktop Project command | one Project use case | Project or workspace result | Workflow/Asset repository join |
| Project open use case | `ProjectWorkflowSummaryReaderInterface` | optional current Workflow summary | imported Workflow aggregate/rules |
| Desktop Workflow command | one Workflow use case | Workflow result/DTO translation | repository or capability call |
| Workflow Run executor | `WorkflowNodeCapabilityInterface` | immediate typed output or durable waiting handoff | provider or Asset repository call |
| provider-backed Node Capability | `NodeCapabilityGenerationTaskStarterInterface` | durable waiting handoff | provider interface or vendor client lookup |
| local/media Node Capability | node-owned media interfaces | managed input or immediate output | path, URL, or Asset repository |
| Generation Task generic dispatch | `GenerationProviderInterface` through `GenerationProviderRegistry` | exact focused capability contribution | vendor-name branch or active Settings substitution |
| Generation Task type-specific execution | matching Text/Image/Video/Voice provider interface | normalized provider outcome | another kind interface or concrete vendor lookup |
| Desktop node/Asset bridge | Asset use cases | translated managed-media value | copied Asset invariants |
| Workflow presentation | `WorkflowMediaPreviewIssuerInterface` | opaque preview value | Asset lease or path import |
| Assistant | Assistant-owned Workflow bridge interfaces | bounded snapshot/evaluation/apply result | direct Workflow repository mutation |
| provider composite | focused capability and private route interfaces | safe capability contract and normalized result | another capability or Asset creation |
| Desktop post-commit worker | one of three committed effect types | bounded external follow-up | direct effect before commit |

## End-To-End Interactions

### Startup

```text
DesktopCompositionRoot
  -> acquire the held-open OS-exclusive data-root lock for the process lifetime
  -> open SQLite, create fresh epoch-2 storage or validate its exact schema version
  -> load and validate SQLite backend configuration
  -> construct focused SQLite plaintext credential repositories
  -> construct Project, Asset, Workflow, Generation Task, and Assistant repositories/use cases
  -> construct Project/Workflow and other cross-context bridges
  -> construct profile catalog, provider composites, exact routes, registry, and availability reader
  -> construct seven capability implementations and WorkflowNodeCapabilityRegistry
  -> reconcile bounded Pending Assets and reclaim prior-instance or expired Generation Task effects
  -> preserve Runs waiting on authoritative non-terminal Generation Tasks
  -> mark only unsafe non-terminal Workflow Runs InterruptedByRestart
  -> recover safe Asset/Assistant effects and abandon unsafe Workflow Run effects
  -> register commands, preview protocol, post-commit worker, and Generation Task worker
```

A missing credential or unhealthy provider marks only affected profiles unavailable. It does not
prevent graph editing, Asset access, deterministic tests, or Assistant-independent use.

### Project Create And Open

```text
project_create / project_list / project_open
  -> Project use case and ProjectRepositoryInterface
  -> ProjectOpenUseCase reads current Workflow through ProjectWorkflowSummaryReaderInterface
  -> existing Workflow summary, or workflow_create for the opened Project
```

Every Project-scoped command resolves the supplied `ProjectId` through Project before attaching it
as trusted context. Project names and IDs never substitute for Workflow, Asset, or Assistant rules.

### Human Or Assistant Edit

```text
WorkflowApplyMutationRequestDto
  -> WorkflowApplyMutationUseCase
  -> WorkflowAggregate validates the complete candidate
  -> WorkflowAggregateRepositoryInterface commits revision + idempotency receipt
  -> WorkflowDto plus current structured readiness issues
```

Assistant may maintain an `AssistantProductionPlanAggregate`, then creates and reviews an immutable
`AssistantWorkflowChangeAggregate`. Human approval uses a stable `WorkflowMutationRequestId`
derived from the change identity and calls the same use case. A stale base revision fails; MVP never
silently rebases an approved change. After apply, an approval-derived `WorkflowRunRequestId` starts
the same canonical Run path used by the UI.

### Profile Discovery And Run Admission

```text
generation_profile_list_for_capability
  -> immutable compatibility catalog + expiring availability observations
  -> provider-independent selectable profiles

workflow_start_run
  -> reload exact Workflow revision
  -> validate graph, Assets, capability registrations, profiles, and current availability
  -> build immutable WorkflowExecutionPlan
  -> atomically persist Queued Run + node executions + event + request receipt
     + WorkflowExecuteRunEffect
  -> return before provider work starts
```

`Unavailable` and `Indeterminate` profiles both block admission in the MVP. Run admission is the
last availability check: after it, the immutable Task target is authoritative, task admission
resolves only that structural binding, and neither execution nor recovery repeats the availability
probe.

### Node Execution And Media Publication

```text
DesktopPostCommitEffectWorker consumes WorkflowExecuteRunEffect
  -> WorkflowExecuteRunUseCase coordinates ready nodes within the concurrency bound
  -> WorkflowNodeCapabilityRegistry resolves the exact implementation
  -> capability resolves inputs through NodeCapabilityManagedMediaReaderInterface
  -> provider-backed capability creates one durable GenerationTask for the Node Execution
  -> Workflow records WaitingForExternalCompletion and completes the current Run effect
  -> GenerationTaskEffectWorker submits through the exact provider adapter
  -> accepted remote task ID is committed before delayed polling
  -> completed media is validated and published through the Asset boundary
  -> task commits its terminal result and NotifyWorkflow
  -> Workflow completion bridge commits node output/failure and a new Run effect
  -> Desktop worker schedules newly-ready downstream nodes and emits committed events
```

Text output skips Asset storage. A media node succeeds only after every required Asset is Available.
`AssetNodeOutputKey` makes `(node execution, output key, ordinal)` idempotent; the same key with
different bytes is a structured conflict.

### Failure, Cancellation, And Restart

| Situation | Closed outcome |
| --- | --- |
| provider or media operation fails | Generation Task becomes Failed and idempotently notifies Workflow; node becomes Failed, descendants become Blocked, independent branches finish |
| Asset publication fails after Pending commit | node does not publish output; startup reconciliation completes or marks the Asset Missing |
| cancellation wins before output commit | no new node output is attached; active work is signalled and remaining nodes become Cancelled/Blocked |
| Asset becomes Available just before cancellation wins | the Asset remains durable with provenance, but the late node output is rejected |
| event emission fails after commit | its safe outbox effect retries; Run remains authoritative and UI can query the gap |
| process exits after accepted provider work | task outbox reclaims the poll by persisted remote ID and later resumes the same waiting node |
| process exits before a durable task handoff or during an unsafe submission | startup fails closed; only the affected unverifiable Run uses `InterruptedByRestart` or the task records `AmbiguousSubmission` |
| duplicate mutation or Run request | matching request hash returns the prior receipt; mismatched reuse returns an idempotency conflict |

### Preview And Reopen

```text
workflow_get_node_presentation
  -> latest relevant successful Workflow output
  -> WorkflowMediaPreviewIssuerInterface
  -> AssetIssuePreviewUseCase
  -> short-lived Project-scoped preview lease
  -> Desktop protocol URL
  -> React renderer
```

Workflow, Generation Task, remote provider task, and Asset IDs survive restart. Preview leases,
URLs, playback state, and loaded credential values do not.

## Transaction And Side-Effect Order

| Use case | Atomic durable write | Effect after commit | Recovery |
| --- | --- | --- | --- |
| Project create/rename | Project + revision + mutation receipt | return committed Project | retry by request ID |
| Workflow mutation | snapshot + revision + mutation receipt | return the committed result | retry by request ID |
| Run admission | Run + executions + event + request receipt + execute effect | execute the Run | replay queued work or classify unsafe execution |
| Run transition | transition + outputs + durable events | emit undispatched events | replay/query event rows |
| Generation Task create/transition | task + optional result + consumed/enqueued task effect | submit, poll, cancel, finalize, or notify Workflow | reset a prior-process claim and continue the exact task |
| Asset node output | Pending + finalization + output key + finalize effect | publish managed bytes | safely replay exact finalization |
| Asset availability | Available transition + completed finalization | allow node output commit | verify exact digest/length |
| Assistant decision | transition + apply effect | idempotent apply, resume, and Run effects | safely replay stable request IDs |

No SQLite transaction remains open during filesystem, provider, sidecar, or Tauri work.
The outbox admits only `WorkflowExecuteRunEffect`, `AssetFinalizeContentEffect`, and
`AssistantApplyWorkflowChangeEffect`. Durable Workflow Run event rows are their own delivery
outbox. Asset import and node-output use cases make the first exact finalization attempt after
commit; the Desktop worker only retries an unfinished effect. There is no generic task payload or
handler registry.

## Composition And Representations

Only `src-tauri/composition.rs` knows concrete adapters. Constructor injection is mandatory for
every database, filesystem, provider, clock, identity, credential, sidecar, event, and cross-context
boundary. Stable pure algorithms remain concrete.

[`BACKEND_APPLICATION.md`](BACKEND_APPLICATION.md#representation-boundaries) owns directional DTO
and cross-context translators; [`BACKEND_STORAGE.md`](BACKEND_STORAGE.md#persistence-names) owns Row
translation rules. Rows, DTOs, Views, paths, URLs, credentials, and provider-native identifiers
never become domain objects or public Workflow parameters.

## MVP Acceptance

The architecture is closed only when:

1. Projects can be created, renamed, listed, opened, and isolated across Workflow, Asset, and Assistant;
2. the seven active capabilities can be edited, saved, reopened, run, and presented;
3. each of the three model-powered capabilities supports per-node profile selection and current
   availability without exposing provider/native model identity;
4. one Mock route passes each exact active provider contract, including restart-safe remote polling;
5. imported and node-produced media use the same Asset availability and preview path;
6. every Run, Generation Task, cancellation, failure, event gap, Pending Asset, and restart has the outcome defined
   above;
7. an Assistant-approved change reaches Workflow and execution only through the canonical mutation
   and Run use cases, and repair repeats the same review/approval chain;
8. fake, Mock, and every later production implementation pass the same interface contract suites;
9. Rust/TypeScript DTO fixtures remain mechanically aligned;
10. architecture tests reject inward concrete dependencies, duplicate semantic owners, generic jobs, unregistered
   MVP capabilities, and concrete construction outside the composition root;
11. `./scripts/e2e.sh` passes.

Item 7 binds when the Assistant implementation track starts (see the freeze discipline above);
core-pipeline closure comprises the remaining items.

## Deferred Architecture

Text-to-video, image-to-image, reference-based generation, mixed-media generation, text generation,
music, crop, upscale, frame extraction, concatenation, and storyboard analysis retain explicit
roadmap names in the capability document. They are not MVP runtime behavior.

Automatic generation retry, failover after acceptance, dynamic/plugin capabilities, cross-Run
cache, Project lifecycle management, unreviewed Assistant apply/repair,
Asset lifecycle management, server mode, cloud sync, collaboration, 3D, and scenes each require a
separate decision that updates this freeze.
