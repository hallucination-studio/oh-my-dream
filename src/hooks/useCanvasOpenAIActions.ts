import { useCallback, type Dispatch, type SetStateAction } from "react";
import { createMediaResource, nowIso, uid } from "../fixtures";
import { generateOpenAIImage, generateOpenAIText, openAIImageRequestParams } from "../services/openai";
import type { AppConfig, Asset, CanvasNodeData, GenerationHistory, LibNode, TaskRecord } from "../types";

type OpenAIConfig = AppConfig["providers"]["openai"];

export function useCanvasOpenAIActions({
  nodes,
  openai,
  addHistory,
  setAssets,
  setHistory,
  setTasks,
  updateNodeData
}: {
  nodes: LibNode[];
  openai: OpenAIConfig;
  addHistory: (item: Omit<GenerationHistory, "id" | "createdAt">) => GenerationHistory;
  setAssets: Dispatch<SetStateAction<Asset[]>>;
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
  setTasks: Dispatch<SetStateAction<TaskRecord[]>>;
  updateNodeData: (id: string, patch: Partial<CanvasNodeData>) => void;
}) {
  const runOpenAIText = useCallback(
    async (id: string) => {
      const node = nodes.find((item) => item.id === id);
      const prompt = node?.data.prompt?.trim() || node?.data.text?.trim() || "生成一段创意脚本。";
      if (!openai.enabled || !openai.apiKey.trim()) {
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 0, message: "请先配置 OpenAI" }
        });
        return;
      }
      updateNodeData(id, { taskInfo: { status: "running", progress: 25, message: "OpenAI 生成中" } });
      const taskId = uid("task");
      setTasks((items) => [
        {
          id: taskId,
          kind: "generate",
          status: "running",
          title: "文本生成",
          provider: "openai",
          sourceNodeId: id,
          progress: 25,
          createdAt: nowIso(),
          updatedAt: nowIso()
        },
        ...items
      ]);
      const started = addHistory({
        kind: "text",
        provider: "openai",
        model: openai.models.text,
        prompt,
        status: "running",
        progress: 25
      });
      try {
        const text = await generateOpenAIText(openai, prompt);
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
        setTasks((items) =>
          items.map((task) =>
            task.id === taskId
              ? { ...task, status: "done", progress: 100, detail: "文本已写回节点", updatedAt: nowIso() }
              : task
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
        setTasks((items) =>
          items.map((task) =>
            task.id === taskId
              ? { ...task, status: "failed", progress: 100, detail: message, updatedAt: nowIso() }
              : task
          )
        );
      }
    },
    [addHistory, nodes, openai, setHistory, setTasks, updateNodeData]
  );

  const runOpenAIImage = useCallback(
    async (id: string) => {
      const node = nodes.find((item) => item.id === id);
      const prompt = node?.data.prompt?.trim() || node?.data.name || "生成一张电影感创意视觉图。";
      if (!openai.enabled || !openai.apiKey.trim()) {
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 0, message: "请先配置 OpenAI" }
        });
        return;
      }
      updateNodeData(id, { taskInfo: { status: "running", progress: 20, message: "OpenAI 图像生成中" } });
      const taskId = uid("task");
      setTasks((items) => [
        {
          id: taskId,
          kind: "generate",
          status: "running",
          title: "图片生成",
          provider: "openai",
          sourceNodeId: id,
          progress: 20,
          createdAt: nowIso(),
          updatedAt: nowIso()
        },
        ...items
      ]);
      const started = addHistory({
        kind: "image",
        provider: "openai",
        model: openai.models.image,
        prompt,
        status: "running",
        progress: 20,
        params: openAIImageRequestParams
      });
      try {
        const { resultUrl, revisedPrompt, requestParams } = await generateOpenAIImage(openai, prompt);
        const resource = createMediaResource("image", `${node?.data.name ?? "OpenAI 图像"}`, resultUrl);
        updateNodeData(id, {
          url: resultUrl,
          output: {
            resources: [resource],
            preview: {
              id: uid("preview"),
              title: node?.data.name ?? "OpenAI 图像",
              kind: "image",
              items: [resource]
            }
          },
          workflowType: "generated",
          taskInfo: { status: "done", progress: 100, message: "图像已写回节点" }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id
              ? { ...item, status: "done", progress: 100, resultUrl, revisedPrompt, resultResources: [resource] }
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
            model: openai.models.image,
            prompt: revisedPrompt ?? prompt,
            params: requestParams,
            createdAt: nowIso(),
            resource,
            sourceNodeId: id,
            tags: ["OpenAI", "生成"],
            uses: 0
          },
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
        const message = error instanceof Error ? error.message : "图像生成失败";
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 100, message }
        });
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
    [addHistory, nodes, openai, setAssets, setHistory, setTasks, updateNodeData]
  );

  return { runOpenAIText, runOpenAIImage };
}
