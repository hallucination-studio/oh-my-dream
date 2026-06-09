import {
  AudioLines,
  BookOpen,
  Box,
  Clock3,
  FileImage,
  FileText,
  FileVideo,
  Film,
  Grid2X2,
  Image as ImageIcon,
  Mic2,
  Music,
  PenLine,
  SquareSplitHorizontal,
  Type,
  Upload,
  Video
} from "lucide-react";
import type { ReactNode } from "react";
import { imageCovers } from "../fixtures";
import { historyDisplayText } from "../services/generation";
import type { CanvasNodeData, GenerationHistory, LibNode, NodeKind } from "../types";

export function AddNodePanel({
  onAdd,
  onUpload,
  history,
  onImportHistory
}: {
  onAdd: (kind: NodeKind, name: string, extra?: Partial<CanvasNodeData>) => LibNode | undefined;
  onUpload: (files: FileList | File[]) => void;
  history: GenerationHistory[];
  onImportHistory: (item: GenerationHistory) => void;
}) {
  const groups: { title: string; entries: { kind: NodeKind; name: string; badge?: string; icon: ReactNode }[] }[] = [
    {
      title: "文本",
      entries: [
        { kind: "text", name: "剧本", icon: <FileText size={16} /> },
        { kind: "text", name: "广告词", icon: <Type size={16} /> },
        { kind: "text", name: "品牌文案", icon: <PenLine size={16} /> }
      ]
    },
    {
      title: "图片",
      entries: [
        { kind: "image", name: "海报", icon: <FileImage size={16} /> },
        { kind: "image", name: "分镜", icon: <Grid2X2 size={16} /> },
        { kind: "image", name: "角色设计", icon: <ImageIcon size={16} /> }
      ]
    },
    {
      title: "视频",
      entries: [
        { kind: "video", name: "创意广告", icon: <FileVideo size={16} /> },
        { kind: "video", name: "动画", icon: <Film size={16} /> },
        { kind: "video", name: "电影", icon: <Video size={16} /> }
      ]
    },
    {
      title: "增强",
      entries: [
        { kind: "compose", name: "视频合成", badge: "Beta", icon: <SquareSplitHorizontal size={16} /> },
        { kind: "director", name: "导演台", badge: "NEW", icon: <Box size={16} /> },
        { kind: "script", name: "脚本", badge: "Beta", icon: <BookOpen size={16} /> }
      ]
    },
    {
      title: "音频",
      entries: [
        { kind: "audio", name: "音效", icon: <AudioLines size={16} /> },
        { kind: "audio", name: "配音", icon: <Mic2 size={16} /> },
        { kind: "audio", name: "音乐", icon: <Music size={16} /> }
      ]
    }
  ];
  const latestHistory = history.filter((item) => item.status === "done").slice(0, 4);

  return (
    <div className="drawer-body">
      {groups.map((group) => (
        <section className="node-group" key={group.title}>
          <h3>{group.title}</h3>
          <div className="node-entry-grid">
            {group.entries.map((entry) => (
              <button
                key={`${entry.kind}-${entry.name}`}
                type="button"
                className="node-entry"
                onClick={() =>
                  onAdd(entry.kind, entry.name, {
                    prompt: `${entry.name} 节点`,
                    url: entry.kind === "image" ? imageCovers[Math.floor(Math.random() * imageCovers.length)] : undefined
                  })
                }
              >
                {entry.icon}
                <span>{entry.name}</span>
                {entry.badge && <em>{entry.badge}</em>}
              </button>
            ))}
          </div>
        </section>
      ))}
      <section className="node-group">
        <h3>添加资源</h3>
        <label className="upload-zone">
          <Upload size={18} />
          <span>上传图片、视频、音频文件</span>
          <input
            name="addNodeUpload"
            type="file"
            multiple
            accept="image/*,video/*,audio/*"
            onChange={(event) => event.target.files && onUpload(event.target.files)}
          />
        </label>
        <div className="history-mini-list">
          {latestHistory.length === 0 ? (
            <p className="empty-copy">暂无生成历史可选。</p>
          ) : (
            latestHistory.map((item) => (
              <button key={item.id} type="button" onClick={() => onImportHistory(item)}>
                <Clock3 size={14} />
                <span>{historyDisplayText(item)}</span>
              </button>
            ))
          )}
        </div>
      </section>
    </div>
  );
}
