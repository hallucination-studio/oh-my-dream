<!-- harness-repo-bootstrap:managed -->
# Frontend

## Scope

User-facing or operator-facing frontend work is expected.

## Stack Notes

Detected frameworks: React, Vite. This is a user-facing creative desktop-style interface with a high polish bar. Prioritize calm, precise workflows, WCAG AA basics, keyboard reachability for primary actions, and validation of dense canvas interactions over flashy presentation.

## Validation Loop

- Run local UI changes in a browser.
- Check desktop behavior first because the primary product target is a desktop-style local client. Validate smaller breakpoints when the affected surface is expected to remain usable there.
- Verify key flows, empty states, failure states, and recovery after reload for project list and canvas interactions.
- When changing canvas UI, test selection, panel toggles, navigation, and persistence of the edited state.
- Record reusable UI findings in `docs/design-docs/`.

## Workbench IA Notes

- The home surface should behave like a local desktop workbench: continue work, create/open/import, task overview, provider configuration health, and backup status before template browsing.
- Templates should remain secondary starter material and should not look like the primary project inventory.
- Canvas chrome should expose saved state, workspace path, and task state directly. Avoid inert save/status controls.
- The app shell should not contain a lone centered project navigation button on the home surface. Keep the top row quiet: brand at left, settings/status affordances at right, and place project navigation inside contextual workbench actions such as "打开项目库" or "全部项目".
- Canvas density should follow professional creative-tool conventions: sidebars, toolbars, node titles, parameter chips, and media frames should stay compact enough that multiple nodes remain visible without zooming out heavily. Prefer 11-13px labels, 28-34px controls, and 8-12px radii for dense canvas chrome.
- Canvas IA should use a three-zone workbench: project navigator on the left, React Flow canvas in the center, and inspector/queue/review on the right. Bottom chrome is a shortcut layer, not the primary workspace.
- Nodes should behave like workflow cards: title, media/text summary, key state, input/output counts, and connection handles. Detailed generation parameters, image tools, result review, and lineage controls belong in the inspector/review surfaces.
- Generated work should keep visible lineage across node, task, history, asset, and derived batch records so users can locate sources, compare outputs, and reuse parameters without reconstructing context.

## Style System Notes

- Global app styling is owned by `src/styles/index.css`, which imports cascade layers in this order: `tokens`, `reset`, `base`, `layout`, `components`, `surfaces`, `utilities`.
- New CSS files must wrap rules in the matching `@layer`; do not add unlayered global UI rules or late override sections.
- Shared product primitives belong in `src/styles/components/`, while page and workbench chrome belong in `src/styles/surfaces/`.
- Canvas styling must stay split by responsibility: shell positioning and React Flow chrome in `canvas-shell.css`, left project navigation in `canvas-navigator.css`, inspector/queue/review in `canvas-workbench.css`, bottom shortcuts in `canvas-drawer.css`, and workflow-card nodes in `canvas-node.css`.
- React Flow ships unlayered CSS, so root sizing overrides must live in `src/styles/react-flow-overrides.css` after the layered imports; otherwise third-party `height: 100%` can outrank app layer rules and collapse the canvas.
- Keep the visual language restrained: neutral light surfaces, thin borders, compact controls, one blue accent for primary action/selection/focus, and low shadows. Avoid decorative gradients, default glass blur, high-saturation inactive states, and border-plus-large-blur card patterns.
- Treat CSS files over roughly 700 lines as a refactor trigger. Prefer adding or moving rules within the existing layered surface/component boundaries over extending one catch-all stylesheet.
