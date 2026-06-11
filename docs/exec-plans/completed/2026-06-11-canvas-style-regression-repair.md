<!-- harness-repo-bootstrap:managed -->
# Execution Plan: Canvas Style Regression Repair

## Goal

Repair the canvas visibility and chrome alignment regressions introduced by the layered CSS refactor while preserving the new style-system structure.

## Scope

Included: React Flow sizing override priority, canvas topbar centering, bottom floating toolbar/control alignment, drawer placement, browser validation, and durable frontend guidance.

Excluded: provider behavior, storage seed/reset policy, generation data flow, and local workspace persistence semantics.

## Constraints

- Do not restore `src/styles.css`.
- Do not reset or reseed localStorage to hide the issue.
- Preserve the layered style architecture and keep CSS files under 700 lines.
- Use the selected two-floating-group canvas chrome model.

## Steps

1. Completed: created this active execution plan.
2. Completed: moved React Flow sizing overrides into a higher-priority style entry.
3. Completed: reworked canvas chrome positioning around shared canvas left/right/center variables.
4. Completed: aligned bottom toolbar, canvas controls, and drawer placement into two related floating groups.
5. Completed: validated build, browser geometry, responsive behavior, and harness checks.
6. Completed: captured the third-party CSS layering rule in durable docs.

## Validation

- `npm run build`: passed.
- Browser smoke checks for home and `/canvas/reference-local`: passed.
- `.canvas-page .react-flow` has nonzero viewport-height geometry after overriding React Flow inline sizing.
- Inspector, Queue, Review, drawer, navigator collapse, and narrow viewport were checked.
- `python3 .codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check --repo .`: passed with 0 issues.

## Quality Gate

Status: pass
Minimum score: 8.0
Average score: 8.2
Last scored: 2026-06-11T06:58:58Z

| Dimension | Score | Notes |
| --- | ---: | --- |
| Product correctness | 8.4 | Canvas content visibility is restored without resetting local data; the two-floating-group chrome model is preserved. |
| UX and operator clarity | 8.2 | React Flow no longer collapses; mobile topbar and bottom tools are constrained to the canvas area so they do not cover the workbench. |
| Architecture and maintainability | 8.5 | React Flow inline and unlayered CSS are handled in a dedicated override file while preserving the layered style system. |
| Reliability and observability | 8.0 | Build passes and browser geometry checks verified nonzero React Flow dimensions plus drawer/workbench coexistence. |
| Security and data handling | 8.0 | No provider, API key, storage, or local data semantics changed. |
## Durable Knowledge To Capture

- Third-party unlayered CSS can outrank app cascade layers; React Flow sizing overrides need a deliberate post-import override path.

## Completion Notes

Canvas visibility was restored by adding a dedicated `react-flow-overrides.css` entry outside cascade layers with narrowly scoped `!important` rules for the React Flow root inline sizing. Canvas chrome now uses tighter left/right panel widths, a centered topbar for the available canvas area, and mobile rules that keep topbar and floating tools out of the right workbench.

## Rework Required

None. Quality gate passed.
