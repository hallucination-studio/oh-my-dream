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

Users can edit, save, reopen, validate, run, cancel, observe, and preview the exact-seven graph. The Workflow
context is the sole authority for graph revision, typed bindings, readiness, execution planning,
Run state, node state, and output association. It performs no UI, network, filesystem, database,
provider, or Assistant work.

## DDD Structure

`crates/engine/src/workflow/` contains `domain/`, `application/`, and `interfaces/`; names remain
source-first inside each capability-owned layer.

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

`WorkflowCreateRequestId` is a distinct RFC 9562 UUIDv4. Its command hash is SHA-256 over
length-prefixed domain `oh-my-dream/workflow-create/v1` plus Project UUID bytes; request ID is
excluded. `WorkflowCreateReceipt` stores request ID, hash, exact created Workflow snapshot, and a
SHA-256 fingerprint using domain `oh-my-dream/workflow-create-result/v1`. Matching replay returns
that original snapshot after later mutations. A different request for a Project that already has one
returns `WorkflowAlreadyExistsForProject`; mismatched ID reuse is
`WorkflowCreationIdempotencyConflict`. Workflow creation uses schema/revision `1`, an empty graph,
and one observed non-negative UTC-millisecond timestamp.

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
- every selected Generation Profile is reported compatible and currently `Available` by its exact
  capability;
- each exact capability reports no other external readiness issue.

`WorkflowReadinessPolicy` owns pure structural checks. Exact capabilities own parameter and external
readiness semantics, including Generation Profile compatibility and availability. The use case
mechanically projects capability-owned Generation Profile issues into the three Workflow Generation
Profile categories below and preserves every other typed issue as
`WorkflowCapabilityExternalReadinessIssue`; it does not query the catalog or availability reader a
second time. Run admission checks readiness again and never repairs the graph automatically.

Every Workflow external-readiness evaluation, including each invocation of
`WorkflowCheckReadinessUseCase` and each Run-admission evaluation, captures one process-monotonic
start instant and derives one deadline exactly five seconds later. It passes that same deadline
unchanged to every `NodeCapabilityReadinessRequest` in the evaluation. A capability never
truncates, extends, or replaces it; reaching it while later nodes are checked produces the
capability's frozen indeterminate readiness issue.

`WorkflowReadinessResult` is `Ready` or `Blocked { issues }`; blocked issues are non-empty and sorted
by node ID, table-order category tag, then optional target key with absent first. There is no severity.
`WorkflowReadinessIssue` is closed to:

| Category | Safe structured detail |
| --- | --- |
| `WorkflowRequiredParameterMissing` | node ID, parameter key |
| `WorkflowRequiredInputMissing` | node ID, input key |
| `WorkflowReferenceMinimumNotMet` | node ID, input key, required/actual counts |
| `WorkflowAssetUnavailable` | node ID, input key, Asset ID |
| `WorkflowAssetKindMismatch` | node ID, input key, expected/actual media kind |
| `WorkflowCapabilityUnregistered` | node ID, exact capability ref |
| `WorkflowGenerationProfileIncompatible` | node ID, profile ref, capability ref |
| `WorkflowGenerationProfileUnavailable` | node ID, profile ref |
| `WorkflowGenerationProfileAvailabilityIndeterminate` | node ID, profile ref |
| `WorkflowCapabilityExternalReadinessIssue` | node ID, capability-owned typed issue |

The result contains no message-derived category, severity, automatic repair, fallback profile, or
persisted availability observation. D0.3 owns parameter keys, profile refs, and capability issue values.

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

`WorkflowMutationAction` is the closed ten-action source-first union defined once in
`BACKEND_WORKFLOW_GRAPH.md#frozen-mutation-contracts`.

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

`WorkflowRunId`, `WorkflowNodeExecutionId`, and `WorkflowRunRequestId` are distinct RFC 9562 UUIDv4
newtypes. `WorkflowRunCommandHash` is SHA-256 over length-prefixed domain
`oh-my-dream/workflow-start-run/v1`, Workflow UUID bytes, revision as big-endian `u64`, and scope tag
(`0` Whole, `1` ThroughNode plus node UUID); request ID is excluded. `WorkflowRunRequestReceipt`
stores request ID, hash, and admitted Run ID. Matching replay loads that Run; mismatched reuse is
`WorkflowRunIdempotencyConflict`. A missing/corrupt referenced Run is a persistence failure.

## Frozen Execution Plan

`WorkflowExecutionPlan` contains only what a Run needs after admission:

- source Workflow ID and exact revision;
- Run scope and deterministic topological order, breaking ready-node ties by ascending node ID;
- node ID and `WorkflowNodeExecutionId`;
- exact `NodeCapabilityContractRef`;
- `NodeCapabilityNormalizedParameters`;
- named single/ordered bindings with stable item IDs and roles;
- exact source node/output references.

The plan contains no UI position, provider/native model, route, credential, URL, path, preview,
provider task, or mutable availability observation. The current Workflow may be edited after
admission without changing the Run.

Before invoking one exact capability, `WorkflowExecuteRunUseCase` copies the plan's Workflow ID,
revision, node ID, and capability contract ref into `WorkflowNodeExecutionOrigin`. It separately
constructs `WorkflowNodeExecutionContext` from the Project, Run, node-execution, deadline, and
cancellation values. It does not ask a capability or Desktop bridge to reconstruct frozen producer
coordinates.

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

Input/output sets use exact contract keys; a capability returns every output or one structured failure.

`WorkflowManagedAssetIdBoundaryValue` contains exact RFC 9562 UUIDv4 bytes; its content fingerprint
counterpart contains exactly 32 SHA-256 bytes. Image, video, and audio reference types contain both
and fix media kind. They expose canonical bytes, equality, and ordering, never Asset lifecycle, path,
URL, content, Project visibility, or provider state. Outputs require an Available Asset.

Structured text has literals and stable references. Workflow owns integrity; the capability owns normalization/provider mapping. Provider syntax is not persisted.

`WorkflowTextValue` is a non-empty ordered list of at most 1,024 `Literal` or `InputItemReference`
parts with at most 65,536 total UTF-8 literal bytes. Normalization joins adjacent literals and removes
empty ones. Input/output maps reject duplicate keys and never contain null, partial, or untyped media.

These engine-owned values do not own Asset semantics. The Desktop bridge translates Asset ID/digest
and revalidates Project, media kind, and Available state on every access. Engine never depends on
Asset; shared contracts, generic media, legacy readers, and implicit conversion are prohibited.

## Run Aggregate And State

`WorkflowRunAggregate` owns one execution of one frozen plan. Its
`WorkflowNodeExecutionEntity` children own node progress, structured failure, and output set.

The Run stores Run/Project/Workflow identity, source revision, scope, frozen plan, state, optional
failure, and created/updated times. Each node execution stores execution/node identity, state,
optional progress, failure or block reason, optional complete output set, and started/finished times.
Queued/Pending values have none of those optional outcome fields; Running has start time and optional
progress; each terminal value has finish time and exactly the outcome fields required by its state.
Succeeded requires one complete output set, Failed requires one failure, Blocked requires one block
reason, and Cancelled permits none. Restore rejects every other combination.

`WorkflowRunState` is `Queued`, `Running`, `Succeeded`, `Failed`, or `Cancelled`.
`WorkflowNodeExecutionState` is `Pending`, `Running`, `Succeeded`, `Failed`, `Cancelled`, or
`Blocked`. Legal transitions are closed:

```text
Run:  Queued -> Running | Cancelled | Failed
      Running -> Succeeded | Failed | Cancelled
Node: Pending -> Running | Cancelled | Blocked
      Running -> Succeeded | Failed | Cancelled
```

Rules:

- only domain methods perform transitions;
- a node becomes `Succeeded` only with its complete output set;
- one failed node blocks descendants but independent branches finish; the Run becomes Failed after
  all remaining independent nodes are terminal; all-succeeded becomes Succeeded;
- cancellation and its durable event are persisted before signalling active execution tokens;
- cancellation stops new dispatch and rejects late output commits;
- terminal states are immutable;
- retry creates a new Run rather than reopening a terminal Run.

`WorkflowCancelRunUseCase::cancel_workflow_run` accepts one Run ID. Cancelling Queued/Running is
idempotent, atomically commits Run/node cancellation plus events, then signals process tokens.
Cancelling an already Cancelled Run returns it; Succeeded or Failed returns
`WorkflowTerminalStateImmutable`. `WorkflowGetRunUseCase` loads one Run by ID and Project scope.

Times are non-negative `i64` UTC milliseconds. Node progress is integer basis points `0..=10_000`,
monotonic while Running, and absent outside progress events. A Failed Run has exactly one
`WorkflowRunFailure`: `NodeExecutionFailed { sorted_failed_node_ids }` or `InterruptedByRestart`.
A Failed node has one
`WorkflowNodeExecutionFailure` wrapping the structured capability/execution category and safe target.
A Blocked node has `UpstreamNodeFailed { sorted_upstream_node_ids }`. No state stores raw error text.

`WorkflowRunEvent` contains Run ID, non-zero monotonic `u64` sequence, non-negative UTC-millisecond
timestamp, and one closed payload:
`WorkflowRunQueuedEvent`, `WorkflowRunStartedEvent`, `WorkflowNodeStartedEvent`,
`WorkflowNodeProgressedEvent`, `WorkflowNodeSucceededEvent`, `WorkflowNodeFailedEvent`,
`WorkflowNodeBlockedEvent`, `WorkflowNodeCancelledEvent`, `WorkflowRunSucceededEvent`,
`WorkflowRunFailedEvent`, or `WorkflowRunCancelledEvent`. Payloads carry only owning IDs, typed
progress/failure/block facts, and complete output identity where relevant. The state change and event
commit atomically. `WorkflowRunEventPage` reads sequence greater than an optional cursor, ascending,
with limit `1..=500`; `next_sequence` equals the last returned sequence only when another row exists.
Desktop owns cursor JSON.

## Run Execution

Run admission creates one post-commit intent:

```text
WorkflowExecuteRunEffect { workflow_run_id }
```

`DesktopPostCommitEffectWorker` consumes it and calls
`WorkflowExecuteRunUseCase::execute_workflow_run`. That one coordinator selects ready nodes from the
frozen plan, enforces bounded concurrency, commits transitions/events, resolves exact capability
implementations, and calls them outside database transactions. It owns no provider or Asset rules.

The effect remains associated until terminal. It is not replayed after restart: startup marks the
Run Failed with `InterruptedByRestart`, abandons the effect, and retains events. Retry creates a new Run.

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

`WorkflowNodePresentationView` contains node ID, current Workflow revision, capability ref, sorted
current readiness issues, optional latest execution summary (Run ID, node execution ID, state,
progress, typed failure/block fact, producing revision), and exactly one shell derived from the
primary output: `WorkflowTextNodePresentation`, `WorkflowImageNodePresentation`,
`WorkflowVideoNodePresentation`, or `WorkflowAudioNodePresentation`. Text carries bounded text;
media carries the typed Asset reference and optional `WorkflowMediaPreview`; absent complete output
means absent value/preview. Every output summary stores producing revision and `is_stale`; stale
means the node or any ancestor differs from that revision. Presentation never contains parameters,
provider/native model, paths, or persisted URLs.
Latest means maximum `(run_created_at, workflow_run_id)` among plans containing that node.

## Consumer-Owned Interfaces

| Interface | Explicit behavior |
| --- | --- |
| `WorkflowAggregateRepositoryInterface` | `load_workflow`, atomically `commit_workflow_mutation` with revision and receipt |
| `WorkflowRunRepositoryInterface` | atomically admit/transition Runs, outputs, events, request receipts, and `WorkflowExecuteRunEffect` |
| `WorkflowNodeCapabilityInterface` | contract, normalization, external readiness, and exact execution |
| `WorkflowMediaPreviewIssuerInterface` | `issue_workflow_media_preview` |
| `WorkflowClockInterface` | `current_workflow_time` |
| `WorkflowIdentityGeneratorInterface` | `generate_workflow_id`, `generate_workflow_run_id`, and `generate_workflow_node_execution_id` |
| `WorkflowRunEventPublisherInterface` | `publish_committed_workflow_run_event` |

Use cases receive focused interfaces through constructors. The concrete capability registry is injected
as an immutable collection. Only `DesktopCompositionRoot` selects concrete adapters.

## Errors

`WorkflowDomainError` owns the closed graph errors in `BACKEND_WORKFLOW_GRAPH.md` plus
`WorkflowIllegalRunTransition`, `WorkflowIllegalNodeExecutionTransition`,
`WorkflowProgressOutOfRange`, `WorkflowProgressRegression`, `WorkflowRunEventSequenceOverflow`,
`WorkflowIncompleteOutputSet`, and `WorkflowTerminalStateImmutable`.
`WorkflowApplicationError` adds `WorkflowNotFound`, `WorkflowAlreadyExistsForProject`,
`WorkflowCreationIdempotencyConflict`, `WorkflowRevisionConflict`,
`WorkflowMutationIdempotencyConflict`, `WorkflowRunNotFound`,
`WorkflowRunRevisionMismatch`, `WorkflowRunIdempotencyConflict`, `WorkflowNotReady`,
`WorkflowRunEventLimitOutOfBounds`, `WorkflowPersistenceFailure`, `WorkflowCapabilityExecutionFailure`,
`WorkflowMediaPreviewIssueFailure`, and `WorkflowRunEventPublishFailure`.

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
