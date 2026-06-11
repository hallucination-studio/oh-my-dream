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
