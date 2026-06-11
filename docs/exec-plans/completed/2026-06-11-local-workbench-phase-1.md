<!-- harness-repo-bootstrap:managed -->
# Execution Plan: Local Workbench Phase 1

## Goal

Shift the current web-prototype surfaces toward a local-first desktop creative workbench while keeping the implementation scoped to Phase 1 UI, validation, and near-term data-boundary improvements.

## Scope

- Included: home workbench IA, project library affordances, canvas chrome status, workspace status modal language and backup safety, local storage schema normalization, and route-level dynamic imports.
- Excluded: full IndexedDB or desktop file-system persistence, provider capability registry migration, task cancellation/resume backend semantics, and Tauri shell implementation.

## Constraints

- Preserve existing local project data and seed behavior for existing users.
- Do not export provider API keys in workspace backups.
- Keep primary workflows usable in browser fallback while making local/desktop boundaries explicit in the UI and docs.
- Follow the product UI register: restrained, dense, and task-first.

## Steps

1. [completed] Re-read harness, product, frontend, reliability, security, and existing implementation context.
2. [completed] Reshape home, project library, canvas chrome, and workspace status surfaces around local workbench tasks.
3. [completed] Add first-pass workspace schema normalization and desktop boundary interfaces.
4. [completed] Add route-level dynamic imports and run build validation.
5. [completed] Validate in browser, update durable docs, run harness check, and close the plan.

## Validation

- Run `npm run build`.
- Open the app in the browser and inspect home, project, and canvas flows at desktop size.
- Run `python3 .codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check --repo .`.

## Durable Knowledge To Capture

- Current browser persistence is a transitional adapter with schema normalization, not the final local storage architecture.
- Workspace backups intentionally omit provider API keys.
- Templates and mock/demo assets are starter/demo layers, not production project data.

## Completion Notes

- Home now opens as a local workbench with recent work, workspace health, task summary, configuration health, backup/import, and secondary starter templates.
- Project library now supports search, sorting, backup/import access, workspace path metadata, and richer project cards.
- Canvas chrome now exposes workspace path, autosave state, and task state; the navigator includes a task queue tab.
- Browser local persistence now normalizes projects, UI, config, and legacy task statuses against schema v1.
- Added local capability port contracts and a browser adapter for project, asset, secret, and task boundaries.
- Route-level lazy imports split the canvas route; `npm run build` completed without the previous 500 KB chunk warning.
- Browser validation covered home, project library, canvas reload persistence, and task queue status migration at `http://127.0.0.1:5174/`.
