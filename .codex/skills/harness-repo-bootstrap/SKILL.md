---
name: harness-repo-bootstrap
description: Bootstrap or refresh an advanced harness-engineering repository shape for Codex-driven projects. Use when Codex needs to analyze a repository, ask the human to confirm high-impact product and architecture facts, and then create or update AGENTS.md, architecture docs, policy docs, plan folders, reference folders, and SOP-backed starter files for the repository.
---

# Harness Repo Bootstrap

Run the packaged script to inspect the target repository before editing files. Use the generated analysis to decide what to ask the human, what durable knowledge is missing from the repo, and which execution-plan and SOP files must be created or updated.

## Workflow

1. Run `python3 scripts/manage_harness.py analyze --repo <target-repo> --output <analysis.json>`.
2. Read `analysis.json`.
3. Ask the human only the unresolved, high-impact questions from `human_confirmations`.
4. Run `python3 scripts/manage_harness.py sample-answers --analysis <analysis.json> --output <answers.json>`.
5. Fill the placeholders in `answers.json` from the repository and the human's confirmed answers.
6. Run one of:
   - `python3 scripts/manage_harness.py init --repo <target-repo> --answers <answers.json>`
   - `python3 scripts/manage_harness.py update --repo <target-repo> --answers <answers.json>`
7. If the task is multi-step, run `python3 scripts/manage_harness.py plan-start --repo <target-repo> --slug <task-name> --goal "<goal>"`.
8. If you learn durable facts during the work, run `python3 scripts/manage_harness.py knowledge-log --repo <target-repo> --plan <plan-file> --fact "<fact>" --destination <durable-doc>` and keep the returned `id`.
9. Before closing the task, write those facts into their durable docs.
10. Run `python3 scripts/manage_harness.py knowledge-mark-written --repo <target-repo> --plan <plan-file> --id <knowledge-id> --evidence "<text already in durable doc>"`; use `--append` only when the exact fact should be appended mechanically.
11. Close the plan with `python3 scripts/manage_harness.py plan-close --repo <target-repo> --plan <plan-file> --summary "<summary>"`.
12. Before handoff, run `python3 .codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check --repo <target-repo>` from an installed target repository.
13. After changing this skill, run `python3 evals/run_evals.py` and iterate until it passes.

## Reading Order

- Read [references/workflow.md](references/workflow.md) first for the operating model and question policy.
- Read [references/file-map.md](references/file-map.md) when deciding which generated file to update.
- Read [references/question-catalog.md](references/question-catalog.md) when the analysis surfaces ambiguous product, security, reliability, or frontend facts.
- Read [references/knowledge-capture.md](references/knowledge-capture.md) when you discover facts that should survive chat history.
- Read [references/exec-plans.md](references/exec-plans.md) before planning or updating any multi-step work.
- Read [references/sop-index.md](references/sop-index.md) to choose the right SOP for architecture, UI validation, observability, or knowledge capture work.
- Read [references/template-policy.md](references/template-policy.md) before overwriting existing files.
- Read [references/evaluation-loop.md](references/evaluation-loop.md) before changing the skill, templates, scripts, or policy references.

## Command Rules

- Prefer `analyze` before `init` or `update`.
- Prefer the draft, test, evaluate, iterate loop for changes to this skill.
- Prefer `init` when the target repo has none of the managed files.
- Prefer `update` when the repo already contains any managed file or a partial harness layout.
- Do not overwrite existing files unless the human asked for it or you pass `--force`.
- Treat the generated files as starting points. After generation, tighten them with repository-specific details instead of leaving placeholders behind.
- Treat `docs/exec-plans/` as required state for multi-step work, not optional notes.
- Treat `docs/sops/` as mechanical operating procedures, not background reading.
- When you answer a question using facts that are not yet in the repo but should be reusable, write them into a durable doc before finishing.
- Prefer `knowledge-mark-written --id ... --evidence ...` so durable docs can use natural wording instead of duplicated exact fact strings.
- Use `plan-close` as the final guardrail so plan state and durable docs stay synchronized.
- Use `check` as the local handoff guardrail for user repositories.
- Run `python3 evals/run_evals.py` after skill changes and treat failures as iteration input.
- Do not add CI to user repositories unless the human explicitly asks for it.

## Output Rules

- Keep `AGENTS.md` short and routing-oriented.
- Keep durable knowledge in repo docs, not in chat-only explanations.
- Keep plans under `docs/exec-plans/active/` and move finished plans to `docs/exec-plans/completed/`.
- Keep generated material under `docs/generated/`.
- Keep external, model-friendly references under `docs/references/`.
- Keep SOPs explicit and task-triggered so the next agent can follow the same path mechanically.

## Assets

- Scaffold templates live under [assets/repo-template](assets/repo-template).
- SOP starter docs live under [assets/sops](assets/sops).
