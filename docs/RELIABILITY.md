<!-- harness-repo-bootstrap:managed -->
# Reliability

## Reliability Targets

Local data persistence, crash recovery, and offline behavior are all critical. The core workflow must remain usable in local-only mode, project state should survive reloads and interruptions, and changes to canvas and project state should be validated with a focus on durability before merge.

## Runtime Validation

- Treat loss of project structure, canvas state, assets metadata, or provider configuration as a high-severity failure.
- Validate the smallest useful local loop before merge: create or open a project, change canvas state, reload, and confirm the state is preserved.
- When touching persistence code, explicitly test interruption scenarios such as refresh, malformed stored state, and partial recovery from older local data.
- Offline behavior should degrade gracefully. Browsing local projects and inspecting local state should remain available even when generation providers are unavailable.
- Capture recurring incidents, recovery gaps, or persistence edge cases in repo docs.
