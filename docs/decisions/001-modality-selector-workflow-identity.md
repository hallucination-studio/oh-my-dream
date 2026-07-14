# ADR-001: Select Exact Capabilities By Workflow Modality And Mode

## Status

Accepted

## Date

2026-07-14

## Context

Media Workflow nodes currently use transformation names such as `TextToImage` and
`ImageToVideo`. Multiple input variants need to coexist under one output modality without
replacing the existing exact capability identity and versioning model.

The engine already owns exact `CapabilityRef { id, version }` registration, normalization, ports,
and execution. The minimum viable change is a discriminator in front of that registry, not a new
identity system.

## Decision

Persist the output modality in Workflow `type` and a discriminator in `params.mode`:

```text
(type_id, mode) -> current CapabilityRef -> exact CapabilityRegistration
```

The initial selectors are:

```text
Text  / literal -> TextPrompt@1.0
Image / text    -> TextToImage@1.0
Video / image   -> ImageToVideo@1.0
Video / concat  -> VideoConcat@1.0
Audio / text    -> TextToAudio@1.0
```

`Text / literal` only brings the existing `TextPrompt` capability under the same lookup rule.

Workflow nodes retain `contract_version`. The selector resolves the stable capability id, then the
persisted version resolves the exact registration. Ports and normalized params are derived only from
that exact registration. `NodeRegistry` owns this resolution; execution, patching, discovery, and
boundary projections consume the same result rather than reimplementing selector semantics.

The registry rejects selector rebinding: once a selector is associated with a capability id, later
current versions for that selector must keep the same id. Exact `CapabilityRef` structure and exact
registration remain unchanged. Each exact registration declares one selector so discovery can
project an exact ref back to its selector. Registration and current marking are atomic; a rejected
rebind does not leave a partially inserted exact registration.

Modal defaults include the canonical mode. Normalization fills an omitted mode and rejects a
non-string or different mode.

New nodes are persisted in selector form. Existing exact-id node types use a frozen compatibility
path and are rewritten deterministically at the existing Workflow normalization/write boundary.
This preserves old Workflow JSON without adding a migration subsystem.

Workflow patching canonicalizes the legacy base copy before applying operations and normalizes the
candidate again before validation. An existing exact-id node can therefore change mode in one
ordinary `replace_params` operation.

Mode changes use the existing `replace_params` operation. Candidate resolution selects the new exact
registration, and existing Workflow validation atomically rejects incompatible bindings.

Rust catalog and assistant discovery projections expose both the exact ref and selector. React uses
the paginated UI capability search, filtered by selector type, to populate mode choices. Mode
pagination uses request-local state and does not replace the Node Library's active search results.
Python assistant production code remains unchanged.

## Consequences

- The selector is one additional registry lookup key, not a registry rewrite.
- Multiple exact capabilities can appear as modes under one output modality.
- The exact ref remains the identity used by discovery, versioning, and execution registration.
- Workflow serialization changes only through existing `type`, `contract_version`, and `params`.
- Existing MVP Workflows remain readable and executable during and after the rollout.
- `TextToAudio` joins the exact registry mechanically; its behavior does not change.
- Runtime node ids, backend request names, and asset provenance remain exact capability ids.
- Future modes require their own product issues and are not implied by this foundation.

## Rejected Alternatives And Expansion

- Add a persisted `capability` field to Workflow nodes.
- Redesign `CapabilityRef` or replace the exact registry.
- Add selector versions, selector aliases, or a selector migration service.
- Add a dedicated `select_mode` patch operation.
- Use assistant top-five discovery results as the complete React mode list.
- Implement any new HELL-941 child capability, port, value type, provider request, Storyboard, group,
  asset-node, or real concat behavior in this batch.
- Modify Python assistant production logic.
- Accept unrelated architecture or cleanup work suggested during review.
