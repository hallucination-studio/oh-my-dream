# oh-my-dream Assistant Design

> Status: proposed v6 | Last updated: 2026-07-10

This is the single architecture and delivery contract for the in-app assistant; existing settings remain in [`ASSISTANT_CONFIG.md`](ASSISTANT_CONFIG.md).

The assistant is the operation entry point for an open Project. It starts from the desired result, reads authoritative workspace state, discovers only the contracts it needs, and turns model reasoning into one editable Workflow.

## 1. Product contract

The first complete journey is a paragraph to an editable multi-shot video:

1. The user describes a video in an open Project.
2. The assistant reads the current Workflow, selection, and relevant Assets.
3. The model decides treatment, shots, prompts, durations, and composition, then discovers only the executable capabilities needed.
4. One atomic patch creates or updates the Workflow; React projects the returned head as one undo unit.
5. The user reviews or edits that same Workflow and requests execution.
6. One trusted run confirmation covers the exact prepared generation, not each node or Task.
7. Progress uses no model turns. Completion surfaces final Assets; failure returns a friendly diagnosis and recovery.

For "make something like this video with my assets", selection is only a pointer. The Workflow stores visible `AssetSource` nodes with stable Asset ID/hash; target inputs and ordered bindings express role and order. The model interprets evidence, while Rust validates and executes without producing a shot plan.

The assistant must work as a **co-author** of an editable Workflow, an **operator** of one approved Run, and a **monitor** that returns with final Assets or actionable recovery. A user should not need to learn node names, Task IDs, webhooks, or Agent internals.

## 2. Product vocabulary and Workflow lifecycle

| Term | Definition and ownership |
|---|---|
| **Project** | Durable named scope. It has zero or one current Workflow and many associated Runs and Assets. |
| **Workspace** | Ephemeral application context for the open Project: optional Workflow head, selection, viewport/drafts, Project conversation, and Run projections. It has no table or ID and is not creative state. User-facing copy says Project. |
| **Workflow** | Portable, versioned DAG containing capability refs, normalized params, bindings, and node positions. It is the Project's sole editable creative state. It contains no conversation, schema documents, Run/Task state, or Agent plan. |
| **WorkflowHead** | Storage envelope `{ project_id, revision, workflow }`. Revision exists only for CAS and undo; it is excluded from exports, Asset receipts, cache identity, and execution identity. |
| **Asset** | Immutable managed media plus provenance. An Asset's Workflow snapshot is an execution receipt, not another editable Workflow; Reveal Workflow is read-only unless the user explicitly restores a copy. |
| **Run** | Durable execution attempt for one authorized Workflow fingerprint. It is created at accepted dispatch before provider work; later Workflow edits never mutate it. |
| **Task** | Operational child step of a Run for provider or assembler work. Users see friendly stages; Task/provider IDs stay under Diagnostics. |

A new Project has `workflow_head: null`. The first successful creative mutation creates revision 1 atomically: adding or dropping a node/Asset, choosing Use as Input, or an assistant's first non-empty patch. Project creation/open, Asset browsing/selection, settings, schema loading, explanations, monitoring, and other read-only turns create nothing.

There is no `workflow_create` tool or synthetic empty graph. After creation, removing every node leaves an existing empty Workflow at a newer revision. Absent, empty, invalid, and runnable are distinct states; Run is disabled for the first three. An unrelated goal in a non-empty Workflow requires a user choice to edit/replace it or create another Project, never a hidden second Workflow.

**Co-authoring** is any user/model collaboration that reversibly edits this Workflow. **Reference creation** is co-authoring informed by inspected Assets, not a second state or schema. **Execution** prepares an immutable fingerprint from the same Workflow and creates a Run after approval; it never turns Run or Task state back into creative state.

## 3. Operating model and authority

- V1 uses one user-facing OpenAI Agents SDK `Agent`, without handoffs, agents-as-tools, or subagents. `Runner.run_streamed` owns the model/tool loop; product code adds no ReAct loop or Agent workflow engine.
- The model owns creative interpretation, prompts, capability choice, graph shape, and repair proposals. Rust owns contracts, validation, persistence, policy, dispatch, Tasks, Assets, idempotency, providers, and events.
- Reversible local edits need no approval. Paid, destructive, data-egress, and other external effects require approval for the exact prepared action.
- Product code owns chat, cron, webhook, task, and domain-event activation. LangSmith is not a dependency. A PyInstaller Tauri sidecar requires no user-installed Python, Codex, or service.

```text
user goal -> workspace_get_snapshot -> model reasoning
          -> capability_search -> capability_describe(selected refs)
          -> workflow_apply_patch -> Rust WorkflowHead -> React projection
```

```text
author -> reversible Workflow edit -> prepare -> waiting_approval -> dispatch
       -> original SDK run ends -> waiting_event -> new same-session invocation
       -> final Asset or recovery
```

`waiting_approval` resumes the serialized SDK `RunState`, original Agent, and session. `waiting_event` keeps no SDK run open; a product event starts a new invocation in the same Project-scoped session.

| State | Authority |
|---|---|
| Optional Workflow head, patch journal, revisions | Rust Workflow service |
| Conversation and pending SDK approval | File-backed SDK session and serialized `RunState` |
| Runs, Tasks, Assets, dispatch ledger, continuations | Rust product services |
| Selection, viewport, field drafts, optimistic overlay | React transient state |

### 3.1 SDK-first and boundary rules

1. **Use Agents SDK primitives first.** Use its public `Agent`, `Runner`, `FunctionTool`, Sessions, `RunState`, `needs_approval`, streaming events, MCP clients, and `ModelProvider` extension points. Product code must not recreate the model/tool loop, session history, approval suspension/resume, tool parsing, MCP transport, or streaming lifecycle. A custom replacement requires a demonstrated SDK gap, a code-review justification, and one isolated adapter with contract tests.
2. **Depend on contracts at real boundaries.** Rust orchestration depends on narrow traits for Workflow, Asset, Run, Event, and provider boundaries; Python uses SDK public interfaces and a `Protocol` only where the product genuinely needs substitution. The composition root selects concrete implementations. Domain code never imports SDK/provider internals, but a pass-through interface that merely renames an SDK method is also forbidden.
3. **Preserve canonical fields.** One concept has one wire name and serialized shape across Rust DTO, generated JSON Schema, Python tool arguments, NDJSON, and TypeScript: the existing Workflow names `type`, `params`, `inputs`, and `position`, plus additive `contract_version`, remain unchanged instead of becoming `node_type`, `capability_name`, `config`, `data`, or `payload`. A language-level identifier such as Rust `type_id` with `#[serde(rename = "type")]` stays on the owning type and is not a reason to add a mapping DTO. SDK-owned state keeps SDK fields and is stored opaque inside the versioned envelope rather than reconstructed as local models.
4. **Translate once per real ownership boundary.** Named adapters may translate wire DTO to/from application/domain command, storage row to/from domain type, domain provider request/result to/from provider payload/event, SDK stream/approval item to product IPC event, legacy Workflow to current Workflow, and Workflow to the one-way React Flow projection. Reuse the same type directly when no semantic distinction exists. Capability params normalize once inside their registration. Every adapter has a round-trip or contract test; chains of same-shape DTOs and repeated serialize/parse/normalize cycles are forbidden.

Generated operation schemas are passed directly to SDK `FunctionTool`. Python models only transport envelopes: `ToolRequest { operation_id, call_id, arguments_json }` and `ToolResponse { call_id, output_json }`. The handler copies the SDK's original argument JSON string byte-for-byte into `arguments_json`; Rust derives a separate trusted `RequestContext { project_id, session_id, request_id, selected_node_ids, selected_asset_ids, tool_version, approved_effect? }`, parses only the inner JSON into the canonical input DTO, and passes context separately. In M3, `workspace_get_snapshot` has an empty model input object; its Project selection comes only from that trusted context. Rust serializes canonical output JSON once; Python returns the valid `output_json` string to `FunctionTool` unchanged.

Argument-sensitive effects use a Rust-prepared immutable proposal followed by a separate execution `FunctionTool` with static `needs_approval=True`. Its model-visible arguments contain only `proposal_id`. Approving the SDK interruption creates a one-use Rust authorization bound to the SDK `call_id` and proposal fingerprint; Rust consumes it through `RequestContext`, never model input. Product Python never uses a dynamic approval callback to parse arguments or duplicate policy. Tests and mocks implement the same boundary interfaces instead of adding production branches.

## 4. Unified contract layer

There is one authored contract system, not separate co-author, reference, execution, Python, and React schemas. Rust owns the semantics: engine/domain types, application boundary DTOs, and implementation-owned registrations are canonical in their layers; named translators isolate them. JSON Schema is a generated boundary representation. CI emits and validates TypeScript types for core and capability/status envelopes. Python builds SDK `FunctionTool`s from operation registrations. During migration, the existing fixture gate remains until generated types replace handwritten mirrors.

Contracts use three document families, physically split so no consumer loads a mega-schema:

| Family | Documents | Consumers and loading |
|---|---|---|
| **Core wire contracts** | `core/project`, `core/workflow`, `core/execution`: Project/open result, WorkflowHead/Patch/Error, AssetRef, Proposal/Authorization/Run/Task/Event/results | Rust application boundary; React loads Project/Workflow documents on open and execution documents with a Run; Agent relies on fixed tool schemas rather than copying all core JSON into the prompt. |
| **Capability contracts** | One immutable `CapabilityContract` per exact `{id, version}` | Rust validator/executor; React loads summaries plus contracts for visible/selected nodes; Agent uses bounded search then describes only selected refs. |
| **Operation registrations** | ID/version/description/effect plus references to canonical input/output DTO types and a handler for workspace, Asset, media inspection, patch, prepare/dispatch, Run/Task reads, and optional UI actions | Generated schemas bundle the referenced DTOs; no input/output schema is authored again. Sidecar bootstrap creates a small stable SDK tool surface; React binds optional executors such as `ui_open_asset` by the same operation ID. |

Co-authoring, reference creation, execution, and monitoring are loading policies over these same contracts, not schema families or persisted modes. Co-authoring selects core Workflow plus described capabilities; reference creation also uses AssetRef and `media_inspect`; execution/monitoring selects core execution.

Core wire DTOs are not database rows or engine types. Persistence models remain private behind named translators; only deliberately portable/versioned documents such as Workflow and immutable receipts are stored as documents. A wire change therefore does not imply a storage migration.

```text
CapabilityRegistration
  contract() -> immutable CapabilityContract
  validate_and_normalize(raw_params) -> NormalizedParams
  instantiate(NormalizedParams) -> ExecutableNode
```

Each registration owns one typed params definition, its normalizer, executor, and presentation metadata. The derived immutable `CapabilityContract` contains only execution semantics: exact ID/version, ports/cardinality, params JSON Schema, normalized default params, and effects. Derived defaults must normalize and validate. JSON Schema `default` remains annotation only; it never defines behavior. A change to validation, normalization, ports, effects, or externally observable execution semantics requires a new contract version. `$id` identifies the nested params schema and `$ref` stays inside its local `$defs`.

Labels, search terms, examples, UI hints, and display attribution form a non-authoritative `CapabilityPresentation` projection from that same registration. It is keyed by ref, may change under its own ETag, and is never persisted or fingerprinted. Policy-relevant provenance remains in versioned effects/policy data. An application-owned `CapabilityStatusService` separately supplies live availability, reason, provider health, and `status_revision`. Effects stay in the versioned contract because they govern approval. The immutable contract ETag is transport integrity only: caches are keyed by exact ref, and a different contract digest for the same ref is an invariant violation rather than another identity.

The registry is keyed by exact `CapabilityRef { id, version }`, rejects duplicates, and separately marks a current version for new-node search. Workflow nodes persist both `type` and `contract_version`; resolution never falls forward to latest. Missing old versions reopen as preserved degraded nodes and block Run until explicit migration. Existing refs may be described directly; new refs must come from search.

`CapabilitySummary` is an ephemeral projection of presentation plus status, never another authored or persisted document. React uses paged server search for its palette and loads exact contract/presentation bundles lazily; Project-open batches are bounded by the enforced Workflow node/ref limit. The Agent has no list-all or pagination path: `capability_search(query, kinds)` requires a non-empty goal and returns fixed top-k refs/status; each `capability_describe(refs)` returns at most three exact bundles from that search or refs already in the current Workflow. Multiple describe calls are allowed under an invocation budget of eight distinct refs and 96 KiB of schema bodies.

## 5. Workflow mutation, validation, and React

The portable Workflow format version changes only for serialization migrations. Each node stores `id`, `type`, `contract_version`, normalized `params`, explicit inputs, and optional `position`. `AssetSource` stores only Asset ID/hash; target port names and tagged bindings express role and order.

The Agent-visible operation is:

```text
workflow.apply_patch(expected_revision: null | u64, operations[])
```

`null` means create only if absent. Rust supplies `RequestContext` separately, so the model cannot choose Project scope, tool version, or idempotency identity. Operations are a closed tagged enum: `add_node`, `replace_params`, `set_input`, `clear_input`, `remove_node`, and `set_position`. Unknown fields are denied except inside capability `params`; patch and stored Workflow reuse the same tagged `InputBinding` union: `single { source }` or `ordered_many { sources }`.

```text
NodeRef = { kind: "id", id } | { kind: "alias", alias }
add_node       { op, alias, capability: CapabilityRef, params, position? }
replace_params { op, node: NodeRef, params }
set_input      { op, node: NodeRef, input, binding: InputBinding<NodeRef> }
clear_input    { op, node: NodeRef, input }
remove_node    { op, node: NodeRef }
set_position   { op, node: NodeRef, position }
```

Aliases are unique within one patch, may be used only by later operations, and never persist. `replace_params` replaces the complete params object rather than merging missing/null fields. Operations apply in order; a failure rolls back all of them and identifies its index. `remove_node` also removes incident bindings and reports newly blocked nodes. V1 rejects more than 128 operations or 512 KiB before model-controlled data reaches the engine.

`workflow_apply_patch` is the one SDK `FunctionTool` with `strict_json_schema=False` because capability params are open-ended. Rust still strictly decodes the operation envelope and validates params against the exact registration. Other fixed tools use closed strict schemas with nullable optional values.

In one SQLite transaction Rust checks the dedupe receipt before CAS, verifies the request hash, applies ordered operations to a copy, normalizes, validates persistability, persists revision + 1 plus one undo journal record, and returns the canonical `WorkflowHead`, alias resolutions, and current readiness blockers. A normalized no-op keeps the revision. Errors include stable code, JSON Pointer, operation index, constraint details, and current revision.

`EngineValidator` remains pure and returns one `ValidationReport`: malformed format/params, duplicate IDs, invalid existing bindings, incompatible types, and cycles are persistence errors; missing required inputs and minimum cardinality are readiness blockers. This lets users add an unconnected node or disconnect an input while Run remains disabled. Patch rejects only persistence errors; newly introduced refs also pass a narrow status/admission check, while provider downtime never blocks position, removal, or repair of existing nodes. Prepare and run compose the same report with Asset ID/hash resolution, live status, and policy blockers through `WorkflowValidationService`. Legacy migrations map omitted fields and aliases through frozen legacy contracts, never current defaults.

Ordered inputs use that core tagged binding, for example `clips: { kind: "ordered_many", sources: [{ node_id: "shot-01", output: "video" }, { node_id: "shot-02", output: "video" }] }`. Order is never inferred from topology, IDs, positions, or duplicate params. `VideoConcat.clips` needs at least two videos to run and emits one video.

React uses a discriminated workspace state: `booting | no_project | opening | ready(Project, WorkflowHead | null) | blocked`. It never falls back to project `"default"`. `OpenProjectResult` hydrates Project plus optional head; schema resolution/migration completes before editing is enabled.

`no_project` offers Create/Open Project while global Assets and settings remain available; `opening` preserves the prior surface read-only under a non-destructive busy state; `ready(..., null)` shows Describe a result, Add first step, and Use an Asset without persisting until action; `blocked` preserves readable nodes and offers Retry/Diagnostics. Every disabled Run control gives a plain-language reason. Keyboard operation, focus restoration, labeled errors, and announced async state are acceptance requirements, not later polish.

```text
Rust WorkflowHead -> WorkspaceController -> schema-backed Workflow projection
                  -> React Flow nodes/edges + Inspector + palette
UI gesture -> transient typed draft/optimistic patch -> serialized patch queue
           -> Rust canonical head -> replace projection or show CAS conflict
```

React Flow nodes/edges are revision-tagged projections, not a serializable authority. Local drafts are allowed for responsiveness, but every acknowledgement replaces them with Rust's normalized head. A `WorkspaceController` write barrier flushes and awaits the serialized patch queue before sending an assistant message, preparing a Run, undo/redo, Project switching, or normal window/application close; conflict or validation failure aborts the requested action. Failed close offers Keep Editing or Discard and Close. Assistant edits appear only after one atomic patch, preserve valid selection/viewport, briefly identify changes, and fit view only on first creation.

The palette loads paged derived summaries and live status. Opening a Project batch-loads exact bundles for bounded distinct refs already present; selection/addition loads one into a ref-keyed cache with separate contract/presentation ETag revalidation. Unknown contracts render stable recovery placeholders and are never dropped. A schema-driven form renderer combines semantic JSON Schema with presentation annotations keyed by JSON Pointer, parses numeric/enum/Asset values before patching, and removes handwritten TypeScript capability definitions and the duplicate connection validator.

Project switching flushes drafts, ignores stale responses, and atomically swaps Project/head/contracts; failure keeps the previous Project active. Conversation, Run, and progress projections are keyed by Project/run. Until durable correlated Runs land, the legacy uncorrelated runner blocks switching during execution.

The assistant composer sends only Project ID, Workflow revision/presence, and selected IDs. It never injects a full React canvas snapshot; the Agent calls `workspace_get_snapshot` for authoritative content.

## 6. Agent schema and context policy

The system prompt contains only product vocabulary, safety/effect rules, the discovery protocol, and a compact type/cardinality vocabulary generated from canonical Rust enums. Installed capability IDs, JSON schemas, presentation metadata, templates, and provider tables are never placed there. Fixed tool schemas are already sent by the SDK and are not duplicated in prose.

| Moment | Schema/context read by the one Agent |
|---|---|
| Every invocation | Small fixed operation tool schemas and vocabulary/rules only. |
| Workspace-dependent turn | Read one authoritative snapshot through `workspace_get_snapshot`; do not inject a React canvas copy. |
| Read-only answer or monitoring | No capability schema. Use workspace, Run, Task, and Asset reads. |
| Add/replace capability | Search by desired transformation, then describe the best one to three exact refs. |
| Edit existing node | Describe its persisted exact ref directly before changing params or ports. |
| Reference creation | Read local metadata with fixed `media_inspect`; external inspection uses prepare/proposal/approved-dispatch, then describe only capabilities selected for the resulting Workflow. |
| Execute | Call prepare; Rust validates all current contracts/status. Do not reload node schemas. |
| Failure repair | Describe only the failed capability and adjacent alternatives if a patch will be proposed. |

Each invocation owns a transient `SchemaSet` keyed by exact `CapabilityRef`. Adding a node requires search then describe; mutating existing params or bindings requires describing that persisted ref; position and removal need no capability schema. A ref that becomes unavailable before new-node admission returns `CAPABILITY_STALE` and requires re-description, while existing refs can still be repaired or removed. A prepared Run records all exact refs and its immutable contract receipt, including pre-existing nodes; the ETag is audit/cache metadata rather than execution identity.

Discovery starts from the desired output, not a known node name. After describe reveals required inputs, the model may search again for an unresolved input transformation until the graph is connected or user intent is genuinely missing. For example, a text-to-video goal may discover image-to-video first, then search for a text-to-image producer. Product code never supplies this creative chain.

SDK sessions persist tool calls/results, so `RunConfig.session_input_callback` atomically replaces prior `capability_describe` call/result pairs with exact-ref receipts; it never leaves an orphaned tool call. A final `call_model_input_filter` enforces the context budget while preserving the current invocation's `SchemaSet`. Future turns receive exact refs, not old schema bodies.

SDK hosted tool search is not used for Workflow capabilities: it requires pre-registered tools, is Responses-only, and the standard Runner does not execute client-dynamic search. It may later defer a large fixed integration-tool surface without changing Workflow authoring.

`max_turns` limits model invocations; product limits separately bound discovery calls, described contracts, tool calls, payloads, and wall time. Drain streamed results before reading interruptions/output. OpenAI models use Responses; other providers require an explicit Responses, Chat Completions, or custom `ModelProvider` adapter plus compatibility probes.

## 7. Effects, approval, Runs, and events

| Effect | Approval |
|---|---|
| Local read; visible undoable Workflow patch; Run proposal | None |
| Prepared generation/render Run, including disclosed egress | Once for the exact complete Run |
| Outside-Run data egress, MCP write, provider inspection, destructive action | Exact call |

Both UI and assistant use the same domain operations: `workflow.prepare_run(expected_revision) -> RunProposal` and `workflow.dispatch_run(proposal_id, RunAuthorization) -> run_id`. The Agent-visible dispatch tool exposes only `proposal_id`; the trusted host supplies `RunAuthorization` after the exact SDK interruption is approved. The write barrier runs first, and prepare verifies the displayed head revision. `RunProposal` includes `expires_at`, pricing revision, currency, and quoted range/maximum or an explicit unknown-cost marker. Trusted UI shows output, providers/models, source Assets, full media versus samples, purpose, count, cost/range/unknown, limits, and warnings. Buttons say **Start generation** and **Not now**; IDs/fingerprint stay under Details. Only an execution-fingerprint change stales the creative proposal and prepares an updated review; position-only revisions remain valid.

One `RunAuthorization` covers every disclosed and fingerprinted provider egress, Task, assembler step, and final Asset write in that Run; undisclosed bytes never leave. The dispatch `FunctionTool` has static `needs_approval=True`. After draining the stream, persist an envelope containing SDK/Agent/tool/contract/policy versions plus `result.to_state().to_json(strict_context=True)`; restore with `await RunState.from_json(..., context_override=..., strict_context=True)`, then resume the original Agent and same file-backed session. Rust rechecks policy and effect identity. Sticky approval is not exposed.

The execution fingerprint includes normalized semantic Workflow content, exact capability refs, ordered bindings, providers/models, immutable input hashes, and options. It excludes ETags, CAS revision, position, and mutable pricing. `RunAuthorization` binds the proposal, currency, expiry, and approved maximum, or explicitly accepts an unknown estimate under existing user/product spend limits. Dispatch reprices and performs no provider call if the proposal expired, the new quote exceeds that authorization, or required limits are absent; it returns an updated proposal for review. Selected capability status and policy are also rechecked at dispatch.

Record the dispatch ID before external work. Duplicate delivery returns the same Run; an unreconciled post-request crash becomes `outcome_unknown`, never a blind replay. When Agent dispatch returns `run_id`, product code atomically records a continuation `{ product_run_id, session_id, terminal_status, attention_cursor, expires_at }`; Tasks correlate through the Run.

Terminal/attention events are `workflow.run_completed`, `workflow.run_failed`, `workflow.run_needs_attention`, and `workflow.run_cancelled`. Progress streams directly to React and never wakes the model. An attention event advances a deduplicated cursor and starts one bounded same-session invocation without consuming terminal delivery. A terminal event consumes `terminal_status` exactly once. If the continuation expires, the persistent Run and product notification remain available, but the UI states that automatic assistant follow-up expired and offers Open Run; no stale model invocation is replayed.

V1 permits at most one nonterminal Run per Project. A second prepare/dispatch returns `RUN_ACTIVE` and opens that Run; different Projects may run concurrently because every projection and event is keyed by Project/run. The persistent Run projection shows a plain-language stage, reliable completed count or indeterminate progress, elapsed time, Cancel, and Needs attention across Project switches/relaunch. Confirmed idempotent cancellation sets `stopping`, blocks new dispatch for that Project, asks providers to stop, states possible cost/partial outputs, and becomes `cancelled` after settling; unknown provider outcome blocks further dispatch.

`ProductRunResult.final_assets[]` comes from terminal media outputs. The Asset library selects final results; auto-open only while the user is waiting in that Run context, otherwise show a durable non-modal thumbnail. Actions are View, Export, Reveal Workflow, and Use as Input. Intermediate Assets are never called final.

`RunFailure` stores code, failed node/Task IDs, safe details, retry disposition, partial Assets, and reconciliation state. Users see a friendly failed-step label and recommended action; IDs stay in Diagnostics. Creative/provider changes wait for acknowledgment, unknown outcomes block retry, and nothing auto-reruns.

Exact-schema `media_inspect` reads existing workspace metadata without approval and never sends content externally. External analysis uses `media.prepare_inspection -> InspectionProposal`, which discloses Assets, full media versus samples/transcript, provider, and purpose, followed by `media.dispatch_inspection(proposal_id)` with static SDK `needs_approval=True`. Refusal sends nothing, continues locally, and suppresses the same proposal for that invocation. Sharing never authorizes generation; Rust rechecks the proposal before execution.

Provider adapters configure webhooks once; Workflows store no URLs, sessions, callbacks, or subscriptions. Desktop adapters poll when callbacks are unreachable. Authenticate, validate, deduplicate, correlate, and normalize raw provider events before internal delivery. Events never authorize effects.

## 8. MCP, transport, and security

MCP is an optional SDK integration, not the Workflow schema. V1 allows selected trusted local stdio or Streamable HTTP servers; hosted MCP is deferred as a separate provider boundary. Filters are not sandboxes and schema conversion is best-effort. Untrusted servers require OS-enforced filesystem/process/network limits; argument-sensitive effects use a Rust proposal/approved-dispatch operation, while an exposed MCP write uses SDK `require_approval="always"`.

The sidecar uses framed NDJSON over inherited stdin/stdout and opens no listener. Stdout is protocol-only; invalid messages fail closed. A file-backed SDK session owns history, while the sidecar application layer owns the atomic UI snapshot/event cursor. Rust relays events but stores no second transcript; gaps force a fresh snapshot without replaying effects.

Freeze Python dependencies with PyInstaller, disable SDK tracing and LangSmith, exclude secrets from prompts/state/logs, and validate model, provider, MCP, event, schema, and media content at their boundaries.

## 9. Boundaries and not doing in V1

The media Workflow is the user-visible execution graph, not an Agent workflow. Multiple agents add conflicting context and approval routing; reconsider only with measured need for permission isolation, context quarantine, or parallel independent work. One assistant must remain user-facing and use the same product contracts.

- No full capability inventory prompt, scenario-specific schema copies, generated capability tool union, Rust shot planner, or Workflow macro.
- No persisted VideoPlan, Creative IR, assistant draft graph, storyboard/timeline/shot state, or `VideoSequence` value.
- No handoffs/subagents, plan/todo state, LLM grading/evals, hill climbing, long-term memory, or autonomous prompt mutation/retry.
- No raw host tools, hosted MCP, untrusted unsandboxed MCP, or broad integration surface before the core journey is proven.

## 10. Delivery plan

| Milestone | Complete vertical outcome |
|---|---|
| **M0 SDK/schema proof** | Pin current stable SDK; characterize legacy defaults/aliases; prove Rust DTO -> JSON Schema -> direct FunctionTool/TS fixture path with opaque argument/output JSON and no field remapping, non-strict patch, Rust proposal plus static SDK approval/resume, Sessions/input filters, streaming, MCP interface, framed stdio, and packaging. |
| **M1 Versioned contracts** | Add exact CapabilityRefs, immutable contract/live status split, duplicate rejection, frozen legacy migration, registry-driven pure validation, and core wire contracts. |
| **M2 Workflow authority** | Add optional WorkflowHead, lazy first-patch creation, CAS/dedupe/undo transactions, structured errors, stored-head execution, Project hydration/switching, and remove `"default"` fallback. |
| **M3 React/paragraph co-author** | Add contract cache/status, dynamic palette/forms/ports, serialized patch queue and projection, bounded Agent discovery, paragraph-to-canvas, then remove handwritten capability mirrors, raw save, and node-by-node mutation. |
| **M4 Reference co-author** | Bind Asset sources by ID/hash/input role/order, disclose model sharing, inspect, reopen, and author the same Workflow without plan state. |
| **M5 Operator** | Add provider execution, proposals, one UI/SDK run approval, fingerprints, dispatch ledger, cancellation, reconciliation, and final Assets. |
| **M6 Monitor** | Add Run/Task reads, continuations/events, persistent progress, restart recovery, final-Asset actions, and acknowledged failure repair. |
| **M7 Integrations/hardening** | Add selected trusted local MCP/prompt-only skills, then exercise crash boundaries, hostile inputs, provider compatibility, no-egress, packaging, performance, and full E2E. |

Delivery is vertical and blocked by two product gates:

1. **Co-author gate (M3 exit):** the fixture "Create a 12-second, three-shot video: sunrise over a city, a cyclist crossing a bridge, and coffee steaming by a window" enters a real SDK Agent loop. It must discover the current exact refs for `TextPrompt`, `TextToImage`, `ImageToVideo`, and `VideoConcat`; create three connected prompt-to-image-to-video branches whose normalized durations total 12 seconds, feeding one ordered concat with one terminal video and zero readiness blockers in one atomic patch; and render that graph on the canvas. A human rubric confirms that the three branches respectively represent the requested sunrise/city, cyclist/bridge, and coffee/window scenes, while prompt wording and visual treatment remain model-owned. The user then changes the second shot prompt through React, receives a new Rust revision, and reopens the same persisted edit. A mocked model plan, unrelated prompt, disconnected graph, or direct UI node insertion does not pass.
2. **Operator gate (M6 exit):** that same Workflow prepares an exact proposal, receives one informed approval, dispatches through at least one real provider, and returns a final Asset. While it is nonterminal, the tester switches Project or relaunches the app, then returns and verifies that the same persisted Run projection and progress resume without redispatch. The same path must also surface one injected provider failure with actionable recovery and no duplicate effect.

M4 cannot begin before the co-author gate passes; M7 cannot begin before the operator gate passes. Horizontal completion of schemas, services, or UI components alone does not advance a gate.

Both gates are staging/release evidence, not flaky per-PR tests. Deterministic CI keeps the mock backend as its contract; the operator success gate records a controlled real-provider smoke run, while its failure path uses the deterministic fault adapter.

## 11. Required acceptance scenarios

1. Project/open/read-only activity creates no Workflow; first UI or Agent patch creates revision 1, concurrent first patches conflict, and a later empty Workflow remains present.
2. Project switching hydrates the correct optional head only after exact contracts resolve, ignores stale responses, preserves degraded nodes, and leaks no conversation/Run/progress state across Projects.
3. One typed capability change derives Rust validation/defaults, Agent description, React controls/ports, and TypeScript boundary output without another authored schema; semantic changes require a new exact ref, while live status does not rewrite it.
4. The Agent sees no full inventory, each describe returns at most three refs, and one invocation stays within eight refs/96 KiB while supporting the required four-capability workflow. It cannot use undescribed/stale refs, and future model calls receive receipts rather than old schema bodies.
5. A paragraph creates three shot branches plus ordered `VideoConcat` in one patch/undo unit. Incomplete manual nodes/bindings persist with readiness blockers; malformed params, invalid existing bindings/types/cycles, or stale revision writes nothing and returns indexed diagnostics.
6. Reference sources reopen with the same ID/hash and input role/order; missing content offers Relink/Remove; sharing sends only disclosed data, while refusal sends none and does not reprompt.
7. React numeric/enum/Asset controls commit typed normalized params; unavailable versions render preserved placeholders; schema loading never creates or revises a Workflow. A keyboard-only first-time user can create a Project, understand the empty canvas, recover focus/errors, and reach an editable Workflow.
8. Editing and immediately sending a message, preparing a Run, or closing waits for the write barrier; a failed flush causes no Agent call/dispatch and close requires an explicit discard. Relaunch shows the acknowledged edit. UI and SDK prepare/dispatch the same proposal; one review discloses and authorizes every Run egress and Task, while position-only edits stay valid. Expired or over-limit repricing causes no provider call and requires a new review.
9. Duplicate patch/dispatch delivery changes state once. Lost-response retry returns its receipt; crash windows reconcile or become `outcome_unknown` without replay.
10. Progress uses no Agent turns. One Project rejects a second nonterminal Run while another Project may continue independently. `needs_attention -> reconcile -> completed` causes one deduplicated attention invocation and one terminal invocation. Cancel survives relaunch, blocks new work for its Project, reports limits/partial output, and emits one terminal event even under duplicate webhooks.
11. Completion exposes View/Export/Reveal Workflow/Use as Input; failure applies no repair before acknowledgment, unknown outcomes block retry, and reruns need approval.
12. The frozen sidecar needs no Python/Codex or listener, exports no tracing/LangSmith traffic, and passes the full repository E2E gate.
13. Static architecture checks reject replacement loop/session/approval/stream/MCP implementations, concrete SDK/provider dependencies in engine/domain crates, same-shape DTO hops, and redundant serialize/parse chains. Transport tests separately prove the decoded `arguments_json` inner string is byte-for-byte the SDK input, trusted `RequestContext` never enters model arguments, Rust emits valid canonical `output_json` once, Python returns that inner string unchanged, and generated Rust/TypeScript contracts retain the same wire field names.

## 12. Risks and official sources

- Contract generation and the renderable JSON Schema subset must be proven before deleting current mirrors. Frozen migration fixtures must capture today's conflicting UI/Rust defaults and aliases.
- The non-strict patch params envelope favors extensibility; measure repair rates before considering deferred tools. Real video assembly still needs explicit codecs, licensing, progress, and atomic-output choices.
- Current stable SDK is `openai-agents==0.18.1`; implementation rechecks, pins, and excludes prereleases.

Official references:

- SDK Runner and tools: https://openai.github.io/openai-agents-python/running_agents/ and https://openai.github.io/openai-agents-python/tools/
- Session input shaping: https://openai.github.io/openai-agents-python/sessions/#control-how-history-and-new-input-merge and https://openai.github.io/openai-agents-python/running_agents/#call-model-input-filter
- Strict schemas and approval: https://developers.openai.com/api/docs/guides/function-calling#strict-mode and https://openai.github.io/openai-agents-python/human_in_the_loop/
- Models, MCP, tracing, and package: https://openai.github.io/openai-agents-python/models/ and https://openai.github.io/openai-agents-python/mcp/ and https://openai.github.io/openai-agents-python/config/#tracing and https://pypi.org/project/openai-agents/
