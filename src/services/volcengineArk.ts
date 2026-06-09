import type { AppConfig, GenerationParams, LocalMediaResource } from "../types";

type ArkConfig = AppConfig["providers"]["volcengineArk"];

export interface ArkVideoInput {
  prompt: string;
  images: LocalMediaResource[];
  audios: LocalMediaResource[];
  params: GenerationParams;
}

export interface ArkVideoTask {
  id: string;
}

export interface ArkVideoResult {
  id: string;
  status: "queued" | "running" | "cancelled" | "succeeded" | "failed" | "expired";
  error?: { code?: string; message?: string } | null;
  content?: {
    video_url?: string;
    last_frame_url?: string;
  };
}

export async function generateArkImage(ark: ArkConfig, prompt: string, params: GenerationParams) {
  const image = stringParam(params, "image", "");
  const payload = await requestArkJson(ark, "/images/generations", "火山图片生成失败", {
    model: stringParam(params, "model", ark.models.image),
    prompt,
    ...(image ? { image } : {}),
    size: stringParam(params, "size", ark.defaults.imageSize),
    response_format: "url",
    watermark: booleanParam(params, "watermark", ark.defaults.watermark)
  });
  const record = payload as { data?: unknown };
  const data = Array.isArray(record.data) ? record.data : [];
  const resources = data
    .map((item) => {
      const image = item as { url?: unknown; b64_json?: unknown; error?: { message?: unknown } };
      if (typeof image.error?.message === "string") {
        throw new Error(`火山图片生成失败：${image.error.message}`);
      }
      if (typeof image.url === "string" && image.url) {
        return image.url;
      }
      if (typeof image.b64_json === "string" && image.b64_json) {
        return `data:image/png;base64,${image.b64_json}`;
      }
      return "";
    })
    .filter(Boolean);
  if (resources.length === 0) {
    throw new Error("火山图片响应中没有可显示图像");
  }
  return { urls: resources, requestParams: params };
}

export async function createArkVideoTask(ark: ArkConfig, input: ArkVideoInput): Promise<ArkVideoTask> {
  const body = {
    model: stringParam(input.params, "model", ark.models.video),
    content: buildVideoContent(input),
    resolution: normalizeResolution(stringParam(input.params, "resolution", ark.defaults.videoResolution)),
    ratio: stringParam(input.params, "ratio", ark.defaults.videoRatio),
    duration: numberParam(input.params, "duration", ark.defaults.videoDuration),
    generate_audio: booleanParam(input.params, "generateAudio", ark.defaults.generateAudio),
    watermark: booleanParam(input.params, "watermark", ark.defaults.watermark)
  };
  const payload = await requestArkJson(ark, "/contents/generations/tasks", "火山视频任务创建失败", body);
  const id = (payload as { id?: unknown }).id;
  if (typeof id !== "string" || !id) {
    throw new Error("火山视频任务响应中没有任务 ID");
  }
  return { id };
}

export async function getArkVideoTask(ark: ArkConfig, id: string): Promise<ArkVideoResult> {
  return requestArkJson(
    ark,
    `/contents/generations/tasks/${encodeURIComponent(id)}`,
    "火山视频任务查询失败",
    undefined,
    "GET"
  ) as Promise<ArkVideoResult>;
}

export async function deleteArkVideoTask(ark: ArkConfig, id: string) {
  await requestArkJson(
    ark,
    `/contents/generations/tasks/${encodeURIComponent(id)}`,
    "火山视频任务删除失败",
    undefined,
    "DELETE"
  );
}

function buildVideoContent(input: ArkVideoInput) {
  if (input.audios.length > 0 && input.images.length === 0) {
    throw new Error("Seedance 不支持单独音频输入，请连接图片参考。");
  }
  const content: Record<string, unknown>[] = [];
  const prompt = input.prompt.trim();
  if (prompt) {
    content.push({ type: "text", text: prompt });
  }
  input.images.slice(0, 9).forEach((image, index) => {
    const url = mediaPayloadUrl(image);
    if (!url) {
      throw new Error(`图片参考「${image.title}」缺少 Base64 数据。`);
    }
    content.push({
      type: "image_url",
      image_url: { url },
      role: imageRole(input.images.length, index, input.audios.length > 0)
    });
  });
  input.audios.slice(0, 3).forEach((audio) => {
    const url = mediaPayloadUrl(audio);
    if (!url) {
      throw new Error(`音频参考「${audio.title}」缺少 Base64 数据。`);
    }
    if (!url.startsWith("data:audio/") && !url.startsWith("asset://")) {
      throw new Error(`音频参考「${audio.title}」必须是 Base64 data URL 或 Ark 素材 ID。`);
    }
    if (audio.fileSize && audio.fileSize > 15 * 1024 * 1024) {
      throw new Error(`音频参考「${audio.title}」超过 15MB。`);
    }
    content.push({
      type: "audio_url",
      audio_url: { url },
      role: "reference_audio"
    });
  });
  if (content.length === 0) {
    throw new Error("请输入视频提示词或连接图片参考。");
  }
  return content;
}

function imageRole(total: number, index: number, multimodalReference: boolean) {
  if (multimodalReference) {
    return "reference_image";
  }
  if (total === 1) {
    return "first_frame";
  }
  if (total === 2) {
    return index === 0 ? "first_frame" : "last_frame";
  }
  return "reference_image";
}

function mediaPayloadUrl(resource: LocalMediaResource) {
  return resource.dataUrl || (resource.remoteUrl?.startsWith("asset://") ? resource.remoteUrl : "");
}

async function requestArkJson(
  ark: ArkConfig,
  path: string,
  failureLabel: string,
  body?: Record<string, unknown>,
  method = "POST"
) {
  const response = await fetch(`${normalizedBaseUrl(ark.baseUrl)}${path}`, {
    method,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${ark.apiKey}`
    },
    body: body ? JSON.stringify(body) : undefined
  });
  if (!response.ok) {
    throw new Error(await arkErrorMessage(response, failureLabel));
  }
  if (response.status === 204) {
    return {};
  }
  const text = await response.text();
  return text ? (JSON.parse(text) as unknown) : {};
}

async function arkErrorMessage(response: Response, failureLabel: string) {
  try {
    const payload = (await response.json()) as { error?: { message?: unknown }; message?: unknown };
    if (typeof payload.error?.message === "string" && payload.error.message) {
      return `${failureLabel}：${payload.error.message}`;
    }
    if (typeof payload.message === "string" && payload.message) {
      return `${failureLabel}：${payload.message}`;
    }
  } catch {
    // Fall back to the HTTP status when the response body is not JSON.
  }
  return `${failureLabel}：${response.status}`;
}

function normalizedBaseUrl(baseUrl: string) {
  return (baseUrl.trim() || "https://ark.cn-beijing.volces.com/api/v3").replace(/\/$/, "");
}

function stringParam(params: GenerationParams, key: string, fallback: string) {
  const value = params[key];
  return typeof value === "string" && value ? value : fallback;
}

function numberParam(params: GenerationParams, key: string, fallback: number) {
  const value = params[key];
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function booleanParam(params: GenerationParams, key: string, fallback: boolean) {
  const value = params[key];
  return typeof value === "boolean" ? value : fallback;
}

function normalizeResolution(value: string) {
  return value.replace(/P$/, "p");
}
