import type { WorkflowApi } from "../api/index.ts";
import type { AssetKind, AssetSort } from "../api/index.ts";
import type { Workflow } from "../workflow/types.ts";

export type CanvasTab = "nodes" | "assets";

export interface CanvasActions {
  addNode: (type: string, position?: { x: number; y: number }) => void;
  connectNodes: (args: ConnectNodesArgs) => void;
  setParam: (nodeId: string, name: string, value: unknown) => void;
  deleteNode: (nodeId: string) => void;
  selectNode: (nodeId: string | null) => void;
  switchTab: (tab: CanvasTab) => void;
  getCanvasState: () => unknown;
  getSelection: () => unknown;
}

interface ConnectNodesArgs {
  source_node_id: string;
  source_output: string;
  target_node_id: string;
  target_input: string;
}

export interface CapabilityExecutor {
  execute: (capability: string, args: Record<string, unknown>) => Promise<unknown>;
}

export function createCapabilityExecutor({
  actions,
  api,
}: {
  actions: CanvasActions;
  api: WorkflowApi;
}): CapabilityExecutor {
  return {
    execute: async (capability, args) => {
      switch (capability) {
        case "workflow.add_node":
          actions.addNode(requiredString(args, "node_type"), optionalPosition(args.position));
          return null;
        case "workflow.connect_nodes":
          actions.connectNodes(connectArgs(args));
          return null;
        case "workflow.set_param":
          actions.setParam(requiredString(args, "node_id"), requiredString(args, "name"), args.value);
          return null;
        case "workflow.delete_node":
          actions.deleteNode(requiredString(args, "node_id"));
          return null;
        case "workflow.select_node":
          actions.selectNode(optionalString(args.node_id));
          return null;
        case "workflow.switch_tab":
          actions.switchTab(requiredString(args, "tab") as CanvasTab);
          return null;
        case "workflow.get_canvas_state":
          return actions.getCanvasState();
        case "workflow.get_selection":
          return actions.getSelection();
        case "project.create":
          return api.createProject(requiredString(args, "name"));
        case "project.list":
          return api.listProjects();
        case "project.open":
          return api.openProject(requiredString(args, "id"));
        case "workflow.save":
          await api.saveWorkflow(requiredWorkflow(args.workflow));
          return null;
        case "workflow.load":
          return api.loadWorkflow(requiredString(args, "project_id"));
        case "asset.list":
          return api.listAssets({
            kind: optionalEnum<AssetKind>(args.kind, ["image", "video", "audio"]),
            project_id: optionalString(args.project_id) ?? undefined,
            model: optionalString(args.model) ?? undefined,
            prompt: optionalString(args.prompt) ?? undefined,
            sort: optionalEnum<AssetSort>(args.sort, ["newest", "oldest", "cost_desc", "cost_asc"]),
          });
        case "asset.get":
          return api.getAsset(requiredString(args, "id"));
        case "provider.list":
          return api.getProviders();
        case "provider.set_active":
          await api.setActiveProvider(requiredString(args, "provider_id"));
          return null;
        case "provider.set_key":
          await api.setProviderKey(requiredString(args, "provider_id"), requiredString(args, "key"));
          return null;
        case "assistant.get_config":
          return api.getAssistantConfig();
        case "assistant.set_config":
          await api.setAssistantConfig(
            requiredRecord(args, "input") as unknown as Parameters<typeof api.setAssistantConfig>[0],
          );
          return null;
        case "skill.list":
          return api.listSkills();
        case "skill.install":
          return api.installSkill(requiredString(args, "path"));
        case "skill.set_enabled":
          await api.setSkillEnabled(requiredString(args, "name"), requiredBoolean(args, "enabled"));
          return null;
        case "skill.uninstall":
          await api.uninstallSkill(requiredString(args, "name"));
          return null;
        default:
          throw new Error(`Unknown capability: ${capability}`);
      }
    },
  };
}

function connectArgs(args: Record<string, unknown>): ConnectNodesArgs {
  return {
    source_node_id: requiredString(args, "source_node_id"),
    source_output: requiredString(args, "source_output"),
    target_node_id: requiredString(args, "target_node_id"),
    target_input: requiredString(args, "target_input"),
  };
}

function requiredString(args: Record<string, unknown>, name: string): string {
  const value = args[name];
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`Capability argument \`${name}\` must be a non-empty string`);
  }
  return value;
}

function optionalString(value: unknown): string | null {
  if (value === null || typeof value === "undefined") {
    return null;
  }
  if (typeof value !== "string") {
    throw new Error("Optional string capability argument must be a string or null");
  }
  return value;
}

function requiredBoolean(args: Record<string, unknown>, name: string): boolean {
  const value = args[name];
  if (typeof value !== "boolean") {
    throw new Error(`Capability argument \`${name}\` must be a boolean`);
  }
  return value;
}

function requiredRecord(args: Record<string, unknown>, name: string): Record<string, unknown> {
  const value = args[name];
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`Capability argument \`${name}\` must be an object`);
  }
  return value as Record<string, unknown>;
}

function requiredWorkflow(value: unknown): Workflow {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("Capability argument `workflow` must be a workflow object");
  }
  return value as Workflow;
}

function optionalEnum<T extends string>(value: unknown, allowed: T[]): T | undefined {
  if (value === null || typeof value === "undefined" || value === "") {
    return undefined;
  }
  if (typeof value === "string" && allowed.includes(value as T)) {
    return value as T;
  }
  throw new Error(`Capability enum argument must be one of: ${allowed.join(", ")}`);
}

function optionalPosition(value: unknown): { x: number; y: number } | undefined {
  if (typeof value === "undefined" || value === null) {
    return undefined;
  }
  if (
    typeof value === "object" &&
    "x" in value &&
    "y" in value &&
    typeof value.x === "number" &&
    typeof value.y === "number"
  ) {
    return { x: value.x, y: value.y };
  }
  throw new Error("Capability argument `position` must be { x, y } when provided");
}
