# Evaluation Loop

Use this loop when changing the skill, templates, scripts, or policy references:

1. Draft the behavior in `SKILL.md`, `references/`, templates, or scripts.
2. Test it with the deterministic commands in `scripts/manage_harness.py`.
3. Evaluate it with `python3 evals/run_evals.py`.
4. Read the structured `harness-eval-report.v1` output: aggregate metrics, per-case results,
   findings, user message, and recommended actions.
5. Iterate until the runner passes, the score stays at 100, and failed-case output would be
   actionable for a user if the eval regressed.

## What The Evals Cover

- first-time initialization of an empty repository
- frontend-aware repository analysis
- execution-plan and knowledge-capture closure
- quality gates that block closure and force rework when scores fail
- phase continuity and workstream recovery for resumable work
- structured eval report output with per-case findings and recommended actions
- preservation of unmanaged user-owned docs
- local harness checks that do not require user-project CI

Add a new eval case whenever a regression would be easy to miss by reading the files manually.
