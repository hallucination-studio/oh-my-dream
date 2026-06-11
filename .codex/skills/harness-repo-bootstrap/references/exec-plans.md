# Execution Plans

Execution plans are required for multi-step work, risky changes, or tasks that need coordination across files.

## When To Create One

- more than one implementation step is required
- validation is non-trivial
- architecture, product, reliability, or security decisions are involved
- work will span enough time that another agent may resume it later

## Location

- Workstream recovery ledger: `docs/exec-plans/workstreams.md`
- Active: `docs/exec-plans/active/`
- Completed: `docs/exec-plans/completed/`

## Minimum Sections

- goal
- scope
- constraints
- steps
- validation
- quality gate
- defects to resolve
- rework required
- phase continuity
- durable knowledge to capture
- completion notes

## Operating Rule

Update the active plan during the work. When the work is done, score it, complete any required rework, record phase continuity for resumable work, move it to `completed`, and leave behind any durable facts in the right permanent docs.

## Closed Loop

Use the script, not ad hoc manual edits, for the lifecycle:

- `plan-start`: create a new active execution plan
- `knowledge-log`: append a durable fact that still needs to be written into permanent docs and return its stable id; use `--fact-file` for shell-sensitive facts
- `knowledge-mark-written`: verify and mark a logged fact as written into its permanent doc; prefer `--id <knowledge-id> --evidence-file <file>` for shell-sensitive evidence, and use `--append` only to append the exact fact first
- `defect-log`: record a bug found by validation, evals, browser testing, or code review; this forces the quality gate to fail and makes the defect the next rework input
- `defect-resolve`: mark a logged defect fixed with validation or code evidence; re-run validation and `quality-score` before closing
- `quality-score`: write a scored quality gate into the plan; if it fails, the generated `## Rework Required` section becomes the next implementation input
- `phase-set`: declare whether phased or resumable work continues, pauses, stops, or completes
- `workstream-upsert`: update `docs/exec-plans/workstreams.md` so interrupted work can be recovered without chat history
- `plan-close`: refuse to close cleanly until the quality gate passes, phase continuity is recorded, and the listed knowledge items are marked as written to durable docs
- `check`: run a local handoff check without requiring target-repo CI
