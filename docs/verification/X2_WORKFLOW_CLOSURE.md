# X2 Workflow Closure Evidence

Date: 2026-07-17

This is a verification record, not a source of backend semantics. The authoritative behavior remains
in `docs/BACKEND*.md`.

## Verified loops

| Required evidence | Executable proof |
| --- | --- |
| Deterministic Text-to-Image, Image-to-Video, and Text-to-Speech routes through the configured routers | `deterministic_routes_satisfy_the_three_exact_provider_contracts_through_routers` |
| Exact configured Fal Text-to-Image request, polling, result, download, and safe failure behavior | `provider_adapters::fal_text_to_image::tests` |
| Exact configured Fal Image-to-Video request and returned silent MP4 | `provider_adapters::fal_image_to_video::tests` |
| Exact configured ElevenLabs Text-to-Speech voice path, request body, and secret-safe failures | `provider_adapters::elevenlabs_text_to_speech::tests` |
| Text-to-Image followed by Image-to-Video with persisted output media | `workflow::executes_full_generation_workflow_and_persists_video_asset` |
| Text-to-Speech with persisted audio media | `workflow::executes_text_to_audio_and_persists_audio_asset` |
| Imported image identity reaches Image-to-Video and remains project-scoped | `media_boundary::asset_prefixed_local_image_reaches_the_video_generator` and `canonical_asset_slice_imports_lists_gets_and_issues_signed_preview` |
| Provider and media failures remain structured; cancellation wins at every documented boundary | `generation_capability_faults`, `generation_capability_interruptions`, and `workflow_run_execution::cancellation_commits_before_signalling_and_rejects_late_results` |
| UI event duplicate suppression and bounded gap repair | `App.runLifecycle.test.tsx` |
| Pending Asset durability and exact replay/finalization | `asset_import_use_case::inline_publish_failure_returns_the_durable_pending_asset` and `asset_import_use_case::pending_import_becomes_available_when_its_exact_publication_is_replayed` |
| Run admission idempotency and deterministic scope | `workflow_run_admission::through_node_admits_only_ancestors_in_deterministic_order_and_replays` |
| Project isolation across Workflow Run and Asset queries | `canonical_workflow_slice_creates_mutates_admits_queries_and_cancels` and `canonical_asset_slice_imports_lists_gets_and_issues_signed_preview` |
| Restart interruption | `workflow_run_execution::restart_interruption_fails_a_queued_run_without_dispatch` |

## Focused commands

The following local checks passed:

```text
cargo test -p backends --test integration
cargo test -p nodes --test integration
cargo test -p engine --test integration
cargo test -p assets --test integration
cargo test -p oh-my-dream-tauri --test integration canonical_workflow_slice_creates_mutates_admits_queries_and_cancels
cargo test -p oh-my-dream-tauri --test integration canonical_asset_slice_imports_lists_gets_and_issues_signed_preview
cargo test -p oh-my-dream-tauri --lib provider_adapters::fal_text_to_image::tests
cargo test -p oh-my-dream-tauri --lib provider_adapters::fal_image_to_video::tests
cargo test -p oh-my-dream-tauri --lib provider_adapters::elevenlabs_text_to_speech::tests
cd ui && npm run test -- --run src/App.runLifecycle.test.tsx
```

The configured vendor checks use deterministic in-process transports. They verify the exact native
wire contracts and credential handling without spending external quota or depending on live vendor
availability. Live vendor calls remain behind their separately controlled gate.
