# Question Catalog

Use these prompts only when the repo analysis cannot answer them.

## Product

- What core user outcome does this repository serve?
- Which flows matter enough to deserve explicit product specs first?
- Which non-goals should the harness make visible?

## Reliability

- What failure is unacceptable in production?
- What recovery time or uptime expectation matters most?
- Which runtime environments must be validated locally before merge?

## Security

- Does the repo handle credentials, customer data, regulated data, or privileged actions?
- Are there required review gates for authentication, authorization, or secrets handling?

## Frontend

- Is the product expected to have a polished user-facing interface, an internal tool UI, or no frontend?
- Which browsers, devices, or accessibility expectations are non-negotiable?

## References

- Which external docs are worth copying into `docs/references/` because the team uses them repeatedly?
