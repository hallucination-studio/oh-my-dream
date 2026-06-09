import type { Dispatch, SetStateAction } from "react";
import type {
  AppConfig,
  Asset,
  CanvasNodeData,
  GenerationHistory,
  LibEdge,
  LibNode,
  NodeKind,
  TaskRecord,
  DerivedBatch
} from "../types";
import { useCanvasHistoryActions } from "./useCanvasHistoryActions";
import { useCanvasImportActions } from "./useCanvasImportActions";
import { useCanvasLocalActions } from "./useCanvasLocalActions";
import { useCanvasOpenAIActions } from "./useCanvasOpenAIActions";
import { useCanvasSeedanceActions } from "./useCanvasSeedanceActions";
import { useCanvasArkActions } from "./useCanvasArkActions";
import { mockProvidersEnabled } from "../env";

export function useCanvasActions({
  nodes,
  edges,
  config,
  setAssets,
  setHistory,
  setTasks,
  setBatches,
  setEdges,
  addCanvasNode,
  addNodeNear,
  updateNodeData
}: {
  nodes: LibNode[];
  edges: LibEdge[];
  config: AppConfig;
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
  updateNodeData: (id: string, patch: Partial<CanvasNodeData>) => void;
}) {
  const { addHistory } = useCanvasHistoryActions({ setHistory });
  const importActions = useCanvasImportActions({ setAssets, addCanvasNode });
  const openAIActions = useCanvasOpenAIActions({
    nodes,
    openai: config.providers.openai,
    addHistory,
    setAssets,
    setHistory,
    setTasks,
    updateNodeData
  });
  const arkActions = useCanvasArkActions({
    nodes,
    edges,
    ark: config.providers.volcengineArk,
    addHistory,
    setAssets,
    setHistory,
    setTasks,
    updateNodeData
  });
  const seedanceActions = useCanvasSeedanceActions({
    nodes,
    seedance: config.providers.seedanceMock,
    addHistory,
    setAssets,
    setHistory,
    setTasks,
    updateNodeData
  });
  const localActions = useCanvasLocalActions({
    nodes,
    config,
    addHistory,
    setAssets,
    setHistory,
    setTasks,
    setBatches,
    setEdges,
    addCanvasNode,
    addNodeNear,
    updateNodeData
  });

  const runImageGeneration = (id: string) => {
    const node = nodes.find((item) => item.id === id);
    const provider = String(node?.data.params?.provider ?? config.capabilityDefaults.image);
    if (provider === "openai") {
      openAIActions.runOpenAIImage(id);
      return;
    }
    arkActions.runArkImage(id);
  };

  const runVideoGeneration = (id: string, kind: "video" | "audio" | "compose" = "video") => {
    const node = nodes.find((item) => item.id === id);
    const provider = String(node?.data.params?.provider ?? config.capabilityDefaults.video);
    if (provider === "seedance-mock") {
      if (!mockProvidersEnabled) {
        updateNodeData(id, { taskInfo: { status: "failed", progress: 0, message: "Mock 仅调试模式可用" } });
        return;
      }
      seedanceActions.runSeedanceMock(id, kind);
      return;
    }
    if (kind !== "video") {
      updateNodeData(id, { taskInfo: { status: "failed", progress: 0, message: "音频/合成节点暂不支持真实生成" } });
      return;
    }
    arkActions.runArkVideo(id);
  };

  return {
    ...importActions,
    ...openAIActions,
    ...arkActions,
    ...seedanceActions,
    ...localActions,
    runImageGeneration,
    runVideoGeneration
  };
}
