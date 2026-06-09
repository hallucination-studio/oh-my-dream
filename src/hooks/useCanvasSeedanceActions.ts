import { useCallback, type Dispatch, type SetStateAction } from "react";
import { createMediaResource, nowIso, uid } from "../fixtures";
import { createSeedanceMockJob, type SeedanceMockKind } from "../services/seedanceMock";
import type {
  AppConfig,
  Asset,
  CanvasNodeData,
  GenerationHistory,
  GenerationParams,
  LibNode,
  TaskRecord
} from "../types";

export function useCanvasSeedanceActions({
  nodes,
  seedance,
  addHistory,
  setAssets,
  setHistory,
  setTasks,
  updateNodeData
}: {
  nodes: LibNode[];
  seedance: AppConfig["seedance"];
  addHistory: (item: Omit<GenerationHistory, "id" | "createdAt">) => GenerationHistory;
  setAssets: Dispatch<SetStateAction<Asset[]>>;
  setHistory: Dispatch<SetStateAction<GenerationHistory[]>>;
  setTasks: Dispatch<SetStateAction<TaskRecord[]>>;
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
      const taskId = uid("task");
      setTasks((items) => [
        {
          id: taskId,
          kind: "generate",
          status: "queued",
          title: `${kind === "audio" ? "音频" : "视频"}生成`,
          provider: "seedance-mock",
          sourceNodeId: id,
          progress: 0,
          createdAt: nowIso(),
          updatedAt: nowIso()
        },
        ...items
      ]);
      let progress = 0;
      const timer = window.setInterval(() => {
        progress = Math.min(100, progress + Math.ceil(100 / job.steps));
        const running = progress < 100;
        const resource = running ? undefined : createMediaResource(job.mediaKind, `${node?.data.name ?? "生成结果"}`, job.resultUrl);
        updateNodeData(id, {
          url: running ? node?.data.url : job.resultUrl,
          output: running || !resource
            ? node?.data.output
            : {
                resources: [resource],
                preview: {
                  id: uid("preview"),
                  title: node?.data.name ?? "生成结果",
                  kind: job.mediaKind,
                  items: [resource]
                }
              },
          workflowType: "generated",
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
                  resultUrl: running ? item.resultUrl : job.resultUrl,
                  resultResources: running || !resource ? item.resultResources : [resource]
                }
              : item
          )
        );
        setTasks((items) =>
          items.map((task) =>
            task.id === taskId
              ? {
                  ...task,
                  status: running ? "running" : "done",
                  progress,
                  detail: running ? "任务执行中" : "结果已保存到本地资产",
                  updatedAt: nowIso()
                }
              : task
          )
        );
        if (!running) {
          window.clearInterval(timer);
          if (!resource) {
            return;
          }
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
              createdAt: nowIso(),
              resource,
              sourceNodeId: id,
              tags: ["Seedance", "生成"],
              uses: 0
            },
            ...items
          ]);
        }
      }, job.intervalMs);
    },
    [addHistory, nodes, seedance, setAssets, setHistory, setTasks, updateNodeData]
  );

  return { runSeedanceMock };
}
