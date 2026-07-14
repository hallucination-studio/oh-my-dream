# Benchmark Precedent Research

## Scope and method

This research informs the workflow-agent benchmark contract without creating provider-specific infrastructure. Sources are first-party documentation, original papers, or official open-source benchmark repositories. Claims were checked against the cited source on 2026-07-14.

The research questions are interaction, partial credit, repeated runs, multimodal measurement, judge calibration, abstention, private data, and failure reporting. Every resulting proposal is classified as adopted, deferred, or rejected.

## Verified primary sources

| ID | Publisher and date | Source | Verified claim used here |
| --- | --- | --- | --- |
| `openai-eval-practices` | OpenAI; current documentation accessed 2026-07-14 | [Evaluation best practices](https://developers.openai.com/api/docs/guides/evaluation-best-practices) | Generative outputs vary for the same input; evals should be task-specific, continuous, fully logged, and calibrated against human feedback. Workflow and agent architectures should evaluate intermediate steps, tool selection, arguments, and handoffs rather than only final text. |
| `openai-graders` | OpenAI; current documentation accessed 2026-07-14 | [Graders](https://developers.openai.com/api/docs/guides/graders) | Graders can combine exact checks, similarity, executable checks, model scoring, and multiple sub-graders. Smooth scores can represent improvement, but model graders require stability checks, human ground truth, examples across quality levels, and reward-hacking review. |
| `openai-data-controls` | OpenAI; current documentation accessed 2026-07-14 | [Data controls in the OpenAI platform](https://platform.openai.com/docs/guides/your-data) | API data is not used to train OpenAI models unless the customer opts in. Default abuse-monitoring retention can be up to 30 days; `/v1/evals` stores application state until deleted and is not Zero Data Retention eligible. |
| `simpleqa` | OpenAI; 2024-11-07 | [Measuring short-form factuality in large language models](https://arxiv.org/abs/2411.04368) and [openai/simple-evals](https://github.com/openai/simple-evals) | SimpleQA grades answers as correct, incorrect, or not attempted and treats calibrated non-attempts as preferable to confident wrong answers. |
| `sima` | Google DeepMind; 2024-03-13 | [Scaling Instructable Agents Across Many Simulated Worlds](https://arxiv.org/abs/2404.10179) | SIMA evaluates language-instructed agents acting in real time through generic image observations and keyboard-and-mouse actions across multiple environments. This supports evaluating instruction following through trajectories, not only terminal prose. |
| `perception-test` | Google DeepMind; 2023-05-23 | [Perception Test: A Diagnostic Benchmark for Multimodal Video Models](https://arxiv.org/abs/2305.13786) | The benchmark spans video, audio, and text with multiple-choice and grounded QA, object and point tracks, and temporal action and sound segments. It separates memory, abstraction, physics, semantics, and reasoning types. |
| `tau-bench` | Sierra and Princeton; 2024-06-17 | [tau-bench: A Benchmark for Tool-Agent-User Interaction in Real-World Domains](https://arxiv.org/abs/2406.12045) | The benchmark uses dynamic conversations, a simulated user, domain policies, tools, and final database-state comparison. Its `pass^k` metric measures whether an agent succeeds consistently across multiple trials. |
| `helm` | Stanford CRFM; 2022-11-16, TMLR 2023 | [Holistic Evaluation of Language Models](https://arxiv.org/abs/2211.09110) | HELM standardizes scenarios while reporting multiple dimensions rather than accuracy alone. It explicitly exposes coverage gaps and trade-offs and publishes detailed run artifacts for analysis. |
| `vbench` | Open-source VBench authors; 2023-11-29 | [VBench: Comprehensive Benchmark Suite for Video Generative Models](https://arxiv.org/abs/2311.17982) and [Vchitect/VBench](https://github.com/Vchitect/VBench) | VBench decomposes generated-video quality into 16 dimensions including identity consistency, motion smoothness, temporal flicker, and spatial relationships, with human-preference validation per dimension. |
| `fad` | Google Research; 2018-12-20 | [Frechet Audio Distance: A Metric for Evaluating Music Enhancement Algorithms](https://arxiv.org/abs/1812.08466) | FAD is a reference-free distributional audio metric validated against artificial distortions and shown to correlate better with human perception than several signal-level alternatives in its study. |
| `clipscore` | Salesforce Research; 2021-04-18 | [CLIPScore: A Reference-free Evaluation Metric for Image Captioning](https://arxiv.org/abs/2104.08718) | CLIPScore is a learned, reference-free image-text compatibility metric for image caption evaluation. Its scope does not by itself establish generated-image quality, identity, anatomy, or reference fidelity. |

## Findings by benchmark requirement

### Interaction and workflow trajectories

`tau-bench` and SIMA both treat action sequences as the evaluated object. `tau-bench` adds a simulated user, policy constraints, tool calls, and a terminal state; SIMA evaluates real-time perception-action behavior through a generic interface. OpenAI's agent-evaluation guidance separately calls out tool selection, argument precision, and handoff accuracy.

Implication: a successful final artifact cannot erase harmful questions, invalid mutations, wrong tool choices, or destructive intermediate state. The benchmark should retain the interaction trace, workflow mutations, provider attempts, reviews, and recovery actions.

### Partial credit and hard gates

OpenAI graders support continuous or combined sub-scores, while HELM demonstrates the value of reporting dimensions separately. Neither precedent justifies allowing an attractive output to compensate for a broken contract.

Implication: use partial credit inside interaction, workflow, creative, image, video, audio, recovery, revision, cost, and continuity dimensions. Apply hard gates to missing groups, missing required modalities, illegal graph mutations, absent real-execution evidence, and privacy violations.

### Repeated runs and reliability

OpenAI documents output variability for identical inputs. `tau-bench` operationalizes reliability with multiple trials and `pass^k`, which falls when any of the repeated trials fail.

Implication: report per-trial scores, success rate, failure distribution, and a strict all-trials reliability measure. Do not report one lucky run as representative. Exact run counts and confidence intervals remain a Task 23 design decision because provider cost and runtime budgets are not approved yet.

### Multimodal metrics

Perception Test demonstrates that video, audio, text, temporal localization, grounding, and reasoning can be measured separately. VBench demonstrates fine-grained generated-video dimensions aligned against human preferences. FAD provides a useful distributional audio precedent, but it does not replace per-clip checks for content, timing, loudness, clipping, silence, or voice identity. CLIPScore provides an image-text compatibility precedent, but it does not measure the full quality or production contract of generated images.

Implication: combine deterministic media validation, source-backed semantic checks, dimension-specific learned metrics, and calibrated human or model judgment. Never use one embedding, aesthetic, FAD, or aggregate video score as the sole modality grade.

### Judge calibration

OpenAI recommends validating automated and model graders against trusted human labels, using examples at multiple quality levels, checking stability across candidate outputs, and monitoring grader or reward hacking. VBench independently validates each dimension against human preference rather than assuming metric alignment.

Implication: Task 23 must include calibration cases for exact, equivalent, improved, attractive-but-wrong, missing-evidence, provider-failure, and abstention outcomes. Judge agreement must be measured per rubric dimension before cost-optimized graders replace stronger judges or human review.

### Abstention and insufficient evidence

SimpleQA separates correct, incorrect, and not attempted. That structure maps directly to source reconstruction: an agent that states that lineage authority or media evidence is insufficient should outperform an agent that invents a join or silently selects a branch.

Implication: define structured abstention reasons such as `insufficient_source_evidence`, `authority_unresolved`, `provider_unavailable`, `budget_exhausted`, and `unsafe_or_disallowed`. Abstention does not earn full task credit, but a correct, evidence-backed abstention avoids hallucination penalties and preserves recoverability.

### Private benchmark data

OpenAI's API data-control documentation distinguishes model-training use, abuse-monitoring retention, and application-state retention. In particular, `/v1/evals` application state persists until deleted and the endpoint is not eligible for Zero Data Retention. HELM's public release of raw prompts and completions is valuable for transparency but is inappropriate for this authenticated private source.

Implication: commit only de-identified fixtures and derived requirements. Keep raw authenticated snapshots, prompts, media, source identifiers, and provider credentials outside version control and outside third-party eval storage unless a later approval verifies retention, deletion, access, and contractual controls.

### Failure reporting

OpenAI recommends logging the full development and evaluation process. HELM reports disaggregated dimensions and coverage rather than one score. `tau-bench` exposes inconsistency across repeated trajectories.

Implication: every run should retain structured failure categories, the failed group and stage, evidence availability, provider status, retries, smallest rerun, cost, and latency. Reports must distinguish model failure, workflow-contract failure, invalid media, judge failure, missing evidence, provider failure, budget exhaustion, and correct abstention.

## Proposal decisions

### Adopted

| Proposal | Source basis | Application to this benchmark |
| --- | --- | --- |
| Evaluate complete interaction and mutation trajectories | `openai-eval-practices`, `tau-bench`, `sima` | Score questions, proposals, confirmations, tool or node choices, graph mutations, reviews, and recovery actions in addition to final artifacts. |
| Use terminal-state and invariant checks | `tau-bench`, `openai-graders` | Verify accumulated workflow state, exact group coverage, preserved upstream state, legal edges, required artifacts, and declared mutation boundaries. |
| Report dimension-level partial credit with hard gates | `helm`, `openai-graders` | Keep all benchmark dimensions separate; block full success for missing groups, required modalities, real evidence, or legal workflow state. |
| Run repeated trials and report strict reliability | `openai-eval-practices`, `tau-bench` | Preserve each trial, report success distribution and an all-trials reliability statistic, and separate checkpoint from continuation reliability. |
| Use layered multimodal evaluation | `perception-test`, `vbench`, `fad`, `clipscore` | Combine file validity and properties, semantic requirements, temporal or audio localization, defect checks, cross-modal alignment, and calibrated judgment. |
| Calibrate model judges against human Gold labels | `openai-eval-practices`, `openai-graders`, `vbench` | Require judge calibration cases, agreement reporting, score-level examples, and grader-hacking checks. |
| Make abstention a structured outcome | `simpleqa` | Reward evidence-backed non-invention relative to confident fabrication while withholding full completion credit. |
| Keep raw private evidence out of committed and third-party eval state | `openai-data-controls`, contrasted with `helm` | Use de-identified Gold fixtures and local or explicitly approved private storage; document deletion and retention controls before any upload. |
| Report failures and coverage separately from averages | `helm`, `tau-bench`, `openai-eval-practices` | Publish group, modality, dimension, failure-type, retry, cost, latency, and continuation-impact matrices. |

### Deferred

| Proposal | Reason for deferral |
| --- | --- |
| Fix the number of repeated trials and confidence-interval method | Requires approved provider budgets, latency limits, and the final task inventory. Define in Task 23. |
| Use FAD as an auxiliary project-level audio distribution metric | Useful only when enough comparable audio samples exist; it is not meaningful for every single group clip. |
| Use CLIP-derived compatibility as an auxiliary image-semantic signal | Requires validation on the approved visual domain and must remain subordinate to identity, reference, defect, and human-judgment checks. |
| Adopt selected VBench implementations directly | The dimensions are relevant, but code, model dependencies, licenses, runtime cost, and suitability for short source-specific clips need separate review. |
| Release de-identified traces publicly | Transparency is valuable, but re-identification and rights review must precede any external release. |

### Rejected

| Proposal | Reason for rejection |
| --- | --- |
| Depend on the OpenAI Evals platform as benchmark infrastructure | OpenAI documentation says the platform becomes read-only on 2026-10-31 and is scheduled to shut down on 2026-11-30. The benchmark contract must remain implementation-neutral. |
| Use one aggregate score as the primary result | HELM and the source task both require exposed trade-offs, coverage, and hard failures. One average hides missing groups and modalities. |
| Use an uncalibrated LLM judge | OpenAI explicitly requires alignment with trusted human labels and stability checks; uncalibrated judging is not defensible Gold. |
| Use FAD as the only audio score | FAD is distributional and was validated for music-enhancement distortions; it does not verify per-item content, timing, voice, loudness, or synchronization. |
| Use CLIPScore as the only image score | Its validated task is image-caption compatibility, not complete generated-image production quality or reference fidelity. |
| Publish or upload raw source prompts, media, and authenticated responses for reproducibility | The source is private, and current eval endpoint retention is incompatible with an assumed zero-retention workflow. Reproducibility must use de-identified fixtures and authorized evidence access. |
| Count provider failure as creative success or silently drop the case | Reliability and failure reporting require the failure to remain visible; recovery and abstention may receive their own credit but cannot fabricate a successful artifact. |

## Research conclusion

The strongest shared precedent is not a single metric. It is a layered evaluation system: exact state and contract checks, source-specific semantic scoring, modality-specific measurements, calibrated judgment, repeated trials, explicit abstention, and disaggregated failure reporting. That structure fits the Script Ledger design and should govern Tasks 9-24 after the Ledger-Set Approval Gate.
