# X3 Assistant Closure Evidence

Date: 2026-07-17

This is a verification record, not a source of Assistant or Workflow semantics. The authoritative
behavior remains in `docs/BACKEND*.md`.

## Verified loop

| Required evidence | Executable proof |
| --- | --- |
| A model invocation is isolated by Project and Session and cannot overlap itself | `send_message::tests::different_projects_may_invoke_the_same_session_identity_independently` and `send_message::tests::same_project_session_rejects_a_concurrent_invocation_and_releases_after_completion` |
| Exactly eleven Rust-owned tools are exposed; direct Workflow authority is rejected | `tools::tests::catalog_contains_exactly_the_eleven_frozen_tools` and `tools::tests::dispatcher_executes_plan_tools_and_rejects_direct_authority` |
| Proposal input is strict, canonical, alias-safe, and persisted without applying Workflow changes | `tools::tests::dispatcher_evaluates_without_persistence_and_proposes_with_persistence` and `assistant_workflow_bridge::tests` |
| Reviewer must fetch exact facts before a verdict; passing review reaches Awaiting Approval | `review_workflow_change::tests::exact_fetch_fact_is_required_and_consumed_only_after_persisted_verdict` |
| Human approval atomically records Applying plus the post-commit effect; rejection is terminal | `decide_workflow_change::tests::approval_atomically_commits_applying_with_effect` and `decide_workflow_change::tests::rejection_consumes_continuation_only_after_terminal_commit` |
| The post-commit effect applies the canonical Workflow mutation, admits the canonical Run, records the Run link, and resumes safely | `apply_workflow_change_effect::tests::effect_recovers_apply_resume_and_run_link_without_repeating_completed_work` |
| The Desktop worker routes only the exact three effects and preserves retry/terminal behavior | `post_commit_worker::worker::tests` |
| A factual failed Run creates one repair activation and starts a new reviewed turn with canonical Run facts | `repair_activation::tests::only_created_activation_starts_a_repair_turn_with_canonical_failed_run_facts` |
| Repair remains a new proposal/review/approval cycle rather than direct mutation authority | `workflow_change::tests::pass_review_then_approval_reaches_applied_only_through_applying` and `interfaces::tests::repair_contract_is_idempotent_per_project_and_failed_run` |
| Canonical V5 commands resolve project scope and fail closed when the model is disabled | `assistant_commands_v5::canonical_assistant_commands_resolve_project_and_fail_closed_when_disabled` |
| Typed presentation events deduplicate and repair gaps through pending authority | `AssistantDock.test.tsx` |

## Focused commands

The following local checks passed:

```text
cargo test -p assistant
cargo test -p oh-my-dream-tauri --lib assistant_workflow_bridge::tests
cargo test -p oh-my-dream-tauri --lib post_commit_worker::worker::tests
cargo test -p oh-my-dream-tauri --test integration canonical_assistant_commands_resolve_project_and_fail_closed_when_disabled
cd ui && npm run test -- --run src/assistant/AssistantDock.test.tsx
```

The production model runner remains fail-closed when its bounded configuration is absent. The
focused closure checks do not invoke a live model or expose credentials; transport compatibility is
covered by the shared Rust/Python protocol fixtures and the PR gate.
