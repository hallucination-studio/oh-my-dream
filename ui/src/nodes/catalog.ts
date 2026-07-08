// Node type catalog for the palette and canvas rendering.
//
// This is the UI-side mirror of the node contracts the `nodes` crate will
// register in the engine. Keeping it declarative lets the palette, the node
// body, and wiring validation all read from one source.

import type { PortType } from "../workflow/types.ts";

export interface PortSpec {
  name: string;
  type: PortType;
}

export interface ParamSpec {
  name: string;
  label: string;
  kind: "text" | "int" | "float" | "model";
  default: unknown;
}

export interface NodeTypeSpec {
  type: string;
  label: string;
  category: string;
  inputs: PortSpec[];
  outputs: PortSpec[];
  params: ParamSpec[];
}

// The first-milestone pipeline: producer nodes auto-save generated media.
export const NODE_TYPES: NodeTypeSpec[] = [
  {
    type: "TextPrompt",
    label: "Text Prompt",
    category: "Input",
    inputs: [],
    outputs: [{ name: "text", type: "string" }],
    params: [{ name: "text", label: "Prompt", kind: "text", default: "" }],
  },
  {
    type: "TextToImage",
    label: "Text to Image",
    category: "Image",
    inputs: [{ name: "prompt", type: "string" }],
    outputs: [{ name: "image", type: "image" }],
    params: [
      { name: "model", label: "Model", kind: "model", default: "mock-image" },
      { name: "steps", label: "Steps", kind: "int", default: 28 },
      { name: "seed", label: "Seed", kind: "int", default: 42 },
    ],
  },
  {
    type: "ImageToVideo",
    label: "Image to Video",
    category: "Video",
    inputs: [{ name: "image", type: "image" }],
    outputs: [{ name: "video", type: "video" }],
    params: [
      { name: "model", label: "Model", kind: "model", default: "mock-video" },
      { name: "duration", label: "Duration (s)", kind: "float", default: 4 },
      { name: "fps", label: "FPS", kind: "int", default: 24 },
    ],
  },
  {
    type: "TextToAudio",
    label: "Text to Audio",
    category: "Audio",
    inputs: [{ name: "prompt", type: "string" }],
    outputs: [{ name: "audio", type: "audio" }],
    params: [
      { name: "model", label: "Model", kind: "model", default: "mock-audio" },
      { name: "seed", label: "Seed", kind: "int", default: 42 },
    ],
  },
];

export function findNodeType(type: string): NodeTypeSpec | undefined {
  return NODE_TYPES.find((spec) => spec.type === type);
}

/** Node types grouped by category, preserving declaration order. */
export function nodesByCategory(): { category: string; nodes: NodeTypeSpec[] }[] {
  const groups: { category: string; nodes: NodeTypeSpec[] }[] = [];
  for (const spec of NODE_TYPES) {
    let group = groups.find((g) => g.category === spec.category);
    if (!group) {
      group = { category: spec.category, nodes: [] };
      groups.push(group);
    }
    group.nodes.push(spec);
  }
  return groups;
}
