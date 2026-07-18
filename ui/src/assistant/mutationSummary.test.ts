import { expect, it } from "vitest";
import { summarizeMutations } from "./mutationSummary.ts";

const label = (nodeId: string) => ({ "node-a": "Text", "node-b": "Generate image" })[nodeId] ?? "a node";

it("summarizes each mutation kind in creator language", () => {
  const lines = summarizeMutations(
    [
      { kind: "add_node", node_id: "new-1", capability: { id: "image.generate_from_text", version: "1.0" }, parameters: [], canvas_position: { x: 0, y: 0 } },
      { kind: "bind_single_input", target: { node_id: "node-b", input_key: "prompt" }, item: { input_item_id: "i1", source_node_id: "node-a", source_output_key: "text", input_role_key: null } },
      { kind: "remove_node", node_id: "node-a" },
      { kind: "replace_node_parameters", node_id: "node-b", parameters: [] },
      { kind: "select_node_capability", node_id: "node-b", capability: { id: "x", version: "1" }, parameters: [] },
      { kind: "move_node", node_id: "node-b", canvas_position: { x: 1, y: 2 } },
    ],
    label,
  );
  expect(lines).toEqual([
    "Add Generate image",
    "Connect Text → Generate image to prompt",
    "Remove Text",
    "Change settings on Generate image",
    "Change the type of Generate image",
  ]);
});

it("deduplicates repeated summaries and degrades safely", () => {
  expect(summarizeMutations([{ kind: "move_node", node_id: "x", canvas_position: { x: 0, y: 0 } }], label)).toEqual([]);
  expect(summarizeMutations(["opaque"], label)).toEqual(["Change the workflow"]);
  expect(
    summarizeMutations(
      [
        { kind: "remove_node", node_id: "node-a" },
        { kind: "remove_node", node_id: "node-a" },
      ],
      label,
    ),
  ).toEqual(["Remove Text"]);
});
