# Desktop Creation Workspace Specification

## Authority

This document is the frozen presentation and interaction authority for the desktop workspace.
It defines the target design only — it intentionally contains no implementation status, progress,
or issue tracking. What is implemented, what is pending, and every known gap lives in
[`ROADMAP.md`](ROADMAP.md). Backend semantics remain owned by the documents mapped from
[`BACKEND.md`](BACKEND.md).

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
  Capability, Workflow, Run, Asset, and Assistant commands, plus provider/assistant configuration
  and Generation Task get/list commands exposed by the desktop backend.
- The exact seven Node Capabilities and the existing whole/through-node Run scopes.
- Empty, editing, blocked, queued, running, succeeded, failed, cancelled, and stale
  presentation states mechanically derived from existing DTOs.
- Keyboard-operable node selection, connection, deletion, and Run controls.
- A deterministic mock Text-to-Image -> Image-to-Video path with typed Asset read-back.
- Asset Library view, Assistant dock, and Settings dialog presentation as specified below.
- Run history and multiple Workflows per Project are product scope. Their presentation design
  is pending; it will be frozen in this document before any implementation.

### Out of scope

- Mobile or touch-first layouts.
- Retry-in-place, cost accounting, provider-native task
  controls, or provider-native progress.
- Backend commands, DTO fields, business states, or compatibility rules beyond the frozen contracts.
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
| Generation Task | Generation | Durable model work associated with one waiting Step. |
| ThroughNode | Run to here | Run this node and every dependency it needs. |
| WholeWorkflow | Run all | Run the complete workflow. |
| Asset | Asset | A saved image, video, or audio result in this Project. |
| Readiness | Ready to run | Whether all required connections, settings, models, and Assets are valid. |
| Production Plan | Plan | The assistant's working outline for the current request. |
| Assistant Workflow Change | Proposed change | The exact workflow edit the assistant asks the creator to approve. |
| Approval decision | Review change | Approving applies the proposed change; rejecting discards it. |
| Generation Task | Step details | The model-side record of one step, shown only as step details inside Run details. |

Raw identifiers, enum keys, error debug strings, `generation_profile_ref`, tool identifiers, and
provider route names must not appear as primary UI text. Technical identifiers may appear only in a
copyable diagnostics section after a failure.

### Labels and copy

Every visible label follows one copy system, on the node body, in the Inspector, and in the Work
Drawer alike:

- Sentence case: capitalize only the first word (`Aspect ratio`, never `Aspect Ratio` or
  `ASPECT RATIO`). CSS never uppercases a parameter or control label; only structural eyebrows
  (`Nodes`, `Parameters`, `Outputs`) may use tracked small caps.
- Human words with units in parentheses: `Text`, `Aspect ratio`, `Duration (seconds)`, `Asset`,
  `Generation model`. Parameter keys (`aspect_ratio`, `duration_seconds`) never appear.
- Every enum option gets a human label: `square` → `Square 1:1`, `landscape_16_9` →
  `Landscape 16:9`, `landscape_4_3` → `Landscape 4:3`, `portrait_3_4` → `Portrait 3:4`,
  `portrait_9_16` → `Portrait 9:16`. The same rule applies to any future option set.
- The same parameter carries the identical label on the node body and in the Inspector.
- Live values (elapsed time, progress, revision) use the tabular monospace face; labels never do.
- Actions are verb-first and name the outcome (`Run all`, `Add to canvas`, `Delete node`);
  failures follow `Action · reason` and name the recovery step.

## Information Architecture

The workspace uses one canvas-first desktop shell. The graph remains full-bleed beneath workspace
chrome and does not collapse into a mobile layout.

```text
+--------------------------------------------------------------------------------------+
| Project / saved state       Run summary                              [Run all] [more] |
+------+-------------------------------------------------------------------------------+
|      |  (left overlay)                                          (right overlay)      |
| rail |  NODE LIBRARY /        WORKFLOW CANVAS                      WORK DRAWER /      |
|      |  ASSET LIBRARY         full-bleed, never resized           ASSISTANT          |
|      |  + detail              by any panel                        (one at a time)    |
|      |                        [zoom] [fit] [minimap]                                 |
+------+-------------------------------------------------------------------------------+
```

### Shell regions

Only three regions are inline and permanent: the top bar, the rail (56 px), and the canvas.
Everything else is an overlay.

- Top bar: Project context, saved state, a compact current/last Run summary, and the primary Run
  action. The primary action is always `Run all`; node-scoped `Run to here` stays in `Configure`.
  Closing any overlay never hides the Run summary.
- Rail: 56 px; switches the left overlay between Nodes and Assets, toggles the Assistant in the
  right slot, and opens Settings as a modal dialog, without navigating away from the workspace.
- Canvas: fills every pixel the top bar and rail leave. Opening or closing any panel never
  resizes it, never moves nodes, and never bends saved edges.

### Overlay rules

- Every panel — Node Library, Asset Library with its 300 px detail companion, Work Drawer, and
  Assistant dock — is an overlay: it floats above the canvas with a shadowed edge and never
  changes layout. Overlays sit between the top bar and the rail; they never cover either.
- The right edge has one overlay slot. The Work Drawer (380 px, `Configure` and `Run` tabs) and
  the Assistant dock (320 px) share it; exactly one is visible at a time. Switching preserves the
  hidden surface's state — selection, scroll position, and the conversation. Until the Work
  Drawer lands, the Inspector occupies the same slot under the same rules.
- The right slot is empty until it has content: `Configure` when a node is selected, `Run` when
  a Run is admitted or inspected, Assistant when toggled from the rail. With nothing selected
  and no Run context, the slot stays closed instead of showing an empty panel.
- The left edge has one overlay slot: the Node Library (304 px, grouped Inputs and Generate) or
  the Asset Library with its detail companion, switched from the rail and pinnable for the
  current UI session.
- Node placement and fit-into-view measure the visible canvas — the area not covered by open
  overlays — so a new node never lands beneath a panel and a fit never hides a node behind one.
- Canvas controls: zoom, fit, and minimap cluster against the lower-left visible canvas edge,
  offset past an open left overlay, and remain reachable while any overlay is open.
- These rules hold identically at every supported size. No media query changes layout mechanics;
  the overlay model is the only behavior from 1280x720 upward.
- Settings: a modal dialog over the workspace. It is infrequent and must not compete with the
  canvas, so it is not a workspace view.

The page must never render a node beneath another node. New nodes use a deterministic staggered
placement inside the visible canvas and the canvas fits the new selection into view.

## DVStudio Reference Adaptation

DVStudio is a visual-interaction reference, not a second source of product semantics. The following
patterns are adopted because they solve observed desktop usability problems:

- a full-bleed graph with overlay workspace chrome, keeping the creation path visually primary;
- separate visible ports and screen-space pointer hit layers, with magnetic target feedback;
- explicit empty image/video states and disabled media controls until a resource exists;
- node-local progress plus a larger Run detail surface for the whole execution;
- an Asset browser with thumbnails, grid/list modes, and drag-to-canvas creation.

The following DVStudio patterns are deliberately not adopted:

- provider-native task IDs, provider-native state synchronization, or retry controls;
- frontend-owned connection compatibility rules rather than engine-owned contract findings;
- node type switching, copy, resize, or refresh actions absent from the current mutation surface;
- unused-Asset analysis or metadata that existing Asset DTOs cannot establish;
- glass blur, particles, animated glow, decorative corner brackets, or raw node identifiers.

## Visual Direction

The workspace is a bright studio: clean white chrome on a light floor, with two signature
moments — the typed connection path and the aurora.

### Chrome palette

- `Studio Floor` `#EDF0F3`: the canvas ground.
- `Panel White` `#FFFFFF`: overlay panels, cards, and node bodies.
- `Panel Mist` `#FAFBFC` and `Panel Frost` `#F0F3F6`: recessed fields and strips.
- `Hairline` `#E2E6EA` / `#D5DBE1`: one-pixel structural dividers.
- `Ink` `#1A1D21`, `Ink Two` `#454D56`, `Ink Three` `#7C8690`, `Ink Faint` `#9AA3AC`: the text
  ladder; hierarchy comes from ink depth, not size games.
- Typography: the existing system UI stack for interface copy; a tabular monospace face only for
  elapsed time, progress, revision, and diagnostic IDs.

### The aurora (signature accent)

The product's dream lives in one controlled gradient — `Dusk Blue` `#6D7BF2` into `Dream Violet`
`#A05CF0` at 135°, always paired with a soft violet glow. It appears only at brand moments and at
the solid primary action of a surface (`Run all`, `Run to here`, `Done`, `Approve`). A flat
companion, `Dream Ink` `#6C5CE7`, carries every text-level accent: selection, focus, links, and
active chips. The aurora never fills large surfaces.

### Type colors and status

- `Text Cyan` `#2F9DB2`, `Image Amber` `#C07F2E`, `Video Violet` `#6F5BD8`, and `Audio Pink`
  `#C2417C`: typed ports, edges, and the node header tint — never status on their own.
- `Running Gold` `#D8AD4D` (text variant `#B8860B`), `Failure Coral` `#CF4F42`, and `Success
  Green` `#2E9E73`: execution states, always paired with labels or icons.

### The typed connection path (signature element)

A connection carries the output media color from the source port through the edge to the matching
target port. Everything else remains quiet. No glass blur, particles, decorative cards, oversized
rounding, or shadow stacks are introduced.

### Nodes

A node is a white card whose identity lives entirely in its header: a pastel tint of the node's
type color (the type color at about 14% into white) with deep type-colored text. The body stays
white and the port strip is `Panel Frost`. The 3 px type bar is retired — the tinted header is
the type identity. Generation nodes keep their 16:9 result region at a 176 px minimum.

### Canvas

The canvas floor is `Studio Floor` with a blueprint grid: fine blue-gray lines at 22 px and a
stronger line every fifth, giving graph-paper structure without noise.

### Geometry

Corners follow a layered radius scale instead of one uniform value: 6 px on controls and inputs,
8 px on chips and small containers, 10 px on nodes, cards, and panels, 12 px on modal dialogs,
and a full pill radius on status pills and toggles. Tiny identity marks (category dots, badges)
may stay square. One-pixel borders everywhere.

### Controls

Each surface has exactly one solid primary action: the aurora gradient with white text. Every
other action is a ghost button — transparent fill, one-pixel hairline border, ink text, and a
hover that brightens the border or lifts the background. A destructive action is a coral ghost:
coral text and border on hover, never a bare red text link and never a solid red fill. A disabled
primary loses its fill entirely (frost surface, faint text, hairline border) so it can never be
mistaken for an active action.

Selection and focus are always `Dream Ink`. Execution status uses the status palette — Running
Gold for running borders, pills, and progress; Failure Coral and Success Green likewise — and is
always paired with a label. Data-type color appears only in the wiring and the node header tint,
never in selection, status, or buttons.

### Assistant presentation

The conversation is quiet: the creator's messages are right-aligned `Panel Frost` bubbles, never
accent-filled; assistant replies are plain full-width text without avatars or bubble chrome, and
suggestion starters are ghost chips. The composer is a recessed field with a ghost icon send
action. Tool activity reads as labeled steps with status icons, not as chat bubbles.

### Motion

Motion exists only where something is alive; an idle studio is perfectly still.

- `Aurora drift`: the aurora gradient slowly travels its track on the brand mark and solid
  primary buttons (10 s loop, background-position ease-in-out).
- `Edge flow`: a connection whose target step is running shows energy moving along the typed path
  (dash offset, 0.9 s linear).
- `Running pulse`: a running node breathes one gold ring outward (1.7 s ease-out) beside its
  progress bar and blinking Running pill.
- `Node mount`: a newly added node rises 8 px and fades in (180 ms ease-out, 60 ms stagger).
- UI transitions (panels, selection, hover) run 120–250 ms ease-out.
- `prefers-reduced-motion` disables all of the above.

## Graph Editing

### Creating nodes

- Clicking a library item adds one node at a visible, non-overlapping position and selects it.
- Dragging remains available for spatial placement.
- The library uses `Text`, `Generate image`, `Generate video`, and `Generate speech` as visible
  labels, grouped in the stable order Inputs, Generate. Asset nodes are not in the palette: an
  Asset node is created by dragging a card from the Asset Library onto the canvas.
- Library search also matches creator-language aliases such as `prompt`, `t2i`, `clip`, and `voice`,
  never only the contract identifier.
- Input and Asset nodes are 304 px wide; generation nodes are 336 px wide. They do not scale down
  into compact or mobile variants.

### Connecting nodes

- Every port shows its name and media type; it is not an unlabeled dot.
- The visible port gem is 10 px. A separate zoom-invariant screen-space hit layer provides a 22 px
  radius pointer target without making the node look oversized.
- During a drag, a compatible target highlights as the pointer approaches and locks when the
  pointer enters its 22 px hit radius; the connection commits when released anywhere inside that
  radius. The radius is a screen-space value and does not shrink when the canvas is zoomed out.
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
| waiting for external completion | Running | `Waiting for the model`, with the last supplied progress |
| cancelled | Cancelled | no fabricated result |

An Image or Video preview is rendered only when the current node presentation contains a complete,
non-stale typed output and a preview URI. Before that, the same area is a compact instructional empty
state. A Video node never shows a play affordance without a video preview URI.

## Generation Model Selection

The `Configure` tab label is `Generation model`, not `Generation profile`.

Each option presents:

- display name, such as `Fast image model`;
- availability: `Ready` or the existing structured reason it cannot run.

Provider IDs, credential IDs, and secrets are never displayed. If exactly one structurally
compatible model exists, it is selected by default and shown as a read-only row. If multiple
compatible models exist, the choice stays inline in `Configure`. A compatible but unavailable or
indeterminate model remains visible and disabled with its structured reason. With no compatible
model, the control reads `No generation model supports this node type`, and the node is not ready
to run.

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

Rows show Workflow state and progress basis points when present. A waiting provider-backed row also
loads its Project-scoped Generation Task projection and shows normalized task state, known progress,
and safe failure information. It never infers progress or a pending reason and never displays the
provider-native task ID. Selecting a row selects its node and result.

The timeline is projected without creating another execution model:

- `WorkflowRunDto.node_executions` supplies deterministic order, identities, state, and progress;
- `WorkflowNodePresentationDto` supplies failure, block, stale, and output facts only when both its
  Run and node-execution identities match the displayed row;
- `GenerationTaskDto` supplies durable generation state for the exact waiting node execution; it
  does not replace Workflow state or decide whether the Step succeeded;
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

- `Models`: the generation model list from the provider settings commands. Each model row shows
  its display name, the provider route serving it, and an enable toggle. Raw profile references
  and revision counters never appear. Provider API keys are managed outside the app until a
  backend credential command exists; no key field is shown before then.
- `Assistant`: the master enable, and the OpenAI-protocol connection — Base URL, Model, and a
  write-only API key. This section stays unbuilt until an assistant configuration command exists.
  The Skills list and Developer mode from the reference mockup are likewise gated on backend
  commands that do not exist yet.
- `Canvas`: editor preferences. None exist yet; the section shows a short factual empty state.
- `About`: application name and version, read from the frontend package itself.

A Storage section returns only when a backend query can report where Project data lives. Saving
announces success or failure in place. Secrets never appear in copy, logs, or diagnostics.

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
- Display secrets, provider-native task IDs, managed paths, or preview tokens.
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
