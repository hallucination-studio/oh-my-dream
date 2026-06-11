<!-- harness-repo-bootstrap:managed -->
# Security

## Security Constraints

User project data must remain local and must not leak. API tokens and similar secrets require strict handling, must stay under user control, and should never be transmitted anywhere except directly to the configured provider endpoints needed for generation.

## Review Rules

- Treat provider API keys, project prompts, generated assets, and project history as sensitive local data.
- Do not add telemetry, remote sync, or background uploads without an explicit product decision recorded in durable docs.
- Review secrets handling and outbound network behavior explicitly whenever config, persistence, or generation services change.
- Prefer storage and UX patterns that make it obvious to the user where sensitive settings live and when they are used.
- Record security-sensitive assumptions in durable docs.

## Current Secret Handling

- Workspace backup export must omit provider API keys by default.
- Browser local storage is a transitional fallback for configuration and local workspace state; it is not the target secret store.
- Desktop adapters should move OpenAI and Volcengine Ark keys into a system keychain-backed `SecretStore` so workspace JSON never contains long-lived provider secrets.
- Provider and local capability settings should be presented as user-controlled local configuration. UI copy should make it clear when backups include workspace data and that provider API keys are excluded by default.
