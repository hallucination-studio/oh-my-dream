# Backend Assistant Architecture

> Status: frozen Strong Assistant MVP architecture
> Owner: Assistant business capability in Rust; model loop in the Python adapter
> Scope: plan, propose, review, approve, apply, run, observe, and propose repair

The Assistant is a Project-scoped Workflow co-author. It may plan and propose creative changes, but
it is never authoritative for Workflow, Node Capability, Generation Profile, Asset, or Run state.
Its domain imports only `ProjectId` from `crates/projects`, never Project aggregate or repository
types.

## Closed Product Loop

```text
user message
  -> bounded authoritative workspace snapshot
  -> AssistantProductionPlanAggregate
  -> immutable AssistantWorkflowChangeAggregate
  -> read-only Reviewer verdict with Rust-verified evidence
  -> exact human approval
  -> canonical WorkflowApplyMutationUseCase
  -> canonical WorkflowStartRunUseCase
  -> factual Workflow Run events
  -> success, or a separately reviewed and approved repair proposal
```

The user sees one editable Workflow and one Run authority. A Production Plan is working memory, not
a hidden graph or scheduler queue. An Assistant Workflow Change is an immutable proposal, not a
second editable Workflow.

## Semantic Owners

| Fact | Only owner |
| --- | --- |
| plan items and legal plan-item transitions | `AssistantProductionPlanAggregate` |
| exact candidate, lineage, digest, review, approval scope, and decision | `AssistantWorkflowChangeAggregate` |
| repeated model/tool loop and opaque continuation bytes | `PythonAgentsAssistantModelRunnerAdapterImpl` |
| canonical graph, revision, mutation receipt, and validation | Workflow bounded context |
| Run/node lifecycle, progress, failure, and events | `WorkflowRunAggregate` |
| Asset identity, visibility, kind, and availability | Asset bounded context |
| capability contracts and profile compatibility | Node Capability and Generation Profile modules |
| UI presentation of plan/candidate/approval/Run | React projections |

Assistant DTOs, model messages, Reviewer prose, SDK state, and persistence rows own no Workflow or
Run transition.

## DDD Structure

```text
crates/assistant/src/
  domain/       production plan, workflow change, review, approval, errors
  application/  send, inspect pending change, decide, apply, activate repair
  interfaces/   model runner, continuation storage, workspace, Workflow bridges, clock

src-tauri/src/assistant/
  commands.rs   Tauri admission
  dto.rs        boundary representations
  translation.rs
  adapters/     sidecar, SQLite, Workflow/Asset/capability bridges
```

`crates/assistant` imports no Tauri command, SQL, filesystem, Python, SDK, or concrete Workflow
adapter. Desktop adapters implement its interfaces and are selected only in composition.

## Production Plan Aggregate

```rust
pub struct AssistantProductionPlanAggregate {
    pub id: AssistantProductionPlanId,
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub title: AssistantPlanTitle,
    pub items: Vec<AssistantPlanItemEntity>,
    pub revision: AssistantProductionPlanRevision,
}
```

The plan contains at most 128 user-meaningful items with goal, acceptance note, and optional blocked
reason. Its closed transitions are:

```text
Pending -> InProgress -> Completed
Pending -> Blocked -> InProgress
InProgress -> Blocked -> InProgress
```

Every mutation uses compare-and-swap revision. Rust validates transitions; the model chooses which
item to discuss or update. Product code never consumes the plan as a queue, selects the next item,
starts one model Runner per item, or treats plan completion as Workflow success.

`AssistantProductionPlanId`, `AssistantSessionId`, `AssistantWorkflowChangeId`,
`AssistantApprovalScopeId`, `AssistantModelInvocationId`, and `AssistantRepairActivationId` use the
backend UUIDv4 contract. Plan revision starts at non-zero `u64` value `1`. Plan title, item goal,
acceptance note, and blocked reason are bounded to 120, 2,000, 2,000, and 1,000 Unicode scalar
values respectively; required text is trimmed and non-empty.
`AssistantWorkflowMutationDigest` and `AssistantWorkflowFingerprint` are distinct 32-byte SHA-256
newtypes over D0.2 canonical mutation bytes and resulting canonical graph bytes respectively.

## Workflow Change Aggregate

```rust
pub struct AssistantWorkflowChangeAggregate {
    pub id: AssistantWorkflowChangeId,
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub base_workflow_revision: WorkflowRevisionBoundaryValue,
    pub ordered_mutations: NonEmptyVec<AssistantWorkflowMutation>,
    pub mutation_digest: AssistantWorkflowMutationDigest,
    pub resulting_workflow_fingerprint: AssistantWorkflowFingerprint,
    pub review: Option<AssistantReviewReceipt>,
    pub approval_scope_id: AssistantApprovalScopeId,
    pub continuation_ref: Option<AssistantModelContinuationRef>,
    pub state: AssistantWorkflowChangeState,
    pub expires_at: AssistantWorkflowChangeExpiry,
}
```

Assistant-owned boundary values are translated to Workflow types by a bridge; the Assistant module
does not copy Workflow validation or import a persistence row.

```text
AssistantWorkflowChangeState:
  Proposed -> ReviewRejected
  Proposed -> AwaitingApproval
  AwaitingApproval -> Rejected | Applying | Expired
  Applying -> Applied | ApplyFailed
```

`Applying` is recoverable, not a claim that Workflow changed. `ApplyFailed` records a permanent
stale-revision, fingerprint, or authority failure; transient storage failures leave the change in
`Applying` for recovery. The stable Workflow mutation request
ID is derived from `(change ID, mutation digest)`, so retrying the canonical apply returns the same
`WorkflowMutationReceipt` after an uncertain result.

Candidate content and digest are immutable. Revision, Project, Session, review identity, approval
scope, and expiry must all match before each transition. A stale Workflow revision fails closed;
the MVP never silently rebases an approved change.

## Consumer-Owned Interfaces

| Interface | Explicit behavior |
| --- | --- |
| `AssistantModelRunnerInterface` | start or resume one bounded model/tool turn and stream typed events |
| `AssistantModelContinuationStoreInterface` | persist, load, and consume opaque versioned continuation state |
| `AssistantWorkspaceSnapshotReaderInterface` | read one bounded authoritative Project/Workflow/Asset/Run projection |
| `AssistantNodeCapabilityCatalogReaderInterface` | list/describe only active exact contracts and selectable profiles |
| `AssistantWorkflowMutationEvaluatorInterface` | evaluate bounded mutations without canonical commit |
| `AssistantWorkflowMutationApplierInterface` | apply an approved exact mutation through Workflow's idempotent use case |
| `AssistantWorkflowRunStarterInterface` | start the canonical Run with a stable request ID |
| `AssistantWorkflowRunReaderInterface` | read committed Run state and ordered events for monitoring/repair |
| `AssistantProductionPlanRepositoryInterface` | load and revision-CAS one plan aggregate |
| `AssistantWorkflowChangeRepositoryInterface` | persist transitions and query one pending change per Session |
| `AssistantRepairActivationRepositoryInterface` | record-or-get one factual activation per failed Run |
| `AssistantClockInterface` | supply deterministic timestamps and expiry checks |

`DesktopAssistantWorkflowBridgeAdapterImpl` implements the four Workflow-facing interfaces by invoking
Workflow application use cases. It never reaches a Workflow repository directly. An Asset/capability
snapshot bridge likewise invokes their public queries and reuses their visibility/compatibility
rules.

## Desktop Boundary

[`BACKEND_APPLICATION.md`](BACKEND_APPLICATION.md#frozen-tauri-surface) owns the three Assistant
commands and their DTO-to-use-case mapping.

`AssistantSendMessageRequestDto` contains an untrusted Project ID, user-observed Workflow
presence/revision, bounded selected node/Asset IDs, and bounded user text. The Desktop boundary
resolves Project before Rust supplies session, invocation, request, and approval identities.
Model-facing JSON never contains trusted context, local paths, media bytes, credentials, or an
operation contract chosen by the model.

Only one active invocation and one pending approval may exist per `AssistantSessionId`. Different
Projects may proceed independently.

Assistant-internal DTO bounds are frozen before any model call: user text is 1..=16 KiB UTF-8;
selected node and Asset IDs are unique lists of at most 32 each; a workspace snapshot is at most 1
MiB; a candidate has 1..=128 mutations and at most 1 MiB canonical JSON; Reviewer prose and final
model text are each at most 16 KiB. Boundary DTOs use tagged unions, reject unknown fields and
duplicate keys, and translate through named `try_from_*_dto` functions. D0.6 alone owns Tauri DTO
field names and encodings.

## Bounded Observation And Tools

The workspace snapshot includes the current Workflow revision, selected nodes/Assets, a small recent
Asset page, active Run facts, and active capability/profile summaries. It omits paths, bytes,
preview URLs, provider details, and unbounded history.

Model tool IDs are source-first and versioned:

| Tool ID | Effect |
| --- | --- |
| `assistant.workspace.get_snapshot@1` | bounded authoritative read |
| `assistant.node_capability.list@1` | bounded active contract discovery |
| `assistant.node_capability.describe@1` | exact contract/profile description |
| `assistant.production_plan.get@1` | read plan working memory |
| `assistant.production_plan.create@1` | create one plan |
| `assistant.production_plan.replace@1` | revision-CAS plan replacement |
| `assistant.production_plan.update_item@1` | aggregate-owned item transition |
| `assistant.workflow.evaluate_mutation@1` | non-mutating Workflow evaluation |
| `assistant.workflow.propose_change@1` | persist immutable candidate |
| `assistant.workflow.get_change@1` | read exact candidate evidence |
| `assistant.workflow.request_apply@1` | approval-gated request for exact apply |

Rust generates strict input/output schemas from canonical DTOs, validates before deserialization,
dispatches typed handlers, and validates typed output before serialization. Workflow mutation bodies
may contain capability-declared parameter objects; the enclosing operation remains closed and
bounded.

The Assistant cannot call a direct canonical mutation or Run-start tool. Only the Rust approval
orchestrator can use the apply and Run starter interfaces after verifying the exact persisted change.

## Candidate And Review

`AssistantWorkflowMutationEvaluatorInterface` evaluates a bounded ordered mutation list against the
authoritative base revision. Candidate creation stores:

- Project, Session, user intent, and base revision;
- exact ordered mutations and stable aliases;
- engine-derived readiness issues;
- mutation digest and resulting Workflow fingerprint;
- lineage, expiry, and approval scope.

It never advances the canonical Workflow.

The main model passes only `AssistantWorkflowChangeId` to a read-only Reviewer Agent. The Reviewer
must fetch the exact candidate through Rust and returns a typed pass/reject verdict. Rust accepts a
pass only when transport evidence proves that exact ID and digest were fetched under the current
Reviewer contract. Reviewer prose alone is never evidence.

A verified rejection transitions to `ReviewRejected`. A verified pass stores
`AssistantReviewReceipt`, persists the versioned model continuation, and transitions to
`AwaitingApproval`. The approval card is reconstructed from this aggregate, never from an old UI
event.

## Human Decision And Exact Apply

The decision request must match Project, Session, change ID, approval scope, mutation digest, and
current expiry. Rejection is terminal and consumes the continuation.

Approval follows this recoverable sequence:

```text
persist Applying decision + AssistantApplyWorkflowChangeEffect
  -> DesktopPostCommitEffectWorker consumes the effect
  -> load and verify exact model continuation
  -> AssistantWorkflowMutationApplierInterface
  -> WorkflowApplyMutationUseCase with change-derived WorkflowMutationRequestId
  -> persist Applied plus WorkflowMutationReceipt
  -> resume the same model turn with the trusted result
  -> AssistantWorkflowRunStarterInterface with approval-derived WorkflowRunRequestId
```

The Assistant effect stays open until apply, continuation resume, and Run admission have reached a
durable outcome. If the process stops after Workflow commit but before `Applied`, retrying uses the same mutation
request ID and receives the prior receipt. If it stops after Run admission, the same Run request ID
returns the prior Run. The Assistant aggregate then catches up without duplicating a Workflow
revision or paid execution.

Opaque model continuation is not business authority. If a sidecar resume result is ambiguous, Rust
marks that continuation interrupted instead of replaying it; the already-applied Workflow and
admitted Run remain the truthful outcome.

Approval proof is Rust-created and never supplied in model JSON. Exact mutation content is replayed
from the aggregate, not accepted again from the UI or model.

## Run And Repair

The Assistant does not have a private mock or provider runner. It starts the exact canonical
Workflow through `AssistantWorkflowRunStarterInterface`; Run admission creates
`WorkflowExecuteRunEffect`, and `DesktopPostCommitEffectWorker` executes the normal path.

Run progress shown in the Assistant UI is a projection of committed `WorkflowRunEvent` values. On
failure, Rust creates one factual `AssistantRepairActivation` containing Project, Session, Run ID,
Workflow revision, failed node/error category, and safe reason. It starts at most one new bounded
model turn in the same Session.

`AssistantRepairActivationRepositoryInterface::record_or_get_repair_activation` is the sole
persistence boundary for this fact. The unique key is `(project_id, failed_workflow_run_id)`; the
stored value also contains activation ID, Session ID, exact failed Run facts, and creation time.
`Created` alone authorizes `AssistantActivateRepairUseCase` to start the one process-scoped turn;
`Existing` returns the original fact without another invocation. There is no repair queue, retry
state, selected repair action, or method on `AssistantWorkflowChangeRepositoryInterface` for this
behavior.

The model decides whether and how to repair. Every repair is a new Assistant Workflow Change and
must pass the same review, human approval, idempotent apply, and canonical Run path. Product code
never chooses a repair step or mutates the Workflow automatically.

## Sidecar Boundary

The Python process implements `AssistantModelRunnerInterface` through a strict framed NDJSON protocol.
Rust owns tool schemas, trusted context, persistence, validation, and side effects; Python owns the
Agents SDK Runner, model composition, read-only Reviewer, SDK Session, and opaque continuation
serialization.

The protocol has an explicit version, exact frame kinds, contiguous per-direction sequence,
UTF-8/JSON depth and size bounds, invocation deadline, event/tool-call limits, and fail-closed state
transitions. Unknown fields, duplicate keys, non-finite numbers, sequence gaps, oversized frames,
partial frames, and incompatible continuation envelopes fail the invocation.

Protocol version and Assistant contract epoch are both `1`. Every line is one UTF-8 JSON object
`{ protocol_version, invocation_id, direction_sequence, kind, payload }`; sequence starts at `1`
independently in each direction. Rust sends exactly `InvocationStart`, `ToolResult`,
`ContinuationResume`, or `InvocationCancel`. Python sends exactly `InvocationAccepted`,
`ModelOutputDelta`, `ToolCall`, `ReviewerVerdict`, `ContinuationEnvelopeReady`,
`InvocationCompleted`, or `InvocationFailed`. Typed payloads are respectively: start kind plus exact
trusted context/tool contracts/budgets; call ID/tool ID/typed result; envelope plus trusted resume
result; cancel reason; Agent ID; bounded text delta; call ID/tool ID/typed arguments; change
ID/digest/fetch receipt/verdict/prose; opaque envelope; bounded final text; or failure category and
safe message. Start requires exactly one bounded user message or repair activation. Cancel reason is
`Deadline` or `ProcessShutdown`. No payload accepts an arbitrary extension map.

Frame size is at most 8 MiB, nesting depth 32, and one invocation permits at most 512 inbound
events, 64 tool calls, 16 model turns, 16 MiB total bytes in each direction, and 10 minutes. Tool
calls execute serially. Exhaustion fails once and never automatically continues.
Legal order is Start or Resume, Accepted, then bounded Delta or one-at-a-time ToolCall/ToolResult
pairs; ReviewerVerdict may follow its exact candidate fetch, EnvelopeReady may follow a paused
approval turn, and exactly one Completed or Failed terminates. Cancel permits only Failed next.
Duplicate call IDs, a second terminal, or any frame after terminal is a protocol violation.

A continuation envelope records protocol version, Assistant contract epoch, SDK version, Agent
identity, complete tool-version set, and opaque state. Resume rejects any mismatch. Old epochs are
not migrated into new model/tool contracts; canonical Workflow, Asset, and provider-independent
state remain unaffected.

The envelope is at most 4 MiB and is consumed once by exact Project, Session, invocation, Agent
identity, and tool-version set. Agent identities are `workflow_coauthor@1` and
`workflow_change_reviewer@1`; the complete tool set is exactly the eleven IDs in this document.

## Configuration And Credentials

Non-secret `AssistantModelConfig` has exactly four fields: `schema_version`, `enabled`,
`model_profile_ref: AssistantModelProfileRef`, and
`credential_id: AssistantModelCredentialId`. `AssistantModelProfileRef` is a stable,
provider-independent product choice. `DesktopCompositionRoot` maps it to the one shipped native
SDK route; arbitrary endpoints, native model strings, and provider options are not MVP inputs.

`AssistantModelProfileId` follows the Generation Profile lowercase dot-segment contract; its
version is a non-zero `u32`, and its canonical ref is `<id>@<version>`.
The catalog contains only `assistant.workflow_coauthor@1`, displayed as `Workflow Co-author`. It
maps privately to the OpenAI Responses route using explicit native model `gpt-5.4`, HTTP/SSE,
`parallel_tool_calls = false`, and SDK `max_turns = 16`; the Reviewer uses the same route and model
through its distinct fixed Agent identity. SDK defaults, environment model names, Chat Completions,
WebSocket transport, hosted tools, handoffs, and hosted multi-agent orchestration are not selected.
The sidecar lock pins `openai-agents==0.18.1`; its exact version is recorded in every continuation
envelope, and changing it requires a new reviewed Assistant contract epoch.

Invocation, frame, model-turn, tool-call, output-size, snapshot-size, candidate-size, and approval-
expiry bounds are validated `DesktopBackendConfig` values rather than model-editable settings.
Temperature overrides, prompt suffixes, product-installed skills, and separate Reviewer model
selection are deferred.

The frozen defaults and maxima are identical: approval expires after 30 minutes; the remaining
invocation/frame/turn/tool/output/snapshot/candidate bounds are the exact values above. Startup
rejects a config that weakens or exceeds them; D0.6 owns their non-secret wire representation.

`AssistantModelCredentialVaultInterface` uses the operating-system credential facility. Public DTOs
expose only credential presence. Plaintext never enters JSON, SQLite, model messages, unrelated
sidecar frames, errors, or logs; an ephemeral development environment variable may be supplied
directly to composition but is never persisted.

The composition root must enforce `enabled`, load the selected model configuration, and supply the
credential when constructing the runner adapter. Missing/denied credentials make Assistant
unavailable without affecting manual Workflow and Asset behavior.

## Failure And Security

The Assistant fails closed on stale revision, Project/Session mismatch, concurrent invocation,
expired change, invalid review evidence, changed tool version, approval reuse, continuation
mismatch, fingerprint mismatch, unavailable Asset/Profile, protocol violation, timeout, and budget
exhaustion.

Public errors are structured `AssistantApplicationError` values translated once to
`DesktopErrorDto`. Stored secrets, paths, media bytes, provider bodies, prompt history, and opaque
SDK state never enter UI events or ordinary logs.
Its categories are exactly `NotFound`, `NotVisible`, `RevisionConflict`, `InvalidTransition`,
`ConcurrentInvocation`, `PendingApprovalExists`, `StaleWorkflowRevision`, `ApprovalMismatch`,
`ApprovalExpired`, `ReviewEvidenceInvalid`, `CandidateFingerprintMismatch`, `ContinuationIncompatible`,
`ContinuationInterrupted`, `ModelUnavailable`, `ProtocolViolation`, `BudgetExceeded`,
`DeadlineExceeded`, and `ExternalBoundaryFailed`. Only a boundary failure explicitly safe before any
model request or canonical mutation is retryable.

## Verification

- Production Plan tests cover every legal/illegal transition and revision conflict;
- Workflow Change tests cover immutability, lineage, review evidence, approval scope, expiry, and
  state transitions;
- bridge contract tests prove Workspace/Workflow/Asset/capability authority is reused, not copied;
- model adapter tests cover schemas, limits, framed transport, continuation compatibility, and
  exact approval resume;
- recovery tests stop before/after Workflow apply and Run admission and prove receipt reuse;
- Run/repair tests prove canonical events, factual activation, and a new reviewed approval cycle;
- security tests prove trusted-context separation, path/secret redaction, and fail-closed protocol;
- cross-language fixtures keep Rust, Python, and TypeScript contracts aligned.

## Post-MVP

Parallel model tools, several pending approvals per Session, automatic continuation after turn
exhaustion, distributed Session leases, multi-device continuation, broad Asset search, autonomous
unreviewed repair, provider billing decisions, and generalized agent scheduling remain deferred.
