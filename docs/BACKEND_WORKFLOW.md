# Backend Workflow Architecture

> Status: frozen MVP design
> Owner: `crates/engine`
> Scope: graph editing, readiness, execution, durable Run state, and output association

`WorkflowAggregate` and `WorkflowRunAggregate` are the two aggregate roots in the Workflow bounded
context. There is no third Generation Task aggregate.

Each Workflow carries the authoritative `ProjectId` from `crates/projects`. The MVP allows exactly
one current Workflow per Project. Project owns workspace identity; Workflow owns this uniqueness
rule and never stores Project metadata.

## MVP Goal

The frozen capability registry supports:

```text
Text -> Image -> Video
  |
  +------------> Speech

Imported Image / Video / Audio -> matching Asset-read nodes
```

Users can edit, save, reopen, validate, run, cancel, observe, and preview this graph. The Workflow
context is the sole authority for graph revision, typed bindings, readiness, execution planning,
Run state, node state, and output association. It performs no UI, network, filesystem, database,
provider, or Assistant work.

## DDD Structure

```text
crates/engine/src/workflow/
  domain/       graph and Run aggregates, entities, values, policies, errors
  application/  source-first use cases, commands, queries, results
  interfaces/   repositories, capability execution, clock, IDs, events, preview
```

Aggregates approve transitions. Use cases coordinate aggregates and consumer-owned interfaces.
Repositories persist already-approved state and expose no arbitrary state setter.

## Graph Authority

[`BACKEND_WORKFLOW_GRAPH.md`](BACKEND_WORKFLOW_GRAPH.md) owns `WorkflowAggregate`, nodes, typed
single inputs, ordered reference inputs, stable input-item identities, roles, structured prompt
references, persistence ordering, and graph invariants.

Ordered references remain a frozen graph primitive because the approved roadmap already requires
reference-image and mixed-media operations. They do not activate any roadmap capability, provider
interface, UI form, or output data type in the MVP.

React derives `WorkflowNodeShellKindDto` from the primary output type. The MVP has only `Text`,
`Image`, `Video`, and `Audio` shells. Shell kind is presentation state and is never persisted as a
domain node kind.

## Project Association

`WorkflowCreateUseCase::create_workflow` accepts a Project-resolved `ProjectId` and an idempotent
request ID. `WorkflowAggregateRepositoryInterface` atomically creates the first Workflow for that
Project or returns `WorkflowAlreadyExistsForProject`; concurrent requests cannot create two.

`WorkflowGetCurrentUseCase::get_current_workflow` loads by `ProjectId`. It is used by
`workflow_get_current` and by `DesktopProjectWorkflowBridgeAdapterImpl` when opening a Project. Neither
Project nor Desktop reads Workflow rows directly.

## Draft Validity And Run Readiness

An incomplete graph remains editable. Validation therefore has two levels.

`WorkflowAggregate` always enforces draft validity:

- unique typed node and input-item IDs;
- known active capability contract versions;
- valid values for parameters that are present;
- existing nodes and named inputs/outputs;
- correct binding shape, role, cardinality, and concrete type;
- valid structured prompt references;
- no self-edge or cycle.

`WorkflowCheckReadinessUseCase` adds execution readiness:

- every required parameter and input is present;
- every ordered-reference minimum is met;
- every referenced Asset is visible, Available, and the exact kind;
- one implementation is registered for every node in scope;
- every selected Generation Profile is compatible and currently `Available`;
- each exact capability reports no external readiness issue.

`WorkflowReadinessPolicy` owns pure structural checks. Exact capabilities own parameter and external
readiness semantics. The use case merges structured issues without copying either rule set. Run
admission checks readiness again and never repairs the graph automatically.

## Mutation Use Case

All semantic and canvas-position edits enter one idempotent compare-and-swap boundary:

```rust
pub struct WorkflowApplyMutationCommand {
    pub mutation_request_id: WorkflowMutationRequestId,
    pub workflow_id: WorkflowId,
    pub base_revision: WorkflowRevision,
    pub actions: NonEmptyVec<WorkflowMutationAction>,
}
```

`WorkflowMutationAction` is a closed union whose payload types remain source-first:

```text
WorkflowAddNodeAction
WorkflowRemoveNodeAction
WorkflowReplaceNodeParametersAction
WorkflowSelectNodeCapabilityAction
WorkflowMoveNodeAction
WorkflowBindSingleInputAction
WorkflowInsertReferenceItemAction
WorkflowMoveReferenceItemAction
WorkflowRemoveInputItemAction
WorkflowSetInputItemRoleAction
```

`WorkflowApplyMutationUseCase::apply_workflow_mutation`:

1. loads the current aggregate;
2. verifies the requested base revision;
3. applies all actions to one candidate;
4. validates the complete candidate;
5. commits the next snapshot and `WorkflowMutationReceipt` atomically;
6. returns the persisted Workflow and current structured readiness issues.

Reusing a request ID with the same canonical command hash returns its receipt. Reusing it with a
different hash returns `WorkflowMutationIdempotencyConflict`. This makes human and
Assistant-approved edits safe after an uncertain response.

Changing capability never silently drops incompatible inputs. The same atomic mutation must remove
or replace them. Moving a reference item preserves its stable ID, source, and role. Moving a node on
the canvas increments the document revision but never changes execution semantics.

## Run Scope And Admission

```rust
pub enum WorkflowRunScope {
    WholeWorkflow,
    ThroughNode(WorkflowNodeId),
}

pub struct WorkflowStartRunCommand {
    pub run_request_id: WorkflowRunRequestId,
    pub workflow_id: WorkflowId,
    pub workflow_revision: WorkflowRevision,
    pub scope: WorkflowRunScope,
}
```

`WholeWorkflow` contains every node. `ThroughNode` contains the selected node and all transitive
ancestors; it never executes unrelated branches.

`WorkflowStartRunUseCase::start_workflow_run`:

```text
load the exact current revision
  -> check structural and external readiness
  -> topologically order the selected subgraph
  -> normalize parameters and freeze ordered input bindings
  -> create WorkflowRunAggregate and WorkflowNodeExecutionEntity values
  -> atomically persist Queued Run + node executions + first event + request receipt
     + WorkflowExecuteRunEffect
  -> return the durable queued Run
```

Provider work starts only after commit. Reusing a Run request ID with the same canonical hash
returns the admitted Run; different content is an idempotency conflict.

## Frozen Execution Plan

`WorkflowExecutionPlan` contains only what a Run needs after admission:

- source Workflow ID and exact revision;
- Run scope and deterministic dependency order;
- node ID and `WorkflowNodeExecutionId`;
- exact `NodeCapabilityContractRef`;
- `NodeCapabilityNormalizedParameters`;
- named single/ordered bindings with stable item IDs and roles;
- exact source node/output references.

The plan contains no UI position, provider/native model, route, credential, URL, path, preview,
provider task, or mutable availability observation. The current Workflow may be edited after
admission without changing the Run.

The MVP does not define a separate planned/dispatch hash or cross-Run cache key.
`WorkflowNodeExecutionId` is sufficient for provider submission idempotency inside one durable Run.

## Runtime Values

```rust
pub enum WorkflowRuntimeValue {
    Text(WorkflowTextValue),
    Image(WorkflowManagedImageRef),
    Video(WorkflowManagedVideoRef),
    Audio(WorkflowManagedAudioRef),
}

pub struct WorkflowRuntimeInputItem {
    pub input_item_id: WorkflowInputItemId,
    pub input_role_key: Option<NodeCapabilityInputRoleKey>,
    pub value: WorkflowRuntimeValue,
}

pub enum WorkflowNodeInputValue {
    Single(WorkflowRuntimeInputItem),
    OrderedReferences(NonEmptyVec<WorkflowRuntimeInputItem>),
}
```

`WorkflowNodeInputSet` and `WorkflowNodeOutputSet` are maps keyed by exact contract input/output keys. A
capability returns every declared output or one structured failure.

Managed media references contain Asset ID, exact media kind, and content fingerprint, never bytes,
paths, provider URLs, or preview URLs. Generated media enters a runtime output only after Asset
storage returns an Available reference.

Structured `WorkflowTextValue` may contain literal parts and stable input-item references. Workflow
owns cross-node referential integrity; the exact capability owns normalization and provider prompt
mapping. Provider placeholder syntax is never persisted.

## Run Aggregate And State

`WorkflowRunAggregate` owns one execution of one frozen plan. Its
`WorkflowNodeExecutionEntity` children own node progress, structured failure, and output set.

```text
WorkflowRunState:
  Queued -> Running | Cancelled | Failed(InterruptedByRestart)
  Running -> Succeeded | Failed | Cancelled

WorkflowNodeExecutionState:
  Pending -> Running -> Succeeded | Failed | Cancelled | Blocked
```

Rules:

- only domain methods perform transitions;
- a node becomes `Succeeded` only with its complete output set;
- one failed node blocks descendants but independent branches may finish;
- cancellation and its durable event are persisted before signalling active execution tokens;
- cancellation stops new dispatch and rejects late output commits;
- terminal states are immutable;
- retry creates a new Run rather than reopening a terminal Run.

`WorkflowRunEvent` has a monotonic sequence per Run and is committed before Desktop emission. The
durable event record is also its delivery outbox: Desktop publishes undispatched records, clients
deduplicate by `(workflow_run_id, sequence)`, and queries repair gaps.

## Run Execution

Run admission creates one post-commit intent:

```text
WorkflowExecuteRunEffect { workflow_run_id }
```

`DesktopPostCommitEffectWorker` consumes it and calls
`WorkflowExecuteRunUseCase::execute_workflow_run`. That one coordinator selects ready nodes from the
frozen plan, enforces bounded concurrency, commits transitions/events, resolves exact capability
implementations, and calls them outside database transactions. It owns no provider or Asset rules.

The effect remains associated with the Run until it is terminal. It is not replayed after process
restart because a paid provider submission may have been accepted before the crash. Startup marks
the Run `Failed(InterruptedByRestart)`, abandons the effect, and leaves durable events available for
delivery/query. The user creates a new Run to retry.

## Output And Preview Association

Workflow output state stores bounded Text or typed managed Asset references. It never stores
provider bytes, URLs, tasks, native IDs, or an Asset row.

`WorkflowGetNodePresentationUseCase` joins:

```text
WorkflowNodeEntity
  + NodeCapabilityContract
  + current Workflow readiness issues
  + latest relevant WorkflowNodeExecutionEntity
  + latest complete WorkflowNodeOutputSet
  + optional WorkflowMediaPreview
  -> WorkflowNodePresentationView
```

Text is presented inline. Image, Video, and Audio use `WorkflowMediaPreviewIssuerInterface`, implemented
by a Desktop bridge to `AssetIssuePreviewUseCase`. A short-lived preview URL may enter the final DTO
but can never be supplied as Workflow input.

Each output records its producing revision. Editing the node or an ancestor marks the prior preview
stale rather than rebinding it to the new graph.

## Consumer-Owned Interfaces

| Interface | Explicit behavior |
| --- | --- |
| `WorkflowAggregateRepositoryInterface` | `load_workflow`, atomically `commit_workflow_mutation` with revision and receipt |
| `WorkflowRunRepositoryInterface` | atomically admit/transition Runs, outputs, events, request receipts, and `WorkflowExecuteRunEffect` |
| `WorkflowNodeCapabilityInterface` | contract, normalization, external readiness, and exact execution |
| `WorkflowMediaPreviewIssuerInterface` | `issue_workflow_media_preview` |
| `WorkflowClockInterface` | `current_workflow_time` |
| `WorkflowIdentityGeneratorInterface` | create Workflow, node, Run, execution, and request identities |
| `WorkflowRunEventPublisherInterface` | `publish_committed_workflow_run_event` |

Use cases receive focused interfaces through constructors. The concrete capability registry is injected
as an immutable collection. Only `DesktopCompositionRoot` selects concrete adapters.

## Errors

`WorkflowDomainError` owns invariant and transition failures. `WorkflowApplicationError` adds
repository, capability, preview, and orchestration failures. Stable categories include revision
conflict, idempotency conflict, unknown/unregistered capability, invalid parameter, missing
node/input/output, type mismatch, occupied input, invalid cardinality/role/reference, cycle, already exists
for Project, not ready,
unavailable Asset/Profile, upstream failure, cancellation, interrupted restart, and execution
failure.

Errors contain safe typed IDs and structured details. Message text never controls behavior.

## Verification

- graph tests prove draft invariants, typed/ordered bindings, stable item identity, and cycles;
- Project-association tests prove one current Workflow per Project and idempotent concurrent create;
- mutation tests prove atomic action sets, revision CAS, request idempotency, and capability changes;
- readiness tests prove structural/external ownership and admission recheck;
- plan tests prove whole/through-node scope, dependency order, normalized parameters, and persisted
  reference order;
- Run tests cover every legal/illegal transition, branches, failure, cancellation, late output,
  event sequence, and terminal immutability;
- repository contract tests prove admission/transition/output/event atomicity and idempotency;
- execution tests prove effect claiming, bounded concurrency, event delivery/query, effect abandonment, and interrupted startup;
- presentation tests cover Text/Image/Video/Audio, stale output, and preview isolation.

## Post-MVP

Registering roadmap capabilities may add Image Sequence and Video Storyboard runtime values.
Multiple Workflows per Project, history, backend undo, retry-in-place, provider-task resume,
cross-Run cache, dynamic inputs/outputs, plugins, batches, groups, conditions, subgraphs, 3D, and scenes
require separate decisions.
