import { AudioLines, FileText, Image as ImageIcon } from "lucide-react";
import type { GenerationHistory } from "../types";

export function MediaThumb({ kind, url }: { kind: GenerationHistory["kind"]; url?: string }) {
  if (kind === "text") {
    return (
      <div className="audio-thumb text-thumb">
        <FileText size={22} />
      </div>
    );
  }
  if (kind === "image" && url) {
    return <img className="media-thumb" src={url} alt="" loading="lazy" />;
  }
  if (kind === "video" && url) {
    return <video className="media-thumb" src={url} muted />;
  }
  if (kind === "audio") {
    return (
      <div className="audio-thumb">
        <AudioLines size={22} />
      </div>
    );
  }
  return (
    <div className="audio-thumb">
      <ImageIcon size={22} />
    </div>
  );
}
