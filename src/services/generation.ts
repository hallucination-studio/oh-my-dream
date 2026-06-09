import type { GenerationHistory, GenerationParams, NodeKind } from "../types";

export function historyToNodeKind(kind: GenerationHistory["kind"]): NodeKind {
  if (kind === "text") {
    return "text";
  }
  return kind;
}

export function historyDisplayText(item: GenerationHistory) {
  return item.resultText || item.prompt;
}

export function formatGenerationParams(params?: GenerationParams) {
  if (!params) {
    return "";
  }
  return Object.entries(params)
    .filter(([, value]) => value !== "" && value !== undefined)
    .map(([key, value]) => `${key}: ${String(value)}`)
    .join(" · ");
}

export function stringParam(params: GenerationParams, key: string, fallback: string) {
  const value = params[key];
  return typeof value === "string" && value ? value : fallback;
}

export function numberParam(params: GenerationParams, key: string, fallback: number) {
  const value = params[key];
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

export function extractResponseText(payload: unknown) {
  const record = payload as Record<string, unknown>;
  if (typeof record.output_text === "string") {
    return record.output_text;
  }
  const output = Array.isArray(record.output) ? record.output : [];
  const chunks: string[] = [];
  output.forEach((item) => {
    const content = (item as { content?: unknown }).content;
    if (!Array.isArray(content)) {
      return;
    }
    content.forEach((part) => {
      const maybe = part as { text?: unknown };
      if (typeof maybe.text === "string") {
        chunks.push(maybe.text);
      }
    });
  });
  return chunks.join("\n").trim();
}

export function extractImageResult(payload: unknown) {
  const record = payload as { data?: unknown };
  const [first] = Array.isArray(record.data) ? record.data : [];
  const image = first as { b64_json?: unknown; url?: unknown; revised_prompt?: unknown } | undefined;
  if (!image) {
    return {};
  }
  const revisedPrompt =
    typeof image.revised_prompt === "string" && image.revised_prompt ? image.revised_prompt : undefined;
  if (typeof image.b64_json === "string" && image.b64_json) {
    return { url: `data:image/png;base64,${image.b64_json}`, revisedPrompt };
  }
  if (typeof image.url === "string" && image.url) {
    return { url: image.url, revisedPrompt };
  }
  return { revisedPrompt };
}
