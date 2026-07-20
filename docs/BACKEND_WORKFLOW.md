# Backend Workflow Architecture

> Status: frozen MVP design
> Owner: `crates/engine`
> Scope: graph editing, readiness, execution, durable Run state, and output association

`WorkflowAggregate` and `WorkflowRunAggregate` are the two aggregate roots in the Workflow bounded
context. `GenerationTaskAggregate` belongs to the separate Generation Task bounded context defined
by [`BACKEND_TASK.md`](BACKEND_TASK.md); it is not a third Workflow aggregate.

Each Workflow carries the authoritative `ProjectId` from `crates/projects`. The MVP allows exactly
one current Workflow per Project. Project owns workspace identity; Workflow owns this uniqueness
rule and never stores Project metadata.

## MVP Goal

Users can edit, save, reopen, validate, run, cancel, observe, and preview the exact-seven graph,
including the universal Video generation capability. The Workflow
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

Ordered references are an active graph primitive used by universal Video generation for ordered
Image, Video, and Audio inputs. Workflow owns only binding identity, order, role declaration, and
concrete type validity; the Video capability owns mode and model calibration.

React derives `WorkflowNodeShellKindDto` from the primary output type. The MVP has only `Text`,
`Image`, `Video`, and `Audio` shells. Shell kind is presentation state and is never persisted as a
domain node kind.

## Project Association

`WorkflowCreateUseCase::create_workflow` accepts a Project-resolved `ProjectId` and an idempotent
request ID. `WorkflowAggregateRepositoryInterface` atomically creates the first Workflow for that
Project or returns `WorkflowAlreadyExistsForProject`; concurrent requests cannot create two.

`WorkflowGetCurrentUseCase::get_current_workflow` loads by `ProjectId` and returns
`Result<Option<WorkflowCurrentView>, WorkflowGetCurrentError>`, containing the exact loaded `WorkflowAggregate` and the structured
readiness issues evaluated from that same immutable snapshot. It is used by
`workflow_get_current` and by `DesktopProjectWorkflowBridgeAdapterImpl` when opening a Project. Neither
Project nor Desktop reads Workflow rows directly. The bridge maps an empty issue set to `Ready` and
any non-empty set to `Blocked`; it never performs a second load or a second readiness evaluation.
External timeout or unavailable evidence is represented by the existing structured indeterminate
or unavailable readiness issue and does not fail Project open. Only repository failure or an
invalid persisted Workflow returns `WorkflowGetCurrentError`, which `ProjectOpenUseCase` propagates
as its existing Workflow-summary read failure without creating a partial workspace view.

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

For every add, capability-select, parameter-replace, restore, and mutation replay,
`WorkflowAggregate` calls the exact contract's `validate_draft_parameters`. The returned canonical
stored map may omit required keys and contains no inserted defaults. The aggregate never calls
execution normalization while validating an editable draft.

`WorkflowCheckReadinessUseCase` adds execution readiness:

- every required parameter and input is present;
- every ordered-reference minimum is met;
- every referenced Asset is visible, Available, and the exact kind;
- one implementation is registered for every node in scope;
- every selected Generation Profile is compatible with its exact capability;
- every selected Generation Model is structurally available and calibrated against the node's
  parameters and binding snapshot;
- each exact capability reports no other external readiness issue.

Readiness calls `normalize_parameters_for_execution`; missing required keys become the structured
issues below rather than mutation failures. Run admission repeats that same operation before freezing
the execution plan. This is the only path that produces `NodeCapabilityNormalizedParameters`.

`WorkflowReadinessPolicy` owns pure structural checks. Exact capabilities own parameter, profile,
model, and calibration semantics. The use case mechanically projects capability-owned typed issues
into the Workflow categories below; it does not reimplement Seedance tables, query another
availability source, or mutate the graph. Ordinary editing readiness resolves current model
revisions. Run admission first bulk-resolves all selected revisions from one Settings snapshot and
then repeats capability readiness against those exact frozen revisions. It never repairs the graph
automatically.

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
| `WorkflowGenerationModelUnavailable` | node ID, stable model ID and structured safe reason |
| `WorkflowGenerationModelAvailabilityIndeterminate` | node ID, stable model ID |
| `WorkflowGenerationModelCalibrationRequired` | node ID, model ID, rule code, typed parameter/input/input-item target, and closed correction proposal |
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
  -> topologically order the selected subgraph
  -> check structural validity and normalize the static parameter superset
  -> resolve every selected GenerationModelId against one consistent Settings snapshot
  -> repeat external readiness and model calibration against those exact revisions
  -> freeze normalized parameters, ordered input bindings, and one model revision per powered node
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
- for each model-powered node, one `WorkflowGenerationModelRevisionSelection` containing only the
  selected stable model UUID and non-zero immutable configuration revision;
- named single/ordered bindings with stable item IDs and roles;
- exact source node/output references.

The plan contains no UI position, provider/native model, route, credential, URL, path, preview,
remote task handle, or mutable availability observation. The application-owned model revision
selection is not provider configuration. The current Workflow or Generation Settings may be
edited after admission without changing the Run.

Before invoking one exact capability, `WorkflowExecuteRunUseCase` copies the plan's Workflow ID,
revision, node ID, and capability contract ref into `WorkflowNodeExecutionOrigin`. It separately
constructs `WorkflowNodeExecutionContext` from the Project, Run, node-execution, deadline, and
cancellation values. It supplies the matching model revision as `WorkflowNodeExternalAdmission`
for model-powered capabilities and `None` otherwise. It does not ask a capability or Desktop bridge
to reconstruct frozen producer coordinates or re-resolve current Settings.

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
URL, content, Project visibility, or provider state. Creating a direct capability output requires an
Available Asset. Generation Task notification instead attaches the Task's immutable Asset
ID/kind/digest result, which proves availability at Task success and remains valid historical output
identity even if the Asset later becomes Missing.

Structured text has literals and stable references. Workflow owns integrity; the capability owns normalization/provider mapping. Provider syntax is not persisted.

`WorkflowTextValue` is a non-empty ordered list of at most 1,024 `Literal` or `InputItemReference`
parts with at most 65,536 total UTF-8 literal bytes. Normalization joins adjacent literals and removes
empty ones. Input/output maps reject duplicate keys and never contain null, partial, or untyped media.

These engine-owned values do not own Asset semantics. The Desktop bridge translates Asset ID/digest
and revalidates Project, media kind, and Available state on every access. Engine never depends on
Asset; shared contracts, generic media, legacy readers, and implicit conversion are prohibited.
Current downstream byte access and preview issuance revalidate Project, media kind, digest, and
Available state. Notification replay does not revalidate current byte availability and therefore
cannot turn one already-Succeeded Task into a different Workflow history after restart.

## Run Aggregate And State

`WorkflowRunAggregate` owns one execution of one frozen plan. Its
`WorkflowNodeExecutionEntity` children own graph-execution state, structured failure, and output set.

The Run stores Run/Project/Workflow identity, source revision, scope, frozen plan, state, optional
failure, and created/updated times. Each node execution stores execution/node identity, state,
optional progress, failure or block reason, optional complete output set, and started/finished times.
Queued/Pending values have none of those optional outcome fields; Running has start time;
WaitingForExternalCompletion has start time but no provider/task identity or duplicated progress;
each terminal value has finish time and exactly the outcome fields required by its state.
Succeeded requires one complete output set, Failed requires one failure, Blocked requires one block
reason, and Cancelled permits none. Restore rejects every other combination.

`WorkflowRunState` is `Queued`, `Running`, `Succeeded`, `Failed`, or `Cancelled`.
`WorkflowNodeExecutionState` is `Pending`, `Running`, `WaitingForExternalCompletion`, `Succeeded`,
`Failed`, `Cancelled`, or `Blocked`. Legal transitions are closed:

```text
Run:  Queued -> Running | Cancelled | Failed
      Running -> Succeeded | Failed | Cancelled
Node: Pending -> Running | Cancelled | Blocked
      Running -> WaitingForExternalCompletion | Succeeded | Failed | Cancelled
      WaitingForExternalCompletion -> Succeeded | Failed | Cancelled
```

Rules:

- only domain methods perform transitions;
- a node becomes `Succeeded` only with its complete output set;
- one failed node blocks descendants but independent branches finish; the Run becomes Failed after
  all remaining independent nodes are terminal; all-succeeded becomes Succeeded;
- cancellation and its durable event are persisted before signalling active execution tokens;
- cancellation stops dispatch of nodes not already handed off and rejects every late output commit;
- terminal states are immutable;
- retry creates a new Run rather than reopening a terminal Run.

`WorkflowCancelRunUseCase::cancel_workflow_run` accepts one Run ID. Cancelling Queued/Running is
idempotent, atomically commits Run/node cancellation plus events, then signals process tokens.
Cancelling an already Cancelled Run returns it; Succeeded or Failed returns
`WorkflowTerminalStateImmutable`. `WorkflowGetRunUseCase` loads one Run by ID and Project scope.

A provider call already authorized by an exact waiting handoff may race with the cancellation
commit because Workflow and Generation Task deliberately have no cross-context lock or transaction.
The first durable commit owns the bounded race. When cancellation commits before Task finalization,
Generation Task converges to Cancelled and requests remote cancellation when that complete
capability exists. When Task finalization commits first, the Task may still succeed and retain its
Asset, but the cancelled Workflow rejects the late result attachment. External work may still
finish or charge, which is the documented best-effort remote cancellation trade-off.

Times are non-negative `i64` UTC milliseconds. Immediate node progress is integer basis points
`0..=10_000` and monotonic while Running. Provider generation progress belongs only to
`GenerationTaskAggregate` and is projected rather than copied into the Node Execution. A Failed Run has exactly one
`WorkflowRunFailure`: `NodeExecutionFailed { sorted_failed_node_ids }` or `InterruptedByRestart`.
A Failed node has one
`WorkflowNodeExecutionFailure` wrapping the structured capability/execution category and safe target.
A Blocked node has `UpstreamNodeFailed { sorted_upstream_node_ids }`. No state stores raw error text.

`WorkflowRunEvent` contains Run ID, non-zero monotonic `u64` sequence, non-negative UTC-millisecond
timestamp, and one closed payload:
`WorkflowRunQueuedEvent`, `WorkflowRunStartedEvent`, `WorkflowNodeStartedEvent`,
`WorkflowNodeProgressedEvent`, `WorkflowNodeWaitingForExternalCompletionEvent`,
`WorkflowNodeSucceededEvent`, `WorkflowNodeFailedEvent`,
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

An immediate capability returns a complete output. A provider-backed capability returns
`WaitingForGenerationTask` only after durable task creation; Workflow commits
`WaitingForExternalCompletion` and completes the current effect when no other node is ready. A task
terminal notification calls the canonical completion use case, which commits that node's
Succeeded or Failed transition and enqueues a new `WorkflowExecuteRunEffect` for downstream work.
An unexpected provider-originated Task cancellation is the structured node failure
`GenerationTaskCancelled`; Workflow-owned Run cancellation has already made the node terminal and
rejects late Task attachment. Notification replay is idempotent.

Startup classifies non-terminal Runs before interruption. A queued Run is replayable. A Running
Node Execution with an exact Queued Generation Task, unconsumed SubmitTask, and no remote handle is
also replayable: the task worker is forbidden to submit until Workflow commits the waiting state,
so replay idempotently completes that handoff. A Run whose
active work includes waiting Node Executions with matching authoritative non-terminal Generation
Tasks or terminal tasks with pending Workflow notification remains Running. Its Workflow effect is
replayed: the executor schedules any independent ready branches and naturally completes when every
active node is waiting. A Running node without durable handoff is unsafe and retains the existing
`InterruptedByRestart` outcome.

`WorkflowClassifyRunsAfterRestartUseCase` reads non-terminal Runs in stable Run-ID pages of at most
100 and asks `WorkflowGenerationTaskRecoveryReaderInterface` for exact Running or waiting Node
Execution origins. The reader distinguishes safe queued pre-handoff, active task work, terminal task
with pending notification, completed notification, and absent/corrupt handoff. The Desktop host
replays `ReplaySafe` Runs and invokes `WorkflowInterruptRunsAfterRestartUseCase` for
`InterruptUnsafe` Runs.
Classification never reads provider state, route names, or implementation types; the bridge returns
only whether each exact node has an authoritative recoverable task. The classifier returns
`ReplaySafe` or `InterruptUnsafe`; task waiting is evidence for safe replay, not a separate owner of
Workflow scheduling. Repeating classification and interruption is idempotent and produces no
duplicate event.

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
| `WorkflowGenerationModelAdmissionResolverInterface` | resolve all model-powered nodes against one Settings snapshot and return exact provider-independent model revisions in request order |
| `WorkflowGenerationTaskRecoveryReaderInterface` | classify exact Running pre-handoff and waiting Node Execution task states during startup |
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
`WorkflowGenerationModelAdmissionFailed`,
`WorkflowRunEventLimitOutOfBounds`, `WorkflowPersistenceFailure`, `WorkflowCapabilityExecutionFailure`,
`WorkflowGenerationTaskCompletionConflict`, `WorkflowGenerationTaskRecoveryReadFailure`,
`WorkflowMediaPreviewIssueFailure`, and `WorkflowRunEventPublishFailure`.

Errors contain safe typed IDs and structured details. Message text never controls behavior.

## Verification

- graph tests prove draft invariants, typed/ordered bindings, stable item identity, and cycles;
- Project-association tests prove one current Workflow per Project and idempotent concurrent create;
- mutation tests prove atomic action sets, revision CAS, request idempotency, and capability changes;
- readiness tests prove structural/external ownership and admission recheck;
- plan tests prove whole/through-node scope, dependency order, normalized parameters, exact model
  revision freeze, Settings-snapshot consistency, and persisted reference order;
- Run tests cover every legal/illegal transition, branches, failure, cancellation, late output,
  event sequence, and terminal immutability;
- repository contract tests prove admission/transition/output/event atomicity and idempotency;
- execution tests prove immediate completion, durable waiting handoff, task completion replay,
  bounded concurrency, event delivery/query, and safe/unsafe restart classification;
- presentation tests cover Text/Image/Video/Audio, stale output, and preview isolation.

## Post-MVP

Registering roadmap capabilities may add Image Sequence and Video Storyboard runtime values.
Multiple Workflows per Project, history, backend undo, retry-in-place, cross-Run cache, dynamic
inputs/outputs, plugins, batches, groups, conditions, subgraphs, 3D, and scenes
require separate decisions.
