import { useCallback, type Dispatch, type SetStateAction } from "react";
import { nodeLabels } from "../constants";
import { nowIso, uid } from "../fixtures";
import { historyDisplayText, historyToNodeKind } from "../services/generation";
import type { Asset, AssetKind, CanvasNodeData, GenerationHistory, LibNode, NodeKind } from "../types";

function fileToAssetKind(file: File): AssetKind {
  if (file.type.startsWith("video")) {
    return "video";
  }
  if (file.type.startsWith("audio")) {
    return "audio";
  }
  return "image";
}

export function useCanvasImportActions({
  setAssets,
  addCanvasNode
}: {
  setAssets: Dispatch<SetStateAction<Asset[]>>;
  addCanvasNode: (
    kind: NodeKind,
    name: string,
    extra?: Partial<CanvasNodeData>,
    position?: { x: number; y: number }
  ) => LibNode | undefined;
}) {
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

  return { importAsset, importHistory, handleUpload };
}
