import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const episode = resolve(root, "episodes/zhong-kui-reversal");
const groupIds = [
  "ledger-01-threat",
  "ledger-02-mobilization",
  "ledger-03-charge",
  "ledger-04-reveal",
  "ledger-05-restoration",
];
const stages = ["shot-plan", "workflow", "image", "video", "audio", "review-recovery"];
const parse = async (path) => JSON.parse(await readFile(resolve(root, path), "utf8"));
const writeJson = async (path, value) => {
  const target = resolve(root, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, `${JSON.stringify(value, null, 2)}\n`);
};

const index = await parse("source/script-ledger-index.json");
const authority = await parse("source/lineage-authority.json");
const sourceLedgers = Object.fromEntries(
  await Promise.all(groupIds.map(async (id) => [id, await parse(`source/ledgers/${id}.json`)])),
);

const contracts = {
  interaction: {
    required: ["state assumptions", "ask only decision-changing questions", "request confirmation before mutation"],
    evidence: ["questions", "proposal", "confirmation", "decision log"],
  },
  workflow: {
    required: ["legal typed graph", "ordered dependencies", "bounded mutation", "preserved upstream state"],
    evidence: ["graph before", "patch", "graph after", "validation result"],
  },
  creative: {
    required: ["approved semantic beats", "declared alternatives", "continuity handoff", "no invented authority"],
    evidence: ["beat mapping", "constraint mapping", "alternative rationale"],
  },
  execution: {
    required: ["bounded candidates", "cost and latency", "provenance", "failure and abstention reporting"],
    evidence: ["attempt log", "artifact manifest", "measurements", "terminal status"],
  },
};

const taskSchema = {
  $schema: "https://json-schema.org/draft/2020-12/schema",
  $id: "https://oh-my-dream.local/evals/workflow-agent/task.schema.json",
  title: "Workflow Agent Evaluation Task",
  type: "object",
  additionalProperties: false,
  required: ["schema_version", "task_id", "episode_id", "stage", "modes", "contracts", "source_refs", "mutation_boundary", "requirements", "evidence", "budgets", "gold", "hard_gates", "rubric_ids", "reporting"],
  properties: {
    schema_version: { const: "1.0.0" },
    task_id: { type: "string", minLength: 3 }, episode_id: { const: "zhong-kui-reversal" },
    group_id: { anyOf: [{ enum: groupIds }, { type: "null" }] },
    stage: { enum: ["novice-script", "pro-script-intake", "script-ledger-set", ...stages, "cross-group-continuity"] },
    modes: { type: "array", minItems: 1, uniqueItems: true, items: { enum: ["checkpoint", "continuation"] } },
    contracts: { type: "object", required: Object.keys(contracts), properties: Object.fromEntries(Object.keys(contracts).map((key) => [key, { type: "object" }])), additionalProperties: false },
    source_refs: { type: "array", minItems: 1, uniqueItems: true, items: { type: "string" } },
    mutation_boundary: { type: "object", required: ["may_change", "must_preserve"], properties: { may_change: { type: "array", items: { type: "string" } }, must_preserve: { type: "array", items: { type: "string" } } }, additionalProperties: false },
    requirements: { type: "array", minItems: 1, items: { type: "string" } }, evidence: { type: "array", minItems: 1, items: { type: "string" } },
    budgets: { type: "object", required: ["max_attempts", "max_candidates", "max_cost_usd", "max_latency_seconds"], properties: { max_attempts: { type: "integer", minimum: 0 }, max_candidates: { type: "integer", minimum: 0 }, max_cost_usd: { type: "number", minimum: 0 }, max_latency_seconds: { type: "integer", minimum: 1 } }, additionalProperties: false },
    gold: { type: "object", required: ["exact", "semantic", "professional"], properties: { exact: { type: "array", items: { type: "string" } }, semantic: { type: "array", items: { type: "string" } }, professional: { type: "array", items: { type: "string" } } }, additionalProperties: false },
    hard_gates: { type: "array", items: { type: "string" } }, rubric_ids: { type: "array", minItems: 1, uniqueItems: true, items: { type: "string" } },
    reporting: { type: "object", required: ["partial_run_label", "full_project_eligible", "required_fields"], properties: { partial_run_label: { type: "string" }, full_project_eligible: { type: "boolean" }, required_fields: { type: "array", items: { type: "string" } } }, additionalProperties: false },
  },
};

const rubricSchema = {
  $schema: "https://json-schema.org/draft/2020-12/schema",
  $id: "https://oh-my-dream.local/evals/workflow-agent/rubric.schema.json",
  title: "Workflow Agent Evaluation Rubric",
  type: "object", additionalProperties: false,
  required: ["schema_version", "rubric_id", "dimensions", "hard_gates", "aggregation"],
  properties: {
    schema_version: { const: "1.0.0" }, rubric_id: { type: "string" },
    dimensions: {
      type: "array", minItems: 1,
      items: {
        type: "object", additionalProperties: false, required: ["id", "weight", "scoring"],
        properties: {
          id: { type: "string" }, weight: { type: "number", exclusiveMinimum: 0 },
          scoring: { type: "object", required: ["0", "1", "2", "3", "4"], additionalProperties: { type: "string" } },
        },
      },
    },
    hard_gates: { type: "array", items: { type: "string" } },
    aggregation: { type: "object", required: ["method", "range", "partial_run_policy"], properties: { method: { const: "weighted_mean_after_gates" }, range: { const: "0-4" }, partial_run_policy: { type: "string" } }, additionalProperties: false },
  },
};

await writeJson("schemas/task.schema.json", taskSchema);
await writeJson("schemas/rubric.schema.json", rubricSchema);

const genericBudgets = { max_attempts: 3, max_candidates: 4, max_cost_usd: 10, max_latency_seconds: 900 };
const goldLayers = (group, stage) => ({
  exact: [`Use approved group ${group.group_id}`, `Preserve shots ${group.script_locator.shot_start}-${group.script_locator.shot_end}`, `Preserve ${group.script_locator.time_start_seconds}-${group.script_locator.time_end_seconds}s group interval`],
  semantic: [group.narrative_purpose, ...group.atomic_beats.map((beat) => beat.summary)],
  professional: [`Deliver a production-usable ${stage} artifact`, "Allow equivalent implementations when declared tolerances and continuity are satisfied", "Abstain rather than invent missing authority"],
});

const goldGroups = [];
for (const group of index.groups) {
  const authorityGroup = authority.groups.find((item) => item.group_id === group.group_id);
  const gold = {
    schema_version: "1.0.0", group_id: group.group_id, semantic_owner: `source/script-ledger-index.json#${group.group_id}`,
    script_version: "shots-revised-029", order: group.order, script_locator: group.script_locator,
    narrative_purpose: group.narrative_purpose, beats: group.atomic_beats,
    modalities: ["image", "video", "audio"], continuity: { incoming: group.incoming_continuity, outgoing: group.outgoing_continuity },
    canonical_facts: authorityGroup.canonical_facts, tolerances: { implementation: "Professional equivalents are accepted when semantic beats and handoffs remain measurable.", unresolved_cases: authorityGroup.unresolved },
    prohibited_inference: ["raw prompt reconstruction", "canvas-layout authority", "provider-specific assumptions", "silent promotion of experiments"],
    source_ledger: `source/ledgers/${group.group_id}.json`, terminal_artifact: group.terminal_artifact,
  };
  goldGroups.push(gold);
  await writeJson(`episodes/zhong-kui-reversal/gold/${group.group_id}.json`, gold);
}
await writeJson("episodes/zhong-kui-reversal/gold/script-ledger-index.json", {
  schema_version: "1.0.0", episode_id: "zhong-kui-reversal", approved_at: "2026-07-14", master_script: "shots-revised-029",
  runtime_seconds: 180, shot_count: 29, ordered_group_ids: groupIds,
  group_gold: goldGroups.map((gold) => ({ group_id: gold.group_id, path: `${gold.group_id}.json`, modalities: gold.modalities })),
  unresolved_policy: "Retain declared conflicts as scored alternatives; never infer exact implementation authority.",
});

const stageRequirements = {
  "shot-plan": ["Map every beat to timed shots", "Declare purpose, camera, staging, motion, sound, assets, and both continuity handoffs"],
  workflow: ["Return a legal typed subgraph", "Declare ordered dependencies, versions, media operations, and the smallest rerun boundary"],
  image: ["Produce or abstain after bounded candidates", "Report dimensions, aspect ratio, semantic fidelity, references, defects, provenance, cost, and latency"],
  video: ["Produce or abstain after bounded candidates", "Report duration, frame properties, motion, camera intent, identity, temporal defects, provenance, cost, and latency"],
  audio: ["Produce or abstain after bounded candidates", "Report format, duration, loudness, peak, clipping, silence, content, voice, cue timing, artifacts, provenance, cost, and latency"],
  "review-recovery": ["Review all three modalities and their alignment", "Preserve approved qualities, invalidate stale outputs, and rerun only the smallest affected subgraph"],
};
const stageEvidence = {
  "shot-plan": ["timed shot table", "beat coverage", "asset list", "handoff record"], workflow: ["graph before", "ordered patch", "graph after", "type validation"],
  image: ["candidate manifest", "image measurements", "review disposition"], video: ["candidate manifest", "temporal measurements", "review disposition"],
  audio: ["candidate manifest", "audio measurements", "cue sheet", "review disposition"], "review-recovery": ["defect localization", "revision instruction", "invalidation list", "rerun result"],
};

const makeTask = (group, stage) => ({
  schema_version: "1.0.0", task_id: `${group.group_id}-${stage}`, episode_id: "zhong-kui-reversal", group_id: group.group_id, stage,
  modes: ["checkpoint", "continuation"], contracts, source_refs: [`gold/${group.group_id}.json`, `source/ledgers/${group.group_id}.json`],
  mutation_boundary: { may_change: [`${group.group_id}:${stage}`, "declared downstream stale outputs"], must_preserve: ["approved prior groups", "source ledger", "unrelated workflow branches"] },
  requirements: [
    ...stageRequirements[stage],
    ...(sourceLedgers[group.group_id].media[stage] ? [sourceLedgers[group.group_id].media[stage].reason, `Treat ${sourceLedgers[group.group_id].media[stage].evidence_status} as unresolved rather than rendered authority`] : []),
  ],
  evidence: stageEvidence[stage], budgets: genericBudgets, gold: goldLayers(group, stage),
  hard_gates: ["approved group required", "required evidence present", "no mutation outside boundary", "no omitted required modality", "budget respected"],
  rubric_ids: ["workflow-agent-v1"],
  reporting: { partial_run_label: `${group.group_id}:${stage}`, full_project_eligible: false, required_fields: ["status", "evidence", "cost_usd", "latency_seconds", "attempts", "failures", "abstention_reason"] },
});
for (const group of index.groups) for (const stage of stages) await writeJson(`episodes/zhong-kui-reversal/groups/${group.group_id}/tasks/${stage}.json`, makeTask(group, stage));

const entryTask = (id, stage, requirements) => ({
  schema_version: "1.0.0", task_id: id, episode_id: "zhong-kui-reversal", group_id: null, stage, modes: ["checkpoint"], contracts, source_refs: ["gold/script-ledger-index.json", "source/script-ledger-index.json"],
  mutation_boundary: { may_change: ["proposed script artifact", "decision log"], must_preserve: ["confirmed user facts", "existing approved material"] },
  requirements, evidence: ["question log", "proposal", "confirmation", "structured artifact"], budgets: { max_attempts: 2, max_candidates: 2, max_cost_usd: 0, max_latency_seconds: 600 },
  gold: { exact: ["Produce a structured full-script artifact"], semantic: ["Preserve confirmed intent and expose unresolved choices"], professional: ["Questions are high-information and bounded", "No unnecessary rewrite"] },
  hard_gates: ["confirmation before mutation", "no invented user decision", "complete structured artifact"], rubric_ids: ["workflow-agent-v1"],
  reporting: { partial_run_label: id, full_project_eligible: false, required_fields: ["status", "questions", "confirmation", "artifact", "unresolved"] },
});
await writeJson("episodes/zhong-kui-reversal/tasks/novice-script.json", entryTask("novice-script", "novice-script", ["Ask high-information questions", "Offer meaningful alternatives", "Obtain confirmation", "Create a complete structured script"]));
await writeJson("episodes/zhong-kui-reversal/tasks/pro-script-intake.json", entryTask("pro-script-intake", "pro-script-intake", ["Perform gap analysis", "Ask bounded decision-changing questions", "Propose normalization without rewriting approved material", "Obtain confirmation"]));
await writeJson("episodes/zhong-kui-reversal/tasks/script-ledger-set.json", entryTask("script-ledger-set", "script-ledger-set", ["Cover every beat exactly once", "Justify production-sized boundaries", "Declare continuity and modality-operation matrix", "Retain valid equivalent partitions"]));

await writeJson("episodes/zhong-kui-reversal/user-simulator.json", {
  schema_version: "1.0.0", simulator_id: "zhong-kui-reversal-user-v1",
  personas: {
    novice: { knowledge: "creative intent without workflow vocabulary", answer_policy: "Answer one decision-changing question at a time using approved facts; reject invented details." },
    professional: { knowledge: "existing script and production constraints", answer_policy: "Prefer preservation; approve normalization only after a concrete gap analysis." },
    reviewer: { knowledge: "approved Ledger Set", answer_policy: "Target one measurable defect, preserve approved qualities, and accept professional equivalents." },
  },
  adversarial_cases: ["under-questioning", "over-questioning", "premature generation", "unnecessary rewrite", "simulator keyword gaming", "feedback that targets one constraint"],
  stop_conditions: ["required decision is confirmed", "question budget is exhausted", "evidence is insufficient and abstention is correct"],
});

await writeJson("episodes/zhong-kui-reversal/tasks/cross-group-continuity.json", {
  ...makeTask(index.groups[4], "review-recovery"), task_id: "cross-group-continuity", group_id: null, stage: "cross-group-continuity", modes: ["continuation"],
  source_refs: ["gold/script-ledger-index.json", ...groupIds.map((groupId) => `gold/${groupId}.json`)],
  mutation_boundary: { may_change: ["smallest defective group subgraph", "stale downstream outputs"], must_preserve: ["approved groups and assets unaffected by the defect"] },
  requirements: ["Validate all five group links and complete 29-shot/180-second coverage", "Check identity, props, geography, style, motion, motifs, timing, transitions, and inherited state", "Reject isolated local passes that fail globally"],
  evidence: ["ordered checkpoint chain", "boundary measurements", "global continuity report", "smallest-rerun proof"],
  gold: { exact: ["All five approved groups in order", "All four boundaries linked"], semantic: authority.project_level.canonical_facts, professional: ["Equivalent continuity solutions accepted when measurable", "Accumulated errors attributed to their earliest observable boundary"] },
  hard_gates: ["all groups represented", "all modalities represented", "no full score for partial run", "coverage has no gap or overlap"], rubric_ids: ["workflow-agent-v1"],
  reporting: { partial_run_label: "full-continuation", full_project_eligible: true, required_fields: ["status", "group_scores", "boundary_scores", "global_score", "coverage", "failures", "cost_usd", "latency_seconds"] },
});

const dimensionIds = ["interaction", "workflow", "creative", "image", "video", "audio", "cross_modal", "execution_recovery", "revision", "cost", "reliability", "coverage", "global_continuity"];
const scoring = { "0": "Missing, invalid, fabricated, or gate-breaking", "1": "Major omissions or contradictions", "2": "Partially correct with material repair needed", "3": "Complete and professionally usable with minor issues", "4": "Fully satisfies exact or approved-equivalent requirements with strong evidence" };
const rubric = { schema_version: "1.0.0", rubric_id: "workflow-agent-v1", dimensions: dimensionIds.map((id) => ({ id, weight: ["coverage", "global_continuity"].includes(id) ? 2 : 1, scoring })), hard_gates: ["invalid workflow", "missing approved group", "missing required modality", "mutation outside boundary", "fabricated authority", "unbounded execution"], aggregation: { method: "weighted_mean_after_gates", range: "0-4", partial_run_policy: "Report only executed dimensions and label partial; never award a full-project score." } };
await writeJson("rubrics/rubric.json", rubric);
await writeJson("rubrics/calibration-cases.json", {
  schema_version: "1.0.0", rubric_id: rubric.rubric_id,
  cases: [
    ["exact", 4, "Matches approved exact constraints and evidence"], ["equivalent", 4, "Different implementation preserves measurable semantics"],
    ["improved", 4, "Improves quality without violating authority or continuity"], ["attractive-but-wrong", 1, "High polish contradicts approved meaning"],
    ["missing-group", 0, "Coverage hard gate"], ["missing-modality", 0, "Modality hard gate"], ["invalid-workflow", 0, "Workflow hard gate"],
    ["provider-failure", 2, "Correct bounded recovery and honest report earn process credit"], ["abstention", 3, "Correct abstention when evidence is insufficient"],
    ["simulator-gaming", 0, "Keyword matching without contract evidence"], ["cascading-error", 1, "Later local passes cannot hide an earlier broken handoff"],
  ].map(([case_id, expected_score, rationale]) => ({ case_id, expected_score, rationale })),
});

const allTaskPaths = [
  "tasks/novice-script.json", "tasks/pro-script-intake.json", "tasks/script-ledger-set.json", "tasks/cross-group-continuity.json",
  ...groupIds.flatMap((groupId) => stages.map((stage) => `groups/${groupId}/tasks/${stage}.json`)),
];
await writeJson("episodes/zhong-kui-reversal/checkpoints.json", {
  schema_version: "1.0.0", checkpoint_mode: groupIds.map((group_id, index) => ({ checkpoint_id: `checkpoint-${index + 1}`, group_id, starts_from: index === 0 ? "project-open" : `approved-${groupIds[index - 1]}`, ends_at: `approved-${group_id}`, full_project_eligible: false })),
  continuation_mode: { ordered_group_ids: groupIds, requires_all_prior_checkpoints: true, terminal_task: "cross-group-continuity", full_project_eligible: true },
});
await writeJson("episodes/zhong-kui-reversal/manifest.json", {
  schema_version: "1.0.0", episode_id: "zhong-kui-reversal", task_schema: "../../schemas/task.schema.json", rubric_schema: "../../schemas/rubric.schema.json",
  gold_index: "gold/script-ledger-index.json", rubric: "../../rubrics/rubric.json", calibration_cases: "../../rubrics/calibration-cases.json", user_simulator: "user-simulator.json", checkpoints: "checkpoints.json",
  tasks: allTaskPaths.map((path) => ({ path, rubric_id: "workflow-agent-v1" })),
  coverage: { groups: groupIds, modalities_per_group: ["image", "video", "audio"], group_task_count: groupIds.length * stages.length, total_task_count: allTaskPaths.length, full_project_scoring_requires: "continuation mode plus cross-group-continuity" },
});

console.log(`Generated ${groupIds.length} Gold fixtures and ${allTaskPaths.length} tasks.`);
