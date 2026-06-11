# Execution Plans

Execution plans are required for multi-step work, risky changes, or tasks that need coordination across files.

## When To Create One

- more than one implementation step is required
- validation is non-trivial
- architecture, product, reliability, or security decisions are involved
- work will span enough time that another agent may resume it later

## Location

- Active: `docs/exec-plans/active/`
- Completed: `docs/exec-plans/completed/`

## Minimum Sections

- goal
- scope
- constraints
- steps
- validation
- durable knowledge to capture
- completion notes

## Operating Rule

Update the active plan during the work. When the work is done, move it to `completed` and leave behind any durable facts in the right permanent docs.

## Closed Loop

Use the script, not ad hoc manual edits, for the lifecycle:

- `plan-start`: create a new active execution plan
- `knowledge-log`: append a durable fact that still needs to be written into permanent docs and return its stable id
- `knowledge-mark-written`: verify and mark a logged fact as written into its permanent doc; prefer `--id <knowledge-id> --evidence "<doc text>"`, and use `--append` only to append the exact fact first
- `plan-close`: refuse to close cleanly until the listed knowledge items are marked as written to durable docs
- `check`: run a local handoff check without requiring target-repo CI
