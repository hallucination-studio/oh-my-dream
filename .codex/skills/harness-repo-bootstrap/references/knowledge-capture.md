# Knowledge Capture

Write durable knowledge into the repository whenever one of these is true:

- the fact changed your implementation plan
- the fact would likely be needed by another agent later
- the fact came from a human answer rather than directly from code
- the fact explains why a policy, architecture choice, or validation loop exists
- the fact would be annoying to rediscover from scratch

## Where To Write It

- Product behavior or workflow intent: `docs/product-specs/`
- Design rationale or UX rules: `docs/design-docs/`
- Runtime validation, incidents, or observability loops: `docs/RELIABILITY.md` or `docs/sops/`
- Security constraints or review gates: `docs/SECURITY.md`
- Architecture boundaries or integration seams: `ARCHITECTURE.md`
- Reusable external material: `docs/references/`

## Minimum Rule

If a useful fact would otherwise live only in chat, move it into the repo before closing the task.

## Closed Loop

Prefer the script workflow:

1. Log the fact into the active execution plan with `knowledge-log`.
2. Write the fact into its permanent destination doc.
3. Mark the plan item complete with `knowledge-mark-written --id <knowledge-id> --evidence "<text present in durable doc>"`.
4. Close the plan with `plan-close`.

`knowledge-log` returns a stable id. Prefer id-based closure so permanent docs can use concise, natural wording rather than duplicating the exact plan fact.

`knowledge-mark-written` verifies that the destination file contains either the provided evidence text or, for legacy calls, the exact fact. Use `--append` only when the exact fact should be appended to the destination doc by the tool.
