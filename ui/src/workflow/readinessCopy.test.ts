import { expect, it } from "vitest";
import { projectReadinessIssues } from "./readinessCopy.ts";

const inputType = (nodeId: string, key: string) =>
  nodeId === "video-node" && key === "image"
    ? "image"
    : nodeId === "gen-node" && key === "prompt"
      ? "text"
      : null;

it("translates engine findings into creator-language guidance", () => {
  const views = projectReadinessIssues(
    {
      state: "blocked",
      issues: [
        { kind: "required_input_missing", node_id: "video-node", detail: { input_key: "image" } },
        {
          kind: "required_parameter_missing",
          node_id: "gen-node",
          detail: { parameter_key: "generation_profile_ref" },
        },
        { kind: "required_input_missing", node_id: "gen-node", detail: { input_key: "prompt" } },
      ],
    },
    inputType,
    ["gen-node", "video-node"],
  );
  expect(views.map((view) => view.copy)).toEqual([
    "Choose a generation model.",
    "Connect a Text output to Prompt.",
    "Connect an Image output to Image.",
  ]);
});

it("returns no views when ready and degrades safely on unknown shapes", () => {
  expect(projectReadinessIssues({ state: "ready" }, inputType, [])).toEqual([]);
  const views = projectReadinessIssues(
    { state: "blocked", issues: ["opaque", { kind: "asset_unavailable", node_id: "n" }] },
    inputType,
    [],
  );
  expect(views.map((view) => view.copy)).toEqual([
    "This step needs attention before it can run.",
    "The selected Asset is not available.",
  ]);
});
