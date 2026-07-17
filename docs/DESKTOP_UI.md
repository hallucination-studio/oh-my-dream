# Desktop Creation Workspace Specification

## Status

Living document. The canvas workspace, Node Library, graph editing, Run lifecycle, Asset
Library, and Assistant dock are implemented in `ui/`. The Work Drawer with its Run timeline,
engine-owned readiness presentation, keyboard connection, magnetic ports, deterministic mock
Asset production, Settings content, and the dark visual direction are specified here and still
pending. This document defines presentation and interaction only. Backend semantics remain
owned by the documents mapped from [`BACKEND.md`](BACKEND.md).

### Implementation snapshot (2026-07)

| Area | State | Gap to this specification |
| --- | --- | --- |
| Workspace shell (rail, panels, canvas, top bar) | Implemented | Panels are inline and resize the canvas; the overlay Work Drawer is pending. Rail is 44 px, target 56 px. |
| Node Library | Implemented | Groups follow backend categories; creator-language grouping (Inputs, Generate, Assets) is pending. |
| Graph editing by pointer | Implemented | Pending-edge semantics, magnetic ports, compatible-target highlight, and keyboard connection are pending. |
| Node presentation | Implemented | One contract-driven node component; creator-language labels, stale treatment, and instructional empty states are pending. |
| Generation model selection | Implemented | The label still reads `Generation profile`; single-model default and the no-model empty state are pending. |
| Run controls and monitoring | Implemented | Run all, Run to here, cancel, and per-node progress work; the Run tab step timeline and admission-open behavior are pending. |
| Asset Library | Implemented | Grid browsing, kind filter, search, import, detail, and drag-to-canvas work; list mode, dimensions/duration, and Export are pending. |
| Assistant dock | Implemented | Streaming, tool activity, and approval cards work; availability gating is a stub. |
| Settings | Shell only | The dialog and navigation exist; every section is an empty stub. |
| Deterministic browser mock | Partial | Runs, cancellation, and failure injection work; typed Asset production, previews, canonical readiness, and model listing are pending. |
| Visual direction | Pending | The implemented theme is light; the dark workbench palette below is not applied yet. |

## Objective

Build a desktop-only creation workspace in which a creator can understand, connect, configure, run,
and inspect a Workflow built from the exact seven active Node Capabilities without learning backend
terminology. The first acceptance path is:

1. enter a text prompt;
2. connect Text to Text to Image;
3. connect the generated Image to Image to Video;
4. choose understandable generation settings;
5. run through Image to Video;
6. observe the Workflow Run and every Node Execution;
7. inspect the generated image and video after their Assets become available.

Success means the path is understandable and testable in both the deterministic desktop backend and
the browser mock. A mock success must produce typed mock Assets and previews; `Done` with zero outputs
is not success.

## Scope

### In scope

- Desktop viewports from 1280x720 upward, with 1440x900 as the reference canvas.
- Project selection, node library, graph editing, node configuration, Run monitoring, Asset
  inspection, Assistant co-authoring, and Settings using the existing command surface: Project,
  Capability, Workflow, Run, Asset, and Assistant commands, plus the provider and assistant
  configuration commands already exposed by the desktop backend.
- The exact seven Node Capabilities and the existing whole/through-node Run scopes.
- Empty, editing, blocked, queued, running, succeeded, failed, cancelled, and stale
  presentation states mechanically derived from existing DTOs.
- Keyboard-operable node selection, connection, deletion, and Run controls.
- A deterministic mock Text-to-Image -> Image-to-Video path with typed Asset read-back.
- Asset Library view, Assistant dock, and Settings dialog presentation as specified below.

### Out of scope

- Mobile or touch-first layouts.
- Multiple Workflows, Workflow history, retry-in-place, provider task/resume, cost accounting, or
  provider-native progress.
- New backend commands, DTO fields, business states, or compatibility rules.
- Skills management and Developer mode in Settings: the reference mockup specifies them, but no
  backend command exists. Do not ship UI that implies these capabilities work.

## Product Language

UI copy names what the creator controls, never the internal boundary that carries it.

| Backend term | Desktop label | Explanation shown to the creator |
| --- | --- | --- |
| Generation Profile | Generation model | The configured model used for this node. |
| Capability | Node type | What this step does. |
| Workflow Run | Run | One execution of the current workflow revision. |
| Node Execution | Step | The execution state of one node in the Run. |
| ThroughNode | Run to here | Run this node and every dependency it needs. |
| WholeWorkflow | Run all | Run the complete workflow. |
| Asset | Asset | A saved image, video, or audio result in this Project. |
| Readiness | Ready to run | Whether all required connections, settings, models, and Assets are valid. |
| Production Plan | Plan | The assistant's working outline for the current request. |
| Assistant Workflow Change | Proposed change | The exact workflow edit the assistant asks the creator to approve. |
| Approval decision | Review change | Approving applies the proposed change; rejecting discards it. |

Raw identifiers, enum keys, error debug strings, `generation_profile_ref`, tool identifiers, and
provider route names must not appear as primary UI text. Technical identifiers may appear only in a
copyable diagnostics section after a failure.

## Information Architecture

The workspace uses one canvas-first desktop shell. The graph remains full-bleed beneath workspace
chrome and does not collapse into a mobile layout.

```text
+--------------------------------------------------------------------------------------+
| Project / saved state       Run summary                              [Run all] [more] |
+------+----------------------------------------------------------------+--------------+
| rail | NODE LIBRARY          WORKFLOW CANVAS                          | WORK DRAWER  |
|      | Search                                                         | Configure |  |
|      | Text                [Text] -> [Generate image]                  | Run          |
|      | Generate image                       |                         |              |
|      | Create video                         v                         | selected     |
|      | Assets                        [Create video]                    | node/run     |
|      |                     [zoom] [fit] [minimap]                      | details      |
+------+----------------------------------------------------------------+--------------+
```

- Rail: 56 px; switches the left panel between Nodes and Assets, toggles the Assistant dock, and
  opens Settings as a modal dialog, without navigating away from the workspace.
- Node Library: 304 px, optional and pinnable for the current UI session, grouped by creator
  language: Inputs, Generate, and Assets.
- Canvas: full available workspace area. Overlay panels cover it visually but do not resize it or
  mutate graph coordinates, so opening a drawer never moves nodes or bends saved edges.
- Work Drawer: 380 px overlay with `Configure` and `Run` tabs. `Configure` owns selected-node fields,
  readiness guidance, and outputs; `Run` owns the admitted step timeline and Run controls.
- Canvas controls: zoom, fit, and minimap stay against the lower-left visible canvas edge, offset
  past an open left panel, and remain reachable while either overlay panel is open.
- Top bar: Project context, saved state, a compact current/last Run summary, and the primary Run
  action. The primary action is always `Run all`; node-scoped `Run to here` stays in `Configure`.
  Closing the Work Drawer never hides the Run summary.
- Assistant dock: 320 px dock at the right workspace edge, toggled from the rail. Co-authoring stays
  visible beside the canvas; opening or closing it never changes graph layout or execution state.
- Settings: a modal dialog over the workspace. It is infrequent and must not compete with the
  canvas, so it is not a workspace view.

The page must never render a node beneath another node. New nodes use a deterministic staggered
placement and the canvas fits the new selection into view.

## DVStudio Reference Adaptation

DVStudio is a visual-interaction reference, not a second source of product semantics. The following
patterns are adopted because they solve observed desktop usability problems:

- a full-bleed graph with overlay workspace chrome, keeping the creation path visually primary;
- separate visible ports and screen-space pointer hit layers, with magnetic target feedback;
- explicit empty image/video states and disabled media controls until a resource exists;
- node-local progress plus a larger Run detail surface for the whole execution;
- an Asset browser with thumbnails, grid/list modes, and drag-to-canvas creation.

The following DVStudio patterns are deliberately not adopted:

- provider task IDs, provider-native state synchronization, remote-task resume, or retry controls;
- frontend-owned connection compatibility rules rather than engine-owned contract findings;
- node type switching, copy, resize, or refresh actions absent from the current mutation surface;
- unused-Asset analysis or metadata that existing Asset DTOs cannot establish;
- glass blur, particles, animated glow, decorative corner brackets, or raw node identifiers.

## Visual Direction

The workspace should feel like a precise media workbench, not a dashboard or generic AI product.

- `Canvas Black` `#111418`: infinite canvas and the deepest workspace plane.
- `Graphite Panel` `#1B2026`: node bodies and overlay panels.
- `Steel Border` `#343C46`: structural dividers and idle controls.
- `Studio White` `#EBF0F3`: primary copy and high-emphasis icons.
- `Muted Steel` `#9AA6AE`: secondary copy and metadata.
- `Signal Teal` `#26A88F`: selection, focus, and primary actions only.
- `Image Amber` `#D9933D`, `Video Violet` `#806BE8`, and `Text Cyan` `#52B8C7`: typed ports and
  output identity, never status on their own.
- `Running Gold` `#D8AD4D`, `Failure Coral` `#D46355`, and `Success Green` `#55B48B`: execution
  states, always paired with labels or icons.
- Typography: the existing system UI stack for interface copy; a tabular monospace face only for
  elapsed time, progress, revision, and diagnostic IDs.

The signature element is the typed connection path: a connection carries the output media color
from the source port through the edge to the matching target port. Everything else remains quiet.
Surfaces use 4 px corners and one-pixel borders. No gradients, glass blur, particles, decorative
cards, oversized rounding, animated glow, or shadow stacks are introduced.

## Graph Editing

### Creating nodes

- Clicking a library item adds one node at a visible, non-overlapping position and selects it.
- Dragging remains available for spatial placement.
- The library uses `Text`, `Generate image`, `Create video`, `Create speech`, `Image asset`, `Video
  asset`, and `Audio asset` as visible labels, grouped in the stable order Inputs, Generate, Assets.
- Library search also matches creator-language aliases such as `prompt`, `t2i`, `clip`, and `voice`,
  never only the contract identifier.
- Input and Asset nodes are 304 px wide; generation nodes are 336 px wide. They do not scale down
  into compact or mobile variants.

### Connecting nodes

- Every port shows its name and media type; it is not an unlabeled dot.
- The visible port gem is 10 px. A separate zoom-invariant screen-space hit layer provides a 22 px
  radius pointer target without making the node look oversized.
- During a drag, a compatible target begins magnetic pull within 8 px, locks visually within 5 px,
  and commits when released anywhere inside its 22 px hit radius. These distances are screen-space
  values and do not shrink when the canvas is zoomed out.
- The canvas owns pointer capture for the complete drag. Panning is suspended until commit, cancel,
  or pointer loss, and every exit path clears the temporary edge and target highlights.
- Starting a connection highlights only compatible targets and dims invalid targets.
- Dropping on empty canvas or an incompatible target changes nothing and announces why.
- A selected output may also be connected by keyboard: `Enter`, arrow to a compatible target,
  `Enter` to confirm, `Escape` to cancel.
- A pending edge is visually distinct. It becomes committed only after the canonical Workflow
  mutation succeeds; a rejected mutation removes it and announces the authoritative reason.
- Connection errors use engine-owned compatibility findings and never duplicate type rules in React.

### Readiness

`Run all` follows whole-Workflow readiness; `Run to here` follows the selected node presentation's
scoped readiness. Both remain disabled until their engine-owned result is ready. The `Configure` tab
lists actionable issues in canvas order, for example:

- `Connect a Text output to Prompt.`
- `Choose a generation model.`
- `The selected Image asset is not available.`

## Node Presentation

Every node has three stable regions:

1. header: creator-facing node name and state;
2. body: only the two most important editable values;
3. result: output, progress, empty state, or failure.

Detailed parameters live in the `Configure` tab so text cannot collide with ports or previews. Node
labels use a 20 px minimum line height and reserve two lines before truncation. A media result region
uses a 16:9 frame at least 176 px high; it shows either one intentional empty state or a current
preview, never a broken media element.

### State mapping

| Source state | Node label | Result region |
| --- | --- | --- |
| no relevant execution | Not run | `Run this step to create an image/video.` |
| queued/pending | Waiting | dependency position and no preview |
| running | Running | named step plus determinate progress when supplied |
| succeeded with current output | Complete | typed output preview |
| succeeded with stale output | Changed since run | old preview with stale treatment; Run action remains available |
| failed/blocked | Needs attention | structured reason and the field/connection to fix |
| cancelled | Cancelled | no fabricated result |

An Image or Video preview is rendered only when the current node presentation contains a complete,
non-stale typed output and a preview URI. Before that, the same area is a compact instructional empty
state. A Video node never shows a play affordance without a video preview URI.

## Generation Model Selection

The `Configure` tab label is `Generation model`, not `Generation profile`.

Each option presents:

- display name, such as `Fast image model`;
- availability: `Ready` or the existing structured reason it cannot run.

Provider IDs, credential IDs, and secrets are never displayed. If exactly one available model exists,
it is selected by default and shown as a read-only row. If multiple available models exist, the
choice stays inline in `Configure`. With no available model, the control reads `No generation model
is available for this node type`, shows safe availability reasons when present, and the node is not
ready to run. Model configuration UI is not implied because the current command surface does not
provide it.

The frontend does not invent model compatibility. Options come only from
`generation_profile_list_for_capability` for the selected exact capability.

## Run Work Drawer

The `Run` tab in the Work Drawer is the visible task surface. It presents existing Workflow and Node
Execution facts; it is not a new generic task subsystem. Opening or closing it changes only the
overlay state and never changes graph layout or execution state. Successful Run admission opens the
drawer and selects `Run` immediately, so queued work is visible before the first step starts.

### Header

- `Running 2 of 3 steps`, `Run complete`, or the exact terminal state.
- admission time from `created_at_epoch_ms` and elapsed time derived from admission until now for an
  active Run, or until `updated_at_epoch_ms` for a terminal Run.
- `Cancel run` only while cancellation is legal.

### Step timeline

One row per admitted Node Execution in deterministic plan order:

```text
[complete] Prompt
[running ] Generate image                  68%
[waiting ] Create video                    Step 3 of 3
```

Rows show state and progress basis points when present. They do not show provider tasks, inferred
progress, or an invented pending reason. Selecting a row selects its node and result.

The timeline is projected without creating another execution model:

- `WorkflowRunDto.node_executions` supplies deterministic order, identities, state, and progress;
- `WorkflowNodePresentationDto` supplies failure, block, stale, and output facts only when both its
  Run and node-execution identities match the displayed row;
- durable Run events trigger projection refreshes but never become a second authoritative state;
- structured reasons are translated to creator-facing copy, with the original value available only
  in failure diagnostics.

### Completion

A successful Run reports the number of matching non-null node presentations and generated media
Assets. For the acceptance path this is `3 steps complete · 2 assets created`. A missing presentation
for a succeeded output-producing node is treated as a mock/test contract failure rather than
displayed as `Done · 0 outputs`.

## Asset Library View

The Assets rail tab reuses the left panel for the library and adds a 300 px detail panel between the
library and the canvas. The canvas stays mounted behind both panels; browsing Assets never unmounts
the graph or loses viewport position.

### Toolbar

- Search filters the current Project library by visible text.
- Kind filter chips: `All`, `Image`, `Video`, `Audio`.
- Import actions for image, video, and audio files.
- Grid/list mode toggle. Grid is the default; both modes show the same facts.

### Asset card

- A thumbnail for an Available Asset with an issued preview; otherwise a typed empty tile — never a
  broken media element.
- A kind badge pairing an icon with the words `Image`, `Video`, or `Audio`; color is never the sole
  carrier of media type.
- The prompt or name line, the origin Project or node when the DTO establishes it, and the creation
  time.
- Clicking selects the card and opens the detail panel. Dragging a card onto the canvas creates the
  matching Asset node at the drop position.

### Detail panel

Shows Asset DTO facts only:

- the media preview for an Available Asset;
- media type, dimensions or duration when present, the origin node (which selects that node on the
  canvas), and the creation time;
- `Add to canvas` and `Export` actions.

Pending or Missing Assets show their authoritative state and no preview. Metadata the Asset DTOs
cannot establish, such as the generating-model or seed rows in the reference mockup, is not
displayed.

### Refresh and empty states

- The library refreshes after a successful import and after a Run succeeds.
- An empty library explains import as the next valid action; an empty filter result offers to clear
  the filter.

## Assistant Dock

The Assistant is a Project-scoped Workflow co-author. It plans and proposes; it is never
authoritative for Workflow, Run, or Asset state. Every change it suggests becomes real only through
exact human approval, following [`BACKEND_ASSISTANT.md`](BACKEND_ASSISTANT.md).

### Layout

- A 320 px dock at the right workspace edge, toggled from the rail. Opening or closing it changes
  only overlay state, never graph layout or execution state.
- Header: `Assistant`, an availability indicator, and close.
- An empty conversation shows a small set of creator-language suggestions that send on click.

### Conversation stream

- User messages, assistant text streaming in token order, and tool activity steps shown as
  `running`, then `done` or `error`, labelled in creator language — never raw tool identifiers.
- Presentation events carry per-invocation sequences. A sequence gap triggers an authoritative
  re-query of pending state; the UI never fabricates missed content.

### Proposed change review

- When an Assistant Workflow Change awaits a decision, an approval card states exactly what the
  change does — nodes added or removed, connections changed, settings changed — with `Approve` and
  `Reject`.
- The decision is durable: an undecided change resurfaces when the dock or Project reopens,
  projected from `assistant_get_pending_workflow_change`.
- Applied changes are attributed to the creator's approval; assistant copy never claims the
  assistant applied anything by itself.

### Composer and context

- Multiline composer; `Enter` sends and `Shift+Enter` inserts a newline; disabled while sending.
- Each send silently carries the current Project id, Workflow revision, and selected node and Asset
  ids so replies stay grounded in the visible workspace.
- When the Assistant is not configured, the composer is disabled with the reason and a path to
  `Settings → Assistant`.

## Settings Dialog

Settings is a modal dialog over the workspace with a left section list, one content panel, and a
`Done` action. A section with no available capability shows a short factual empty state; it never
implies a function the command surface does not provide.

- `Providers`: the generation provider list from the existing provider commands, choosing the active
  provider, and setting a provider API key. Keys are write-only, stored by the credential vault, and
  never displayed back.
- `Assistant`: the master enable, and the OpenAI-protocol connection — Base URL, Model, and a
  write-only API key — read and saved through the existing assistant configuration commands. The
  Skills list and Developer mode from the reference mockup are specified but gated on backend
  commands that do not exist yet; they are not built in this phase.
- `Canvas`: editor preferences. None exist yet.
- `Storage`: where Project data lives, as a read-only fact.
- `About`: application name and version.

The provider and assistant configuration commands already exist in the desktop backend; wiring them
into the frontend API boundary is a mechanical addition, not a new backend capability.

Saving announces success or failure in place. Secrets never appear in copy, logs, or diagnostics.

## Asset Flow

- Successful generated media appears in the node result and Asset Library only after the backend
  exposes an Available Asset and issues a preview.
- Generated Image output is the bound input to Image to Video; the UI never copies preview URLs into
  Workflow data.
- The Asset Library refreshes when a Run succeeds, as specified in the Asset Library View.
- Opening an output shows media type, dimensions/duration when present, origin node, and creation
  time using Asset DTO facts only.
- Pending or Missing Assets show their authoritative state and no broken preview element.

## Deterministic Browser Mock

The mock implements the same user-visible contract without network or vendor calls:

- expose at least one available model for each generation capability;
- enforce canonical readiness rather than returning `ready` unconditionally;
- emit observable queued, started, progress, succeeded, and terminal Run events in stable order;
- produce one typed deterministic Image Asset for Text to Image;
- consume that Asset reference and produce one typed deterministic Video Asset for Image to Video;
- return stable local preview fixtures and list both Assets in project order;
- preserve cancellation and failure injection cases used by UI tests;
- answer Assistant messages with a deterministic streamed reply and support the injected approval
  flow used by UI tests;
- once Settings sections are wired, serve deterministic provider and assistant configuration state.

Mock preview fixtures must be visibly labelled as deterministic samples. They are test data, not
claims that a vendor model ran.

## Loading, Empty, Error, and Accessibility Requirements

- Loading contracts, models, presentations, and Assets use bounded skeletons without moving the
  canvas.
- Every empty state explains the next valid action.
- Errors identify the failed action and the creator's recovery action; raw `String(error)` is not
  primary copy.
- Nodes, ports, edges, Run controls, model options, and Asset actions have accessible names and
  visible focus.
- Dynamic Run changes use one polite live region; failures use an assertive announcement once.
- Text must not overlap, clip, or wrap through controls at 1280x720, 1440x900, or 1920x1080 with
  100% and 125% text scaling. The mechanism is one shared truncation system: single-line values
  end in an ellipsis, titles and prompts clamp at two lines, buttons and pills never wrap, and
  every flex row child holding text carries `min-width: 0`.
- Color is never the sole carrier of media type or execution state.

## Project Structure

- `ui/src/components/`: shell, Work Drawer, library, Settings, and top-bar composition.
- `ui/src/canvas/`: React Flow interaction and canvas layout.
- `ui/src/nodes/`: node, port, edge, and result presentation.
- `ui/src/workflow/`: canonical projection and Run orchestration.
- `ui/src/assets/`: Asset Library view and typed preview states.
- `ui/src/assistant/`: Assistant dock, approval card, and stream projection.
- `ui/src/api/`: Tauri and deterministic mock boundary implementations.
- colocated `*.test.ts(x)`: focused behavior tests.

Components remain capability-focused and below the repository line limits. React state owns viewport,
selection, open panels, and playback; backend DTOs own durable facts.

## Commands and Verification

Focused implementation checks:

```bash
npm run typecheck --prefix ui
npm run test --prefix ui
npm run dev --prefix ui -- --host 127.0.0.1 --port 5273
```

Manual desktop verification uses 1280x720, 1440x900, and 1920x1080. It covers pointer and keyboard
creation/connection, model selection, Run to here, progress, cancellation, failure, stale output,
Asset read-back, console output, and the accessibility tree. Full Cargo and E2E remain PR CI gates.

## Testing Strategy

- Unit: user-facing state/copy mapping, preview eligibility, model option projection, mock Asset
  production, Run timeline projection, Asset card/detail projection, and assistant stream
  projection.
- Component: labelled ports, compatible-target highlighting, Configure readiness, empty preview,
  timeline progress/failure, library filtering, approval-card decisions, and Settings section
  behavior once wired.
- Cross-language contract: unchanged DTO fixtures remain authoritative.
- Browser: the complete deterministic Text -> Image -> Video path with two Assets and no console
  errors, and an Assistant propose -> review -> approve round trip.
- Backend E2E: existing Workflow, Asset, and Assistant behavior remains unchanged.

## Boundaries

### Always

- Derive compatibility, readiness, Run state, and Asset state from authoritative backend contracts.
- Use creator-facing language and preserve the exact typed media path.
- Make mock success behaviorally meaningful and deterministic.
- Keep the reference experience desktop-only.

### Ask first

- Any new backend command, DTO field, dependency, persistent preference, or Settings capability.
- Any change to the exact-seven capabilities or frozen Workflow semantics.

### Never

- Reimplement Workflow compatibility or readiness rules in the frontend.
- Display secrets, provider task IDs, managed paths, or preview tokens.
- Render a media preview without a current typed output and preview URI.
- Report a generated-media node as successfully producing zero outputs.
- Present a Settings capability that has no backing command.
- Add mobile navigation or responsive product behavior in this phase.

## Acceptance Criteria

- A first-time desktop user can build and run Text -> Image -> Video without knowing backend terms.
- All connections can be completed by pointer and keyboard and persist after Project reopen.
- No node overlaps another when created through the library.
- `Generation profile` is absent from visible UI copy.
- The Work Drawer's `Run` tab shows every admitted step and its authoritative state/progress/failure.
- No Image or Video preview appears before a current output exists.
- The deterministic mock Run creates one Image Asset and one Video Asset with usable previews, and
  the Asset Library lists both after the Run succeeds.
- The successful path never displays `Done · 0 outputs`.
- The Assistant dock completes a propose -> review -> approve round trip against the deterministic
  mock, and an undecided proposal resurfaces after the dock or Project reopens.
- Settings never displays secrets and never presents a capability without a backing command.
- UI text does not collide or clip at the three desktop verification sizes.
- Browser console has zero errors and warnings for the acceptance path.

## Resolved Interaction Decisions

- The top-bar action is always `Run all`; remembering the last scope would introduce hidden state and
  a persistent preference that this phase does not need.
- Successful admission switches the Work Drawer to `Run` immediately; the creator must see queued
  work rather than wait for the first execution event.
- One available generation model is selected automatically; multiple available models are selected
  inline in `Configure`. Model configuration is not added without a backend command.
- The Assistant lives in a rail-toggled dock rather than a full page: co-authoring stays beside the
  canvas and never replaces it.
- Settings is a modal dialog rather than a workspace view: it is infrequent and must not compete
  with the canvas.
- Settings sections without backend commands stay unbuilt; a non-functional stub that implies
  capability is worse than an absent section.

## Fix Batch 1 — Copy, Layout, Mock Production, and Run Summary

The first fix batch closes the gap between the implemented shell and this specification. It is
frontend-only: no backend commands, DTO fields, or Workflow semantics change.

### Problems observed

1. **Jargon in visible copy.** The Inspector labels the model control `Generation profile`, the
   selector's accessible name and placeholder say `profile`, node labels use contract vocabulary
   (`Text to Image`), and state pills use internal vocabulary (`Idle`, `Done`, `Cached`, `Error`).
   All violate the Product Language table.
2. **Text collision and wrapping bugs.** Node titles and state pills share one flex row with no
   truncation rules; parameter labels wrap awkwardly in a 56 px grid column; the Top Bar project
   name and Run summary lack `min-width: 0` containment; Node Library leaves and Asset card
   metadata can overflow at 1280x720.
3. **Undersized, under-specified nodes.** Nodes are 216 px wide instead of 304/336 px. A Video
   node renders a play affordance without a preview URI. Generation nodes show no instructional
   empty state before their first Run.
4. **The deterministic acceptance path is impossible.** The browser mock lists no generation
   models, reports `ready` unconditionally, emits `node_succeeded` with `outputs: []`, and has no
   Asset store — so a mock Run ends with `Done · 0 outputs`, which this specification forbids.
   Separately, the Run controller settles every successful Run with `outputs: {}`, so even the
   real backend cannot produce a node preview or an honest completion summary through this path.
5. **Model selection has no product behavior.** One available model is not selected by default,
   and zero available models renders an empty `<select>` instead of the specified empty state.

### DVStudio adaptations for this batch

Only CSS-level and vocabulary-level patterns transfer from the Vue 3 reference:

| Pattern | Application |
| --- | --- |
| Truncation system (ellipsis trio, two-line clamp, flex `min-width: 0`) | The shared mechanism required by the accessibility section, applied to node titles, pills, buttons, the Top Bar, library leaves, and Asset cards. |
| Node title two-line clamp | Title keeps a 20 px minimum line height and clamps at two lines; the state pill never wraps. |
| Progress row with percent | Running nodes show the bar plus a determinate percent when the backend supplies basis points. |
| Filter chips with counts | Asset Library kind chips show per-kind counts. |
| Search aliases | Node presentations carry creator-language aliases and the Node Library matches them (Graph Editing). |
| Verb-first failure copy | Failure copy keeps the `Action · reason` shape already used in the Top Bar. |

Not adopted: glow and bloom effects, L-shaped corner brackets, glass blur, sci-fi square radii,
Canvas-2D edge rendering, the `Task`/`Blueprint Log` vocabulary, provider task panels, and
DVStudio's 240 px resizable nodes.

### Batch decisions

1. **Copy map.** `Generation profile` becomes `Generation model` everywhere, including accessible
   names; `Select a profile` becomes `Select a model`; the pill mapping is idle `Not run`, running
   `Running`, done and cached `Complete`, error `Needs attention`; the Top Bar action reads
   `Run all`; the Inspector node action reads `Run to here`. `generation_profile_ref` remains a
   parameter key only and never appears as visible copy.
2. **Node presentation.** Widths move to the frozen 304/336 px via a generation modifier class.
   The result region keeps its 16:9 frame at a 176 px minimum height. Generation nodes with no
   current typed output show `Run this step to create an image. / a video. / audio.` The video
   play affordance renders only when a preview URI exists.
3. **Model selection** follows the Generation Model Selection section exactly: one available
   model auto-selects and renders read-only; none renders the specified empty sentence plus safe
   availability reasons; several render the inline select with disabled, reason-labelled options.
4. **Deterministic mock** follows the Deterministic Browser Mock section, with these concrete
   fixtures: `mockAssets` becomes a real in-memory per-project store; previews are inline SVG data
   URIs visibly labelled `Deterministic sample — mock image / video / audio`;
   `generationProfileListForCapability` returns exactly one available `Fast … model (sample)` per
   generation capability. Mock readiness mirrors the canonical minimum from the stored Workflow —
   required inputs bound, `generation_profile_ref` set on generation nodes, `asset_id` set on
   Asset nodes — and `workflowStartRun` rejects a blocked Workflow as the real backend does.
   `executeMockRun` advances in timed steps so queued, started, progress, and terminal events are
   observable in stable order, honors cancellation between steps, creates one typed Asset per
   generation node with `workflow_node_output` origin, and emits `node_succeeded` outputs in the
   real backend's `{ key, value: { type, asset_id?, preview_uri? } }` payload shape.
   `workflowGetNodePresentation` returns the node's current typed output and `preview_uri` once
   its Asset exists.
5. **Run summary contract (frontend types only).** The succeeded terminal status gains a `steps`
   count known by the Run controller at settle time. The controller accumulates `RunOutputs` from
   `node_succeeded` payloads instead of settling with `{}`: media outputs prefer `preview_uri`
   and fall back to the opaque `asset_id`; text outputs keep their literal. The projection passes
   only `data:`, `https?:`, and `blob:` URIs into preview elements, so neither opaque refs nor raw
   asset ids ever reach an `<img>`. The Top Bar success copy is `3 steps complete · 2 assets
   created`, omitting the assets clause when zero; `Done · 0 outputs` can no longer appear.
6. **Truncation and overflow system.** The shared mechanism from the accessibility section lands
   per surface, and the node parameter label column widens from 56 px to 64 px with a two-line
   clamp.

### Out of this batch

Tracked in the implementation snapshot, not in this batch: the overlay Work Drawer with its Run
step timeline; the dark workbench palette; readiness gating on Run buttons and the Configure
issue list; magnetic ports, compatible-target highlighting, and keyboard connection; real-backend
node previews driven by `workflowGetNodePresentation`; Settings section wiring; Asset list mode
and Export. Register items UI-5, UI-8, UI-9, UI-15, UI-17, and UI-18 carry their own later
dispositions in the Open Issue Register, as do UI-23, UI-24, and UI-25.

### Task breakdown

Each task lands with its focused tests green.

- **T1 Copy and labels** — decision 1 plus library group ordering and alias matching, with the
  exact-capability, node, Inspector, and Node Library tests updated.
- **T2 Model selection** — decision 3, with auto-select, read-only, and empty-state test cases.
- **T3 Node presentation and truncation** — decisions 2 and 6 across node, Top Bar, Inspector,
  library, and Asset card styles, with node empty-state and play-affordance tests.
- **T4 Run summary** — decision 5 across Run types, controller, projection, and Top Bar, with the
  run-lifecycle tests updated.
- **T5 Deterministic mock** — decision 4, with readiness-rejection and two-Asset acceptance-path
  tests.
- **T6 Docs and sweep** — refresh the implementation snapshot; run the focused checks below.
- **T7 Wiring and ports** — Open Issue Register UI-1 through UI-4, UI-6, UI-7, and UI-29,
  including the transient editor notice channel that takes editor messages out of Run status and
  the fit-new-node-into-view behavior.
- **T8 Small sweeps** — register UI-10 through UI-14, UI-16, and UI-19: Top Bar label and saved
  state, Export removal, real jump-to-source, search placeholder, edge glow removal, and
  verb-first error copy.
- **T9 First-run usability** — register UI-20 through UI-22 and UI-26 through UI-32: canvas empty
  state, multiline prompt editing, empty-Run blocking, approval-card summary, Asset card cleanup,
  loading skeletons, empty-state copy, Project switcher failures, and a keep-alive Assistant dock.

T1–T3 are independent; T4 and T5 are independent of each other; T7–T9 are independent;
T6 is last.

### Verification

The focused commands from Commands and Verification, plus a manual browser pass at the three
desktop sizes: build Text -> Generate image -> Create video in the mock, run it, and confirm two
previews, `3 steps complete · 2 assets created`, both Assets in the Library with labelled sample
previews, and no text collision at any size. The word `profile` must not appear in visible UI
copy.

## Open Issue Register

Every known UI problem awaiting resolution, each with code evidence and a planned disposition.
Severities: `bug` (user-visible wrong behavior), `spec` (violates this specification), `ux`
(confusing but functional), `perf` (wasteful), `watch` (verify later). Dispositions name the
batch that owns the fix; `Ask first` items must not be fixed unilaterally.

### Wiring and ports

| ID | Severity | Problem (evidence) | Required resolution | Disposition |
| --- | --- | --- | --- | --- |
| UI-1 | bug | Every edge animates on any node progress: `useRunProjection.applyProgress` sets `running: true` on all edges. | Scope the running treatment to edges feeding the currently running node, or drop it until scoped data exists. | Fix Batch 1 |
| UI-2 | spec | Ports are unlabeled dots; the two inputs of Create video are indistinguishable. The Graph Editing section requires every port to show its name and media type. | Render port name and media type next to every port. | Fix Batch 1 |
| UI-3 | bug | Port vertical offsets are hardcoded (`PORT_TOP = 64 + i*24`) and collide with wrapped titles, Asset rows, and recovery banners. | Position ports from their logical content rows, not constants. | Fix Batch 1 |
| UI-4 | bug | Drop placement uses fixed `clientX - 320 / clientY - 90` offsets — wrong under pan/zoom and with any open panel. | Convert drop coordinates with `screenToFlowPosition` against the measured canvas. | Fix Batch 1 |
| UI-5 | ux | No live connection feedback: React Flow receives no `isValidConnection`, so invalid targets are discovered only on drop. | Live validation during the drag, highlighting compatible targets and dimming invalid ones. | Interaction batch, with magnetic ports |
| UI-6 | bug | A rejected connection is reported as a Run failure in the Top Bar, clobbering real Run state. | A transient editor notice channel (toast plus live region); Run status carries Run facts only. | Fix Batch 1 |
| UI-7 | bug | Library placement staggers by node count (`140 + n*60`), so additions after deletions overlap existing nodes. | Deterministic first-free-slot placement that never overlaps. | Fix Batch 1 |
| UI-8 | perf | Dragging a node enqueues a serialized `move_node` mutation per animation frame. | Persist the position only on drag stop, using the `dragging` flag. | Performance batch |
| UI-9 | spec | Port compatibility is re-implemented in React (`canConnectPorts`); Graph Editing requires engine-owned connection findings. | Engine findings at connect time; the UI pre-check is load-bearing until then, so it stays. | Ask first |

### Top Bar and Run

| ID | Severity | Problem (evidence) | Required resolution | Disposition |
| --- | --- | --- | --- | --- |
| UI-10 | spec | `Running · {nodeId}…` shows the raw node UUID as primary copy, which Product Language forbids. | Show the creator-facing node label and the determinate percent. | Fix Batch 1 |
| UI-11 | ux | No saved-state indicator; only booting, opening, and unavailable states appear. | Show `Saving…` / `Saved` derived from the persistence queue. | Fix Batch 1 |

### Assets

| ID | Severity | Problem (evidence) | Required resolution | Disposition |
| --- | --- | --- | --- | --- |
| UI-12 | spec | `Export` in the Asset detail panel has no handler — a capability with no backing command. | Remove the button until an export command exists. | Fix Batch 1 |
| UI-13 | bug | Jump to source only switches the rail tab; it neither selects nor centers the source node. | Select the source node and fit it into view. | Fix Batch 1 |
| UI-14 | ux | The library search placeholder `Search by prompt or model…` promises a model fact Asset DTOs do not carry. | Placeholder names searchable facts only, such as `Search by prompt or name…`. | Fix Batch 1 |

### Nodes and canvas

| ID | Severity | Problem (evidence) | Required resolution | Disposition |
| --- | --- | --- | --- | --- |
| UI-15 | spec | Node bodies inline every parameter; Node Presentation allows only the two most important editable values. | Accepted interim until the Configure tab exists; trim bodies when it lands. | Work Drawer batch |
| UI-16 | spec | The running edge treatment uses an animated `drop-shadow` glow, which Visual Direction forbids. | Keep the dash-flow animation, drop the glow. | Fix Batch 1 |
| UI-17 | watch | Video previews render through `<img>`; a video-file preview URI would break. | Confirm the backend preview protocol always issues thumbnails; otherwise render `<video>`. | Watch |
| UI-18 | ux | MiniMap colors are hardcoded light hexes. | Move to tokens. | Palette batch (deferred) |

### Copy and errors

| ID | Severity | Problem (evidence) | Required resolution | Disposition |
| --- | --- | --- | --- | --- |
| UI-19 | spec | Several paths surface raw `String(error)` as primary copy, for example Run status and the Asset library error row. | Verb-first `Action · reason` copy naming the failed action and the recovery action. | Fix Batch 1 |

### First-run and everyday usability

Issues found by walking the acceptance path as a first-time user: launch, create a Project, build,
connect, run, inspect, browse Assets, and review an Assistant proposal.

| ID | Severity | Problem (evidence) | Required resolution | Disposition |
| --- | --- | --- | --- | --- |
| UI-20 | spec | The empty canvas gives no next-action guidance — neither with no Project nor with an empty Workflow, though every empty state must explain the next valid action. | An on-canvas empty state that names the first step (`Create a Project` / `Add a Text node to begin`). | Fix Batch 1 (T9) |
| UI-21 | ux | The prompt parameter allows 64 KB of text per contract but edits in a single-line input, both on the node and in the Inspector. | A multiline editor for long text parameters in the Inspector; the node body keeps a one-line summary until Configure exists. | Fix Batch 1 (T9) |
| UI-22 | ux | `Run all` on an empty Workflow admits and reports a meaningless success. | Block admission with a notice that names the next step. | Fix Batch 1 (T9) |
| UI-23 | spec | Node and edge deletion is keyboard-only and undiscoverable; Graph Editing requires pointer and keyboard operability. | A visible delete action on the selected node and edge, plus shortcut hints. | Interaction batch |
| UI-24 | ux | No undo/redo: destructive deletes are unrecoverable. `runUndo`/`runRedo` barrier wrappers exist but no history model backs them. | A history design keyed to canonical mutation receipts. | Needs design |
| UI-25 | ux | Two nodes of the same type are indistinguishable; there is no instance label. | A Workflow node-label field. | Ask first |
| UI-26 | spec | The approval card renders raw mutation JSON, a hex digest, and `Repair Run {uuid}`; the Assistant Dock section requires an exact creator-language summary. | Summarize each mutation as creator-language facts (node added/removed, connection changed, setting changed); digests and ids move to copyable diagnostics. | Fix Batch 1 (T9) |
| UI-27 | ux | Every Asset card repeats `Current project`, and the jump action renders even for imported Assets with no source node — a dead affordance. | Drop the redundant meta line; show jump only when an origin node exists. | Fix Batch 1 (T9) |
| UI-28 | spec | No loading skeletons anywhere: contracts, models, presentations, and Assets load silently. | Bounded skeletons per the accessibility section. | Fix Batch 1 (T9) |
| UI-29 | ux | Adding a node does not fit it into view; Information Architecture requires the canvas to fit each new selection into view. | Fit the newly added node into view on creation. | Fix Batch 1 (T7) |
| UI-30 | ux | The empty-library copy only says to run a workflow — it ignores Import and gives wrong guidance when no Project is open. | Contextual copy: import as an equal next action; open-a-Project guidance when none is open. | Fix Batch 1 (T9) |
| UI-31 | ux | The Project switcher's empty list gives no guidance, and create/rename failures are silent (no `catch`). | An empty-list hint plus surfaced, verb-first failures. | Fix Batch 1 (T9) |
| UI-32 | ux | Closing the Assistant dock unmounts it and discards the visible conversation; no history command exists. | Keep the dock mounted and hidden as the interim; durable history needs a backend command. | Fix Batch 1 (T9) interim; history Ask first |

Already tracked in the implementation snapshot and not repeated here: readiness gating on Run
buttons, stale output treatment, Assistant availability gating, Settings sections, inline panels
versus overlay chrome, deterministic mock production, and the Work Drawer itself.
