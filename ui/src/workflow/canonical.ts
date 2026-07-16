import type {
  NodeCapabilityContractDto,
  WorkflowDto,
  WorkflowMutationActionDto,
  WorkflowParameterValueDto,
} from "../api/types.ts";
import type { OutputRef, Workflow, WorkflowInputBinding } from "./types.ts";

export function canonicalToEditorWorkflow(workflow: WorkflowDto): Workflow {
  const bindings = new Map<string, Record<string, WorkflowInputBinding>>();
  for (const binding of workflow.input_bindings) {
    const target = bindings.get(binding.target_node_id) ?? {};
    const sources = binding.items.map(
      (item) => [item.source_node_id, item.source_output_key] as OutputRef,
    );
    if (binding.kind === "single" && sources[0]) {
      target[binding.input_key] = { kind: "single", source: sources[0] };
    } else {
      target[binding.input_key] = { kind: "ordered_many", sources };
    }
    bindings.set(binding.target_node_id, target);
  }
  return {
    version: `${workflow.schema_version}.0`,
    project_id: workflow.project_id,
    nodes: workflow.nodes.map((node) => ({
      id: node.node_id,
      type: node.capability_id,
      contract_version: node.capability_version,
      params: Object.fromEntries(
        node.parameters.map(({ key, value }) => [key, editorParameterValue(value)]),
      ),
      inputs: bindings.get(node.node_id) ?? {},
      position: [node.canvas_position.x, node.canvas_position.y],
    })),
  };
}

export function editorMutationActions(
  base: WorkflowDto,
  draft: Workflow,
  contracts: readonly NodeCapabilityContractDto[],
): WorkflowMutationActionDto[] {
  const actions: WorkflowMutationActionDto[] = [];
  const baseNodes = new Map(base.nodes.map((node) => [node.node_id, node]));
  const draftNodes = new Map(draft.nodes.map((node) => [node.id, node]));
  for (const node of base.nodes) {
    if (!draftNodes.has(node.node_id)) actions.push({ kind: "remove_node", node_id: node.node_id });
  }
  for (const node of draft.nodes) {
    const previous = baseNodes.get(node.id);
    const parameters = parameterEntries(node.type, node.contract_version ?? "1.0", node.params, contracts);
    if (!previous) {
      actions.push({
        kind: "add_node",
        node_id: node.id,
        capability: { id: node.type, version: node.contract_version ?? "1.0" },
        parameters,
        canvas_position: position(node.position),
      });
      continue;
    }
    const changedCapability =
      previous.capability_id !== node.type ||
      previous.capability_version !== (node.contract_version ?? "1.0");
    if (changedCapability) {
      actions.push({
        kind: "select_node_capability",
        node_id: node.id,
        capability: { id: node.type, version: node.contract_version ?? "1.0" },
        parameters,
      });
    } else if (JSON.stringify(previous.parameters) !== JSON.stringify(parameters)) {
      actions.push({ kind: "replace_node_parameters", node_id: node.id, parameters });
    }
    const nextPosition = position(node.position);
    if (JSON.stringify(previous.canvas_position) !== JSON.stringify(nextPosition)) {
      actions.push({ kind: "move_node", node_id: node.id, canvas_position: nextPosition });
    }
  }
  appendBindingActions(actions, base, draft, draftNodes);
  return actions;
}

function appendBindingActions(
  actions: WorkflowMutationActionDto[],
  base: WorkflowDto,
  draft: Workflow,
  draftNodes: Map<string, Workflow["nodes"][number]>,
): void {
  const baseBindings = new Map(
    base.input_bindings.map((binding) => [
      `${binding.target_node_id}\0${binding.input_key}`,
      binding,
    ]),
  );
  for (const node of draft.nodes) {
    const nextInputs = Object.entries(node.inputs);
    const inputKeys = new Set([
      ...nextInputs.map(([key]) => key),
      ...base.input_bindings
        .filter((binding) => binding.target_node_id === node.id)
        .map((binding) => binding.input_key),
    ]);
    for (const inputKey of inputKeys) {
      const key = `${node.id}\0${inputKey}`;
      const previous = baseBindings.get(key);
      const next = node.inputs[inputKey];
      if (sameSources(previous?.items ?? [], next)) continue;
      for (const item of previous?.items ?? []) {
        actions.push({
          kind: "remove_input_item",
          target: { node_id: node.id, input_key: inputKey },
          input_item_id: item.input_item_id,
        });
      }
      const nextSources = sources(next);
      if (isOrdered(next) && nextSources.length > 0) {
        throw new Error("Ordered reference editing is not active for the exact-seven MVP");
      }
      for (const [index, source] of nextSources.entries()) {
        const [sourceNodeId, sourceOutputKey] = outputTuple(source);
        if (!draftNodes.has(sourceNodeId)) continue;
        const item = {
          input_item_id: crypto.randomUUID(),
          source_node_id: sourceNodeId,
          source_output_key: sourceOutputKey,
          input_role_key: null,
        };
        if (index === 0 && !isOrdered(next)) {
          actions.push({
            kind: "bind_single_input",
            target: { node_id: node.id, input_key: inputKey },
            item,
          });
        }
      }
    }
  }
}

function parameterEntries(
  capabilityId: string,
  version: string,
  values: Record<string, unknown>,
  contracts: readonly NodeCapabilityContractDto[],
) {
  const contract = contracts.find(
    (candidate) =>
      candidate.capability_ref.id === capabilityId &&
      candidate.capability_ref.version === version,
  );
  if (!contract) throw new Error(`Exact capability contract ${capabilityId}@${version} is unavailable`);
  return contract.parameters.flatMap((parameter) =>
    Object.hasOwn(values, parameter.key)
      ? [{ key: parameter.key, value: boundaryParameterValue(parameter.constraint, values[parameter.key]) }]
      : [],
  );
}

function boundaryParameterValue(
  constraint: Record<string, unknown>,
  value: unknown,
): WorkflowParameterValueDto {
  switch (constraint.kind) {
    case "unsigned_integer_range":
    case "unsigned_integer_allowed_values":
      if (!Number.isSafeInteger(value) || Number(value) < 0) throw new Error("Invalid unsigned integer");
      return { kind: "unsigned_integer", value: String(value) };
    case "text_utf8_bytes":
      if (typeof value !== "string") throw new Error("Invalid text parameter");
      return { kind: "text", value };
    case "choice_allowed_keys":
      if (typeof value !== "string") throw new Error("Invalid choice parameter");
      return { kind: "choice", value };
    case "generation_profile_ref": {
      if (typeof value !== "string") throw new Error("Invalid Generation Profile parameter");
      const separator = value.lastIndexOf("@");
      if (separator < 1) throw new Error("Invalid Generation Profile ref");
      return {
        kind: "generation_profile",
        profile_id: value.slice(0, separator),
        version: value.slice(separator + 1),
      };
    }
    case "managed_asset_id":
      if (typeof value !== "string") throw new Error("Invalid managed Asset parameter");
      return { kind: "managed_asset", asset_id: value };
    default:
      throw new Error("Unknown capability parameter constraint");
  }
}

function editorParameterValue(value: WorkflowParameterValueDto): string | number {
  if (value.kind === "unsigned_integer") return Number(value.value);
  if (value.kind === "generation_profile") return `${value.profile_id}@${value.version}`;
  if (value.kind === "managed_asset") return value.asset_id;
  return value.value;
}

function position(value: [number, number] | undefined) {
  return { x: value?.[0] ?? 0, y: value?.[1] ?? 0 };
}

function sameSources(
  previous: WorkflowDto["input_bindings"][number]["items"],
  next: WorkflowInputBinding | undefined,
): boolean {
  return JSON.stringify(previous.map((item) => [item.source_node_id, item.source_output_key])) ===
    JSON.stringify(sources(next).map(outputTuple));
}

function sources(binding: WorkflowInputBinding | undefined): OutputRef[] {
  if (!binding) return [];
  if (Array.isArray(binding)) return [binding];
  if (!("kind" in binding)) return [[binding.node_id, binding.output]];
  return binding.kind === "single" ? [binding.source] : binding.sources;
}

function isOrdered(binding: WorkflowInputBinding | undefined): boolean {
  return !!binding && !Array.isArray(binding) && "kind" in binding && binding.kind === "ordered_many";
}

function outputTuple(reference: OutputRef): [string, string] {
  return Array.isArray(reference)
    ? reference
    : [reference.node_id, reference.output];
}
