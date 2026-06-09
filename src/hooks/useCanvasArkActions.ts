import { useCallback, type Dispatch, type SetStateAction } from "react";
import { createMediaResource, nowIso, uid } from "../fixtures";
import {
  createArkVideoTask,
  generateArkImage,
  getArkVideoTask,
  type ArkVideoResult
} from "../services/volcengineArk";
import type {
  AppConfig,
  Asset,
  CanvasNodeData,
  GenerationHistory,
  GenerationParams,
  LibEdge,
  LibNode,
  LocalMediaResource,
  TaskRecord
} from "../types";

type ArkConfig = AppConfig["providers"]["volcengineArk"];

export function useCanvasArkActions({
  nodes,
  edges,
  ark,
  addHistory,
  setAssets,
  setHistory,
  setTasks,
  updateNodeData
}: {
  nodes: LibNode[];
  edges: LibEdge[];
  ark: ArkConfig;
  addHistory: (item: Omit<GenerationHistory, "id" | "createdAt">) => GenerationHistory;
  setAssets: Dispatch<SetStateAction<Asset[]>>;
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
  setTasks: Dispatch<SetStateAction<TaskRecord[]>>;
  updateNodeData: (id: string, patch: Partial<CanvasNodeData>) => void;
}) {
  const runArkImage = useCallback(
    async (id: string) => {
      const node = nodes.find((item) => item.id === id);
      const params = (node?.data.params ?? {}) as GenerationParams;
      const prompt = node?.data.prompt?.trim() || node?.data.name || "生成一张电影感创意视觉图。";
      if (!ark.enabled || !ark.apiKey.trim()) {
        updateNodeData(id, { taskInfo: { status: "failed", progress: 0, message: "请先配置火山 Ark" } });
        return;
      }
      const taskId = uid("task");
      updateNodeData(id, { taskInfo: { status: "running", progress: 20, message: "火山图片生成中" } });
      setTasks((items) => [
        {
          id: taskId,
          kind: "generate",
          status: "running",
          title: "火山图片生成",
          provider: "volcengine-ark",
          sourceNodeId: id,
          progress: 20,
          createdAt: nowIso(),
          updatedAt: nowIso()
        },
        ...items
      ]);
      const started = addHistory({
        kind: "image",
        provider: "volcengine-ark",
        model: stringParam(params, "model", ark.models.image),
        prompt,
        status: "running",
        progress: 20,
        params
      });
      try {
        const { urls } = await generateArkImage(ark, prompt, params);
        const resources = urls.map((url, index) =>
          createMediaResource("image", `${node?.data.name ?? "火山图像"} ${index + 1}`, url)
        );
        const firstUrl = urls[0];
        updateNodeData(id, {
          url: firstUrl,
          output: {
            resources,
            preview: { id: uid("preview"), title: node?.data.name ?? "火山图像", kind: "image", items: resources }
          },
          workflowType: "generated",
          taskInfo: { status: "done", progress: 100, message: "图像已写回节点" }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id
              ? { ...item, status: "done", progress: 100, resultUrl: firstUrl, resultResources: resources }
              : item
          )
        );
        setAssets((items) => [
          ...resources.map((resource, index) => ({
            id: uid("asset"),
            kind: "image" as const,
            name: resources.length > 1 ? `${node?.data.name ?? "火山图像"} ${index + 1}` : `${node?.data.name ?? "火山图像"}`,
            url: resource.dataUrl ?? resource.remoteUrl ?? "",
            category: "project" as const,
            provider: "volcengine-ark" as const,
            model: stringParam(params, "model", ark.models.image),
            prompt,
            params,
            createdAt: nowIso(),
            resource,
            sourceNodeId: id,
            tags: ["火山", "Seedream"],
            uses: 0
          })),
          ...items
        ]);
        setTasks((items) =>
          items.map((task) =>
            task.id === taskId
              ? { ...task, status: "done", progress: 100, detail: "结果已写入资产库", updatedAt: nowIso() }
              : task
          )
        );
      } catch (error) {
        const message = error instanceof Error ? error.message : "火山图片生成失败";
        updateNodeData(id, { taskInfo: { status: "failed", progress: 100, message } });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id ? { ...item, status: "failed", progress: 100, error: message } : item
          )
        );
        setTasks((items) =>
          items.map((task) =>
            task.id === taskId
              ? { ...task, status: "failed", progress: 100, detail: message, updatedAt: nowIso() }
              : task
          )
        );
      }
    },
    [addHistory, ark, nodes, setAssets, setHistory, setTasks, updateNodeData]
  );

  const runArkVideo = useCallback(
    async (id: string) => {
      const node = nodes.find((item) => item.id === id);
      const params = (node?.data.params ?? {}) as GenerationParams;
      const prompt = node?.data.prompt?.trim() || "生成一个短视频片段。";
      if (!ark.enabled || !ark.apiKey.trim()) {
        updateNodeData(id, { taskInfo: { status: "failed", progress: 0, message: "请先配置火山 Ark" } });
        return;
      }
      const upstream = collectUpstreamMedia(id, nodes, edges);
      const taskId = uid("task");
      const started = addHistory({
        kind: "video",
        provider: "volcengine-ark",
        model: stringParam(params, "model", ark.models.video),
        prompt,
        status: "queued",
        progress: 5,
        params
      });
      updateNodeData(id, { taskInfo: { status: "queued", progress: 5, message: "正在提交 Seedance 任务" } });
      setTasks((items) => [
        {
          id: taskId,
          kind: "generate",
          status: "queued",
          title: "Seedance 视频生成",
          provider: "volcengine-ark",
          sourceNodeId: id,
          progress: 5,
          createdAt: nowIso(),
          updatedAt: nowIso()
        },
        ...items
      ]);
      try {
        const task = await createArkVideoTask(ark, {
          prompt,
          images: upstream.images,
          audios: upstream.audios,
          params
        });
        updateNodeData(id, { taskInfo: { status: "running", progress: 20, message: `Seedance 任务 ${task.id}` } });
        await pollArkVideo({
          ark,
          id: task.id,
          onUpdate: (result, progress) => {
            const localStatus = mapArkStatus(result.status);
            updateNodeData(id, {
              taskInfo: {
                status: localStatus,
                progress,
                message: statusMessage(result)
              }
            });
            setHistory((items) =>
              items.map((item) =>
                item.id === started.id ? { ...item, status: localStatus, progress, error: result.error?.message } : item
              )
            );
            setTasks((items) =>
              items.map((item) =>
                item.id === taskId
                  ? { ...item, status: localStatus, progress, detail: statusMessage(result), updatedAt: nowIso() }
                  : item
              )
            );
          }
        });
        const result = await getArkVideoTask(ark, task.id);
        const videoUrl = result.content?.video_url;
        if (!videoUrl) {
          throw new Error("Seedance 响应中没有视频 URL");
        }
        const resource = createMediaResource("video", `${node?.data.name ?? "Seedance 视频"}`, videoUrl);
        updateNodeData(id, {
          url: videoUrl,
          output: {
            resources: [resource],
            preview: { id: uid("preview"), title: node?.data.name ?? "Seedance 视频", kind: "video", items: [resource] }
          },
          workflowType: "generated",
          taskInfo: { status: "done", progress: 100, message: "视频已写回节点" }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id
              ? { ...item, status: "done", progress: 100, resultUrl: videoUrl, resultResources: [resource] }
              : item
          )
        );
        setAssets((items) => [
          {
            id: uid("asset"),
            kind: "video",
            name: `${node?.data.name ?? "Seedance 视频"}`,
            url: videoUrl,
            category: "project",
            provider: "volcengine-ark",
            model: stringParam(params, "model", ark.models.video),
            prompt,
            params,
            createdAt: nowIso(),
            resource,
            sourceNodeId: id,
            tags: ["火山", "Seedance"],
            uses: 0
          },
          ...items
        ]);
      } catch (error) {
        const message = error instanceof Error ? error.message : "Seedance 视频生成失败";
        updateNodeData(id, { taskInfo: { status: "failed", progress: 100, message } });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id ? { ...item, status: "failed", progress: 100, error: message } : item
          )
        );
        setTasks((items) =>
          items.map((item) =>
            item.id === taskId
              ? { ...item, status: "failed", progress: 100, detail: message, updatedAt: nowIso() }
              : item
          )
        );
      }
    },
    [addHistory, ark, edges, nodes, setAssets, setHistory, setTasks, updateNodeData]
  );

  return { runArkImage, runArkVideo };
}

function collectUpstreamMedia(targetId: string, nodes: LibNode[], edges: LibEdge[]) {
  const sources = edges
    .filter((edge) => edge.target === targetId)
    .map((edge) => nodes.find((node) => node.id === edge.source))
    .filter((node): node is LibNode => Boolean(node));
  return {
    images: sources.flatMap((node) => (node.data.kind === "image" ? nodeResources(node) : [])),
    audios: sources.flatMap((node) => (node.data.kind === "audio" ? nodeResources(node) : []))
  };
}

function nodeResources(node: LibNode): LocalMediaResource[] {
  const outputResources = node.data.output?.resources ?? [];
  if (outputResources.length > 0) {
    return outputResources;
  }
  const url = node.data.url || node.data.remoteUrl;
  if (typeof url !== "string" || !url) {
    return [];
  }
  return [
    {
      id: uid(`media-${node.data.kind}`),
      kind: node.data.kind === "audio" ? "audio" : "image",
      title: node.data.name,
      dataUrl: url.startsWith("data:") ? url : undefined,
      remoteUrl: url.startsWith("asset://") ? url : undefined,
      createdAt: nowIso()
    }
  ];
}

async function pollArkVideo({
  ark,
  id,
  onUpdate
}: {
  ark: ArkConfig;
  id: string;
  onUpdate: (result: ArkVideoResult, progress: number) => void;
}) {
  for (let attempt = 0; attempt < 90; attempt += 1) {
    await new Promise((resolve) => window.setTimeout(resolve, attempt < 5 ? 1600 : 3500));
    const result = await getArkVideoTask(ark, id);
    const progress = result.status === "queued" ? 15 : result.status === "running" ? Math.min(95, 25 + attempt) : 100;
    onUpdate(result, progress);
    if (["succeeded", "failed", "expired", "cancelled"].includes(result.status)) {
      if (result.status !== "succeeded") {
        throw new Error(result.error?.message || statusMessage(result));
      }
      return;
    }
  }
  throw new Error("Seedance 任务查询超时");
}

function mapArkStatus(status: ArkVideoResult["status"]) {
  if (status === "succeeded") {
    return "done";
  }
  if (status === "cancelled") {
    return "canceled";
  }
  if (status === "failed" || status === "expired") {
    return "failed";
  }
  return status;
}

function statusMessage(result: ArkVideoResult) {
  if (result.error?.message) {
    return result.error.message;
  }
  const labels: Record<ArkVideoResult["status"], string> = {
    queued: "Seedance 排队中",
    running: "Seedance 生成中",
    cancelled: "Seedance 任务已取消",
    succeeded: "Seedance 生成完成",
    failed: "Seedance 生成失败",
    expired: "Seedance 任务超时"
  };
  return labels[result.status];
}

function stringParam(params: GenerationParams, key: string, fallback: string) {
  const value = params[key];
  return typeof value === "string" && value ? value : fallback;
}
