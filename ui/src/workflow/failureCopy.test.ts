import { expect, it } from "vitest";
import { failureCopy } from "./failureCopy.ts";

it("names the action and strips the Error prefix", () => {
  expect(failureCopy("Run workflow", new Error("workflow.not_ready"))).toBe(
    "Run workflow failed · workflow.not_ready",
  );
  expect(failureCopy("Import asset", "Error: denied")).toBe("Import asset failed · denied");
  expect(failureCopy("Load library", "")).toBe("Load library failed");
});
