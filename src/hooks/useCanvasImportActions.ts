import { useCallback, type Dispatch, type SetStateAction } from "react";
import { nodeLabels } from "../constants";
import { createMediaResource, nowIso, primaryUrl, uid } from "../fixtures";
import { historyDisplayText, historyToNodeKind } from "../services/generation";
import type { Asset, AssetKind, CanvasNodeData, GenerationHistory, LibNode, NodeKind, PreviewResource } from "../types";

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
        localPath: asset.resource.localPath,
        cachePath: asset.resource.cachePath,
        remoteUrl: asset.resource.remoteUrl,
        workflowType: "asset",
        output: {
          resources: [asset.resource],
          preview: {
            id: uid("preview"),
            title: asset.name,
            kind: asset.kind,
            items: [asset.resource]
          }
        },
        sourceRefs: [{ id: asset.id, label: asset.name, kind: "asset" }],
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
        url: item.resultUrl ?? primaryUrl(item.resultResources?.[0]),
        prompt: item.kind === "text" ? text : item.revisedPrompt ?? item.prompt,
        text: item.kind === "text" ? text : undefined,
        params: item.params,
        workflowType: item.resultResources?.length ? "generated" : "reference",
        output:
          item.kind === "text"
            ? undefined
            : {
                resources: item.resultResources ?? [],
                batchId: item.batchId,
                preview: item.resultResources?.length
                  ? ({
                      id: uid("preview"),
                      title: text,
                      kind: item.kind,
                      items: item.resultResources
                    } satisfies PreviewResource)
                  : undefined
              },
        sourceRefs: [{ id: item.id, label: text, kind: "history" }],
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
            createdAt: nowIso(),
            resource: createMediaResource(kind, file.name, url, {
              mimeType: file.type,
              localPath: file.name,
              fileSize: file.size
            }),
            tags: ["本地导入"],
            uses: 0
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
