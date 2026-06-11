import { Handle, Position, type NodeProps } from "@xyflow/react";
import {
  AudioLines,
  BookOpen,
  Box,
  FileText,
  Image as ImageIcon,
  Layers3,
  LoaderCircle,
  SquareSplitHorizontal,
  Type,
  Video
} from "lucide-react";
import type { ReactNode } from "react";
import { nodeLabels } from "../constants";
import type { AssetKind, CanvasNodeData, LibNode, NodeKind, PreviewResource } from "../types";

export interface LibNodeComponentProps extends NodeProps<LibNode> {
  onUpdate: (id: string, patch: Partial<CanvasNodeData>) => void;
  onOpenAIText: (id: string) => void;
  onGenerateImage: (id: string) => void;
  onGenerateVideo: (id: string, kind?: "video" | "audio" | "compose") => void;
  onImageTool: (id: string, label: string) => void;
  onDirectorShot: (id: string) => void;
  onStoryboard: (id: string) => void;
  onQuickAction: (id: string, action: string) => void;
  onPreview: (preview: PreviewResource) => void;
  onDownload: (id: string) => void;
}

export function LibNodeComponent({ id, data, selected, onUpdate }: LibNodeComponentProps) {
  const nodeData = data as CanvasNodeData;
  const readonly = Boolean(nodeData.readonly);
  const outputCount = nodeData.output?.resources.length ?? (nodeData.url ? 1 : 0);
  const sourceCount = nodeData.sourceRefs?.length ?? 0;
  const summary = nodeSummary(nodeData);

  return (
    <section
      className={`flow-node flow-node-${nodeData.kind} ${readonly ? "readonly-node" : ""} ${selected ? "selected" : ""}`}
      style={{ width: Number(nodeData.contentWidth ?? defaultNodeWidth(nodeData.kind)) }}
    >
      <Handle type="target" position={Position.Left} />
      <header className="node-head">
        <span>{nodeIcon(nodeData.kind)}</span>
        <input
          className="nodrag"
          name={`node-${id}-name`}
          value={nodeData.name}
          readOnly={readonly}
          onChange={(event) => onUpdate(id, { name: event.target.value })}
          aria-label="节点名称"
        />
        <em>{nodeLabels[nodeData.kind]}</em>
      </header>
      <div className="node-preview-shell">
        <NodePreview data={nodeData} />
        <div className="node-summary">
          <p>{summary}</p>
          <div className="node-meta-row">
            <span>{sourceCount} 输入</span>
            <span>{outputCount} 输出</span>
            <span>{nodeData.workflowType ?? "base"}</span>
          </div>
        </div>
      </div>
      {nodeData.taskInfo && (
        <div className={`task-info ${nodeData.taskInfo.status}`}>
          {nodeData.taskInfo.status === "running" && <LoaderCircle size={13} className="spin" />}
          <span>{nodeData.taskInfo.message ?? nodeData.taskInfo.status}</span>
          <progress value={nodeData.taskInfo.progress ?? 0} max={100} />
        </div>
      )}
      <Handle type="source" position={Position.Right} />
    </section>
  );
}

function NodePreview({ data }: { data: CanvasNodeData }) {
  if ((data.kind === "image" || data.kind === "video") && data.url) {
    return <MediaPreview kind={data.kind} url={data.url} />;
  }
  if (data.kind === "audio" && data.url) {
    return <MediaPreview kind="audio" url={data.url} />;
  }
  if (data.kind === "text" || data.kind === "script" || data.kind === "director") {
    return (
      <div className="text-node-preview">
        <FileText size={16} />
        <span>{data.text || data.prompt || "等待写入创作内容"}</span>
      </div>
    );
  }
  if (data.kind === "group") {
    return (
      <div className="workflow-placeholder">
        <Layers3 size={22} />
        <span>组织 {String(data.params?.count ?? 0)} 个节点</span>
      </div>
    );
  }
  return (
    <div className="workflow-placeholder">
      {nodeIcon(data.kind)}
      <span>{nodeLabels[data.kind]}</span>
    </div>
  );
}

function MediaPreview({ kind, url }: { kind: AssetKind; url?: string }) {
  if (kind === "image" && url) {
    return <img className="node-media" src={url} alt="" />;
  }
  if (kind === "video" && url) {
    return <video className="node-media" src={url} muted playsInline preload="metadata" />;
  }
  if (kind === "audio" && url) {
    return (
      <div className="workflow-placeholder">
        <AudioLines size={22} />
        <span>音频已就绪</span>
      </div>
    );
  }
  return null;
}

function nodeSummary(data: CanvasNodeData) {
  if (data.taskInfo?.status === "running" || data.taskInfo?.status === "queued") {
    return data.taskInfo.message ?? "生成任务进行中";
  }
  if (data.taskInfo?.status === "failed") {
    return data.taskInfo.message ?? "生成失败，查看队列处理";
  }
  if (data.output?.resources.length) {
    return `最近输出 ${data.output.resources.length} 个资源`;
  }
  if (data.prompt) {
    return data.prompt;
  }
  if (data.text) {
    return data.text;
  }
  return "在右侧 Inspector 配置参数和生成动作";
}

function defaultNodeWidth(kind: NodeKind) {
  const widths: Record<NodeKind, number> = {
    text: 300,
    image: 320,
    video: 320,
    audio: 280,
    compose: 300,
    director: 300,
    script: 300,
    group: 420
  };
  return widths[kind];
}

function nodeIcon(kind: NodeKind) {
  const icons: Record<NodeKind, ReactNode> = {
    text: <Type size={15} />,
    image: <ImageIcon size={15} />,
    video: <Video size={15} />,
    audio: <AudioLines size={15} />,
    compose: <SquareSplitHorizontal size={15} />,
    director: <Box size={15} />,
    script: <BookOpen size={15} />,
    group: <Layers3 size={15} />
  };
  return icons[kind];
}
