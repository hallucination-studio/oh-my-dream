<!-- harness-repo-bootstrap:managed -->
# AGENTS

Read this file first, then follow the linked docs.

## Routing

- Read `ARCHITECTURE.md` before changing boundaries, data flow, or integrations.
- Read `docs/PLANS.md` before starting multi-step execution work.
- Read `docs/exec-plans/active/` before resuming in-flight work, and create a plan there for new multi-step work.
- Read `docs/QUALITY_SCORE.md` before evaluating tradeoffs or readiness.
- Read `docs/RELIABILITY.md` for runtime validation and failure handling.
- Read `docs/SECURITY.md` before touching auth, secrets, or sensitive data.
- Read `docs/FRONTEND.md` for UI or terminal interface changes.
- Read the matching file in `docs/sops/` before architecture changes, UI validation, observability work, or knowledge capture.

## Repository Focus

- Project: oh-my-dream
- Domain: Local-first creative production software for AI-assisted video workflows.
- Primary outcome: A local-first creative client for AI-driven video work that keeps ideation, shot planning, assets, prompts, generation, and revision inside one coherent workspace.
- Main users: Independent creators and studio teams using a local client like a professional editing tool to plan and iterate on AI video projects.

## Operating Rules

- Keep durable decisions in repo docs, not only in chat.
- Keep active plans in `docs/exec-plans/active/`.
- Move completed plans to `docs/exec-plans/completed/`.
- Update plans during the work, not only at the end.
- Encode durable facts learned during execution into permanent docs before closing the task.
- Before handoff, run the local harness check: `python3 .codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check --repo .`.
- Keep generated artifacts in `docs/generated/`.
- Keep external references in `docs/references/`.
