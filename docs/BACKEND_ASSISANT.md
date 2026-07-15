# Assistant Backend Architecture

> Status: implemented Strong Assistant MVP
> Last verified against code: 2026-07-15
> Scope: Rust/Tauri Assistant backend, Python Agents SDK sidecar, persistence, review, approval, and repair

## Purpose

The Assistant is a Project-scoped Workflow co-author. The model may decide how to
use trusted operations, but it is not authoritative for Projects, Workflows,
Assets, validation, reviews, approvals, or execution state.

The implementation deliberately uses two runtimes:

- Rust/Tauri owns business semantics, trusted context, persistence, operation
  schemas, validation, side effects, and UI-facing commands.
- The Python sidecar owns the OpenAI Agents SDK `Runner`, Agent composition,
  file-backed SDK Session, nested Reviewer Agent, and opaque `RunState`.
- A strict framed NDJSON protocol over inherited stdin/stdout connects them.

This document explains the implementation. Product behavior and constraints are
also summarized in [`ASSISTANT.md`](ASSISTANT.md), while configuration is owned by
[`ASSISTANT_CONFIG.md`](ASSISTANT_CONFIG.md).

## System boundary

```text
React AssistantDock
  -> Tauri commands + Channel<JSON>
  -> assistant_commands.rs (trusted context)
  -> AssistantRuntime
  <-> framed NDJSON over child stdin/stdout
  <-> Python AgentStdioApp + Agents SDK Runner
  <-> Rust OperationRegistration
  -> Rust domain/application capabilities
```

The dependency direction is intentional. Python receives Rust-generated tool
contracts and forwards calls; it does not reimplement Workflow or Asset rules.
Rust never implements a model/tool planning loop. The only production
`Runner.run_streamed(...)` entry is in `assistant/stdio_app.py`.

## Composition root and durable state

[`src-tauri/src/state.rs`](../src-tauri/src/state.rs) is the composition root.
`AppState::from_roots_with_backend` constructs the node registry, Workflow
authority, run coordinator, Assistant services, repositories, and sidecar
command.

Assistant orchestration state is isolated under a contract epoch:

```text
config_root/
  assistant_config.json
  assistant_epochs/explicit-outputs-v2/
    production_plan.sqlite
    reviewed_change.sqlite
    assistant_approval.sqlite
    sessions/
      project-<stable-project-hash>.sqlite3
```

The epoch is a hard compatibility boundary. Old orchestration databases and SDK
states are not migrated into a new operation/schema contract. Workflow and Asset
data remain outside this epoch and keep their own formats.

| Durable state | Semantic owner | Storage |
| --- | --- | --- |
| Canonical Workflow head, revision, request receipt | `WorkflowAuthority` | `workflow.sqlite` under the config root |
| Production working memory | `ProductionPlanService` | `production_plan.sqlite` |
| Immutable candidates and review receipts | `ReviewedChangeService` | `reviewed_change.sqlite` |
| Interrupted SDK state awaiting a user decision | `PendingApprovalService` | `assistant_approval.sqlite` |
| Conversation/SDK Session | OpenAI Agents SDK `SQLiteSession` | per-Project session SQLite file |
| Active invocation lock | `ActiveAssistantSession` | in-memory `HashSet<String>` |

The Project-scoped session identity is `project:<project_id>`. Only one
invocation and one pending approval may exist for that Session, while different
Projects may run independently.

## Tauri command boundary

The UI reaches the Assistant through three commands registered in
[`src-tauri/src/lib.rs`](../src-tauri/src/lib.rs):

| Command | Responsibility |
| --- | --- |
| `assistant_send` | Validate the displayed Workflow revision and start a new SDK turn |
| `assistant_get_pending_approval` | Reconstruct the exact reviewed candidate shown in the approval card |
| `assistant_decide_approval` | Validate approval identity and resume the same SDK `RunState` |

`AssistantSendInput` contains a trusted `project_id`, the UI-observed Workflow
presence/revision, bounded selected node and Asset IDs, and user text. It never
contains a canvas snapshot, local media bytes, file paths, or an operation
contract chosen by the model.

Before launching the sidecar, Rust verifies:

- Project IDs are ASCII, non-empty, and at most 160 characters;
- text is non-empty and at most 32 KiB of characters;
- each selection contains at most 16 bounded IDs;
- the UI Workflow presence and revision exactly match `WorkflowAuthority`;
- the Project exists;
- the Session has no active invocation and no pending approval.

The command creates Rust-generated invocation and request IDs, resolves the
per-Project session database path, and passes selections through
`TrustedInvocationContext`. This trusted context has no Serde or JSON Schema
representation in the model-facing operation arguments.

Native Responses events are forwarded without field remapping through a Tauri
`Channel<Value>`. Rust-authored run lifecycle events use the
`assistant.workflow_run.*` namespace.

## Sidecar process and transport

Development launches:

```text
python3 -m assistant.stdio_app
```

Packaged builds launch the target-triple-suffixed PyInstaller binary next to the
desktop executable. [`scripts/build-assistant.sh`](../scripts/build-assistant.sh)
creates that binary, and `src-tauri/tauri.conf.json` bundles it as an external
binary.

Each invocation launches a fresh child with piped stdin/stdout and inherited
stderr. Rust adds `OH_MY_DREAM_CONFIG_ROOT`, sets `kill_on_drop`, and applies:

- a 300-second invocation timeout;
- a 5-second shutdown timeout;
- at most 512 incoming frames;
- at most 8 MiB of collected payload/tool-output data.

The transport contract is implemented independently in Rust
([`assistant_transport.rs`](../src-tauri/src/assistant_transport.rs)) and Python
([`stdio_protocol.py`](../assistant/stdio_protocol.py)) with matching limits:

| Rule | Value |
| --- | --- |
| Protocol version | `1` |
| Encoding | one UTF-8 JSON object per newline |
| Maximum encoded frame | 1 MiB, including newline |
| Maximum JSON container depth | 64 |
| Exact integer domain | `0..=9_007_199_254_740_991` for sequence values |
| Ordering | contiguous sequence numbers, independently per direction |
| Top-level fields | exactly `protocol_version`, `sequence`, `kind`, `payload` |

Duplicate keys, non-finite numbers, invalid UTF-8, unknown fields/kinds,
oversized or partial frames, sequence gaps, and invalid state transitions fail
closed. After a protocol failure the decoder is not reused.

Frame families cover invocation, native Responses events, tool request/response,
review submit/check handshakes, approval request/response, snapshots, and
terminal completion/error.

`cancel` exists in the protocol enum but is not part of the current production
Assistant command flow.

## SDK Runner and Session behavior

[`assistant/stdio_app.py`](../assistant/stdio_app.py) builds one main Agent per
sidecar invocation. [`assistant/sdk_runtime.py`](../assistant/sdk_runtime.py)
configures:

- Agent name `workflow_assistant`;
- Agents SDK compatibility version `0.18.1`;
- at most 64 SDK turns;
- `parallel_tool_calls=False`;
- SDK tracing disabled;
- one file-backed `SQLiteSession` per Project.

A new turn passes user text to `Runner.run_streamed`. A resumed turn restores
the opaque SDK `RunState`, applies exactly one approval decision to exactly one
interruption, and gives the restored state back to the Runner.

Current wiring note: `assistant/config.py` can parse `assistant_config.json` and
`OMD_ASSISTANT_*`, but the production `AgentStdioApp` currently constructs the
Agent with `model=None` and does not call `AssistantConfig.load`. Likewise,
`assistant_send` does not currently enforce the stored `enabled` flag. The
configuration command and secret-safe DTO exist, but model/base-URL/API-key
wiring into the live SDK composition root is not implemented in this code.

Interrupted state is wrapped in an envelope containing:

```text
envelope_version = 2
sdk_version = "0.18.1"
agent_name = "workflow_assistant"
operation_versions = sorted [{id, version}, ...]
state_json = opaque strict SDK state
```

Resume rejects any mismatch in envelope fields, SDK version, Agent name, or the
complete operation-version set. This prevents replaying serialized SDK state
against a different tool contract.

## Rust-owned operation contract

[`assistant_operations.rs`](../src-tauri/src/assistant_operations.rs) defines the
consumer-owned operation abstraction. An `OperationRegistration` binds:

- stable ID and version;
- model-facing description;
- effect classification;
- generated input and output JSON Schemas;
- a typed async Rust handler.

Schemas are generated from canonical Rust DTOs with `schemars`, checked by the
repository's schema policy, and compiled as Draft 7 validators with
`jsonschema`. Dispatch performs schema validation, typed deserialization,
handler execution, and typed output serialization in that order.

The four effect classes are:

| Effect | Meaning |
| --- | --- |
| `local_read` | Read trusted local state |
| `assistant_state_mutation` | Mutate durable Assistant memory only |
| `visible_reversible_workflow_patch` | Canonical reversible Workflow mutation; not registered for the co-author path |
| `prepared_approval_execution` | Execute an exact previously reviewed effect after trusted approval |

Strict schemas close every object and require every property. Workflow patch
schemas are the only exception: the canonical node `params` body remains open,
while the enclosing operation and every other object stay closed. Capability
descriptions and Workflow documents use bounded dynamic output policies.

The production co-author registration set is built in
`assistant_commands::operation_registrations`:

| Operation | Version | Effect | Implementation |
| --- | ---: | --- | --- |
| `workspace_get_snapshot` | 1 | local read | bounded Project/Workflow/Asset/run projection |
| `workflow_evaluate_patch` | 2 | local read | engine evaluation without commit |
| `capability_search` | 1 | local read | ranked bounded discovery |
| `capability_describe` | 1 | local read | exact admitted contracts |
| `production_plan_get` | 1 | local read | read Agent memory |
| `production_plan_create` | 1 | Assistant state mutation | create plan at revision 1 |
| `production_plan_replace` | 1 | Assistant state mutation | CAS replacement |
| `production_plan_update_item` | 1 | Assistant state mutation | validated item transition |
| `workflow_prepare_patch` | 2 | Assistant state mutation | create immutable candidate |
| `workflow_candidate_get` | 2 | local read | exact candidate evidence |
| `workflow_apply_reviewed_candidate` | 2 | prepared approval execution | exact replay and atomic commit |

The direct `workflow_apply_patch` registration exists for the canonical/manual
boundary but is intentionally absent from the Assistant co-author operation set.

## Bounded observation and discovery

`workspace_get_snapshot` composes an authoritative projection. It returns no
more than eight recent Asset summaries, sixteen selected Assets/nodes per
collection, one active Run, and 512 KiB serialized output. Asset summaries omit
paths and bytes and expose only stable identity, kind, bounded prompt,
provenance, and timestamps.

Capability discovery is request-scoped. Search returns at most five current
exact references. Describe accepts at most three references per call, eight
distinct references and 96 KiB of parameter schema per invocation. A reference
must have been admitted by search or already be persisted in the current
Workflow. The in-memory admission ledger retains at most 256 active request
entries for ten minutes.

## ProductionPlan semantics

`ProductionPlan` is durable Agent working memory, not an executable queue. It
contains a title and at most 128 user-meaningful items. Rust owns all transitions:

```text
pending -> in_progress -> completed
pending -> blocked -> in_progress
in_progress -> blocked
```

Every mutation uses compare-and-swap revision checks. The product never selects
the next item, reads the plan as a scheduler queue, or starts a Runner per item.

## Candidate, review, and approval pipeline

```text
bounded patch -> immutable candidate -> attested review -> SDK interruption
-> durable human decision -> exact RunState restore -> exact replay and commit
```

### Candidate preparation

`ReviewedChangeService::prepare` loads the authoritative base revision, applies
one bounded `WorkflowPatch` with engine semantics, and persists a new immutable
candidate. Extending `prior_candidate_id` copies its exact patch sequence,
Workflow, and aliases before appending the new patch.

A candidate stores Project and Session scope, user intent, base revision,
ordered patches, aliases, resulting Workflow, readiness blockers, SHA-256 patch
digest, SHA-256 Workflow fingerprint, and a one-hour expiration. Preparation
never advances the canonical Workflow revision.

### Reviewer attestation

The main Agent calls the nested `review_workflow_candidate` tool with only a
candidate ID. The Reviewer has only `workflow_candidate_get`, must fetch exactly
once, and returns a typed `pass` or `reject` verdict.

The Python wrapper verifies that the fetched candidate ID matches the request,
extracts the Rust candidate digest, and hashes the exact Rust tool output. It
sends the resulting attestation through the internal `review_submit` frame.
Rust verifies Project/Session/candidate/digest/expiry and persists the receipt.
Receipt creation is not a model-facing operation.

### Dynamic approval and exact apply

The SDK marks `workflow_apply_reviewed_candidate` as approval-gated only when
Rust confirms that its `review_receipt_id` names a current passed receipt in the
same Project and Session. The sidecar serializes the interrupted `RunState` and
exact call identity; Rust persists both.

The UI decision must match the pending `approval_scope_id` and candidate digest.
On approval, runtime dispatch also requires the same call ID, operation ID,
operation version, and exact `arguments_json`. Approval proof is constructed by
Rust and cannot be supplied in model JSON.

Apply replays every stored patch against the current authoritative base,
revalidates Asset Source visibility/kind/file availability, recomputes the
Workflow fingerprint, and commits only if it equals the reviewed candidate.
The approval scope becomes the Workflow request ID, so a post-commit replay
returns the prior receipt rather than creating another revision or undo unit.

## Run and repair lifecycle

After a successful approved apply, `AssistantRepairService` derives a stable Run
ID from Project ID, approval scope, and Workflow revision. It executes the exact
canonical Workflow through the existing local mock runner and streams start,
progress, and terminal events.

On failure, Rust creates a factual `RepairActivation` containing only kind,
Project/Session, Run ID, Workflow revision, and reason. That JSON starts one new
Runner invocation in the same SDK Session. The Agent must prepare, review, and
request approval for a replacement candidate. Product code never chooses a
repair step or mutates the Workflow automatically.

## Failure and security model

The implementation fails closed on stale revisions, Project/Session mismatch,
expired candidates, invalid review evidence, changed operation versions,
approval reuse, rejected-effect execution, fingerprint mismatch, unavailable
Assets, protocol violations, timeouts, and resource-budget exhaustion.

Stored secrets remain in `assistant_config.json` or environment variables.
Public Rust and TypeScript configuration DTOs expose only `has_key`. API keys,
local Asset paths, media bytes, and SDK internal context do not enter model tool
arguments or UI stream events.

## Known limitations and optimization direction

The core authority and approval chain is coherent, but the current MVP has
material implementation gaps. The detailed, prioritized backlog is in
[`BACKEEND_ASSISANT_DISCOVER.md`](BACKEEND_ASSISANT_DISCOVER.md#review-findings-and-prioritized-optimization-backlog).
Configuration wiring is a confirmed functional bug when the settings UI is part
of the product contract; plaintext key persistence is a security debt. The
remaining rows are robustness, operability, performance, or scope limitations.

| Area | Current limitation | Direction |
| --- | --- | --- |
| Configuration and secrets | Stored settings are not wired into the live SDK; `enabled` is ignored; keys are plain JSON | Centralize model composition, enforce disabled state, and move keys to the OS credential store |
| Contracts and lifecycle | Commands expose string errors; UI parses sentinels; `cancel` is unimplemented | Add closed error DTOs and an end-to-end cancellation state machine |
| Retention and coordination | Expired review data is not pruned; Session locking is process-local | Add reference-safe pruning now; add durable leases only for a real multi-process requirement |
| Performance and diagnostics | Every turn spawns a sidecar and recompiles schemas; metrics are absent | Instrument and benchmark before introducing cached validators or supervised process reuse |
| Product maturity | Reviewer configuration is shared; execution remains mock/local without outbox reconciliation | Isolate Reviewer configuration, then add provider ports and reconciliation as separate milestones |

The highest-priority sequence is configuration correctness and secret storage,
structured errors, cancellation, then observability. Process reuse and
distributed coordination should follow measurements or a concrete deployment
requirement, not speculative abstraction.

## Verification map

Rust coverage lives in `src-tauri/tests/assistant_*` and
`src-tauri/tests/reviewed_change_*`; Python SDK/transport/Reviewer coverage is
under `assistant/tests/`; UI contracts and projections are tested in
`ui/src/api/` and `ui/src/assistant/`.

The complete merge gate is:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
./scripts/e2e.sh
```
