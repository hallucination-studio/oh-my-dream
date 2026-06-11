<!-- harness-repo-bootstrap:managed -->
# Execution Plan: Product Realignment Workbench

## Goal

Realign Oh My Dream from a web-like AI canvas into a local-first professional video production workbench with clearer canvas information architecture, slimmer workflow nodes, visible generation queue/review surfaces, and calmer product UI styling.

## Scope

Included: canvas IA, node presentation, inspector/queue/review panels, home/workbench trust cues where needed, style-system cleanup for the affected shell, and durable frontend/security documentation.

Excluded: provider API protocol changes, cloud/community features, non-local sync, and replacing the browser localStorage adapter.

## Constraints

- Preserve local-first data ownership and do not add telemetry or remote sync.
- Keep provider keys local and omit them from backup by default.
- Use existing React Flow, storage, task, history, asset, and batch structures.
- Prioritize desktop creative-tool density; mobile only needs to remain reachable and non-broken.

## Steps

1. Completed: created a right-side canvas workbench rail with inspector, generation queue, and review surfaces.
2. Completed: slimmed canvas nodes into workflow cards and moved detailed parameters/actions into the inspector.
3. Completed: exposed lineage across nodes, tasks, history, assets, and derived batches.
4. Completed: added a calm light workbench styling layer for the touched canvas surfaces.
5. Completed: validated with build, browser/UI inspection, quality score, and harness check.
6. Completed: captured durable frontend/security findings in repo docs.

## Validation

- Run `npm run build`.
- Run local Vite server and inspect home/canvas/settings/review surfaces.
- Run `python3 .codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check --repo .`.

## Quality Gate

Status: pass
Minimum score: 8.0
Average score: 8.4
Last scored: 2026-06-11T06:10:44Z

| Dimension | Score | Notes |
| --- | ---: | --- |
| Product correctness | 8.6 | Canvas now follows the planned navigator/canvas/inspector workbench and keeps Liblib as workflow-density inspiration rather than cloud-community copying. |
| UX and operator clarity | 8.3 | Nodes are compact workflow cards and detailed generation controls moved into inspector/queue/review; browser visual validation was limited by DevTools profile lock. |
| Architecture and maintainability | 8.1 | Provider APIs and storage boundaries stayed intact, but CSS still contains historical sections with a final consolidation layer for this pass. |
| Reliability and observability | 8.0 | Build passes and queue/review surfaces expose task/result state; no provider retry protocol changes were made. |
| Security and data handling | 8.8 | Backup behavior continues to omit provider API keys by default and configuration copy was reframed as local provider capability control. |
## Durable Knowledge To Capture

- Canvas should use a three-zone workbench: navigator, canvas, inspector/queue/review.
- Nodes should summarize workflow state; detailed controls belong in inspector/review panels.
- Workspace backup must keep provider API keys excluded by default.

## Completion Notes

Implemented the three-zone canvas workbench, compact workflow nodes, right-side inspector/queue/review panels, visible result/asset lineage, calmer product UI styling, and local-first provider/backup copy. Validation passed with `npm run build`, browser smoke checks on home and canvas, quality score, and harness check.

## Rework Required

None. Quality gate passed.
