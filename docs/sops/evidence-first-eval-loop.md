<!-- harness-engine:managed -->
# SOP: Evidence-First Eval Loop

1. Convert product requirements into explicit product contract checks before scoring.
2. Run deterministic validation first: tests, API smoke checks, CLI checks, browser actions, and state assertions.
3. Read the Issue Workflows in `AGENTS.md` and the domain docs named there before judging or fixing.
4. For frontend work, capture browser evidence: screenshots, DOM/accessibility snapshots, responsive checks, and layout invariants.
5. For backend, architecture, data, security, and performance work, capture the domain evidence named in `AGENTS.md`.
6. Log every discovered bug or evidence gap with `defect-log` before running `quality-score`.
7. Resolve defects only after fixes have passing evidence, then rerun validation and `quality-score`.
8. Report per-case results, failed assertions, artifact paths, and recommended next actions to the user.
