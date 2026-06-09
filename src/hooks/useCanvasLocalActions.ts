import { useCallback, type Dispatch, type SetStateAction } from "react";
import { nodeLabels } from "../constants";
import { imageCovers, sampleVideo, toolboxPresets, uid } from "../fixtures";
import type {
  AppConfig,
  CanvasNodeData,
  GenerationHistory,
  LibEdge,
  LibNode,
  NodeKind
} from "../types";

export function useCanvasLocalActions({
  nodes,
  seedance,
  addHistory,
  setEdges,
  addCanvasNode,
  addNodeNear
}: {
  nodes: LibNode[];
  seedance: AppConfig["seedance"];
  addHistory: (item: Omit<GenerationHistory, "id" | "createdAt">) => GenerationHistory;
  setEdges: Dispatch<SetStateAction<LibEdge[]>>;
  addCanvasNode: (
    kind: NodeKind,
    name: string,
    extra?: Partial<CanvasNodeData>,
    position?: { x: number; y: number }
  ) => LibNode | undefined;
  addNodeNear: (
    source: LibNode | undefined,
    kind: NodeKind,
    name: string,
    extra?: Partial<CanvasNodeData>
  ) => LibNode | undefined;
}) {
  const runImageTool = useCallback(
    (id: string, label: string) => {
      const source = nodes.find((item) => item.id === id);
      const cover = imageCovers[Math.floor(Math.random() * imageCovers.length)];
      const node = addNodeNear(source, "image", `${label}结果`, {
        url: cover,
        prompt: `${label} mock：${source?.data.prompt ?? source?.data.name ?? ""}`,
        taskInfo: { status: "done", progress: 100, message: "mock 完成" }
      });
      if (!node) {
        return;
      }
      addHistory({
        kind: "image",
        provider: "local",
        model: `image-${label}`,
        prompt: node.data.prompt ?? label,
        status: "done",
        progress: 100,
        resultUrl: cover
      });
    },
    [addHistory, addNodeNear, nodes]
  );

  const runDirectorShot = useCallback(
    (id: string) => {
      const source = nodes.find((item) => item.id === id);
      const cover = imageCovers[1];
      const node = addNodeNear(source, "image", "导演台截图", {
        url: cover,
        prompt: `导演台截图：${source?.data.prompt ?? "场景参考"}`,
        taskInfo: { status: "done", progress: 100, message: "截图已生成" }
      });
      if (!node) {
        return;
      }
      addHistory({
        kind: "image",
        provider: "local",
        model: "director-stage-screenshot",
        prompt: node.data.prompt ?? "导演台截图",
        status: "done",
        progress: 100,
        resultUrl: cover
      });
    },
    [addHistory, addNodeNear, nodes]
  );

  const generateStoryboard = useCallback(
    (id: string) => {
      const source = nodes.find((item) => item.id === id);
      let previous = source;
      [0, 1, 2].forEach((index) => {
        const node = addNodeNear(previous, index === 2 ? "video" : "image", `故事板 ${index + 1}`, {
          url: index === 2 ? sampleVideo : imageCovers[(index + 2) % imageCovers.length],
          prompt: `故事板镜头 ${index + 1}`,
          taskInfo: { status: "done", progress: 100, message: "故事板已生成" }
        });
        if (node) {
          previous = node;
        }
      });
    },
    [addNodeNear, nodes]
  );

  const quickAction = useCallback(
    (id: string, action: string) => {
      const source = nodes.find((node) => node.id === id);
      if (action === "text2video") {
        addNodeNear(source, "video", "文生视频", {
          prompt: source?.data.prompt ?? source?.data.text ?? "",
          params: {
            model: seedance.videoModel,
            modeType: "text2video",
            ratio: "16:9",
            resolution: seedance.resolution,
            duration: seedance.duration
          }
        });
      }
      if (action === "imagePrompt") {
        addNodeNear(source, "text", "图片反推提示词", {
          prompt: "请基于输入图片反推提示词。",
          text: "画面主体、风格、镜头、光线、色彩、构图。"
        });
      }
      if (action === "music") {
        addNodeNear(source, "audio", "文字生音乐", {
          prompt: source?.data.prompt ?? source?.data.text ?? "",
          params: { model: seedance.audioModel, duration: seedance.duration }
        });
      }
    },
    [addNodeNear, nodes, seedance]
  );

  const insertToolboxPreset = useCallback(
    (presetId: string) => {
      const preset = toolboxPresets.find((item) => item.id === presetId) ?? toolboxPresets[0];
      let previous: LibNode | undefined;
      preset.kinds.forEach((kind, index) => {
        const node = addCanvasNode(
          kind,
          `${preset.name} ${nodeLabels[kind]}`,
          {
            prompt: `${preset.description} 第 ${index + 1} 步`,
            url: kind === "image" ? preset.thumb : kind === "video" ? sampleVideo : undefined,
            params: kind === "video" ? { model: seedance.videoModel, duration: seedance.duration } : {}
          },
          {
            x: 120 + index * 460,
            y: 160 + (nodes.length % 2) * 120
          }
        );
        if (!node) {
          return;
        }
        if (previous) {
          const sourceId = previous.id;
          const targetId = node.id;
          setEdges((items) => [...items, { id: uid("edge"), source: sourceId, target: targetId }]);
        }
        previous = node;
      });
    },
    [addCanvasNode, nodes.length, seedance.duration, seedance.videoModel, setEdges]
  );

  return {
    runImageTool,
    runDirectorShot,
    generateStoryboard,
    quickAction,
    insertToolboxPreset
  };
}
