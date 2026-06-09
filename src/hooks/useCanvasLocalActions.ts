import { useCallback, type Dispatch, type SetStateAction } from "react";
import { nodeLabels } from "../constants";
import {
  createMediaResource,
  imageCovers,
  nowIso,
  sampleVideo,
  toolboxPresets,
  uid
} from "../fixtures";
import type {
  AppConfig,
  Asset,
  CanvasNodeData,
  DerivedBatch,
  GenerationHistory,
  ImageToolName,
  LibEdge,
  LibNode,
  NodeKind,
  TaskRecord
} from "../types";

const imageToolMeta: Record<
  string,
  { tool: ImageToolName; count: number; title: string; prompt: string; annotation?: string }
> = {
  "全景 NEW": { tool: "panorama", count: 2, title: "全景扩展", prompt: "扩展构图边界，保持主体一致" },
  多角度: { tool: "angles", count: 4, title: "多角度", prompt: "生成多机位、多视角的画面变体" },
  打光: { tool: "lighting", count: 3, title: "打光", prompt: "保持主体与构图，切换布光方案" },
  九宫格: { tool: "grid", count: 9, title: "九宫格", prompt: "生成多候选组图以供筛选" },
  高清: { tool: "upscale", count: 1, title: "高清增强", prompt: "提升清晰度与细节表现" },
  宫格切分: { tool: "split", count: 4, title: "宫格切分", prompt: "将组图切分为独立素材" },
  标注: { tool: "annotate", count: 1, title: "标注", prompt: "添加局部说明，便于二次编辑", annotation: "主体面部、枪口方向、背景瓶架" },
  "旋转与镜像": { tool: "rotate", count: 2, title: "旋转与镜像", prompt: "旋转构图并生成镜像变体" }
};

export function useCanvasLocalActions({
  nodes,
  seedance,
  addHistory,
  setAssets,
  setHistory,
  setTasks,
  setBatches,
  setEdges,
  addCanvasNode,
  addNodeNear
}: {
  nodes: LibNode[];
  seedance: AppConfig["seedance"];
  addHistory: (item: Omit<GenerationHistory, "id" | "createdAt">) => GenerationHistory;
  setAssets: Dispatch<SetStateAction<Asset[]>>;
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
  setTasks: Dispatch<SetStateAction<TaskRecord[]>>;
  setBatches: Dispatch<SetStateAction<DerivedBatch[]>>;
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
      const meta = imageToolMeta[label] ?? imageToolMeta["多角度"];
      const batchId = uid("batch");
      const taskId = uid("task");
      const taskTitle = `${meta.title} · ${source?.data.name ?? "图片节点"}`;

      setTasks((items) => [
        {
          id: taskId,
          kind: meta.tool === "split" ? "download" : "derive",
          status: "running",
          title: taskTitle,
          provider: "local",
          sourceNodeId: id,
          batchId,
          progress: 12,
          detail: "正在生成本地派生结果",
          createdAt: nowIso(),
          updatedAt: nowIso()
        },
        ...items
      ]);

      setTimeout(() => {
        const resultResources = Array.from({ length: meta.count }, (_, index) =>
          createMediaResource("image", `${meta.title} ${index + 1}`, imageCovers[(index + 1 + nodes.length) % imageCovers.length], {
            localPath: `${meta.tool}/${source?.data.name ?? "image"}-${index + 1}.png`
          })
        );

        const resultNode = addNodeNear(source, "image", meta.title, {
          url: resultResources[0]?.dataUrl ?? resultResources[0]?.remoteUrl,
          urls: resultResources.map((item) => item.dataUrl ?? item.remoteUrl ?? ""),
          prompt: `${meta.prompt}：${source?.data.prompt ?? source?.data.name ?? ""}`,
          workflowType: "derived",
          toolName: meta.tool,
          sourceRefs: source ? [{ id: source.id, label: source.data.name, kind: "node" }] : [],
          output: {
            resources: resultResources,
            batchId,
            preview: {
              id: uid("preview"),
              title: meta.title,
              kind: "image",
              items: resultResources
            },
            downloads: resultResources.map((resource, index) => ({
              id: uid("download"),
              name: `${meta.title} ${index + 1}`,
              kind: "image",
              resourceId: resource.id,
              fileName: `${meta.tool}-${index + 1}.png`,
              targetPath: `${source?.data.name ?? "result"}/exports/${meta.tool}-${index + 1}.png`
            }))
          },
          annotations: meta.annotation ? [meta.annotation] : undefined,
          params: {
            ...(source?.data.params ?? {}),
            outputCount: meta.count,
            grouped: meta.count > 1,
            localMode: true
          },
          taskInfo: { status: "done", progress: 100, message: "派生结果已保存到本地工作区" }
        });

        const assetRecords: Asset[] = resultResources.map((resource, index) => ({
          id: uid("asset"),
          kind: "image",
          name: meta.count > 1 ? `${meta.title} ${index + 1}` : meta.title,
          url: resource.dataUrl ?? resource.remoteUrl ?? "",
          category: "project",
          provider: "local",
          model: `desktop-${meta.tool}`,
          prompt: `${meta.prompt} #${index + 1}`,
          params: { grouped: meta.count > 1, batchIndex: index + 1 },
          createdAt: nowIso(),
          resource,
          sourceNodeId: resultNode?.id,
          batchId,
          tags: [meta.title, "本地派生"],
          uses: 0
        }));

        const history = addHistory({
          kind: "image",
          provider: "local",
          model: `desktop-${meta.tool}`,
          prompt: meta.prompt,
          status: "done",
          progress: 100,
          resultUrl: assetRecords[0]?.url,
          resultResources,
          sourceNodeId: id,
          batchId,
          params: { outputCount: meta.count, localMode: true }
        });

        setAssets((items) => [...assetRecords, ...items]);
        setHistory((items) =>
          items.map((item) =>
            item.id === history.id ? { ...item, resultResources, resultUrl: assetRecords[0]?.url } : item
          )
        );
        setBatches((items) => [
          {
            id: batchId,
            tool: meta.tool,
            sourceNodeId: id,
            resultNodeIds: resultNode ? [resultNode.id] : [],
            resultAssetIds: assetRecords.map((item) => item.id),
            outputCount: meta.count,
            createdAt: nowIso()
          },
          ...items
        ]);
        setTasks((items) =>
          items.map((task) =>
            task.id === taskId
              ? {
                  ...task,
                  status: "done",
                  progress: 100,
                  detail: `已生成 ${meta.count} 个本地结果`,
                  updatedAt: nowIso(),
                  artifacts: resultNode?.data.output?.downloads
                }
              : task
          )
        );
      }, 880);
    },
    [addHistory, addNodeNear, nodes, setAssets, setBatches, setHistory, setTasks]
  );

  const runDirectorShot = useCallback(
    (id: string) => {
      const source = nodes.find((item) => item.id === id);
      const resource = createMediaResource("image", "导演台截图", imageCovers[1], {
        localPath: `director/${source?.data.name ?? "scene"}-shot.png`
      });
      const node = addNodeNear(source, "image", "导演台截图", {
        url: resource.dataUrl ?? resource.remoteUrl,
        prompt: `导演台截图：${source?.data.prompt ?? "场景参考"}`,
        workflowType: "reference",
        sourceRefs: source ? [{ id: source.id, label: source.data.name, kind: "node" }] : [],
        output: {
          resources: [resource],
          preview: {
            id: uid("preview"),
            title: "导演台截图",
            kind: "image",
            items: [resource]
          }
        },
        taskInfo: { status: "done", progress: 100, message: "截图已写入本地工作区" }
      });
      if (!node) {
        return;
      }
      const history = addHistory({
        kind: "image",
        provider: "local",
        model: "director-stage-screenshot",
        prompt: node.data.prompt ?? "导演台截图",
        status: "done",
        progress: 100,
        resultUrl: resource.dataUrl ?? resource.remoteUrl,
        resultResources: [resource],
        sourceNodeId: id
      });
      setAssets((items) => [
        {
          id: uid("asset"),
          kind: "image",
          name: "导演台截图",
          url: resource.dataUrl ?? resource.remoteUrl ?? "",
          category: "project",
          provider: "local",
          model: "director-stage-screenshot",
          prompt: node.data.prompt ?? "导演台截图",
          createdAt: nowIso(),
          resource,
          sourceNodeId: node.id,
          tags: ["导演台", "构图参考"],
          uses: 0
        },
        ...items
      ]);
      setHistory((items) =>
        items.map((item) => (item.id === history.id ? { ...item, resultResources: [resource] } : item))
      );
    },
    [addHistory, addNodeNear, nodes, setAssets, setHistory]
  );

  const generateStoryboard = useCallback(
    (id: string) => {
      const source = nodes.find((item) => item.id === id);
      let previous = source;
      [0, 1, 2].forEach((index) => {
        const kind = index === 2 ? "video" : "image";
        const resource =
          kind === "video"
            ? createMediaResource("video", `故事板 ${index + 1}`, sampleVideo)
            : createMediaResource("image", `故事板 ${index + 1}`, imageCovers[(index + 2) % imageCovers.length]);
        const node = addNodeNear(previous, kind, `故事板 ${index + 1}`, {
          url: resource.dataUrl ?? resource.remoteUrl,
          prompt: `故事板镜头 ${index + 1}`,
          workflowType: "generated",
          sourceRefs: previous ? [{ id: previous.id, label: previous.data.name, kind: "node" }] : [],
          output:
            kind === "image"
              ? {
                  resources: [resource],
                  preview: {
                    id: uid("preview"),
                    title: `故事板 ${index + 1}`,
                    kind: "image",
                    items: [resource]
                  }
                }
              : undefined,
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
          workflowType: "generated",
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
        const url = kind === "image" ? preset.thumb : kind === "video" ? sampleVideo : undefined;
        const node = addCanvasNode(
          kind,
          `${preset.name} ${nodeLabels[kind]}`,
          {
            prompt: `${preset.description} 第 ${index + 1} 步`,
            url,
            workflowType: index === 0 ? "base" : "generated",
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
