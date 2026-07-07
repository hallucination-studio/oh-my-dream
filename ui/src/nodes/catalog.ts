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
  inputs: PortSpec[];
  outputs: PortSpec[];
  params: ParamSpec[];
}

// The first-milestone pipeline: text prompt -> text-to-image -> image-to-video,
// with a terminal save node. Matches docs/DESIGN.md.
export const NODE_TYPES: NodeTypeSpec[] = [
  {
    type: "TextPrompt",
    label: "Text Prompt",
    inputs: [],
    outputs: [{ name: "text", type: "string" }],
    params: [{ name: "text", label: "Prompt", kind: "text", default: "" }],
  },
  {
    type: "TextToImage",
    label: "Text to Image",
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
    inputs: [{ name: "image", type: "image" }],
    outputs: [{ name: "video", type: "video" }],
    params: [
      { name: "model", label: "Model", kind: "model", default: "mock-video" },
      { name: "duration", label: "Duration (s)", kind: "float", default: 4 },
      { name: "fps", label: "FPS", kind: "int", default: 24 },
    ],
  },
  {
    type: "SaveAsset",
    label: "Save Asset",
    inputs: [{ name: "media", type: "video" }],
    outputs: [],
    params: [],
  },
];

export function findNodeType(type: string): NodeTypeSpec | undefined {
  return NODE_TYPES.find((spec) => spec.type === type);
}
