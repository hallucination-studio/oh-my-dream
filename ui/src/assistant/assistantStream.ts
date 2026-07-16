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
