<!-- harness-repo-bootstrap:managed -->
# Execution Plan: Layered Style System

## Goal

Replace the 5004-line `src/styles.css` override stack with a layered, maintainable CSS system that supports a restrained professional creative workbench inspired by Liblib's density and media-first workflow without copying its cloud/community skin.

## Scope

Included: CSS file structure, import entrypoint, design tokens, shared component styles, Home/Project/Config surfaces, Canvas shell/navigator/workbench/drawer/node surfaces, dead style cleanup, visual validation, and durable frontend guidance.

Excluded: provider APIs, storage behavior, generation behavior, React Flow workflow state, and broad TSX class renaming unless required to remove dead style coupling.

## Constraints

- Delete `src/styles.css`; do not keep a compatibility shim.
- Use CSS cascade layers through `src/styles/index.css`.
- Keep each CSS file under 700 lines.
- Keep current class names where practical to reduce behavior risk.
- Canvas clarity, density, and local-first trust cues take priority over decorative styling.

## Steps

1. Completed: created the layered CSS directory and `index.css` import graph.
2. Completed: rebuilt tokens, reset, base, layout, shared components, and surface styles.
3. Completed: updated `src/main.tsx` to import the new style entrypoint and deleted `src/styles.css`.
4. Completed: validated build, browser routes, canvas interactions, and style quality checks.
5. Completed: captured durable style-system guidance in repo docs.

## Validation

- `npm run build`: passed during implementation, rerun before handoff.
- Browser smoke checks: home, project library, canvas, selected Inspector, Queue, Review, add-node drawer, `/config`, and narrow `390x844` surfaces validated.
- Style checks: `src/styles.css` deleted; largest CSS file is under 700 lines; old dead selectors and decorative gradient/glass patterns removed from `src/styles/`.
- Harness check: passed with 0 issues.

## Quality Gate

Status: pass
Minimum score: 8.0
Average score: 8.4
Last scored: 2026-06-11T06:35:17Z

| Dimension | Score | Notes |
| --- | ---: | --- |
| Product correctness | 8.4 | Style refactor preserves local-first creative workbench direction and keeps Liblib as density/workflow reference rather than community skin. |
| UX and operator clarity | 8.5 | Browser smoke covered home, project, canvas inspector/queue/review, add-node drawer, config, and narrow viewport; visual chrome is calmer and denser. |
| Architecture and maintainability | 8.8 | Monolithic CSS removed; cascade layers and surface/component boundaries reduce override risk and keep files under 700 lines. |
| Reliability and observability | 8.1 | No provider/storage behavior changed; build and browser smoke validate UI paths, with harness check still run separately. |
| Security and data handling | 8.0 | No API key, provider, or storage semantics changed; config remains local-first. |
## Durable Knowledge To Capture

- New style work must use cascade layers and the style directory structure.
- Canvas surface styles should stay split by shell, navigator, workbench, drawer, and node responsibility.
- Avoid decorative gradients, glassmorphism, and large blur shadows in product chrome.

## Completion Notes

The monolithic stylesheet was replaced by a cascade-layered system under `src/styles/`. The UI now uses restrained product tokens, shared controls, and page/canvas surface boundaries. Canvas chrome is split by shell, navigator, workbench, drawer, and node responsibilities so future polish can land without late override stacks.

## Rework Required

None. Quality gate passed.
