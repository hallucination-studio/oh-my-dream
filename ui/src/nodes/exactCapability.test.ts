import { expect, it } from "vitest";
import { nodeSpecFromExactContract } from "./exactCapability.ts";

it("projects an exact capability without a legacy bundle", () => {
  const spec = nodeSpecFromExactContract({
    capability_ref: { id: "text.provide_literal", version: "1.0" },
    parameters: [{
      key: "text",
      constraint: { kind: "text_utf8_bytes", minimum: 1, maximum: 65536 },
      presence: { kind: "required" },
    }],
    inputs: [],
    outputs: [{ key: "text", data_type: "text", is_primary: true }],
    execution_kind: "pure_value",
  });
  expect(spec.ref).toEqual({ id: "text.provide_literal", version: "1.0" });
  expect(spec.outputs[0]?.type).toBe("string");
  expect(spec.params[0]?.kind).toBe("text");
});
