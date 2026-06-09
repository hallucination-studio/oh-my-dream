import { useCallback, type Dispatch, type SetStateAction } from "react";
import { nowIso, uid } from "../fixtures";
import { generateOpenAIImage, generateOpenAIText, openAIImageRequestParams } from "../services/openai";
import type { AppConfig, Asset, CanvasNodeData, GenerationHistory, LibNode } from "../types";

export function useCanvasOpenAIActions({
  nodes,
  openai,
  addHistory,
  setAssets,
  setHistory,
  updateNodeData
}: {
  nodes: LibNode[];
  openai: AppConfig["openai"];
  addHistory: (item: Omit<GenerationHistory, "id" | "createdAt">) => GenerationHistory;
  setAssets: Dispatch<SetStateAction<Asset[]>>;
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
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
      const started = addHistory({
        kind: "text",
        provider: "openai",
        model: openai.textModel,
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
    [addHistory, nodes, openai, setHistory, updateNodeData]
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
      const started = addHistory({
        kind: "image",
        provider: "openai",
        model: openai.imageModel,
        prompt,
        status: "running",
        progress: 20,
        params: openAIImageRequestParams
      });
      try {
        const { resultUrl, revisedPrompt, requestParams } = await generateOpenAIImage(openai, prompt);
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
            model: openai.imageModel,
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
    [addHistory, nodes, openai, setAssets, setHistory, updateNodeData]
  );

  return { runOpenAIText, runOpenAIImage };
}
