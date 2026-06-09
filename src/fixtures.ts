import type {
  AppConfig,
  AppUi,
  Asset,
  AssetKind,
  DerivedBatch,
  GenerationHistory,
  LibEdge,
  LibNode,
  LocalMediaResource,
  NodeKind,
  Project,
  TaskRecord
} from "./types";

export const KEY_PROJECTS = "omd.projects";
export const KEY_ASSETS = "omd.assets";
export const KEY_HISTORY = "omd.history";
export const KEY_CONFIG = "omd.config";
export const KEY_UI = "omd.ui";
export const KEY_TASKS = "omd.tasks";
export const KEY_BATCHES = "omd.batches";

function svgDataUrl(svg: string) {
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

function createCoverImage(index: number) {
  const palettes = [
    ["#f6f9ff", "#dcecff", "#8ebcff", "#0f172a"],
    ["#f7f6ff", "#e7e3ff", "#b8b0ff", "#18223d"],
    ["#f8fbf7", "#ddeee5", "#93c8af", "#143025"],
    ["#fff7f3", "#ffe0cf", "#ffb38a", "#352015"],
    ["#f5f8fb", "#e2e8f0", "#94a3b8", "#162033"],
    ["#fdf7ff", "#f2defa", "#d3a3f4", "#2f123f"],
    ["#f8fbfd", "#deedf6", "#8dc3df", "#10283b"],
    ["#fffaf3", "#f8e7c6", "#e7b669", "#3d2a15"]
  ] as const;
  const [surface, tint, accent, ink] = palettes[index % palettes.length];
  return svgDataUrl(`
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 720">
      <defs>
        <linearGradient id="bg-${index}" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stop-color="${surface}" />
          <stop offset="100%" stop-color="${tint}" />
        </linearGradient>
      </defs>
      <rect width="1280" height="720" rx="40" fill="url(#bg-${index})" />
      <circle cx="226" cy="190" r="136" fill="${accent}" opacity=".22" />
      <circle cx="1060" cy="132" r="98" fill="${ink}" opacity=".08" />
      <rect x="124" y="120" width="620" height="320" rx="34" fill="#ffffff" opacity=".78" />
      <rect x="156" y="154" width="296" height="18" rx="9" fill="${ink}" opacity=".14" />
      <rect x="156" y="198" width="442" height="76" rx="24" fill="${ink}" opacity=".88" />
      <rect x="156" y="300" width="360" height="22" rx="11" fill="${ink}" opacity=".18" />
      <rect x="156" y="344" width="314" height="22" rx="11" fill="${ink}" opacity=".12" />
      <rect x="802" y="192" width="316" height="292" rx="38" fill="${ink}" opacity=".08" />
      <rect x="846" y="236" width="228" height="150" rx="28" fill="${accent}" opacity=".7" />
      <rect x="124" y="526" width="1032" height="56" rx="28" fill="#ffffff" opacity=".76" />
    </svg>
  `);
}

function createAvatarImage(name: string, index: number) {
  const tones = [
    ["#0a84ff", "#dcecff"],
    ["#34c759", "#dff5e6"],
    ["#ff9f0a", "#ffedd0"],
    ["#bf5af2", "#f1ddfb"]
  ] as const;
  const [accent, surface] = tones[index % tones.length];
  const initials = Array.from(name.trim())[0] ?? "A";
  return svgDataUrl(`
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96">
      <rect width="96" height="96" rx="48" fill="${surface}" />
      <circle cx="48" cy="48" r="34" fill="${accent}" opacity=".2" />
      <text x="50%" y="54%" text-anchor="middle" dominant-baseline="middle"
        font-family="SF Pro Display, PingFang SC, Arial, sans-serif"
        font-size="34" font-weight="700" fill="${accent}">${initials}</text>
    </svg>
  `);
}

export const imageCovers = Array.from({ length: 8 }, (_, index) => createCoverImage(index));

export const sampleVideo =
  "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4";
export const sampleAudio =
  "https://www.soundhelix.com/examples/mp3/SoundHelix-Song-1.mp3";

export function createMediaResource(
  kind: AssetKind,
  title: string,
  url: string,
  extra: Partial<LocalMediaResource> = {}
): LocalMediaResource {
  return {
    id: uid(`media-${kind}`),
    kind,
    title,
    dataUrl: url.startsWith("data:") ? url : undefined,
    remoteUrl: url.startsWith("http") ? url : undefined,
    localPath: extra.localPath ?? title,
    cachePath: extra.cachePath ?? `.cache/${title}`,
    createdAt: extra.createdAt ?? nowIso(),
    ...extra
  };
}

export function primaryUrl(resource?: LocalMediaResource) {
  return resource?.dataUrl ?? resource?.remoteUrl ?? "";
}

export function makeWorkspacePath(name: string) {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9\u4e00-\u9fa5]+/gi, "-")
    .replace(/^-+|-+$/g, "");
  return `workspace/${slug || "untitled"}`;
}

export const templateCategories = [
  "全部",
  "叙事短片",
  "品牌广告",
  "角色概念",
  "空间氛围",
  "产品视觉",
  "教育内容",
  "工作流参考"
];

export const templates = [
  ["凌晨地铁站的无对白短片", "yoimachigusa", "叙事短片", "先锋", "328", "最佳叙事节奏"],
  ["一镜到底的港口晨雾练习", "ZeteroGeneouZ", "空间氛围", "", "120", ""],
  ["中古家电广告的空间统一性", "三千问Atelier", "品牌广告", "专业", "87", ""],
  ["Y2K 手机开箱自动化工作流", "贾麦子", "工作流参考", "先锋", "64", ""],
  ["角色关系图到镜头拆解", "Tassi", "角色概念", "先锋", "154", "推荐流程"],
  ["黎明之刃角色预告片", "133****2591", "产品视觉", "", "92", ""],
  ["高端饮品 15 秒节奏广告", "是YY呀", "品牌广告", "先锋", "216", ""],
  ["二十四节气节奏海报", "小团长安铺子", "教育内容", "先锋", "56", ""],
  ["Wrong Room 异常空间实验", "简恩", "叙事短片", "", "185", "视觉表现突出"],
  ["迷宫追逐的分镜铺排", "Babluer拜拜", "叙事短片", "先锋", "72", ""],
  ["异常放送 File 02", "Chiraku", "空间氛围", "", "44", ""],
  ["蛋仔派对角色曲 MV", "那边的蛋仔", "角色概念", "先锋", "275", ""],
  ["AI 音乐短片模板", "Zeno", "工作流参考", "", "404", ""],
  ["高端手柄概念广告", "追逐星辰", "产品视觉", "先锋", "113", ""],
  ["惊悚短片情绪板", "niu_456000", "叙事短片", "先锋", "66", ""]
].map(([title, author, category, tier, views, award], index) => ({
  id: `tpl-${index + 1}`,
  title,
  author,
  category,
  tier,
  award,
  avatar: createAvatarImage(author, index),
  cover: imageCovers[index % imageCovers.length],
  views,
  uses: 210 + index * 31
}));

export const toolboxPresets = [
  {
    id: "ad-flow",
    name: "商业广告三段式",
    thumb: imageCovers[2],
    description: "脚本、分镜、视频生成",
    kinds: ["text", "image", "video"] as NodeKind[]
  },
  {
    id: "storyboard",
    name: "短剧故事板",
    thumb: imageCovers[3],
    description: "脚本到多镜头占位",
    kinds: ["script", "image", "image"] as NodeKind[]
  },
  {
    id: "director-shot",
    name: "导演台构图",
    thumb: imageCovers[1],
    description: "场景描述、截图、视频",
    kinds: ["director", "image", "video"] as NodeKind[]
  },
  {
    id: "music-video",
    name: "音乐视频草案",
    thumb: imageCovers[4],
    description: "歌词、音频、画面",
    kinds: ["text", "audio", "video"] as NodeKind[]
  },
  {
    id: "image-tools",
    name: "图片增强工具组",
    thumb: imageCovers[6],
    description: "九宫格、高清、打光",
    kinds: ["image", "image", "image"] as NodeKind[]
  },
  {
    id: "compose-video",
    name: "视频合成 Beta",
    thumb: imageCovers[7],
    description: "多视频片段到成片",
    kinds: ["video", "video", "compose"] as NodeKind[]
  }
];

export const uid = (prefix = "id") =>
  `${prefix}-${Math.random().toString(36).slice(2, 8)}-${Date.now().toString(36)}`;

export const nowIso = () => new Date().toISOString();

export const defaultConfig: AppConfig = {
  providers: {
    openai: {
      apiKey: "",
      baseUrl: "https://api.openai.com/v1",
      enabled: false,
      models: {
        text: "gpt-5.5",
        image: "gpt-image-2"
      }
    },
    volcengineArk: {
      apiKey: "",
      baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
      enabled: false,
      models: {
        image: "doubao-seedream-5-0-lite-251215",
        video: "doubao-seedance-2-0-260128"
      },
      defaults: {
        imageSize: "2048x2048",
        videoResolution: "720p",
        videoRatio: "16:9",
        videoDuration: 5,
        generateAudio: true,
        watermark: false
      }
    },
    seedanceMock: {
      enabled: false,
      models: {
        video: "seedance-2.0-mock",
        audio: "seedance-audio-mock"
      },
      defaults: {
        resolution: "720P",
        duration: 5
      },
      mockLatencyMs: 1800
    }
  },
  capabilityDefaults: {
    text: "openai",
    image: "volcengine-ark",
    video: "volcengine-ark",
    audio: "local"
  }
};

export const defaultUi: AppUi = {
  noticeDismissed: false,
  minimap: false,
  snapToGrid: false,
  folders: []
};

export function createNode(
  kind: NodeKind,
  name: string,
  x: number,
  y: number,
  extra: Partial<LibNode["data"]> = {}
): LibNode {
  const dimensions: Record<NodeKind, { width: number; height: number }> = {
    text: { width: 350, height: 350 },
    image: { width: 623, height: 350 },
    video: { width: 622, height: 350 },
    audio: { width: 420, height: 260 },
    compose: { width: 420, height: 300 },
    director: { width: 430, height: 330 },
    script: { width: 390, height: 350 },
    group: { width: 520, height: 360 }
  };

  return {
    id: uid(kind),
    type: "libNode",
    position: { x, y },
    data: {
      kind,
      name,
      prompt: "",
      contentWidth: dimensions[kind].width,
      contentHeight: dimensions[kind].height,
      workflowType: "base",
      ...extra
    }
  };
}

export function createBlankProject(name = "未命名", seedance = false): Project {
  const createdAt = nowIso();
  const firstNode = seedance
    ? createNode("video", "Seedance2.0 视频", 120, 120, {
        prompt: "用电影感镜头生成一个 5 秒创意广告片段",
        params: {
          model: defaultConfig.providers.volcengineArk.models.video,
          provider: "volcengine-ark",
          modeType: "text2video",
          ratio: "16:9",
          resolution: "720p",
          duration: 5,
          generateAudio: true
        }
      })
    : createNode("text", "创意文本", 120, 120, {
        prompt: "写下你的第一段脚本或广告词。"
      });

  return {
    id: uid("project"),
    name,
    coverUrl: seedance ? imageCovers[0] : imageCovers[2],
    createdAt,
    updatedAt: createdAt,
    nodes: [firstNode],
    edges: [],
    viewport: { x: 0, y: 0, zoom: 0.85 },
    workspacePath: makeWorkspacePath(name),
    exportPath: `${makeWorkspacePath(name)}/exports`
  };
}

export function createTemplateProject(templateId: string, readonly = false): Project {
  const template = templates.find((item) => item.id === templateId) ?? templates[0];
  const createdAt = nowIso();
  const textNode = createNode("script", `${template.title}脚本`, 40, 120, {
    prompt: `基于「${template.title}」生成三幕式创作过程。`,
    text: "1. 建立世界观与角色动机\n2. 输出关键分镜和画面提示词\n3. 合成短视频并加入音效节奏",
    readonly
  });
  const imageNode = createNode("image", "关键视觉", 520, 120, {
    url: template.cover,
    output: {
      resources: [createMediaResource("image", "关键视觉", template.cover)],
      preview: {
        id: uid("preview"),
        title: "关键视觉",
        kind: "image",
        items: [createMediaResource("image", "关键视觉", template.cover)]
      }
    },
    prompt: `${template.title} 的主视觉参考`,
    readonly
  });
  const videoNode = createNode("video", "成片预览", 1210, 120, {
    url: sampleVideo,
    prompt: `${template.title} 的视频预览`,
    params: {
      model: defaultConfig.providers.volcengineArk.models.video,
      provider: "volcengine-ark",
      modeType: "image2video",
      ratio: "16:9",
      resolution: "720p",
      duration: 5,
      generateAudio: true
    },
    readonly
  });
  const edges: LibEdge[] = [
    { id: uid("edge"), source: textNode.id, target: imageNode.id },
    { id: uid("edge"), source: imageNode.id, target: videoNode.id }
  ];

  return {
    id: uid("template"),
    name: readonly ? `${template.title} 创作过程` : `${template.title} 副本`,
    coverUrl: template.cover,
    createdAt,
    updatedAt: createdAt,
    nodes: [textNode, imageNode, videoNode],
    edges,
    viewport: { x: 0, y: 0, zoom: 0.62 },
    readonly,
    workspacePath: makeWorkspacePath(template.title),
    exportPath: `${makeWorkspacePath(template.title)}/exports`
  };
}

export function createReferenceCanvasProject(): Project {
  const createdAt = nowIso();
  const scriptNode = createNode("text", "文本节点 2", 920, 760, {
    text: "尝试：\n自己编写内容\n文生视频\n图片反推提示词\n文字生音乐",
    prompt: "某位来自洛圣都的 NPC，在一次霓虹雨夜里遇见不该出现的人。",
    contentWidth: 250,
    contentHeight: 210
  });
  const groupNode = createNode("group", "角色 3 个节点", 360, 960, {
    params: { count: 3 },
    contentWidth: 780,
    contentHeight: 150
  });

  const characterNodes = [0, 1, 2].map((index) =>
    createNode("image", `图片节点 ${index + 9}`, 390 + index * 250, 985, {
      url: imageCovers[(index + 2) % imageCovers.length],
      prompt: `角色设定 ${index + 1}`,
      contentWidth: 230,
      contentHeight: 130
    })
  );

  const leftColumn = [0, 1, 2, 3, 4].map((index) =>
    createNode(index === 4 ? "video" : "image", `${index === 4 ? "视频" : "图片"}节点 ${index + 40}`, 1280, 240 + index * 210, {
      url: index === 4 ? sampleVideo : imageCovers[(index + 1) % imageCovers.length],
      prompt: `镜头参考 ${index + 1}`,
      contentWidth: 310,
      contentHeight: 180
    })
  );

  const rightColumn = [0, 1, 2, 3, 4, 5, 6].map((index) =>
    createNode(index % 3 === 0 ? "video" : "image", `${index % 3 === 0 ? "视频" : "图片"}节点 ${index + 11}`, 1850, 130 + index * 175, {
      url: index % 3 === 0 ? sampleVideo : imageCovers[(index + 4) % imageCovers.length],
      prompt: `成片镜头 ${index + 1}`,
      contentWidth: 310,
      contentHeight: 170
    })
  );

  const lowerRow = [0, 1, 2, 3, 4, 5].map((index) =>
    createNode("image", `图片节点 ${index + 21}`, 1220 + index * 360, 1420 + (index % 2) * 210, {
      url: imageCovers[(index + 5) % imageCovers.length],
      prompt: `追加镜头 ${index + 1}`,
      contentWidth: 300,
      contentHeight: 170
    })
  );

  const allNodes = [groupNode, ...characterNodes, scriptNode, ...leftColumn, ...rightColumn, ...lowerRow];
  const edges: LibEdge[] = [
    ...characterNodes.map((node) => ({ id: uid("edge"), source: groupNode.id, target: node.id })),
    ...leftColumn.map((node) => ({ id: uid("edge"), source: scriptNode.id, target: node.id })),
    ...rightColumn.map((node, index) => ({
      id: uid("edge"),
      source: leftColumn[index % leftColumn.length].id,
      target: node.id
    })),
    ...lowerRow.map((node, index) => ({
      id: uid("edge"),
      source: index % 2 === 0 ? characterNodes[index % characterNodes.length].id : rightColumn[index % rightColumn.length].id,
      target: node.id
    }))
  ];

  return {
    id: "reference-local",
    name: "凌晨地铁站的无对白短片 - 副本",
    coverUrl: imageCovers[0],
    createdAt,
    updatedAt: createdAt,
    nodes: allNodes,
    edges,
    viewport: { x: 40, y: 40, zoom: 0.28 },
    workspacePath: makeWorkspacePath("romantic-reference"),
    exportPath: `${makeWorkspacePath("romantic-reference")}/exports`
  };
}

export const seedProjects = (): Project[] => [
  createReferenceCanvasProject(),
  createTemplateProject("tpl-1"),
  createTemplateProject("tpl-2")
];

export const seedAssets = (): Asset[] => [
  {
    id: uid("asset"),
    kind: "image",
    name: "城市主视觉",
    url: imageCovers[0],
    category: "scene",
    createdAt: nowIso(),
    resource: createMediaResource("image", "城市主视觉", imageCovers[0]),
    tags: ["城市", "主视觉"],
    uses: 2
  }
];

export const seedHistory = (): GenerationHistory[] => [
  {
    id: uid("history"),
    kind: "image",
    provider: "local",
    model: "mock-image-tool",
    prompt: "首页示例历史图像",
    status: "done",
    progress: 100,
    resultUrl: imageCovers[1],
    createdAt: nowIso(),
    resultResources: [createMediaResource("image", "首页示例历史图像", imageCovers[1])]
  }
];

export const seedTasks = (): TaskRecord[] => [
  {
    id: uid("task"),
    kind: "derive",
    status: "done",
    title: "多角度派生示例",
    provider: "local",
    progress: 100,
    detail: "已写入本地资产库",
    createdAt: nowIso(),
    updatedAt: nowIso()
  }
];

export const seedBatches = (): DerivedBatch[] => [];
