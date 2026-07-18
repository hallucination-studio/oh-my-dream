# Desktop UI Roadmap

Tracking document for the desktop workspace. The **design authority** is
[`DESKTOP_UI.md`](DESKTOP_UI.md) — it freezes the target presentation and interaction design and
contains no status. This document is the only place that records what is implemented, what is
pending, and every known gap. It must never restate or redefine design rules; it points at them.

Backend semantics remain owned by the documents mapped from [`BACKEND.md`](BACKEND.md).

## Verified status (2026-07-18)

- `npm run test --prefix ui`: 118 tests, 33 files, green. `npm run typecheck --prefix ui`: clean.
- Live walkthrough (Chrome DevTools MCP, mock backend, 1440x900): opened Mock Project, added
  Text → Generate image → Create video from the library, connected both edges by keyboard only,
  watched readiness gate `Run all` until the graph was complete, ran to `3 steps complete · 2
  assets created` with both node previews, and found both Assets in the Library. Zero console
  errors or warnings for the whole path.
- Baseline commits: `e39b25e..b7ffe42` (25 tasks, workstreams W1–W8).

## Landed baseline

| Area | Live today |
| --- | --- |
| Workspace shell | Rail + Node Library + canvas + Inspector + Top bar, dark workbench theme, inline panels. |
| Node Library | Creator labels, Inputs/Generate groups, alias search, 1.0 version badge. |
| Graph editing | Labeled ports (name + media type), 22 px hit radius, live `isValidConnection` with compatible-target highlight, keyboard connect, visible node/edge delete, slot placement, drag-stop persistence, transient editor notices. |
| Node presentation | 304/336 px widths, creator pills (Not run/Running/Complete/Needs attention), two-line title clamp, typed-output previews. |
| Generation model selection | Single-model auto-select, read-only row, empty/loading/error states, `Generation model` label. |
| Run | Engine-owned readiness gating with issue list, run details dialog with step timeline, cancel, output accumulation, honest summary (`3 steps complete · 2 assets created`). |
| Asset Library | Kind chips with counts, grid/list toggle, search, import, detail facts (dimensions/duration), jump-to-source (selects and centers), skeletons, contextual empty states. |
| Assistant | Send-path availability gating, keep-alive dock, creator-language approval summary, deterministic mock reply. |
| First-run | Empty-canvas guidance, empty-run block, Project switcher failure surfacing, multiline prompt editing, contract skeletons. |
| Visual | Three-layer tokens (palette → semantic → component), dark React Flow chrome, verified at 1280x720/1440x900/1920x1080. |
| Browser mock | In-memory Asset store, timed run execution, canonical readiness, typed Image+Video Assets with labelled deterministic previews. |

## A. Waiting to implement (ungated)

Ordered by leverage. Each item closes register entries named in brackets.

- **A1 Run-state bugfix batch** (UI-34, UI-35, UI-38…UI-46): pure frontend correctness, no design
  change. UI-39 is the highest-impact open bug.
- **A2 Visual conformance batch** (UI-67…UI-78): CSS/copy only, converges the dark theme on the
  frozen Visual Direction and Product Language.
- **A3 Interaction hardening batch** (UI-47…UI-57): placement/fit respect open panels, modal
  focus traps, wedged-state recovery, keyboard-connect exits.
- **A4 Work Drawer content** (UI-15, UI-73): the Configure tab fields, readiness guidance, and
  Run tab timeline inside the shell S1 provides. The overlay chrome itself moved to S1.
- **A5 Stale output treatment** (pairs with UI-39): the `Changed since run` state from Node
  Presentation, with consistent invalidation across nodes, Top bar, and previews.
- **A6 Run details honesty pack** (UI-60, UI-77): actionable outputs (preview + jump to Asset),
  creator-language step details, no raw `request_kind`/failure codes.
- **A7 Settings content** (UI-59, UI-65): About shows name/version, Storage shows the data
  location fact or the section is removed, decide Canvas.
- **A8 Pending-edge visual semantics**: visually distinct uncommitted edge; the engine-findings
  half stays gated on G1.
- **A9 Hygiene batch** (UI-79…UI-82): dead code removal, named error swallowing, mock reset hooks.
- **A10 Visual convergence (V1)** (UI-63, UI-67, parts of UI-74/76): layered radius scale, one
  solid primary per surface with ghost secondaries and coral-ghost danger, quiet Assistant
  (graphite user bubbles, no avatars, ghost chips and send), node type-vs-accent separation
  (teal selection, gold running status, type color only in the bar and ports). Design frozen
  2026-07-18 in DESKTOP_UI.md (Geometry, Controls, Assistant presentation).
- **A11 Label copy system (V2)** (UI-64, part of UI-77): sentence-case human labels everywhere,
  human option labels for every enum, node/Inspector label parity, no CSS uppercase on labels.
  Frozen in DESKTOP_UI.md (Labels and copy).
- **A12 Small convergences**: restore the seven-label Node Library with the Assets group (D1,
  closes UI-66); move the MiniMap to the lower-left canvas edge (D2); Settings Providers section
  per the frozen spec (D4) plus the About/Storage facts (UI-59).
- **S1 Overlay shell** (UI-48, UI-83, UI-84, UI-85, part of UI-47): the canvas becomes the only
  inline region; left overlay slot (Nodes / Assets + detail), right overlay slot (Inspector →
  Work Drawer, Assistant) with one-at-a-time exclusivity and state preservation; placement and
  fit measure the visible canvas; the <1100 px media query is removed; 56 px rail and 304 px
  library land here. Frozen in DESKTOP_UI.md (Information Architecture, 2026-07-18).

## B. Undesigned surfaces

Pages or surfaces the product needs but that have no frozen design yet. Do not build before the
design lands in `DESKTOP_UI.md`.

- **B1 Providers management** — provider list, active provider, write-only API keys. The Settings
  section in the spec names the capability; detailed states (empty key, invalid key, multiple
  providers, unreachable provider) are undesigned. Current UI ships a different `Models routes`
  section instead — see D4.
- **B2 Assistant configuration section** — master enable, Base URL, Model, write-only key.
  Blocked on G7 (no config query/command pair).
- **B3 Skills list and Developer mode** — blocked on G6 (no backend commands).
- **B4 Project management beyond the switcher** — rename confirmation, delete, duplicate.
- **B5 Keyboard shortcut reference** — discoverability for connect/delete/run/save shortcuts.
- **B6 First-launch onboarding** — anything beyond the current empty states (sample project,
  guided first run).
- **B7 Asset export flow** — blocked on G5 (no export command).
- **B8 Run history browser** — admitted into product scope 2026-07-18; the presentation design
  must be frozen in DESKTOP_UI.md before implementation.
- **B9 Multiple Workflows per Project** — admitted into product scope 2026-07-18; same design-
  first rule.

## C. Interaction issues

Affects how the pages behave day to day. Severities: `bug`, `ux`, `a11y`, `perf`.

| ID | Sev | Problem (evidence) | Required resolution |
| --- | --- | --- | --- |
| UI-47 | bug | Add-node fit centers only the new node, pushing earlier nodes off-canvas or under the library. Verified live: after three adds, the Text node sat at screen x=-365, the second under the panel. | Ensure-visible pan: only pan when the node is outside the visible canvas, by the minimal delta; placement must account for open panels. |
| UI-48 | ux | Inline panels resize the canvas; nodes slide under the library, Asset detail, and Inspector with no compensation (verified live twice). | The overlay Work Drawer (A4) is the design answer; interim: refit on panel toggle. |
| UI-49 | ux | Keyboard-connect mode has no click-away cancel; only Escape exits (App.tsx:302, WorkflowCanvas.tsx:113). | Pane click cancels the intent; show a visible "connecting from X" affordance with its own dismiss. |
| UI-50 | bug | Connect intent survives project switch and source-node deletion; wrong ports highlight and the notice blames type incompatibility (App.tsx:63, useProjectWorkspace.ts:217, App.tsx:340). | Reset connect state on hydrate, node delete, and project switch. |
| UI-51 | ux | Settings and Run details declare `aria-modal` but do not trap focus (SettingsDialog.tsx:96, RunDrawer.tsx:96). | Focus trap plus Escape-to-close in both dialogs. |
| UI-52 | ux | Every run start force-focuses the drawer close button, yanking focus out of the editor (RunDrawer.tsx:49, App.tsx:159). | Open the drawer without stealing focus; announce via the live region. |
| UI-53 | a11y | Asset grid card is a `<figure>` with onClick only — keyboard users cannot select an Asset or open details (AssetCard.tsx:20; confirmed live: cards absent from the a11y tree). | Make grid cards real buttons like list mode. |
| UI-54 | a11y | Keyboard-connect rows are `role="button"` but handle Enter only, not Space (WorkflowFlowNode.tsx:200). | Honor the button keyboard contract (Enter and Space). |
| UI-55 | ux | A failed readiness check collapses to "Checking…" with no retry; the Run button can wedge (useWorkflowReadiness.ts:11). | Retry/backoff plus an explicit error state with a re-check action. |
| UI-56 | ux | "Cancelling…" can wedge forever if `run_cancelled` is missed (useRunController.ts:112). | Timeout fallback returning to a cancellable state with an honest notice. |
| UI-57 | ux | Fit-view `knownIds` is not project-aware: switching projects triggers spurious zooms or none (WorkflowCanvas.tsx:50). | Key the known-id set by project and refit on project open. |

## D. Usability complaints

Things that work but feel wrong or unpolished.

| ID | Sev | Problem (evidence) | Required resolution |
| --- | --- | --- | --- |
| UI-58 | ux | Asset grid renders one giant card per row at 1440px; a video card is a huge empty tile (verified live). | Multi-column grid density targets per panel width; fix the video thumbnail (UI-36). |
| UI-59 | ux | Settings Canvas/Storage/About each render only `NOTHING HERE YET.` (verified live). | About shows app name/version (no backend needed); Storage shows the data-location fact or is removed; weak all-caps copy replaced. |
| UI-60 | ux | Run details Outputs are bare `String text / Image image / Video video` rows — no preview, no jump; `This Run has no available Task for the selected Step.` shows jargon after a success (verified live). | Outputs become actionable (preview + open in Library); step-details section hides or explains in creator language. |
| UI-61 | ux | Assistant empty-state suggestions are hardcoded and name a nonexistent `Text Prompt node` (verified live). | Suggestions derive from real node labels; G7 owns the config surface. |
| UI-62 | ux | The Inspector Mode select is a permanently one-option dead control (App.tsx:590, InspectorPanel.tsx:112). | Remove until a capability has real modes. |
| UI-63 | ux | Disabled `Run all` keeps the full teal fill and reads as active (verified live). | Muted disabled treatment for the primary action. |
| UI-64 | ux | Node param labels uppercase-wrap (`DURATION SECONDS`) while the Inspector shows lowercase — inconsistent casing (verified live). | One casing rule for parameter labels; see D5. |
| UI-65 | ux | Settings shows a meaningless `Rev 1` line (verified live). | Remove or explain in creator language. |
| UI-66 | ux | Node Library lacks the spec'd seven-label Assets group; asset nodes are only creatable from the Asset Library (catalog.ts:107). | Product decision D1, then converge code or spec. |
| UI-83 | ux | The Inspector is a permanent inline right column and the Assistant dock a fifth inline column; opening the Assistant crushes the canvas at supported widths (styles.css:55-69; verified live at 1440x900: canvas ~540 px, nodes sliding under panels). | The overlay shell (S1): the right edge becomes one overlay slot shared by Inspector/Work Drawer and Assistant, visible one at a time. |
| UI-84 | ux | The assistant-as-overlay behavior only engages below 1100 px — under the 1280 px supported minimum, so it never helps at any supported size (styles.css media query). | Removed by S1; overlays are the only behavior at every size. |
| UI-85 | ux | The empty Inspector permanently claims 292 px even with nothing selected (verified live). | S1: the right slot stays closed when it has no content (no selection, no Run, Assistant not toggled). |

## E. UI bugs

Functional defects and spec regressions, ordered by user impact.

| ID | Sev | Problem (evidence) | Required resolution |
| --- | --- | --- | --- |
| UI-39 | bug | Selecting a generation node silently autosaves the workflow and wipes visible run state: the auto-select effect fires on selection → `markWorkflowMutation` → `invalidateRun` (GenerationProfileSelector.tsx:39, InspectorPanel.tsx:144, useProjectWorkspace.ts:75). Verified live: one click erased the `3 steps complete` summary and reverted the video node to `Not run` while the image node kept its preview — inconsistent stale handling. | Auto-select only when the value actually changes, never on selection alone; pair with A5 stale treatment. |
| UI-34 | bug | Run details headline stays `Run queued` through the entire run — `runHeadline` reads `run.state`, which never advances to running in the projection (runTimeline.ts:40; verified live). | Derive the running headline from node executions, not the run record state. |
| UI-35 | bug | Elapsed shows `0:00` for terminal runs — the settle snapshot keeps the admission record's timestamps (useRunController settle; verified live). | Adopt the terminal run record (or its `updated_at`) at settle. |
| UI-36 | bug | Video Asset cards render no thumbnail: the poster preview URI feeds `<video src>`, which cannot render SVG/poster images (AssetMediaPreview.tsx:17; verified live; supersedes UI-17). | Render posters through `<img>`; distinguishing poster vs playable file needs G9. |
| UI-37 | bug | `Run to here` is gated by whole-workflow readiness: a ready upstream node is blocked by downstream issues, and the Inspector lists other nodes' issues (verified live). | Scoped readiness per node — backend query scope, gate G8; attribute issues to their node. |
| UI-38 | bug | `useAssets.refresh` has no project guard; a slow list for project A overwrites project B's library (useAssets.ts:16). | Generation/project guard on async list resolution. |
| UI-40 | bug | The initial `nodeCapabilityList` has no `.catch` — the library spins forever on contract-load failure (App.tsx:76). | Surface a load failure with retry. |
| UI-41 | bug | Mock `workflowCancelRun` flips terminal runs to cancelled and corrupts the durable event log (mockApi.ts:341). | Terminal-state guard mirroring the real backend. |
| UI-42 | bug | `isValidConnection` accepts self-loops and cycles the engine rejects at save time, surfacing later as a confusing save failure (validate.ts:9). | Reject self-loops and cycles at connect time from the graph facts the UI already holds. |
| UI-43 | bug | CloseFailureDialog renders raw `String(error)` as its body (CloseFailureDialog.tsx:18). | Verb-first failure copy with recovery action; raw text to diagnostics. |
| UI-44 | bug | One missed assistant presentation event permanently freezes the invocation — the sequence ref is never resynced, so every later event is dropped (AssistantDock.tsx:98). | Resync the expected sequence from the authoritative refetch. |
| UI-45 | bug | Run-start observer leaks: double-run overwrites the unsubscribe ref; a rejected `workflowStartRun` never unsubscribes (useRunController.ts:86-109). | Single observer lifecycle with rollback on admission failure. |
| UI-46 | bug | A run started before a project switch completes invisibly — no record, no asset refresh, no completion feedback on return (useProjectWorkspace.ts:312). | Reconcile the latest run on project open (list events / run status query). |
| UI-67 | spec | Every node redefines `--accent` to its type color, so the selection ring, running border, progress bar, and compatible-port highlight render in amber/violet instead of Signal Teal, and status is carried by a type color (WorkflowFlowNode.tsx:62; verified live). | Selection/focus/status consume `--accent` (teal); type colors only the port gem and the 3 px type bar. |
| UI-68 | spec | Result region is 16:10 with no ≥176 px minimum (nodeStyles.css:152). | Frozen 16:9 frame with the 176 px floor. |
| UI-69 | spec | Generation nodes render an empty result region pre-run; the `Run this step to create an image/video.` instructional state does not exist (verified live). | Add the instructional empty state per Node Presentation. |
| UI-70 | spec | The video play triangle renders with `preview.url === null` (WorkflowFlowNode.tsx:262; verified live). | Play affordance only with a preview URI. |
| UI-71 | spec | Edge accessible names expose raw node UUIDs (React Flow default aria label; verified live). | Custom edge labels: `{source label} → {target label}`. |
| UI-72 | spec | Settings Models rows display raw profile refs (`image.high_quality_general@1`) as visible sub-lines (SettingsDialog.tsx:205; verified live). | Display name primary; refs only in diagnostics. |
| UI-73 | spec | Frozen sizes unmet: port gem 11 px with 2.5 px border vs 10 px; Node Library 244 px vs 304 px; right panel 292 px vs 380 px drawer (nodeStyles.css:277, styles.css:55). | Converge in A2/A4. |
| UI-74 | spec | Banned treatments live: glass blur + raw `#fff` on the Asset badge (assetCard.css:51-53); shadow stacks on selected/running nodes (nodeStyles.css:25,29); oversized radii (assistantDock 12/14 px, assetCard 7 px, pills 5 px). | Remove blur, collapse shadows, enforce the 4 px scale. |
| UI-75 | spec | Token bypass: raw rgba/hex across ~20 layer-3 spots and inside layer 2; MiniMap inline raw hexes (WorkflowCanvas.tsx:121); no `.react-flow__selection` dark override; no `color-scheme` for scrollbars. | Tokenize; theme the selection rect; declare `color-scheme: dark`. |
| UI-76 | spec | Contrast below WCAG AA: `--err` text 3.9–4.4:1 at 9.5–11 px; assistant approval white-on-teal 4.0:1; video kind badge 4.0:1; `--ink-faint` borderline. | Raise text sizes or adjust tokens; recompute pairs. |
| UI-77 | spec | Raw jargon as primary copy: `request_kind` as the step-details title, `failure.code` as the primary failure line, `Profile reference` diagnostics label (RunDrawer); raw `tool_id` in mono (AssistantDock.tsx:304); raw plan itemId/state enums (StrongAssistantTask); raw MIME/bytes/contentState (AssetDetail.tsx:39); `type_id`/`mode` tokens in the Mode select and node title suffix. | Creator-language labels everywhere; raw values only inside copyable diagnostics. |
| UI-78 | spec | Truncation gaps: the Top bar project name has no `min-width: 0`/ellipsis (topbar.css:53); filter chips and `Run all` can wrap (library.css:69, topbar.css:134). | Apply the shared truncation system. |
| UI-79 | hygiene | Dead surface: `runUndo`/`runRedo` shells, unused `onWorkflowHead`, test-only `paramsForMode`/`nodesByCategory`, `useAssistantAvailability` stub, `Background color="transparent"` dead prop. | Delete or wire; part of A9. |
| UI-80 | hygiene | Silent catches hide failures (useNodePresentation.ts:55, AssistantDock.tsx:79). | Named handling with a log or a surfaced state. |
| UI-81 | perf | Autosave replaces the entire node/edge arrays; the Asset presentation effect recreates all data objects on every refresh (useProjectWorkspace.ts:112, App.tsx:224). | Structural sharing on unchanged entries. |
| UI-82 | hygiene | Mock module stores never reset; run `setTimeout` chains are uncancellable — cross-test and HMR leakage (mockApi.ts:48, mockAssets.ts:12). | Reset hooks per test/project; cancellable timers. |

### Watch list

- **UI-15** (carried): node bodies inline every parameter until the Configure tab exists (A4).
- **Assistant dock toggled during synthetic clicks**: the dock opened and closed once each during
  scripted pointer events without explicit intent; not reproducible deterministically and likely a
  tooling artifact — verify by hand before filing.
- **UI-17** is closed into UI-36 (poster rendering confirmed broken beyond the mock concern).

### Carried gated items (from the retired register)

UI-9 → G1 (engine-owned connection findings), UI-24 → G4 (undo/redo), UI-25 → G2 (node instance
labels), UI-32 → G3 (Assistant durable history; keep-alive interim landed), UI-12 → G5 (Asset
Export command; button removed).

The rest of UI-1…UI-33 landed in W1–W8 and are verified by the test suite and the walkthrough
above.

## Design decisions (resolved 2026-07-18)

- **Geometry**: layered radius scale (6/8/10/12 px + pill) replaces the uniform 4 px.
- **Buttons**: one solid teal primary per surface; ghost secondaries; coral-ghost danger;
  disabled primaries lose their fill.
- **Assistant**: graphite user bubbles, no avatars, ghost suggestions and send.
- **D1 Node Library**: restore the spec's seven visible labels with the Assets group; the Asset
  Library drag-route stays as an additional path.
- **D2 MiniMap**: lower-left with the zoom/fit cluster, per spec.
- **D3 Run history and multiple Workflows**: admitted into scope; design first (B8/B9), no
  implementation before the sections are frozen in DESKTOP_UI.md.
- **D4 Settings**: converge to the spec's Providers section; the Models-routes section's fate is
  decided in that work (fold or keep as a Models section).
- **D5 Labels**: the sentence-case Labels-and-copy system in DESKTOP_UI.md governs every surface.

## Design-authorization gates

No code before the named authority exists.

| Gate | Blocks | Needed authority |
| --- | --- | --- |
| G1 | Engine-owned connection findings at connect time (UI-9, A8) | A contract-findings surface usable at connect time |
| G2 | Node instance labels (UI-25) | A Workflow node-label field |
| G3 | Assistant durable history (UI-32) | A history query command |
| G4 | Undo/redo (UI-24) | A frozen history design keyed to canonical mutation receipts |
| G5 | Asset Export (UI-12, B7) | A backend export command, or confirmation Export stays removed |
| G6 | Settings Skills list / Developer mode (B3) | Backend skill-management commands |
| G7 | Assistant configuration surface (B2) | A config query/command pair (the legacy one was removed) |
| G8 | Scoped readiness for `Run to here` (UI-37) | A node-scoped readiness query |
| G9 | Video preview rendering (UI-36) | A preview-kind declaration: poster image vs playable video file |

## Verification policy

- Every fix lands with focused tests green: `npm run typecheck --prefix ui` and
  `npm run test --prefix ui`.
- Visual or interaction fixes add a live browser pass at 1440x900 (1280x720 and 1920x1080 for
  layout-sensitive changes), zero console errors.
- This register is re-audited after each batch lands; entries close only with evidence
  (test, screenshot, or walkthrough note).
