import type { AppConfig, GenerationParams } from "../types";
import { extractImageResult, extractResponseText } from "./generation";

type OpenAIConfig = AppConfig["providers"]["openai"];

export const openAIImageRequestParams: GenerationParams = {
  size: "1024x1024",
  quality: "medium",
  output_format: "png"
};

export async function generateOpenAIText(openai: OpenAIConfig, prompt: string) {
  const payload = await requestOpenAIJson(openai, "/responses", "OpenAI 请求失败", {
    model: openai.models.text,
    input: prompt
  });
  return extractResponseText(payload) || "生成成功，但响应中没有可显示文本。";
}

export async function generateOpenAIImage(openai: OpenAIConfig, prompt: string) {
  const payload = await requestOpenAIJson(openai, "/images/generations", "OpenAI 图像请求失败", {
    model: openai.models.image,
    prompt,
    size: openAIImageRequestParams.size,
    quality: openAIImageRequestParams.quality,
    output_format: openAIImageRequestParams.output_format
  });
  const { url: resultUrl, revisedPrompt } = extractImageResult(payload);
  if (!resultUrl) {
    throw new Error("OpenAI 响应中没有可显示图像");
  }
  return { resultUrl, revisedPrompt, requestParams: openAIImageRequestParams };
}

async function requestOpenAIJson(
  openai: OpenAIConfig,
  path: "/responses" | "/images/generations",
  failureLabel: string,
  body: Record<string, unknown>
) {
  const response = await fetch(`${normalizedBaseUrl(openai.baseUrl)}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${openai.apiKey}`
    },
    body: JSON.stringify(body)
  });
  if (!response.ok) {
    throw new Error(await openAIErrorMessage(response, failureLabel));
  }
  return response.json() as Promise<unknown>;
}

async function openAIErrorMessage(response: Response, failureLabel: string) {
  try {
    const payload = (await response.json()) as { error?: { message?: unknown } };
    if (typeof payload.error?.message === "string" && payload.error.message) {
      return `${failureLabel}：${payload.error.message}`;
    }
  } catch {
    // Fall back to the HTTP status when the API response body is not JSON.
  }
  return `${failureLabel}：${response.status}`;
}

function normalizedBaseUrl(baseUrl: string) {
  return (baseUrl.trim() || "https://api.openai.com/v1").replace(/\/$/, "");
}
