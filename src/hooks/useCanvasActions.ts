import { useCallback, type Dispatch, type SetStateAction } from "react";
import { nodeLabels } from "../constants";
import { imageCovers, nowIso, sampleAudio, sampleVideo, toolboxPresets, uid } from "../fixtures";
import {
  extractImageResult,
  extractResponseText,
  historyDisplayText,
  historyToNodeKind,
  numberParam,
  stringParam
} from "../services/generation";
import type {
  AppConfig,
  Asset,
  AssetKind,
  CanvasNodeData,
  GenerationHistory,
  GenerationParams,
  LibEdge,
  LibNode,
  NodeKind
} from "../types";

function fileToAssetKind(file: File): AssetKind {
  if (file.type.startsWith("video")) {
    return "video";
  }
  if (file.type.startsWith("audio")) {
    return "audio";
  }
  return "image";
}

export function useCanvasActions({
  nodes,
  config,
  setAssets,
  setHistory,
  setEdges,
  addCanvasNode,
  addNodeNear,
  updateNodeData
}: {
  nodes: LibNode[];
  config: AppConfig;
  setAssets: Dispatch<SetStateAction<Asset[]>>;
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
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
  updateNodeData: (id: string, patch: Partial<CanvasNodeData>) => void;
}) {
  const addHistory = useCallback(
    (item: Omit<GenerationHistory, "id" | "createdAt">) => {
      const record: GenerationHistory = { id: uid("history"), createdAt: nowIso(), ...item };
      setHistory((items) => [record, ...items]);
      return record;
    },
    [setHistory]
  );

  const importAsset = useCallback(
    (asset: Asset) => {
      addCanvasNode(asset.kind, asset.name, {
        url: asset.url,
        prompt: asset.prompt ?? `从我的素材导入：${asset.name}`,
        params: {
          ...(asset.params ?? {}),
          ...(asset.model ? { model: asset.model } : {})
        },
        taskInfo: {
          status: "done",
          progress: 100,
          message: asset.provider ? `${asset.provider} 素材已导入` : "素材已导入"
        }
      });
    },
    [addCanvasNode]
  );

  const importHistory = useCallback(
    (item: GenerationHistory) => {
      const kind = historyToNodeKind(item.kind);
      const text = historyDisplayText(item);
      addCanvasNode(kind, `${nodeLabels[kind]}历史`, {
        url: item.resultUrl,
        prompt: item.kind === "text" ? text : item.revisedPrompt ?? item.prompt,
        text: item.kind === "text" ? text : undefined,
        params: item.params,
        taskInfo: { status: item.status, progress: item.progress }
      });
    },
    [addCanvasNode]
  );

  const handleUpload = useCallback(
    (files: FileList | File[]) => {
      Array.from(files).forEach((file) => {
        const reader = new FileReader();
        reader.onload = () => {
          const url = String(reader.result);
          const kind = fileToAssetKind(file);
          const asset: Asset = {
            id: uid("asset"),
            kind,
            name: file.name,
            url,
            category: kind === "audio" ? "sound" : "project",
            createdAt: nowIso()
          };
          setAssets((items) => [asset, ...items]);
          importAsset(asset);
        };
        reader.readAsDataURL(file);
      });
    },
    [importAsset, setAssets]
  );

  const runOpenAIText = useCallback(
    async (id: string) => {
      const node = nodes.find((item) => item.id === id);
      const prompt = node?.data.prompt?.trim() || node?.data.text?.trim() || "生成一段创意脚本。";
      if (!config.openai.enabled || !config.openai.apiKey.trim()) {
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 0, message: "请先配置 OpenAI" }
        });
        return;
      }
      updateNodeData(id, { taskInfo: { status: "running", progress: 25, message: "OpenAI 生成中" } });
      const started = addHistory({
        kind: "text",
        provider: "openai",
        model: config.openai.textModel,
        prompt,
        status: "running",
        progress: 25
      });
      try {
        const response = await fetch(`${config.openai.baseUrl.replace(/\/$/, "")}/responses`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${config.openai.apiKey}`
          },
          body: JSON.stringify({
            model: config.openai.textModel,
            input: prompt
          })
        });
        if (!response.ok) {
          throw new Error(`OpenAI 请求失败：${response.status}`);
        }
        const payload = await response.json();
        const text = extractResponseText(payload) || "生成成功，但响应中没有可显示文本。";
        updateNodeData(id, {
          text,
          prompt: text,
          taskInfo: { status: "done", progress: 100, message: "已写回节点" }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id
              ? { ...item, resultText: text, status: "done", progress: 100, resultUrl: undefined }
              : item
          )
        );
      } catch (error) {
        const message = error instanceof Error ? error.message : "生成失败";
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 100, message }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id ? { ...item, status: "failed", progress: 100, error: message } : item
          )
        );
      }
    },
    [addHistory, config.openai, nodes, setHistory, updateNodeData]
  );

  const runOpenAIImage = useCallback(
    async (id: string) => {
      const node = nodes.find((item) => item.id === id);
      const prompt = node?.data.prompt?.trim() || node?.data.name || "生成一张电影感创意视觉图。";
      if (!config.openai.enabled || !config.openai.apiKey.trim()) {
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 0, message: "请先配置 OpenAI" }
        });
        return;
      }
      updateNodeData(id, { taskInfo: { status: "running", progress: 20, message: "OpenAI 图像生成中" } });
      const requestParams: GenerationParams = {
        size: "1024x1024",
        quality: "medium",
        output_format: "png"
      };
      const started = addHistory({
        kind: "image",
        provider: "openai",
        model: config.openai.imageModel,
        prompt,
        status: "running",
        progress: 20,
        params: requestParams
      });
      try {
        const response = await fetch(`${config.openai.baseUrl.replace(/\/$/, "")}/images/generations`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${config.openai.apiKey}`
          },
          body: JSON.stringify({
            model: config.openai.imageModel,
            prompt,
            size: requestParams.size,
            quality: requestParams.quality,
            output_format: requestParams.output_format
          })
        });
        if (!response.ok) {
          throw new Error(`OpenAI 图像请求失败：${response.status}`);
        }
        const payload = await response.json();
        const { url: resultUrl, revisedPrompt } = extractImageResult(payload);
        if (!resultUrl) {
          throw new Error("OpenAI 响应中没有可显示图像");
        }
        updateNodeData(id, {
          url: resultUrl,
          taskInfo: { status: "done", progress: 100, message: "图像已写回节点" }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id
              ? { ...item, status: "done", progress: 100, resultUrl, revisedPrompt }
              : item
          )
        );
        setAssets((items) => [
          {
            id: uid("asset"),
            kind: "image",
            name: `${node?.data.name ?? "OpenAI 图像"}`,
            url: resultUrl,
            category: "project",
            provider: "openai",
            model: config.openai.imageModel,
            prompt: revisedPrompt ?? prompt,
            params: requestParams,
            createdAt: nowIso()
          },
          ...items
        ]);
      } catch (error) {
        const message = error instanceof Error ? error.message : "图像生成失败";
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 100, message }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id ? { ...item, status: "failed", progress: 100, error: message } : item
          )
        );
      }
    },
    [addHistory, config.openai, nodes, setAssets, setHistory, updateNodeData]
  );

  const runSeedanceMock = useCallback(
    (id: string, kind: "video" | "audio" | "compose" = "video") => {
      if (!config.seedance.enabled) {
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 0, message: "Seedance mock 未启用" }
        });
        return;
      }
      const node = nodes.find((item) => item.id === id);
      const mediaKind: AssetKind = kind === "audio" ? "audio" : "video";
      const nodeParams = (node?.data.params ?? {}) as GenerationParams;
      const model = stringParam(
        nodeParams,
        "model",
        mediaKind === "audio" ? config.seedance.audioModel : config.seedance.videoModel
      );
      const prompt = node?.data.prompt || `${node?.data.name ?? "Seedance"} mock 生成`;
      const resultUrl = mediaKind === "audio" ? sampleAudio : sampleVideo;
      const generationParams: GenerationParams = {
        model,
        duration: numberParam(nodeParams, "duration", config.seedance.duration),
        ...(mediaKind === "video"
          ? {
              modeType: stringParam(nodeParams, "modeType", "text2video"),
              ratio: stringParam(nodeParams, "ratio", "16:9"),
              resolution: stringParam(nodeParams, "resolution", config.seedance.resolution),
              ...(kind === "compose" ? { transition: stringParam(nodeParams, "transition", "crossfade") } : {})
            }
          : {})
      };
      const record = addHistory({
        kind: mediaKind,
        provider: "seedance-mock",
        model,
        prompt,
        status: "queued",
        progress: 0,
        params: generationParams
      });
      updateNodeData(id, {
        taskInfo: { status: "queued", progress: 0, message: "已加入队列" }
      });
      let progress = 0;
      const steps = Math.max(4, Math.round(config.seedance.mockLatencyMs / 320));
      const timer = window.setInterval(() => {
        progress = Math.min(100, progress + Math.ceil(100 / steps));
        const running = progress < 100;
        updateNodeData(id, {
          url: running ? node?.data.url : resultUrl,
          taskInfo: {
            status: running ? "running" : "done",
            progress,
            message: running ? "Seedance mock 生成中" : "生成完成"
          }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === record.id
              ? {
                  ...item,
                  status: running ? "running" : "done",
                  progress,
                  resultUrl: running ? item.resultUrl : resultUrl
                }
              : item
          )
        );
        if (!running) {
          window.clearInterval(timer);
          setAssets((items) => [
            {
              id: uid("asset"),
              kind: mediaKind,
              name: `${node?.data.name ?? "生成结果"}`,
              url: resultUrl,
              category: mediaKind === "audio" ? "sound" : "project",
              provider: "seedance-mock",
              model,
              prompt,
              params: generationParams,
              createdAt: nowIso()
            },
            ...items
          ]);
        }
      }, Math.max(220, config.seedance.mockLatencyMs / steps));
    },
    [addHistory, config.seedance, nodes, setAssets, setHistory, updateNodeData]
  );

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
            model: config.seedance.videoModel,
            modeType: "text2video",
            ratio: "16:9",
            resolution: config.seedance.resolution,
            duration: config.seedance.duration
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
          params: { model: config.seedance.audioModel, duration: config.seedance.duration }
        });
      }
    },
    [addNodeNear, config.seedance, nodes]
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
            params: kind === "video" ? { model: config.seedance.videoModel, duration: config.seedance.duration } : {}
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
          setEdges((items) => [...items, { id: uid("edge"), source: previous!.id, target: node.id }]);
        }
        previous = node;
      });
    },
    [addCanvasNode, config.seedance.duration, config.seedance.videoModel, nodes.length, setEdges]
  );

  return {
    importAsset,
    importHistory,
    handleUpload,
    runOpenAIText,
    runOpenAIImage,
    runSeedanceMock,
    runImageTool,
    runDirectorShot,
    generateStoryboard,
    quickAction,
    insertToolboxPreset
  };
}
