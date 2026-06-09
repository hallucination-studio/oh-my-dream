import { Handle, NodeToolbar, Position, type NodeProps } from "@xyflow/react";
import {
  AudioLines,
  BookOpen,
  Box,
  FileText,
  Image as ImageIcon,
  Layers3,
  LoaderCircle,
  Sparkles,
  SquareSplitHorizontal,
  Type,
  Video,
  Wand2
} from "lucide-react";
import type { ReactNode } from "react";
import { nodeLabels } from "../constants";
import type { AssetKind, CanvasNodeData, LibNode, NodeKind, PreviewResource } from "../types";
import { Button } from "./ui";

export interface LibNodeComponentProps extends NodeProps<LibNode> {
  onUpdate: (id: string, patch: Partial<CanvasNodeData>) => void;
  onOpenAIText: (id: string) => void;
  onOpenAIImage: (id: string) => void;
  onSeedance: (id: string, kind?: "video" | "audio" | "compose") => void;
  onImageTool: (id: string, label: string) => void;
  onDirectorShot: (id: string) => void;
  onStoryboard: (id: string) => void;
  onQuickAction: (id: string, action: string) => void;
  onPreview: (preview: PreviewResource) => void;
  onDownload: (id: string) => void;
}

export function LibNodeComponent({
  id,
  data,
  selected,
  onUpdate,
  onOpenAIText,
  onOpenAIImage,
  onSeedance,
  onImageTool,
  onDirectorShot,
  onStoryboard,
  onQuickAction,
  onPreview,
  onDownload
}: LibNodeComponentProps) {
  const nodeData = data as CanvasNodeData;
  const readonly = Boolean(nodeData.readonly);
  const params = nodeData.params ?? {};
  const width = Number(nodeData.contentWidth ?? 360);
  const height = Number(nodeData.contentHeight ?? 280);
  const compactNode = width <= 340 || height <= 240;
  const setParam = (key: string, value: string | number | boolean) =>
    onUpdate(id, { params: { ...params, [key]: value } });

  return (
    <section
      className={`flow-node flow-node-${nodeData.kind} ${readonly ? "readonly-node" : ""} ${compactNode ? "compact-node" : ""} ${selected ? "selected" : ""}`}
      style={compactNode ? { width, height } : { width, minHeight: height }}
    >
      <NodeToolbar isVisible={selected && nodeData.kind === "image"} position={Position.Top} align="center">
        <div className="node-toolbar">
          {["全景 NEW", "多角度", "打光", "九宫格", "高清", "宫格切分", "标注", "旋转与镜像"].map((label) => (
            <Button key={label} className="nodrag nopan" size="sm" onClick={() => onImageTool(id, label)}>
              {label}
            </Button>
          ))}
          {nodeData.output?.preview && (
            <Button className="nodrag nopan" size="sm" onClick={() => onPreview(nodeData.output!.preview!)}>
              预览
            </Button>
          )}
          <Button className="nodrag nopan" size="sm" onClick={() => onDownload(id)}>
            下载
          </Button>
        </div>
      </NodeToolbar>
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

      {nodeData.kind === "text" && (
        <div className="node-content">
          <textarea
            className="nodrag"
            name={`node-${id}-text`}
            aria-label="文本节点内容"
            value={nodeData.text ?? nodeData.prompt ?? ""}
            readOnly={readonly}
            onChange={(event) => onUpdate(id, { text: event.target.value, prompt: event.target.value })}
            placeholder="输入剧本、广告词或品牌文案"
          />
          <div className="node-action-row">
            <Button className="nodrag nopan" size="sm" onClick={() => onUpdate(id, { text: nodeData.text ?? "" })}>
              自己编写内容
            </Button>
            <Button className="nodrag nopan" size="sm" onClick={() => onQuickAction(id, "text2video")}>
              文生视频
            </Button>
            <Button className="nodrag nopan" size="sm" onClick={() => onQuickAction(id, "imagePrompt")}>
              图片反推提示词
            </Button>
            <Button className="nodrag nopan" size="sm" onClick={() => onQuickAction(id, "music")}>
              文字生音乐
            </Button>
          </div>
          <Button className="nodrag nopan" variant="primary" onClick={() => onOpenAIText(id)} disabled={readonly}>
            <Wand2 size={14} />
            生成文本
          </Button>
        </div>
      )}

      {nodeData.kind === "image" && (
        <div className="node-content">
          <MediaPreview kind="image" url={nodeData.url} compact={compactNode} />
          {nodeData.output?.resources && nodeData.output.resources.length > 1 && (
            <div className="node-gallery-strip">
              {nodeData.output.resources.slice(0, 5).map((resource) => (
                <button
                  key={resource.id}
                  type="button"
                  className="node-gallery-thumb nodrag nopan"
                  onClick={() =>
                    nodeData.output?.preview &&
                    onPreview({ ...nodeData.output.preview, activeIndex: nodeData.output.resources.findIndex((item) => item.id === resource.id) })
                  }
                >
                  <img src={resource.dataUrl ?? resource.remoteUrl} alt={resource.title} />
                </button>
              ))}
            </div>
          )}
          <textarea
            className="nodrag compact-textarea"
            name={`node-${id}-image-prompt`}
            aria-label="图片提示词"
            value={nodeData.prompt ?? ""}
            readOnly={readonly}
            onChange={(event) => onUpdate(id, { prompt: event.target.value })}
            placeholder="图片提示词"
          />
          <Button className="nodrag nopan" variant="primary" size="sm" onClick={() => onOpenAIImage(id)} disabled={readonly}>
            <Sparkles size={14} />
            OpenAI 生成
          </Button>
          <div className="node-meta-row">
            <span>{String(params.model ?? "本地工作区图片流")}</span>
            <span>{String(params.ratio ?? "16:9")}</span>
            <span>{String(params.outputCount ?? nodeData.output?.resources.length ?? 1)} 张</span>
          </div>
          {nodeData.sourceRefs?.length ? (
            <div className="node-reference-row">
              {nodeData.sourceRefs.map((ref) => (
                <span key={ref.id}>{ref.label}</span>
              ))}
            </div>
          ) : null}
          {nodeData.annotations?.length ? (
            <div className="annotation-list">
              {nodeData.annotations.map((item) => (
                <span key={item}>{item}</span>
              ))}
            </div>
          ) : null}
        </div>
      )}

      {nodeData.kind === "video" && (
        <div className="node-content">
          <MediaPreview kind="video" url={nodeData.url} compact={compactNode} />
          <textarea
            className="nodrag compact-textarea"
            name={`node-${id}-video-prompt`}
            aria-label="视频提示词"
            value={nodeData.prompt ?? ""}
            readOnly={readonly}
            onChange={(event) => onUpdate(id, { prompt: event.target.value })}
            placeholder="视频提示词"
          />
          <div className="param-grid">
            <select
              className="nodrag"
              name={`node-${id}-video-mode`}
              aria-label="视频模式"
              value={String(params.modeType ?? "text2video")}
              onChange={(event) => setParam("modeType", event.target.value)}
              disabled={readonly}
            >
              <option value="text2video">text2video</option>
              <option value="image2video">image2video</option>
            </select>
            <select
              className="nodrag"
              name={`node-${id}-video-ratio`}
              aria-label="视频比例"
              value={String(params.ratio ?? "16:9")}
              onChange={(event) => setParam("ratio", event.target.value)}
              disabled={readonly}
            >
              <option>16:9</option>
              <option>9:16</option>
              <option>1:1</option>
            </select>
            <select
              className="nodrag"
              name={`node-${id}-video-resolution`}
              aria-label="视频分辨率"
              value={String(params.resolution ?? "720P")}
              onChange={(event) => setParam("resolution", event.target.value)}
              disabled={readonly}
            >
              <option>480P</option>
              <option>720P</option>
              <option>1080P</option>
            </select>
            <select
              className="nodrag"
              name={`node-${id}-video-duration`}
              aria-label="视频时长"
              value={String(params.duration ?? 5)}
              onChange={(event) => setParam("duration", Number(event.target.value))}
              disabled={readonly}
            >
              <option value={3}>3s</option>
              <option value={5}>5s</option>
              <option value={6}>6s</option>
              <option value={10}>10s</option>
            </select>
          </div>
          <Button className="nodrag nopan" variant="primary" onClick={() => onSeedance(id, "video")} disabled={readonly}>
            <Video size={14} />
            Seedance mock
          </Button>
        </div>
      )}

      {nodeData.kind === "audio" && (
        <div className="node-content">
          <MediaPreview kind="audio" url={nodeData.url} compact={compactNode} />
          <textarea
            className="nodrag compact-textarea"
            name={`node-${id}-audio-prompt`}
            aria-label="音频提示词"
            value={nodeData.prompt ?? ""}
            readOnly={readonly}
            onChange={(event) => onUpdate(id, { prompt: event.target.value })}
            placeholder="音效、配音或音乐描述"
          />
          <div className="param-grid">
            <input
              className="nodrag"
              name={`node-${id}-audio-model`}
              aria-label="音频模型"
              value={String(params.model ?? "seedance-audio-mock")}
              onChange={(event) => setParam("model", event.target.value)}
              disabled={readonly}
            />
            <select
              className="nodrag"
              name={`node-${id}-audio-duration`}
              aria-label="音频时长"
              value={String(params.duration ?? 5)}
              onChange={(event) => setParam("duration", Number(event.target.value))}
              disabled={readonly}
            >
              <option value={3}>3s</option>
              <option value={5}>5s</option>
              <option value={10}>10s</option>
            </select>
          </div>
          <Button className="nodrag nopan" variant="primary" onClick={() => onSeedance(id, "audio")} disabled={readonly}>
            <AudioLines size={14} />
            音频 mock
          </Button>
        </div>
      )}

      {nodeData.kind === "compose" && (
        <div className="node-content">
          <div className="compose-box">
            <SquareSplitHorizontal size={26} />
            <span>接收多个视频输入并按顺序合成</span>
          </div>
          <div className="param-grid">
            <select
              className="nodrag"
              name={`node-${id}-compose-transition`}
              aria-label="合成转场"
              value={String(params.transition ?? "crossfade")}
              onChange={(event) => setParam("transition", event.target.value)}
              disabled={readonly}
            >
              <option value="crossfade">交叉淡化</option>
              <option value="cut">硬切</option>
              <option value="wipe">擦除</option>
            </select>
            <select
              className="nodrag"
              name={`node-${id}-compose-ratio`}
              aria-label="合成比例"
              value={String(params.ratio ?? "16:9")}
              onChange={(event) => setParam("ratio", event.target.value)}
              disabled={readonly}
            >
              <option>16:9</option>
              <option>9:16</option>
              <option>1:1</option>
            </select>
          </div>
          <Button className="nodrag nopan" variant="primary" onClick={() => onSeedance(id, "compose")} disabled={readonly}>
            生成合成视频
          </Button>
        </div>
      )}

      {nodeData.kind === "director" && (
        <div className="node-content">
          <textarea
            className="nodrag"
            name={`node-${id}-director-prompt`}
            aria-label="导演台提示词"
            value={nodeData.prompt ?? ""}
            readOnly={readonly}
            onChange={(event) => onUpdate(id, { prompt: event.target.value })}
            placeholder="场景描述、镜头、角色、灯光"
          />
          <div className="param-grid">
            <input
              className="nodrag"
              name={`node-${id}-director-camera`}
              aria-label="镜头参数"
              value={String(params.camera ?? "35mm medium shot")}
              onChange={(event) => setParam("camera", event.target.value)}
              disabled={readonly}
            />
            <input
              className="nodrag"
              name={`node-${id}-director-character`}
              aria-label="角色参数"
              value={String(params.character ?? "主角")}
              onChange={(event) => setParam("character", event.target.value)}
              disabled={readonly}
            />
          </div>
          <Button className="nodrag nopan" variant="primary" onClick={() => onDirectorShot(id)} disabled={readonly}>
            <ImageIcon size={14} />
            截图为参考
          </Button>
        </div>
      )}

      {nodeData.kind === "script" && (
        <div className="node-content">
          <textarea
            className="nodrag"
            name={`node-${id}-script`}
            aria-label="脚本内容"
            value={nodeData.text ?? nodeData.prompt ?? ""}
            readOnly={readonly}
            onChange={(event) => onUpdate(id, { text: event.target.value, prompt: event.target.value })}
            placeholder="创意脚本、镜头、对白、场景"
          />
          <Button className="nodrag nopan" variant="primary" onClick={() => onStoryboard(id)} disabled={readonly}>
            <Layers3 size={14} />
            生成故事板
          </Button>
        </div>
      )}

      {nodeData.kind === "group" && (
        <div className="node-content">
          <div className="group-box">
            <Layers3 size={28} />
            <strong>分组 {String(params.count ?? 0)} 个节点</strong>
          </div>
        </div>
      )}

      {nodeData.taskInfo && (
        <div className={`task-info ${nodeData.taskInfo.status}`}>
          {nodeData.taskInfo.status === "running" && <LoaderCircle size={14} className="spin" />}
          <span>{nodeData.taskInfo.message ?? nodeData.taskInfo.status}</span>
          <progress value={nodeData.taskInfo.progress ?? 0} max={100} />
        </div>
      )}
      <Handle type="source" position={Position.Right} />
    </section>
  );
}

function nodeIcon(kind: NodeKind) {
  const icons: Record<NodeKind, ReactNode> = {
    text: <Type size={16} />,
    image: <ImageIcon size={16} />,
    video: <Video size={16} />,
    audio: <AudioLines size={16} />,
    compose: <SquareSplitHorizontal size={16} />,
    director: <Box size={16} />,
    script: <BookOpen size={16} />,
    group: <Layers3 size={16} />
  };
  return icons[kind];
}

function MediaPreview({ kind, url, compact = false }: { kind: AssetKind; url?: string; compact?: boolean }) {
  if (kind === "image" && url) {
    return <img className="node-media" src={url} alt="" />;
  }
  if (kind === "video" && url) {
    return <video className="node-media" src={url} controls={!compact} muted playsInline preload="metadata" />;
  }
  if (kind === "audio" && url) {
    if (compact) {
      return (
        <div className="audio-thumb node-audio-compact">
          <AudioLines size={22} />
        </div>
      );
    }
    return <audio className="node-audio" src={url} controls />;
  }
  return (
    <div className="media-placeholder">
      {kind === "image" ? <ImageIcon size={28} /> : kind === "video" ? <Video size={28} /> : <AudioLines size={28} />}
      <span>{nodeLabels[kind]}</span>
    </div>
  );
}
