# Assistant Backend Discovery and Extension Guide

> Status: code-derived navigation guide
> Last verified against code: 2026-07-15
> Read first: [`BACKEND_ASSISANT.md`](BACKEND_ASSISANT.md)

## What this document is for

Use this guide when diagnosing the Assistant, adding an operation, changing an
approval rule, or tracing a user request across Rust, Python, and React. It maps
questions to authoritative files and gives safe change procedures.

The most important discovery rule is:

> Search for the semantic owner first. Do not repair a boundary projection by
> duplicating the business rule in Python or React.

## Five-minute architecture discovery

Read these files in order:

1. [`src-tauri/src/state.rs`](../src-tauri/src/state.rs) — concrete adapters,
   repositories, epoch root, and service wiring.
2. [`src-tauri/src/assistant_commands.rs`](../src-tauri/src/assistant_commands.rs)
   — Tauri entry, trusted context, operation set, Session guard, and outcomes.
3. [`src-tauri/src/assistant_runtime.rs`](../src-tauri/src/assistant_runtime.rs)
   and its submodules — child lifecycle, frame dispatch, approval resume, limits.
4. [`assistant/stdio_app.py`](../assistant/stdio_app.py) — the sole SDK Runner
   composition and Rust tool bridge.
5. [`src-tauri/src/assistant_operations.rs`](../src-tauri/src/assistant_operations.rs)
   — the typed operation contract and effect/approval enforcement.
6. [`src-tauri/src/reviewed_change/`](../src-tauri/src/reviewed_change/) —
   candidate, receipt, review, and exact replay semantics.
7. [`ui/src/assistant/AssistantDock.tsx`](../ui/src/assistant/AssistantDock.tsx)
   — UI stream and approval projection.

Then use the focused maps below instead of reading the repository linearly.

## Request trace: new user turn

```text
AssistantDock.send
  -> WorkflowApi.sendAssistant
  -> tauriApi.sendAssistant
  -> Tauri assistant_send
  -> assistant_send_with_state
  -> validate_send
  -> runtime_for_state + operation_registrations
  -> AssistantRuntime::invoke_streamed
  -> runner::run_process
  -> sidecar process
  -> AgentStdioApp.run_once
  -> Runner.run_streamed
  -> responses_event/tool_request/... frames
  -> ChannelAssistantSink
  -> AssistantDock.handleEvent
```

Primary files:

| Step | File |
| --- | --- |
| UI context and send | `ui/src/assistant/AssistantDock.tsx` |
| TypeScript API | `ui/src/api/types.ts`, `ui/src/api/tauriApi.ts` |
| Tauri input validation | `src-tauri/src/assistant_commands.rs` |
| Runtime creation | `src-tauri/src/assistant_commands.rs::runtime_for_state` |
| Child lifecycle | `src-tauri/src/assistant_runtime/runner.rs`, `process.rs` |
| Frame state machine | `src-tauri/src/assistant_runtime/frames.rs` |
| SDK Agent | `assistant/stdio_app.py` |
| Prompt policy | `assistant/system_prompt.py` |

If the call fails before the sidecar starts, inspect `validate_send` and Session
guards. If a child starts but no model stream reaches React, inspect frame order,
the invocation ID checks, and `ChannelAssistantSink`.

## Request trace: one model tool call

```text
model chooses FunctionTool
  -> tool_contract.py builds ToolRequest
  -> Python emits tool_request
  -> Rust FrameHandler::handle_tool_request
  -> dispatch_tool
  -> OperationRegistration::dispatch
  -> JSON Schema validation
  -> typed handler with RequestContext
  -> canonical output serialization
  -> Rust emits tool_response
  -> Python returns output_json to the SDK
```

Debug in this order:

1. Confirm the operation appears in
   `assistant_commands::operation_registrations`.
2. Inspect the generated `OperationContract` fixture in
   `ui/src/__fixtures__/assistant_operations.json`.
3. Confirm Python passes `id`, description, schema, strictness, and approval
   metadata unchanged in `assistant/tool_contract.py`.
4. Inspect `assistant_runtime/dispatch.rs` for operation/version/approval context.
5. Inspect the capability handler and its structured `OperationHandlerError`.

Do not add operation-specific parsing to Python. New operation semantics belong
in a Rust capability and are exposed through an `OperationRegistration`.

## Request trace: review and approval

```text
main Agent: review_workflow_candidate({candidate_id})
  -> nested Reviewer Agent
  -> workflow_candidate_get via Rust
  -> reviewer.py attests exact tool output
  -> review_submit internal frame
  -> assistant_review_bridge.rs
  -> ReviewedChangeService::record_review
  -> review receipt persisted

main Agent: workflow_apply_reviewed_candidate({review_receipt_id})
  -> Python review_check
  -> Rust valid_for_approval
  -> SDK interruption
  -> approval_request + waiting snapshot
  -> PendingApprovalService::save
  -> ASSISTANT_APPROVAL_DEFERRED
  -> UI loads exact pending candidate
  -> human approve/reject
  -> assistant_decide_approval
  -> same SDK RunState restored
  -> exact tool call dispatched or rejected
```

Authority map:

| Question | Source of truth |
| --- | --- |
| What candidate was reviewed? | `WorkflowCandidate` in `reviewed_change.sqlite` |
| Did Reviewer fetch exact Rust evidence? | `assistant/reviewer.py::attest_review_result` |
| Is a receipt valid for approval? | `ReviewedChangeService::valid_passed_receipt` |
| What does the human see? | `assistant_commands/pending.rs::pending_approval_dto` |
| Which call resumes? | persisted `AssistantWaitingApproval.pending` |
| Was the exact Workflow committed? | `WorkflowPatchService::apply_sequence` fingerprint check |
| Is replay idempotent? | Workflow authority receipt keyed by approval scope/request hash |

When approval is unexpectedly absent, verify that the receipt verdict is
`pass`, it has not expired, Project and Session match, and Python received
`valid=true` from `review_check_response`. When approval appears stale, compare
the UI-submitted approval scope and candidate digest with the freshly replayed
pending DTO.

## Request trace: approved run and repair

```text
approved apply returns Workflow head
  -> assistant_commands/repair.rs::finish_approval_outcome
  -> AssistantRepairService::execute_with_events
  -> WorkflowRuns::run
  -> assistant.workflow_run.* events
  -> success: return head
  -> failure: create RepairActivation
  -> new invoke_streamed in same Session
  -> candidate/review/approval pipeline repeats
```

The repair activation is factual input, not a repair plan. If product code starts
selecting plan items or patch operations, the architecture has been violated.
`assistant/tests/test_environment_only_architecture.py` protects the single
Runner entry and absence of a plan scheduler.

## Semantic-owner map

| Concept | Authoritative implementation | Boundary projections |
| --- | --- | --- |
| Workflow graph and patch legality | `crates/engine/src/workflow_patch*`, `validation.rs` | Tauri DTOs, TS Workflow types |
| Capability contract and ports | `crates/engine`, `crates/nodes` registry/contracts | discovery DTOs and schemas |
| Canonical Workflow revision/receipt | `src-tauri/src/workflow_authority.rs` | `WorkflowHeadDto` |
| Asset visibility and managed content | `crates/assets`, `managed_asset_access.rs` | workspace/approval Asset summaries |
| ProductionPlan transitions | `src-tauri/src/production_plan/mod.rs` | operation DTOs, UI plan item projection |
| Candidate and receipt identity | `src-tauri/src/reviewed_change` | Reviewer and approval DTOs |
| Operation schema/effect | `OperationRegistration` in Rust | Python `FunctionTool`, fixture JSON |
| Model/tool loop | OpenAI Agents SDK Runner | Rust frame host |
| Pending human decision | `AssistantWaitingApproval` + pending repository | approval card |
| Run lifecycle | `WorkflowRuns` and repair service | Tauri Channel events |

## File map by capability

### Rust host and protocol

- `src-tauri/src/assistant_transport.rs` and `codec.rs`: wire grammar and bounds.
- `src-tauri/src/assistant_sidecar.rs`: child ownership, shutdown, packaged path.
- `src-tauri/src/assistant_runtime/process.rs`: injectable process abstraction.
- `src-tauri/src/assistant_runtime/runner.rs`: timeout-bounded new/resume loop.
- `src-tauri/src/assistant_runtime/frames.rs`: incoming frame state machine.
- `src-tauri/src/assistant_runtime/dispatch.rs`: trusted tool dispatch and approval
  proof.
- `src-tauri/src/assistant_runtime/payload.rs`: exact frame payload DTOs.
- `src-tauri/src/assistant_runtime/types.rs`: invocation, snapshot, pending, and
  outcome types.

### Rust Assistant capabilities

- `workspace_snapshot.rs`: bounded Project workspace read.
- `capability_discovery.rs`: search/describe admission and budgets.
- `production_plan/`: domain transitions, operations, repository, SQLite adapter.
- `reviewed_change/`: immutable candidate lineage, Reviewer receipts, apply gate.
- `workflow_patch_operation.rs`: evaluate/apply, Asset validation, error mapping.
- `assistant_approval.rs`: durable interrupted SDK state.
- `assistant_review_bridge.rs`: internal review receipt adapter.
- `assistant_repair/`: approved mock run and factual failure activation.

### Python sidecar

- `stdio_protocol.py`: Python protocol implementation.
- `stdio_invocation.py`: strict invoke/resume correlation parsing.
- `stdio_app.py`: Agent composition, Runner entry, frame bridge.
- `sdk_runtime.py`: Session, settings, state envelope compatibility.
- `tool_contract.py`: mechanical Rust operation-to-SDK tool projection.
- `reviewer.py`: nested read-only Reviewer and attestation.
- `system_prompt.py`: product behavior constraints.
- `config.py`: file/environment configuration parser; currently not called by
  the production `AgentStdioApp` composition root.

### UI projection

- `ui/src/api/types.ts`: frontend Assistant contracts.
- `ui/src/api/tauriApi.ts`: Tauri invocation and Channel creation.
- `ui/src/assistant/assistantStream.ts`: native event projection.
- `ui/src/assistant/AssistantDock.tsx`: command lifecycle and approval handling.
- `ui/src/assistant/AssistantApprovalCard.tsx`: exact reviewed candidate display.
- `ui/src/assistant/StrongAssistantTask.tsx`: ProductionPlan and Run projection.

## How to add a model-facing read operation

1. Identify the business capability that owns the read. Define its focused
   consumer trait only if it crosses a real substitution boundary.
2. Create canonical Rust input/output DTOs with `serde(deny_unknown_fields)` and
   `JsonSchema`.
3. Register a stable ID/version with `OperationEffect::LocalRead` and a strict
   input schema.
4. Derive Project, Session, request, and UI selection only from `RequestContext`.
5. Add the registration to `assistant_commands::operation_registrations`.
6. Add Rust handler, schema, operation-contract, and runtime dispatch tests.
7. Regenerate/update the cross-language fixture and TypeScript types/tests.
8. Add Python tests only for SDK projection or transport behavior; do not copy
   the Rust rule into Python.

Use bounded outputs. A model-facing read should not expose paths, secrets,
unbounded lists, binary content, or a caller-selected Project scope.

## How to add an Assistant-owned state mutation

Use `OperationEffect::AssistantStateMutation` only for durable state that is not
itself executable or user-visible creative authority. Put state transitions on
the aggregate, use a capability-scoped repository, and use CAS when concurrent
updates can race.

The `ProductionPlan` implementation is the reference pattern:

```text
typed operation -> application service -> aggregate transition
                -> consumer-owned repository -> SQLite adapter
```

Do not introduce a second Workflow, a hidden execution queue, or a product-owned
scheduler under this effect.

## How to add an approval-gated effect

An approval-gated operation requires more than `needs_approval=true`:

1. Prepare and persist immutable effect identity before approval.
2. Produce trusted review/evidence outside the model-facing tool surface.
3. Implement `InternalReviewHandler::valid_for_approval` for the exact operation.
4. Register `OperationEffect::PreparedApprovalExecution`.
5. Bind approval to operation ID/version, call ID, exact arguments, Project,
   Session, candidate digest, and expiration.
6. Revalidate mutable dependencies immediately before commit.
7. Make execution idempotent with a durable request receipt.
8. Test rejection, stale state, mismatch, crash-before-commit, and replay.

Do not expose an approval boolean or `ApprovedEffect` in the operation schema.
Approval proof must be constructed by Rust runtime context.

## How to change an operation schema or version

Operation schemas participate in serialized SDK `RunState` compatibility.
Changing a schema is therefore not only a DTO edit.

Required sequence:

1. Change the canonical Rust DTO and its schema policy tests.
2. Bump the operation version when persisted interruptions must not resume
   against the new contract.
3. Update the Python state-envelope expectation if the SDK/envelope format
   changes.
4. If the whole orchestration state is incompatible, introduce a new
   `ASSISTANT_CONTRACT_EPOCH` instead of parsing legacy state.
5. Regenerate `ui/src/__fixtures__/assistant_operations.json` through the Rust
   contract test.
6. Update TypeScript types, `contract.test.ts`, and Python tool-contract tests.
7. Verify a stale pending approval fails closed.

Never hand-edit generated fixture semantics independently of Rust.

## How to change transport frames

Treat Rust and Python implementations as one protocol change:

1. Update both frame enums and exact payload validators.
2. Preserve independent contiguous sequences in each direction.
3. Decide whether `PROTOCOL_VERSION` must change.
4. Add symmetric encode/decode, oversized, partial, duplicate-key, depth, and
   ordering tests.
5. Update runtime frame-order tests and the fixture child process.
6. Run the frozen sidecar smoke test.

Do not log protocol payloads indiscriminately; they can contain user text and
model output even though secrets should not be present.

## Test selection by change

| Change | Minimum focused verification |
| --- | --- |
| Rust operation DTO/schema | `cargo test -p oh-my-dream-tauri --test assistant_operation_schema --test assistant_operation_contract` |
| stdio codec/frame | Rust `assistant_transport` tests plus `python3 -m pytest assistant/tests/test_stdio_protocol.py assistant/tests/test_agent_transport.py` |
| SDK state/approval | `python3 -m pytest assistant/tests/test_sdk_runtime.py assistant/tests/test_dynamic_approval.py` plus Rust runtime/approval tests |
| candidate/review/apply | reviewed-change, Reviewer, approval, and Workflow patch tests |
| ProductionPlan | production plan operation/persistence tests and co-author tests |
| repair behavior | Rust repair MVP/E2E and Python strong repair/E2E tests |
| UI stream or approval | `npm --prefix ui run typecheck` and relevant Vitest files |
| packaged sidecar | `./scripts/smoke-assistant.sh` |

Before merge, always run the repository gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
./scripts/e2e.sh
```

`./scripts/e2e.sh` runs the whole Rust workspace, all Python Assistant tests,
frontend type checking, and the complete Vitest suite.

## Review findings and prioritized optimization backlog

The authority boundaries, exact replay, schema validation, and fail-closed
approval chain are coherent. Static code review confirms the first row as a
functional bug: `assistant.rs` writes settings, but production `stdio_app.py`
constructs `AgentStdioApp` with `model=None`; only tests call
`AssistantConfig.load`, and `assistant_send` never checks `enabled`.

| Priority/type | Finding | Required optimization and proof |
| --- | --- | --- |
| P0 functional bug | Stored Assistant settings are not used by the production SDK composition root; `enabled=false` is ignored | Wire and validate config once, enforce `enabled`, test file/env precedence and disabled sends |
| P0 security debt | Configured API keys are plain JSON | Use the platform credential store; add migration and prove public DTOs/logs never expose secret values |
| P1 robustness | Tauri errors are strings and UI branches on `ASSISTANT_*` substrings | Add a structured error DTO and cross-language fixtures; remove substring parsing |
| P1 missing behavior | No user cancellation path exists although `cancel` is a frame kind | Define cancellation ownership and ordering; test during model streaming, tool dispatch, approval wait, and shutdown |
| P1 storage leak | Expired candidates/receipts are retained forever | Add reference-safe pruning with retention tests and bounded transaction work |
| P1 coverage gap | The generated operation fixture is a six-operation sample, not the eleven-operation production set; TS does not model `assistant_state_mutation` | Generate the fixture from the same production registration composition and make TS exhaustively represent all four effects |
| P1 diagnostics | Generic sidecar exceptions collapse to `sdk_error` without diagnostic correlation | Emit secret-safe structured Rust/Python diagnostics keyed by invocation ID while keeping UI messages bounded |
| P2 performance | Sidecar spawn and schema compilation repeat per turn | Benchmark cold start and per-turn cost before choosing cached validators or a supervised long-lived process |
| P2 scale limit | Session locking is process-local | Add SQLite leases only when multiple app processes or devices are supported |
| P2 review independence | Co-author and Reviewer share model configuration | Add independent Reviewer configuration and retain the evidence/version receipt contract |
| P2 documentation | Document filenames preserve requested misspellings | Add correctly spelled canonical aliases or rename with link updates when compatibility permits |

Implementation order should be vertical and testable:

1. Configuration/credential slice: composition, disabled state, secret migration,
   focused Rust/Python tests.
2. Contract slice: structured errors, production operation fixture, exhaustive TS
   effect type, regenerated contract tests.
3. Lifecycle slice: cancellation and diagnostic events across Rust/Python/UI.
4. Storage slice: retention policy and safe pruning.
5. Performance/scale slice: measure spawn/schema costs, then decide on reuse and
   cross-process leases.

Do not combine these into one refactor. Each slice changes a different authority
boundary and should remain independently reviewable and reversible.

## Common architecture mistakes

- Adding a second model/tool loop outside `AgentStdioApp`.
- Letting Rust choose the next ProductionPlan item.
- Registering direct `workflow_apply_patch` for Assistant-authored changes.
- Trusting a Reviewer verdict without exactly one matching Rust candidate fetch.
- Reconstructing approval from model prose instead of persisted receipt identity.
- Passing `project_id`, selected Assets, or approval proof through model arguments.
- Reimplementing port compatibility or Workflow validation in Python/React.
- Resuming opaque SDK state after operation-version or epoch mismatch.
- Treating candidate preparation as a canonical Workflow mutation.
- Applying a reviewed candidate without replay/fingerprint and Asset checks.
- Returning unbounded workspace/capability data to the model.

When a proposed change requires one of these, stop and move the responsibility
back to its semantic owner.
