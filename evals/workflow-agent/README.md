# Workflow Agent Evaluation

This implementation-neutral benchmark evaluates an assistant that turns an approved script into a bounded multimodal workflow while preserving authority, continuity, and recoverability.

## Scope

The `zhong-kui-reversal` episode contains five approved Ledger Groups spanning 29 shots and 180 seconds. Every group has shot-plan, workflow, image, video, audio, and review/recovery tasks. The benchmark does not execute providers, include raw prompts or media, or depend on OpenAI Evals or another hosted evaluation service.

## Layout

- `source/` contains de-identified reconstruction evidence and the approved Ledger Set.
- `schemas/` defines implementation-neutral task and rubric contracts.
- `episodes/zhong-kui-reversal/gold/` owns approved group semantics.
- `episodes/zhong-kui-reversal/groups/` contains repeated group-stage tasks.
- `rubrics/` contains scoring dimensions and calibration cases.
- `scripts/generate-benchmark.mjs` deterministically regenerates fixtures.
- `scripts/validate-benchmark.mjs` validates structure, references, coverage, modality completeness, score weights, and privacy constraints.

## Execution modes

Checkpoint mode evaluates one group from an approved prior checkpoint and always reports a partial-run label. Continuation mode carries accumulated state through all groups and becomes eligible for a full-project score only after cross-group continuity review.

No task performs paid execution. A harness supplies provider adapters, artifacts, costs, latency, and attempt logs. Provider failure is reported as an outcome rather than hidden, and correct bounded recovery or abstention can receive process credit.

## Regeneration and validation

```sh
node evals/workflow-agent/scripts/generate-benchmark.mjs
node evals/workflow-agent/scripts/validate-benchmark.mjs
```

The generator reads only approved de-identified source artifacts. Validation rejects missing groups or modalities, broken references, full-project scoring for partial runs, provider-specific contracts, raw URLs, likely secrets, and copied private prompt fields.

## Scoring

Hard gates run before weighted scoring. Scores are reported separately for interaction, workflow, creative correctness, image, video, audio, cross-modal alignment, execution/recovery, revision, cost, reliability, coverage, and global continuity. The overall range is 0–4. Partial runs never receive a full-project aggregate.

Unresolved implementation choices remain explicit scored cases. A professional equivalent can receive full credit when it preserves approved semantic and continuity requirements with measurable evidence. Attractive output that contradicts approved meaning does not receive creative credit.

## Known limitations

- The source snapshot does not contain approved rendered media joins for every group.
- Camera, tiger design, battle outcome, human-visibility, repair-symbol, and audio-timing conflicts remain declared tolerances or unresolved cases.
- No provider-quality baseline is claimed without actual bounded execution.
- The benchmark validates contracts and fixtures; external harness conformance must be tested by the harness owner.
