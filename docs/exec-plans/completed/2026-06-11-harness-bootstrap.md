# Execution Plan: Harness Bootstrap

## Goal

Bootstrap and tailor the repository harness docs for the local-first AI video client.

## Scope

- In scope: generate the harness scaffold, tailor routing and policy docs to this local-first AI video client, and establish durable plan/SOP state for future work.
- Out of scope: application code changes, persistence rewrites, packaging decisions, or new product features.

## Constraints

- Preserve the generated harness structure while replacing generic guidance with repo-specific operating rules.
- Keep the product framed as a pure local client with desktop-style usage expectations.
- Emphasize local persistence, crash recovery, offline behavior, and non-leakage of user data and tokens.

## Steps

1. Analyze the repository and confirm the missing product, reliability, security, and quality-priority facts with the user.
2. Generate the harness scaffold and inspect the written files.
3. Tighten generic docs so they reflect the repo's local-first creative workflow and policy expectations.
4. Record durable facts in permanent docs, mark them written, and run the harness handoff check.

## Validation

- Verify the scaffold is created in the expected managed locations.
- Review the key routing and policy docs to ensure placeholder text is removed where repo-specific facts are known.
- Run `python3 .codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check --repo .` before handoff.

## Durable Knowledge To Capture

- [x] [id:hk-9d37d97f21] The product is a pure local client used like desktop creative software rather than a hosted web app. -> ARCHITECTURE.md | evidence: Deployment targets: Pure local client usage. The product should behave like a desktop creative application, with primary workflows running on the user's machine rather than depending on a hosted web app.
- [x] [id:hk-14ad1ff9b4] Reliability priorities are local data persistence, crash recovery, and graceful offline behavior for project and canvas work. -> docs/RELIABILITY.md | evidence: Local data persistence, crash recovery, and offline behavior are all critical.
- [x] [id:hk-b4fe6386bb] Security priorities require user project data to remain local and API tokens to stay under user control without leaking. -> docs/SECURITY.md | evidence: User project data must remain local and must not leak. API tokens and similar secrets require strict handling, must stay under user control
- [x] [id:hk-65af42f739] Quality priorities rank canvas workflow and canvas state management first, project management second, and other features third. -> docs/QUALITY_SCORE.md | evidence: First priority: canvas workflow and canvas state management. Second priority: project management and navigation around the canvas. Third priority: all remaining supporting features and integrations.
## Completion Notes

Bootstrapped the harness scaffold, tailored the durable docs to the local-first AI video client, synchronized learned facts into permanent docs, and verified the repo with the harness check.
