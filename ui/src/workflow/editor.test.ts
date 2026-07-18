import { expect, it } from "vitest";
import { isPersistedMutation } from "./editor.ts";

it("persists position changes only on drag stop", () => {
  expect(isPersistedMutation({ type: "position", dragging: true })).toBe(false);
  expect(isPersistedMutation({ type: "position", dragging: false })).toBe(true);
  expect(isPersistedMutation({ type: "position" })).toBe(true);
  expect(isPersistedMutation({ type: "add" })).toBe(true);
  expect(isPersistedMutation({ type: "remove" })).toBe(true);
  expect(isPersistedMutation({ type: "select" })).toBe(false);
  expect(isPersistedMutation({ type: "dimensions" })).toBe(false);
});
