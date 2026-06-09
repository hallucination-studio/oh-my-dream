import type { NodeKind } from "./types";

export const nodeLabels: Record<NodeKind, string> = {
  text: "文本",
  image: "图片",
  video: "视频",
  audio: "音频",
  compose: "视频合成",
  director: "导演台",
  script: "脚本",
  group: "分组"
};

export const nodeFootprints: Record<NodeKind, { width: number; height: number }> = {
  text: { width: 350, height: 350 },
  image: { width: 623, height: 540 },
  video: { width: 622, height: 620 },
  audio: { width: 420, height: 330 },
  compose: { width: 420, height: 350 },
  director: { width: 430, height: 390 },
  script: { width: 390, height: 390 },
  group: { width: 520, height: 360 }
};
