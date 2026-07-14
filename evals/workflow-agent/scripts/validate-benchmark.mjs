import { access, readFile, readdir } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const episode = resolve(root, "episodes/zhong-kui-reversal");
const errors = [];
const assert = (condition, message) => { if (!condition) errors.push(message); };
const readJson = async (path) => {
  try { return JSON.parse(await readFile(path, "utf8")); }
  catch (error) { errors.push(`${relative(root, path)}: ${error.message}`); return null; }
};
const walk = async (dir) => (await Promise.all((await readdir(dir, { withFileTypes: true })).map((entry) => {
  const path = resolve(dir, entry.name);
  return entry.isDirectory() ? walk(path) : path;
}))).flat();
const exists = async (path) => { try { await access(path); return true; } catch { return false; } };

const index = await readJson(resolve(root, "source/script-ledger-index.json"));
const authority = await readJson(resolve(root, "source/lineage-authority.json"));
const manifest = await readJson(resolve(episode, "manifest.json"));
const checkpoints = await readJson(resolve(episode, "checkpoints.json"));
const rubric = await readJson(resolve(root, "rubrics/rubric.json"));
const taskSchema = await readJson(resolve(root, "schemas/task.schema.json"));
const rubricSchema = await readJson(resolve(root, "schemas/rubric.schema.json"));
const groupIds = index?.coverage?.ordered_group_ids ?? [];
const modalities = ["image", "video", "audio"];
const stages = ["shot-plan", "workflow", ...modalities, "review-recovery"];

assert(index?.status === "approved_with_scored_unresolved_cases", "Ledger index is not approved");
assert(index?.verification?.ledger_set_approved === true, "Ledger approval verification is false");
assert(authority?.project_level?.master_script?.approved === true, "Master script is not approved");
assert(groupIds.length === 5 && new Set(groupIds).size === 5, "Expected five unique groups");
assert(index?.coverage?.shots_covered_once === 29 && index?.coverage?.seconds_covered_once === 180, "Expected exact 29-shot/180-second coverage");
assert((index?.coverage?.shot_gaps?.length ?? 1) === 0 && (index?.coverage?.shot_overlaps?.length ?? 1) === 0, "Shot coverage contains gaps or overlaps");
assert((index?.coverage?.time_gaps?.length ?? 1) === 0 && (index?.coverage?.time_overlaps?.length ?? 1) === 0, "Time coverage contains gaps or overlaps");

const taskPaths = manifest?.tasks?.map((entry) => resolve(episode, entry.path)) ?? [];
assert(taskPaths.length === 34, `Expected 34 tasks, found ${taskPaths.length}`);
for (const taskPath of taskPaths) assert(await exists(taskPath), `Missing manifest task ${relative(root, taskPath)}`);

const requiredTaskFields = taskSchema?.required ?? [];
for (const taskPath of taskPaths) {
  const task = await readJson(taskPath);
  if (!task) continue;
  for (const field of requiredTaskFields) assert(Object.hasOwn(task, field), `${relative(root, taskPath)} missing ${field}`);
  assert(task.schema_version === "1.0.0", `${relative(root, taskPath)} has wrong schema version`);
  assert(Array.isArray(task.modes) && task.modes.length > 0, `${relative(root, taskPath)} has no execution mode`);
  assert(task.contracts && ["interaction", "workflow", "creative", "execution"].every((key) => Object.hasOwn(task.contracts, key)), `${relative(root, taskPath)} lacks four contracts`);
  assert(task.gold && ["exact", "semantic", "professional"].every((key) => Array.isArray(task.gold[key])), `${relative(root, taskPath)} lacks three Gold layers`);
  assert(task.budgets?.max_attempts >= 0 && task.budgets?.max_candidates >= 0 && task.budgets?.max_cost_usd >= 0 && task.budgets?.max_latency_seconds > 0, `${relative(root, taskPath)} has invalid budgets`);
  assert(task.rubric_ids?.every((id) => id === rubric?.rubric_id), `${relative(root, taskPath)} has a dangling rubric ID`);
  for (const ref of task.source_refs ?? []) {
    const target = ref.startsWith("source/") ? resolve(root, ref) : resolve(episode, ref);
    assert(await exists(target), `${relative(root, taskPath)} has dangling source ref ${ref}`);
  }
  if (task.group_id) assert(groupIds.includes(task.group_id), `${relative(root, taskPath)} references unapproved group`);
  if (task.reporting?.full_project_eligible) assert(task.stage === "cross-group-continuity" && task.modes.includes("continuation"), `${relative(root, taskPath)} awards full-project eligibility incorrectly`);
}

for (const groupId of groupIds) {
  const goldPath = resolve(episode, `gold/${groupId}.json`);
  const gold = await readJson(goldPath);
  assert(gold?.group_id === groupId, `Missing or mismatched Gold for ${groupId}`);
  assert(modalities.every((modality) => gold?.modalities?.includes(modality)), `${groupId} Gold omits a modality`);
  assert(gold?.semantic_owner?.includes(groupId), `${groupId} has no local semantic owner`);
  for (const stage of stages) {
    const taskPath = resolve(episode, `groups/${groupId}/tasks/${stage}.json`);
    const task = await readJson(taskPath);
    assert(task?.group_id === groupId && task?.stage === stage, `${groupId} missing ${stage} task`);
  }
}

assert(checkpoints?.checkpoint_mode?.length === 5, "Expected five checkpoints");
assert(checkpoints?.continuation_mode?.ordered_group_ids?.join("|") === groupIds.join("|"), "Continuation order differs from Ledger order");
assert(checkpoints?.continuation_mode?.full_project_eligible === true, "Continuation path is not full-project eligible");
assert(manifest?.coverage?.group_task_count === 30, "Expected 30 repeated group tasks");
assert(manifest?.coverage?.total_task_count === 34, "Expected 34 total tasks");

const dimensions = rubric?.dimensions ?? [];
assert(dimensions.length === 13 && new Set(dimensions.map((item) => item.id)).size === 13, "Expected 13 unique rubric dimensions");
assert(dimensions.every((item) => item.weight > 0 && ["0", "1", "2", "3", "4"].every((score) => typeof item.scoring?.[score] === "string")), "Rubric scoring or weights are invalid");
assert(rubric?.aggregation?.partial_run_policy?.includes("never award a full-project score"), "Partial-run scoring policy is missing");
assert(taskSchema?.$schema?.includes("2020-12") && rubricSchema?.$schema?.includes("2020-12"), "Schemas must use JSON Schema 2020-12");

const generatedFiles = (await walk(root)).filter((path) => /\.(json|md)$/.test(path) && !path.includes("benchmark-research.md") && !path.includes("source/production-branch-catalog.json") && !path.includes("source/workflow-inventory.json"));
const forbiddenKeys = /"(?:prompt|raw_payload|credential|secret|api_key|media_url)"\s*:/i;
const forbiddenProviders = /\b(?:midjourney|runway|kling|sora|gemini|claude)\b/i;
const suspiciousSecret = /\b(?:sk-[A-Za-z0-9_-]{16,}|AKIA[A-Z0-9]{16})\b/;
for (const path of generatedFiles) {
  const text = await readFile(path, "utf8");
  assert(!forbiddenKeys.test(text), `${relative(root, path)} contains a forbidden raw/private field`);
  assert(!forbiddenProviders.test(text), `${relative(root, path)} couples the benchmark to a provider`);
  assert(!suspiciousSecret.test(text), `${relative(root, path)} contains a likely secret`);
  const urls = text.match(/https?:\/\/[^\s"')]+/g) ?? [];
  assert(urls.every((url) => url.startsWith("https://json-schema.org/") || url.startsWith("https://oh-my-dream.local/")), `${relative(root, path)} contains a non-schema URL`);
}

if (errors.length) {
  console.error(`Benchmark validation failed with ${errors.length} finding(s):`);
  for (const error of errors) console.error(`- ${error}`);
  process.exit(1);
}
console.log(`Benchmark validation passed: ${groupIds.length} groups, 30 repeated group tasks, 34 total tasks, 15 modality requirements, 13 rubric dimensions.`);
