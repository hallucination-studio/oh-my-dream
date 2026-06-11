<!-- harness-repo-bootstrap:managed -->
# Execution Plan: Home And Canvas UI Polish

## Goal

Make the home workbench and canvas workspace feel closer to a precise local creative tool: remove meaningless first-row chrome, improve settings affordances, reduce oversized workspace text, and refine canvas node/frame density.

## Scope

Included: app shell/topbar, home workbench layout, canvas topbar/left controls, navigator density, node typography and media frame styling, and local UI validation.

Excluded: provider integrations, persistence behavior, generation logic, and major information architecture changes beyond the affected UI surfaces.

## Constraints

- Preserve the local-first workbench positioning from `docs/FRONTEND.md`.
- Keep settings and configuration reachable, but make them secondary and visually calm.
- Keep canvas controls keyboard-reachable with labels/tooltips.
- Avoid changing data flow, storage, or provider boundaries.

## Steps

1. Completed: inspected current home/canvas implementation and liblib reference density.
2. Completed: updated UI code and CSS for tighter home and canvas presentation.
3. Completed: validated with build and browser screenshots at desktop width.
4. Completed: captured durable frontend findings in `docs/FRONTEND.md`.

## Validation

- Run `npm run build`.
- Run local Vite server and inspect home plus canvas with Chrome DevTools.
- Run harness check: `python3 .codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check --repo .`.

## Durable Knowledge To Capture

- Record compact canvas density and home workbench guidance in `docs/FRONTEND.md` or `docs/design-docs/`.

## Completion Notes

Removed the meaningless centered project entry from the app shell, refined settings affordances, tightened the home workbench, reduced default node sizes, and compacted canvas chrome, navigator rows, toolbars, and node controls. Browser validation screenshots were saved in `docs/generated/`.
