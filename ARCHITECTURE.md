<!-- harness-repo-bootstrap:managed -->
# Architecture

## System Summary

A local-first creative client for AI-driven video work that keeps ideation, shot planning, assets, prompts, generation, and revision inside one coherent workspace.

## Domain Boundaries

- Product domain: Local-first creative production software for AI-assisted video workflows.
- Primary users: Independent creators and studio teams using a local client like a professional editing tool to plan and iterate on AI video projects.
- Deployment targets: Pure local client usage. The product should behave like a desktop creative application, with primary workflows running on the user's machine rather than depending on a hosted web app.

## Repository Shape

- Detected languages: JavaScript, TypeScript
- Detected package managers: npm
- Detected frameworks: React, Vite

## Reliability Architecture

Local data persistence, crash recovery, and offline behavior are all critical. The core workflow must remain usable in local-only mode, project state should survive reloads and interruptions, and changes to canvas and project state should be validated with a focus on durability before merge.

## Security Architecture

User project data must remain local and must not leak. API tokens and similar secrets require strict handling, must stay under user control, and should never be transmitted anywhere except directly to the configured provider endpoints needed for generation.

## Open Questions

- Document major runtime boundaries, shared libraries, and integration seams here as the codebase grows.
