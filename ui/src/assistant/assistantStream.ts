import type { ResponsesStreamEvent } from "../api/index.ts";
import type { StrongAssistantTaskItem } from "./StrongAssistantTask.tsx";

export type AssistantStreamItem =
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string }
  | {
      kind: "step";
      callId: string;
      capability: string;
      state: "running" | "done" | "error";
      error?: string;
    }
  | StrongAssistantTaskItem;

export function appendAssistantToken(
  items: AssistantStreamItem[],
  delta: string,
): AssistantStreamItem[] {
  const last = items.at(-1);
  if (last?.kind === "assistant") {
    return [...items.slice(0, -1), { kind: "assistant", text: last.text + delta }];
  }
  return [...items, { kind: "assistant", text: delta }];
}

export function setStepState(
  items: AssistantStreamItem[],
  callId: string,
  state: "done" | "error",
): AssistantStreamItem[] {
  return items.map((item) =>
    item.kind === "step" && item.callId === callId ? { ...item, state } : item,
  );
}

export function functionCallFromEvent(
  event: ResponsesStreamEvent,
): { callId: string; name: string; arguments?: string } | null {
  if (!isJsonObject(event.item) || event.item.type !== "function_call") return null;
  if (typeof event.item.call_id !== "string" || typeof event.item.name !== "string") return null;
  return {
    callId: event.item.call_id,
    name: event.item.name,
    arguments: typeof event.item.arguments === "string" ? event.item.arguments : undefined,
  };
}

export function planItemFromCall(
  call: { name: string; arguments?: string },
): StrongAssistantTaskItem | null {
  if (call.name !== "production_plan_update_item" || !call.arguments) return null;
  try {
    const value: unknown = JSON.parse(call.arguments);
    if (!isJsonObject(value) || typeof value.item_id !== "string" || !isJsonObject(value.action)) {
      return null;
    }
    const status = value.action.type;
    if (status !== "start" && status !== "block" && status !== "complete") return null;
    return { kind: "plan", itemId: value.item_id, status };
  } catch {
    return null;
  }
}

export function upsertPlanItem(
  items: AssistantStreamItem[],
  plan: StrongAssistantTaskItem,
): AssistantStreamItem[] {
  if (plan.kind !== "plan") return items;
  const found = items.some((item) => item.kind === "plan" && item.itemId === plan.itemId);
  return found
    ? items.map((item) => (item.kind === "plan" && item.itemId === plan.itemId ? plan : item))
    : [...items, plan];
}

export function upsertRunItem(
  items: AssistantStreamItem[],
  runId: string,
  state: "running" | "succeeded" | "failed" | "cancelled",
  detail: string,
): AssistantStreamItem[] {
  const previous = items.find((item) => item.kind === "run" && item.runId === runId);
  const combinedDetail =
    previous?.kind === "run" && state !== "running" && previous.detail !== detail
      ? `${previous.detail} — ${detail}`
      : detail;
  const run: StrongAssistantTaskItem = { kind: "run", runId, state, detail: combinedDetail };
  const found = previous !== undefined;
  return found
    ? items.map((item) => (item.kind === "run" && item.runId === runId ? run : item))
    : [...items, run];
}

export function responseError(event: ResponsesStreamEvent): string {
  if (typeof event.error === "string") return event.error;
  if (isJsonObject(event.error) && typeof event.error.message === "string") {
    return event.error.message;
  }
  return "Assistant response failed";
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
