# Execution Plan: Harness Engine Reconcile

## Goal

Reconcile the repository harness with the installed harness-engine skill and backfill missing managed harness files.

## Scope

- In scope: rerun harness analysis, refresh sample answers, use repo-derived answers for `init`, backfill missing managed harness files, and run the harness handoff check.
- Out of scope: forcing overwrites of existing unmanaged/bootstrap-managed files, changing product architecture, changing app code, or modifying the installed harness-engine skill implementation.

## Constraints

- Preserve existing user-owned and bootstrap-managed docs unless the harness engine can refresh them without `--force`.
- Keep generated analysis and answer artifacts under `docs/generated/`.
- Do not alter local-first product, reliability, frontend, or security policy facts beyond the repo-derived answer file used by the harness script.
- Leave unrelated `.codex/skills/` rename/delete state untouched.

## Steps

1. Completed: ran harness analysis into `docs/generated/harness-analysis.json`.
2. Completed: generated sample answers into `docs/generated/harness-answers.sample.json`.
3. Completed: created repo-specific reconcile answers in `docs/generated/harness-answers.reconcile.json`.
4. Completed: ran `init` with repo-specific answers, backfilling and refreshing `docs/sops/evidence-first-eval-loop.md`.
5. Completed: ran final harness check and closed this plan.

## Validation

- Run `python3 .codex/skills/harness-engine/scripts/manage_harness.py check --repo .`.
- Confirm `init` reports `operation: reconciled` and no missing SOPs remain in the latest analysis.
- Inspect `docs/sops/evidence-first-eval-loop.md` for a concrete SOP with no placeholders.

## Quality Gate

Status: pass
Minimum score: 8.0
Average score: 8.7
Last scored: 2026-06-11T08:37:21Z

| Dimension | Score | Notes |
| --- | ---: | --- |
| Product correctness | 9.0 | Requested harness-engine init path completed; init reported operation reconciled and no missing SOPs remain in latest analysis. |
| UX and operator clarity | 8.5 | Generated SOP and active plan use concrete operating instructions and no task placeholders remain except the default unused durable-knowledge line. |
| Architecture and maintainability | 8.8 | Reconcile used the packaged harness entrypoint and preserved existing unmanaged/bootstrap-managed files without force overwrites. |
| Reliability and observability | 8.6 | Analysis, sample answers, reconcile answers, init output, and check output are retained under docs/generated or the active plan for handoff evidence. |
| Security and data handling | 8.8 | No app code, secrets, provider settings, or sensitive local project data handling were changed; repo-specific answers preserved existing local-first security constraints. |
## Defects To Resolve

None.

## Rework Required

None. Quality gate passed.
## Phase Continuity

Mode: single-phase
Workstream: none
Current phase: none
Next phase: none
Continuation: none
Next action: none
Closure reason: This plan is not part of a longer workstream.
Resume notes: none

## Durable Knowledge To Capture

- [ ] Add durable facts here as they emerge -> <destination-doc>

## Completion Notes

Ran harness-engine init in reconcile mode, refreshed the evidence-first eval SOP, retained generated analysis and answer artifacts, scored the work, and passed the harness check.
