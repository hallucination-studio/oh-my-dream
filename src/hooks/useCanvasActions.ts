import type { Dispatch, SetStateAction } from "react";
import type {
  AppConfig,
  Asset,
  CanvasNodeData,
  GenerationHistory,
  LibEdge,
  LibNode,
  NodeKind
} from "../types";
import { useCanvasHistoryActions } from "./useCanvasHistoryActions";
import { useCanvasImportActions } from "./useCanvasImportActions";
import { useCanvasLocalActions } from "./useCanvasLocalActions";
import { useCanvasOpenAIActions } from "./useCanvasOpenAIActions";
import { useCanvasSeedanceActions } from "./useCanvasSeedanceActions";

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
  const { addHistory } = useCanvasHistoryActions({ setHistory });
  const importActions = useCanvasImportActions({ setAssets, addCanvasNode });
  const openAIActions = useCanvasOpenAIActions({
    nodes,
    openai: config.openai,
    addHistory,
    setAssets,
    setHistory,
    updateNodeData
  });
  const seedanceActions = useCanvasSeedanceActions({
    nodes,
    seedance: config.seedance,
    addHistory,
    setAssets,
    setHistory,
    updateNodeData
  });
  const localActions = useCanvasLocalActions({
    nodes,
    seedance: config.seedance,
    addHistory,
    setEdges,
    addCanvasNode,
    addNodeNear
  });

  return {
    ...importActions,
    ...openAIActions,
    ...seedanceActions,
    ...localActions
  };
}
