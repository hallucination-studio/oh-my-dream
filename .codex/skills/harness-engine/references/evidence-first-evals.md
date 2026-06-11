# Evidence-First Evals

Use this reference when a task needs stronger validation than an LLM-written quality estimate.
The quality score is the final readiness summary, not the eval itself.

## Core Rule

Every eval must separate four layers:

1. **Product contract checks**: machine-readable assertions derived from `product.md`,
   product specs, acceptance criteria, or the user's prompt.
2. **Runtime behavior checks**: tests, API smoke checks, CLI checks, browser interactions,
   and state assertions that prove the implementation works.
3. **Visual and UX evidence**: screenshots, DOM/accessibility snapshots, responsive viewport
   checks, and layout invariants for user-facing surfaces.
4. **Reviewer judgment**: LLM or human scoring only after the first three layers have produced
   evidence and logged defects.

If a requirement cannot be checked directly, write down why and replace it with the narrowest
observable proxy. Do not silently convert it into a vague score.

## Eval Case Shape

Model each case like an OpenAI eval sample: stable id, input, expected behavior, recorded events,
and aggregate metrics.

Recommended fields:

- `id`: stable case id, versioned when the case changes materially.
- `source`: product spec, user request, bug report, design file, or regression source.
- `risk`: what failure this case is meant to catch.
- `setup`: fixtures, seed data, feature flags, viewport, network state, or browser route.
- `actions`: exact commands, API calls, browser actions, or user flows.
- `assertions`: deterministic checks that must pass.
- `artifacts`: logs, screenshots, traces, DOM snapshots, accessibility snapshots, or diffs.
- `defect_policy`: severity and `defect-log` summary to use if the case fails.
- `metrics`: pass/fail fields and numeric measurements to aggregate.

Do not accept an eval case whose only assertion is "LLM rates this highly".

## Product Contract Checks

Before implementation, extract product requirements into a checklist that can be tested:

- required capabilities and forbidden capabilities
- key user workflows and edge cases
- copy, information architecture, and domain terminology that must appear
- persistence, permissions, latency, error handling, and empty states
- explicit non-goals such as "do not add CI" or "do not introduce auth"

For every product claim in the final answer, there should be a matching command, test, browser
assertion, artifact, or explicitly documented limitation.

## Domain Issue Workflows

Issue triage should be domain-routed before implementation. The generated `AGENTS.md` owns the
current routing table; use it to decide which durable docs and SOPs to read first.

Minimum expectations by domain:

- Product contract: convert requirements, specs, and acceptance criteria into assertions.
- Frontend/UI: capture browser or local-runtime evidence for the affected workflow and viewport.
- Backend/runtime: reproduce the behavior narrowly and verify with tests, API smoke checks, logs,
  or integration evidence.
- Architecture: document boundary, dependency, data-flow, migration, and compatibility impact.
- Data/state: verify fixtures, migrations, rollback or compatibility behavior, and data-loss risk.
- Security/privacy: review sensitive data paths, permissions, auth boundaries, and secret handling.
- Performance/reliability: collect baseline measurement, repeatable benchmark or smoke evidence,
  and before/after comparison.

Confirmed defects or evidence gaps should be logged into the active plan before quality scoring.
Each `quality-score` dimension must include a concrete evidence note. A numeric score without
evidence is not a valid readiness signal.

Use exact evidence when closing knowledge items: the text passed to `knowledge-mark-written`
must already appear in the durable destination doc. If the destination uses different wording,
copy a short phrase from that destination into an evidence file and pass `--evidence-file`.

## Frontend Checks

For frontend work, use browser evidence instead of relying on a screenshot glance:

- Open the live route in a browser, not only static file inspection.
- Capture at least one desktop and one mobile viewport for meaningful UI changes.
- Assert important text, controls, selected state, loading state, empty state, error state,
  and primary interaction outcomes from the DOM or accessibility tree.
- Check layout invariants: no critical overlap, no clipped primary text, stable toolbar/grid
  dimensions, usable tap targets, and visible focus/selected states.
- For canvas/WebGL/game UIs, add pixel or scene-state checks so a blank canvas cannot pass.
- Save screenshots or snapshot paths in the plan or `docs/generated/` when visual evidence
  matters for later review.

If the browser tool is unavailable, record the limitation as validation evidence and replace it
with the strongest available fallback: static DOM checks, component tests, image snapshots, or
API smoke checks. Do not mark UX as fully validated without saying what was missing.

## Frontend Issue Reports

Frontend feedback is an eval trigger even when the harness skill was not explicitly invoked.
Handle any UI, layout, interaction, responsive behavior, visual state, canvas, or design fidelity
question through the repository's frontend workflow.

The correct response is:

- read `docs/FRONTEND.md`, `docs/DESIGN.md`, and the relevant SOP
- inspect the affected route, component, viewport, and user workflow
- reproduce the behavior with browser or local-runtime evidence when possible
- turn the finding into product/UX assertions or a regression case
- log confirmed defects or missing evidence in the active plan
- fix and validate against the same workflow before claiming the UI is acceptable

Do not answer from memory or aesthetic judgment alone when the question is about a concrete
frontend behavior.

## Bug Discovery Evals

Add regression cases for failures that were previously missed.

A good bug-discovery eval proves two things:

- the bad implementation fails a narrow test or observable assertion
- the harness blocks closure through `defect-log`, `quality-score`, `plan-close`, and `check`

Track missed-bug classes separately from generic test pass rate. Examples:

- product-spec drift not detected
- browser layout defect not detected
- generated app behavior bug not detected
- unresolved defect allowed through handoff
- missing visual evidence accepted as UX validation

## Metrics

Record sample-level events first, then aggregate.

Useful aggregate metrics:

- `case_pass_rate`: passed cases divided by total cases
- `product_contract_pass_rate`: product assertions passed divided by product assertions
- `visual_evidence_coverage`: frontend cases with required screenshots/snapshots
- `defect_block_rate`: known defects that blocked closure when injected
- `missed_defect_count`: known defects that reached a passing quality gate
- `artifact_completeness`: required logs/screenshots/traces present
- `llm_judge_agreement`: optional reviewer score agreement with labeled cases

Fail release or handoff when a P0/P1 defect is missed, required product assertions are untested,
or frontend evidence is absent for meaningful UI work.

## Report Output

Eval runners should emit structured JSON that can be shown to users and consumed by tools.
Use a stable schema name and include both aggregate and per-case results.

Recommended top-level fields:

- `schema_version`: stable report schema such as `harness-eval-report.v1`.
- `status`: `pass` or `fail`.
- `score`: whole-number aggregate score from `0` to `100`.
- `summary`: passed, failed, total, and one concise message.
- `metrics`: named aggregate metrics, not only one score.
- `case_results`: one object per case with `id`, `description`, `status`, `score`,
  `duration_seconds`, `findings`, and `recommended_actions`.
- `user_message`: direct text the agent can relay to the user.
- `recommended_actions`: deduplicated next actions for failed cases.

Failure output must name the specific failed case, failed assertion or evidence gap, and the next
action. Passing output should still include per-case scores so the user can see what was actually
covered.

## Meta-Eval Calibration

When an LLM judge is used, keep a small labeled meta-eval set:

- examples that should pass
- examples that should fail product correctness
- examples that should fail visual/UX evidence
- examples with open defects that must block handoff

Run the judge against these labels and treat disagreement as an eval bug. The judge may summarize
evidence and suggest risks, but it must not override deterministic failures.
