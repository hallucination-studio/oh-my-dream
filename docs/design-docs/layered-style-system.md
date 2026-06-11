<!-- harness-repo-bootstrap:managed -->
# Layered Style System

## Decision

The app uses a layered CSS directory under `src/styles/` instead of a monolithic `src/styles.css`.

`src/styles/index.css` declares the cascade order and imports token, reset, base, layout, component, surface, and utility files. UI rules belong in the relevant layer file rather than in late overrides.

## Rationale

The product is a local-first creative workbench, so the interface should feel dense, predictable, and inspectable. A single override-heavy stylesheet made canvas chrome, nodes, home, and settings compete with each other. Layered files make ownership explicit and keep the Liblib-aligned direction focused on professional workflow density rather than decorative community-site styling.

## Rules

- Tokens define color, spacing, type, radius, shadow, z-index, and motion values.
- Components define reusable controls such as buttons, forms, modals, media frames, and status vocabulary.
- Surfaces define product contexts such as Home, Project, Config, and each Canvas zone.
- Utilities stay tiny and generic.
- Canvas styles remain split into shell, navigator, workbench, drawer, and node responsibilities.
- Avoid decorative gradients, glassmorphism, large blur shadows, and saturated inactive states in product chrome.
