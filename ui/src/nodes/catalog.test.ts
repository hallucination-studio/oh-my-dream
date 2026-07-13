import { describe, expect, it } from "vitest";
import nodeContracts from "../__fixtures__/node_contracts.json";
import { NODE_TYPES, arePortTypesCompatible } from "./catalog.ts";

describe("node catalog contract", () => {
  it("uses Rust-generated node ports as its source", () => {
    const actual = NODE_TYPES.map((node) => ({
      type_id: node.type,
      inputs: node.inputs.map((port) => ({
        name: port.name,
        port_type: port.type,
        required: port.required,
      })),
      outputs: node.outputs.map((port) => ({ name: port.name, port_type: port.type })),
    })).sort((left, right) => left.type_id.localeCompare(right.type_id));

    expect(actual).toEqual(nodeContracts.nodes);
  });

  it("uses the Rust-generated compatibility relation", () => {
    expect(arePortTypesCompatible("image", "image")).toBe(true);
    expect(arePortTypesCompatible("image", "string")).toBe(false);
  });
});
