# Strong Assistant MVP

> Status: implemented and code-review approved
> Last updated: 2026-07-15

This document describes the implemented in-app Assistant architecture. Assistant
configuration remains documented in [`ASSISTANT_CONFIG.md`](ASSISTANT_CONFIG.md).

## Product contract

The Assistant is an agentic co-author for the Project's single editable
Workflow. It is not a one-shot graph generator and it does not hide creative
state in a second graph.

For the MVP, one production task follows this path:

1. The user describes a multi-shot music or video result.
2. The Assistant reads bounded Project, Workflow, Asset, capability, and mock-run
   facts.
3. The Assistant creates or updates a durable `ProductionPlan` as working memory.
4. One Agents SDK Runner invocation repeatedly plans, selects plan items,
   evaluates bounded patches, builds immutable Workflow candidates, and corrects
   structured failures.
5. A read-only Reviewer Agent fetches the exact candidate from Rust and either
   rejects it with feedback or produces a Rust-authoritative passed receipt.
6. Only a passed receipt can surface the exact candidate for human approval.
7. Approval resumes the same serialized SDK `RunState` and applies the reviewed
   ordered patch sequence once to the canonical Workflow.
8. The existing mock runner executes the approved Workflow and reports progress.
9. An injected failure supplies a factual lifecycle activation in the same
   Session. The Assistant proposes, reviews, and requests approval for a repair
   before applying and rerunning it.

The user sees one logical production task, one editable Workflow, explicit
approval boundaries, factual progress, and honest incomplete or failed states.

## Core identities

- **Co-author:** translates natural-language intent into a structurally valid,
  editable Workflow through iterative candidate construction.
- **Operator:** applies and runs only the exact change authorized by the user.
- **Monitor/repairer:** observes factual run state, diagnoses a failed subgraph,
  and proposes a separately reviewed and approved repair.

## Authority model

Each concept has one semantic owner:

| Concept | Authority |
| --- | --- |
| Workflow graph, validation, revision, undo, and patch dedupe | Rust Workflow authority and engine semantics |
| ProductionPlan transitions and persistence | Rust `production_plan` capability |
| Candidate lineage, review receipt, and approval scope | Rust `reviewed_change` capability |
| Repeated model/tool turns and approval interruption/resume | OpenAI Agents SDK Runner and `RunState` |
| Conversation continuity | File-backed SDK Session |
| Mock execution state | Existing Workflow run authority |
| Workflow projection and approval presentation | React UI |

The Workflow is the only executable and editable creative state. A
`ProductionPlan` contains user-meaningful items, progress, blocked reasons, and
acceptance notes, but no node graph, capability queue, or execution semantics.
An immutable candidate is a review proposal, not a second editable Workflow.

## SDK-owned loop

`Runner.run_streamed(...)` owns the ReAct loop:

```text
observe -> plan -> act -> validate -> review -> revise -> request approval
```

Product code provides trusted tools, facts, persistence, limits, lifecycle
events, and approval UI. It must not:

- implement a model/tool loop;
- select or activate the next shot or plan item;
- consume `ProductionPlan` as a queue;
- invoke the Runner once per shot, patch, validation failure, or review failure;
- automatically continue after max-turn exhaustion;
- treat model prose as review evidence;
- apply or run an Assistant-authored change outside the review and approval path.

The initial user turn has one production Runner entry. Approval resumes the exact
stored `RunState`; it is not a new creative invocation. A factual mock-run failure
may start one new same-Session lifecycle turn. That activation includes run facts
and never reads the ProductionPlan or chooses repair steps.

MVP model settings disable parallel tool calls and set an explicit maximum turn
count. Time, frame, byte, and tool-call limits remain enforced at the Rust and
sidecar boundaries. Exhaustion and tool failures are reported as incomplete or
blocked, never as success.

## Candidate, review, and approval chain

Assistant-authored changes cannot directly call the canonical mutating
`workflow_apply_patch` operation. Manual UI editing may continue to use the
canonical patch command.

### Candidate preparation

`workflow_prepare_patch` evaluates one bounded patch against an authoritative
Workflow revision or a prior immutable candidate. Rust stores:

- candidate ID and base Workflow revision;
- the exact ordered bounded-patch sequence and digest;
- the resulting Workflow fingerprint;
- engine-derived validation findings;
- lineage and expiration.

Preparation never mutates the canonical Workflow. Extending a candidate keeps
patch-local alias scope and preserves the exact sequence later reviewed and
replayed.

### Exact review

The main Assistant passes only `candidate_id` to the Reviewer Agent. The Reviewer
uses a read-only Rust tool to fetch the candidate, diff, validation report, user
intent, and relevant plan context.

A `pass` verdict is rejected unless nested-run evidence contains a successful
Rust fetch with the same candidate ID and digest. The evidence hash is derived
from that Rust response, not Reviewer-authored text. A trusted internal protocol
frame persists the receipt; receipt creation is not exposed as a model tool.

### Human approval and exact apply

A Rust-backed dynamic approval resolver creates an SDK interruption only for a
valid passed receipt bound to the current Session, stable `approval_scope_id`,
candidate, digest, reviewer version, and expiration. Invalid or mismatched
receipts create neither an interruption nor an effect.

The pending approval and serialized `RunState` are durable. Approve or reject
resumes the same scope, including from a new transport invocation. One Session
may have only one active SDK invocation and one pending approval; other Sessions
remain usable.

On approval, the canonical Workflow patch service replays the exact candidate
sequence and commits one Workflow revision and one undo unit. Its existing
request receipt is the idempotency authority. A crash before commit leaves no
change; replay after commit returns the prior result. Stale revisions, changed
identity, cross-Project use, and receipt reuse fail closed.

## Mock execution and repair

After apply, a stable action-derived Run ID starts the existing mock Workflow
runner. The MVP proves progress, success, injected failure, and repair without
claiming provider-grade exactly-once execution.

A mock failure records factual lifecycle context and starts a new turn in the
same Session. The SDK-managed Assistant decides how to inspect and repair it. A
repair follows the same chain:

```text
prepare candidate -> review rejection -> revise -> review pass
-> human approval -> exact apply -> rerun
```

No repair mutates the Workflow before approval, and no automatic creative step
is selected by product code.

## UI contract

The Assistant UI exposes:

- Agent-authored ProductionPlan progress;
- immutable Workflow candidate preview;
- Reviewer findings and the exact approval card;
- mock execution progress and failure state;
- a replacement repair approval when required.

The canonical Workflow remains visible and editable. Candidate preparation and
review do not change its revision. Only the exact approved candidate replaces
the canonical head.

## MVP boundaries

Implemented:

- SDK-native iterative co-authoring across multiple plan items;
- durable non-executable ProductionPlan;
- immutable candidate lineage and exact replay;
- exact-candidate Reviewer Agent with Rust-authoritative receipts;
- durable scoped approval and idempotent local apply;
- mock execution, factual failure activation, reviewed repair, and rerun;
- transparent UI and deterministic cross-language E2E coverage.

Deliberately post-MVP:

- real production providers and provider reconciliation;
- a generalized outbox or distributed exactly-once guarantees;
- product-owned schedulers, shot loops, or plan queue consumers;
- multi-device leases and distributed Session coordination;
- broad automated media-quality inspection;
- automatic continuation after Runner exhaustion.

## Frozen discovery boundary

The Strong Assistant MVP adds separate production operations and tests. It does
not change the existing Assistant discovery contract or its frozen fixtures:

- `src-tauri/src/capability_discovery.rs`
- `src-tauri/src/capability_discovery/support.rs`
- `src-tauri/tests/capability_discovery.rs`
- `assistant/tests/test_capability_discovery.py`
- `src-tauri/tests/contract/assistant_operation_contract.rs`
- `ui/src/__fixtures__/assistant_operations.json`
- `assistant/tests/test_tool_contract.py`

## Verification

The merge gate is:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
./scripts/e2e.sh
```

Deterministic Python tests prove the SDK loop, Reviewer evidence, approval
interruption/resume, and repair turns. Rust tests prove authoritative state,
CAS, receipt identity, crash boundaries, exact candidate application, and mock
run behavior. UI tests prove plan, preview, review, approval, progress, failure,
and repair presentation.
