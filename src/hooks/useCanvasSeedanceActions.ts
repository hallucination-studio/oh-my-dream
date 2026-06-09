import { useCallback, type Dispatch, type SetStateAction } from "react";
import { nowIso, uid } from "../fixtures";
import { createSeedanceMockJob, type SeedanceMockKind } from "../services/seedanceMock";
import type {
  AppConfig,
  Asset,
  CanvasNodeData,
  GenerationHistory,
  GenerationParams,
  LibNode
} from "../types";

export function useCanvasSeedanceActions({
  nodes,
  seedance,
  addHistory,
  setAssets,
  setHistory,
  updateNodeData
}: {
  nodes: LibNode[];
  seedance: AppConfig["seedance"];
  addHistory: (item: Omit<GenerationHistory, "id" | "createdAt">) => GenerationHistory;
  setAssets: Dispatch<SetStateAction<Asset[]>>;
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
  updateNodeData: (id: string, patch: Partial<CanvasNodeData>) => void;
}) {
  const runSeedanceMock = useCallback(
    (id: string, kind: SeedanceMockKind = "video") => {
      if (!seedance.enabled) {
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 0, message: "Seedance mock 未启用" }
        });
        return;
      }
      const node = nodes.find((item) => item.id === id);
      const nodeParams = (node?.data.params ?? {}) as GenerationParams;
      const prompt = node?.data.prompt || `${node?.data.name ?? "Seedance"} mock 生成`;
      const job = createSeedanceMockJob({ seedance, nodeParams, kind, prompt });
      const record = addHistory({
        kind: job.mediaKind,
        provider: "seedance-mock",
        model: job.model,
        prompt,
        status: "queued",
        progress: 0,
        params: job.generationParams
      });
      updateNodeData(id, {
        taskInfo: { status: "queued", progress: 0, message: "已加入队列" }
      });
      let progress = 0;
      const timer = window.setInterval(() => {
        progress = Math.min(100, progress + Math.ceil(100 / job.steps));
        const running = progress < 100;
        updateNodeData(id, {
          url: running ? node?.data.url : job.resultUrl,
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
                  resultUrl: running ? item.resultUrl : job.resultUrl
                }
              : item
          )
        );
        if (!running) {
          window.clearInterval(timer);
          setAssets((items) => [
            {
              id: uid("asset"),
              kind: job.mediaKind,
              name: `${node?.data.name ?? "生成结果"}`,
              url: job.resultUrl,
              category: job.mediaKind === "audio" ? "sound" : "project",
              provider: "seedance-mock",
              model: job.model,
              prompt,
              params: job.generationParams,
              createdAt: nowIso()
            },
            ...items
          ]);
        }
      }, job.intervalMs);
    },
    [addHistory, nodes, seedance, setAssets, setHistory, updateNodeData]
  );

  return { runSeedanceMock };
}
