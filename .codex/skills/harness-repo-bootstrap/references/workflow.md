# Workflow

Use this skill in two passes.

## Pass 1: Analyze and Confirm

Run `analyze` before editing repository docs.

Ask the human only about facts that cannot be derived safely from the repo, especially:

- product domain and top-level outcomes
- intended users or operators
- production reliability expectations
- security or compliance constraints
- frontend experience bar
- canonical external references worth pinning inside `docs/references/`

Do not ask for facts that can be inferred from source layout, dependency manifests, or existing docs.

Also inspect the analysis for:

- missing durable knowledge that should be written during the task
- missing execution-plan state
- which SOPs should be referenced in the generated router docs

## Pass 2: Scaffold or Refresh

Run `sample-answers`, fill the answers, then run `init` or `update`.

Use `init` for first-time adoption.
Use `update` to add missing managed files or refresh managed files when `--refresh-managed` is passed.

After the script runs, read the generated docs once and tighten weak generic phrases before handing off.

## Ongoing Use

After the scaffold exists:

- create an execution plan before multi-step work
- use `plan-start` instead of creating plan files manually when possible
- log durable facts during execution instead of waiting until the end
- follow the matching SOP for architecture, UI, observability, or knowledge capture work
- encode durable knowledge back into the repository before closing the task
- mark logged knowledge items as written after updating the permanent docs
- use `plan-close` to verify no durable knowledge is left stranded in the active plan
- run `.codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check --repo <target-repo>` before handoff
- do not add CI to the target repository unless the human explicitly asks for it
