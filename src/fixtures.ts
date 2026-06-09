import type { AppConfig, AppUi, Asset, GenerationHistory, LibEdge, LibNode, NodeKind, Project } from "./types";

export const KEY_PROJECTS = "omd.projects";
export const KEY_ASSETS = "omd.assets";
export const KEY_HISTORY = "omd.history";
export const KEY_CONFIG = "omd.config";
export const KEY_UI = "omd.ui";

export const imageCovers = [
  "https://libtv-res.liblib.art/upload-images/a4c1b997d3f84fa8a871ed91d861f88f/996bd408ee659cf13c4ef989b9e84fa949134d1d.png?x-oss-process=image/resize,w_1200,m_lfit/format,webp",
  "https://libtv-res.liblib.art/upload-images/e32469f1cda3481581e3b1fef896d2a7/72051af025c9848e7e7bf7bfdcba77809ea9a4dc.png?x-oss-process=image/resize,w_1200,m_lfit/format,webp",
  "https://libtv-res.liblib.art/upload-images/4516157ad4cf4175bef2cb448d41b9f3/75c546e516b74f1a63df6bf942e11be99313d3fc.png?x-oss-process=image/resize,w_1200,m_lfit/format,webp",
  "https://libtv-res.liblib.art/upload-images/d548bbe5d2194184a0afbc869fd93558/3a1a05f9b7b1362a1207f2c05942ccf56e80b5b1.png?x-oss-process=image/resize,w_1200,m_lfit/format,webp",
  "https://libtv-res.liblib.art/upload-images/1e3a67e7d1214022b9d8cfd35ae3dd7b/d68305126a00f9400ffd2179ed77b700ce0bcf37.png?x-oss-process=image/resize,w_1200,m_lfit/format,webp",
  "https://libtv-res.liblib.art/upload-images/6dd3b41611724db79e60b68a549590cc/2acc72924322941ba7e1b367c5541748cf9c1e6a.jpg?x-oss-process=image/resize,w_1200,m_lfit/format,webp",
  "https://libtv-res.liblib.art/upload-images/72e56fb0d04f4fda82340018214d399b/0202e5021d6896e7eed40ffdec7658cf088c99c3.jpg?x-oss-process=image/resize,w_1200,m_lfit/format,webp",
  "https://libtv-res.liblib.art/upload-images/0c8bad1646dd40ad8d55d1ff6e289ccd/ab57199675db5a66fb418058a0f2230a63b7e312.jpeg?x-oss-process=image/resize,w_1200,m_lfit/format,webp"
];

export const sampleVideo =
  "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4";
export const sampleAudio =
  "https://www.soundhelix.com/examples/mp3/SoundHelix-Song-1.mp3";

export const tvCategories = [
  "全部",
  "大乱斗｜vol.1 显形记",
  "大乱斗｜vol.2《AI，想象和尖叫》",
  "精选画布",
  "专业影视",
  "短剧漫剧",
  "商业广告",
  "动漫游戏",
  "教育生活",
  "TV工具箱"
];

export const banners = [
  {
    title: "Seedance2.0 创意广告流",
    tag: "文生视频",
    cover: "https://liblibai-online.liblib.cloud/banner/1780372235458.webp"
  },
  {
    title: "导演台镜头构图参考",
    tag: "3D 场景",
    cover: "https://libtv-res.liblib.art/upload-images/70a305c50c704a778db114468830617b/9c90f7ec12aa8d8c42fd055abfba849ea193d5d6.webp"
  },
  {
    title: "短剧分镜一键铺排",
    tag: "故事板",
    cover: "https://libtv-res.liblib.art/upload-images/70a305c50c704a778db114468830617b/17c0e59477a29c09914e4727db7fe424c4b1fb27.webp"
  },
  {
    title: "品牌主视觉变体",
    tag: "图片工具",
    cover: "https://liblibai-online.liblib.cloud/banner/1780329415980.webp"
  },
  {
    title: "多节点合成实验",
    tag: "画布模板",
    cover: "https://liblibai-online.liblib.cloud/banner/1775750958026.webp"
  }
];

export const templates = [
  [
    "死于罗曼蒂克 - 某位来自洛圣都的NPCの爱情故事",
    "yoimachigusa",
    "大乱斗｜vol.2《AI，想象和尖叫》",
    "先锋",
    "328",
    "大乱斗｜vol.2《AI，想象和尖叫》 - 最佳画风",
    "https://liblibai-online.liblib.cloud/img/a4c1b997d3f84fa8a871ed91d861f88f/ff003559bede02660ddbfe09b7f93856659fa613018b60102354847275711ff9.jfif?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "AI一镜到底📹｜欢迎来到石湾镇 - Welcome to Stone Bay",
    "ZeteroGeneouZ",
    "精选画布",
    "",
    "20",
    "",
    "https://liblibai-online.liblib.cloud/img/e32469f1cda3481581e3b1fef896d2a7/63fb7366ba33b33c5829459411565e900b6a32d4cd880aa33cb22c0a41dc4972.png?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "中古风室内空间720度空间一致性",
    "三千问Atelier",
    "专业影视",
    "专业",
    "10",
    "",
    "https://liblibai-online.liblib.cloud/img/4516157ad4cf4175bef2cb448d41b9f3/df345921bdcbd6912a6992a2702f9916280444d89f01bc8674e855cdeb27d371.jpg?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "《Y2K-iphone》--自动化工作流",
    "贾麦子",
    "TV工具箱",
    "先锋",
    "12",
    "",
    "https://liblibai-online.liblib.cloud/img/d548bbe5d2194184a0afbc869fd93558/6d7a7e824a181c43360bd265f2563d73ae5af0f9bebd63e9351fb7b694a7a310.png?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "贵司有尾",
    "Tassi",
    "大乱斗｜vol.1 显形记",
    "先锋",
    "369",
    "大乱斗｜vol.1 显形记 - 最佳画布",
    "https://liblibai-online.liblib.cloud/img/1e3a67e7d1214022b9d8cfd35ae3dd7b/56dc99dcc8975afadfa1ceb490c70f7e4a7231b46b7c48105b4944c5f5d1e748.jpg?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "黎明之刃PV",
    "133****2591",
    "动漫游戏",
    "",
    "11",
    "",
    "https://liblibai-online.liblib.cloud/web/avatar/avatar3.png?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "VIVO手机短片《柳宗元的独钓“玄机”》",
    "是YY呀",
    "商业广告",
    "先锋",
    "216",
    "",
    "https://liblibai-online.liblib.cloud/img/72e56fb0d04f4fda82340018214d399b/157ee9ead33568855bf123fa8d52b24a4dc3444a51f4f6503633e7feb5e1b5a6.jpg?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "24节气 | 芒种",
    "小团长安铺子",
    "教育生活",
    "先锋",
    "6",
    "",
    "https://liblibai-online.liblib.cloud/img/0c8bad1646dd40ad8d55d1ff6e289ccd/f53f11ee659e6eb0c67f46192555e6c68a8eb84a01959e3ca17a357fd32a3b6d.jpeg?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "《Wrong Room》",
    "简恩",
    "大乱斗｜vol.2《AI，想象和尖叫》",
    "",
    "185",
    "大乱斗｜vol.2《AI，想象和尖叫》 - 最佳画风",
    "https://liblibai-online.liblib.cloud/img/a379033239b44269ae4bf0ec96dc6773/21e194b33b69d228ebccf4c3c4fc013a19674b48e3b70905976457094f6f669d.jpg?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "奇怪的迷宫",
    "Babluer拜拜",
    "短剧漫剧",
    "先锋",
    "12",
    "",
    "https://liblibai-online.liblib.cloud/img/59aea8d9952445ee9a21201ef7319f3e/5727da5ed292baf8894416934d4567338bea74eba264bd4ba730db14afaa9e25.png?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "《异常放送》丨File 02.形影分离",
    "Chiraku",
    "专业影视",
    "",
    "5",
    "",
    "https://liblibai-online.liblib.cloud/img/b3a333c3f3464d1f8f2c8931291b0a1f/e2780211deedefdbc2c81f09a4dc3a34b39ef5dd3e5fdec3235ca62231011824.jpg?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "Remenber 蛋仔派对 逃出惊魂夜 海瑟角色曲MV",
    "那边的蛋仔",
    "动漫游戏",
    "先锋",
    "275",
    "",
    "https://liblibai-online.liblib.cloud/img/2dfcc8936b424f8db53b641c82b8f3d1/c58ec5fc067e405259f60b6da67a65304ed86dd42d01842b5319d7ca19194011.png?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  ["《UnTouchable》AI音乐MV短片", "Zeno", "精选画布", "", "404", "", ""],
  [
    "沙僧终于不洗了",
    "迈克的AiGC世界",
    "专业影视",
    "专业",
    "12",
    "",
    "https://liblibai-online.liblib.cloud/img/f574471b318b491695503813e0f553cf/f7a9e12155e7b224a9b4934a5ca8f6906e40cc650e6a6f352796eb5b88d1bbba.jpg?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "高端游戏手柄｜世界杯联名款概念TVC",
    "追逐星辰",
    "商业广告",
    "先锋",
    "13",
    "",
    "https://liblibai-online.liblib.cloud/img/bc19270a4e744fa38a01a9ebbdab3244/722e023f39d7dc69916f25658bdde4e848938d9d58c50b42700341045d12e52f.jpg?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
  [
    "见鬼",
    "niu_456000",
    "短剧漫剧",
    "先锋",
    "6",
    "",
    "https://liblibai-online.liblib.cloud/img/b4567961984a4d25be54ada178ab2374/35dfca3166d35102ece91ec209606fb51b3b405a9a6d9effa586465bb2fd3b63.jpg?x-oss-process=image/resize,w_60,m_lfit/format,webp"
  ],
].map(([title, author, category, tier, views, award, avatar], index) => ({
  id: `tpl-${index + 1}`,
  title,
  author,
  category,
  tier,
  award,
  avatar,
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
  openai: {
    apiKey: "",
    baseUrl: "https://api.openai.com/v1",
    textModel: "gpt-5.5",
    imageModel: "gpt-image-2",
    enabled: false
  },
  seedance: {
    enabled: true,
    videoModel: "seedance-2.0-mock",
    audioModel: "seedance-audio-mock",
    resolution: "720P",
    duration: 5,
    mockLatencyMs: 1800
  }
};

export const defaultUi: AppUi = {
  bannerClosed: false,
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
          model: "seedance-2.0-mock",
          modeType: "text2video",
          ratio: "16:9",
          resolution: "720P",
          duration: 5
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
    viewport: { x: 0, y: 0, zoom: 0.85 }
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
    prompt: `${template.title} 的主视觉参考`,
    readonly
  });
  const videoNode = createNode("video", "成片预览", 1210, 120, {
    url: sampleVideo,
    prompt: `${template.title} 的视频预览`,
    params: {
      model: "seedance-2.0-mock",
      modeType: "image2video",
      ratio: "16:9",
      resolution: "720P",
      duration: 5
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
    readonly
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
    id: "libtv-reference-local",
    name: "死于罗曼蒂克 - 某位来自洛圣都的NPCの爱情故事 - 副本",
    coverUrl: imageCovers[0],
    createdAt,
    updatedAt: createdAt,
    nodes: allNodes,
    edges,
    viewport: { x: 40, y: 40, zoom: 0.28 }
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
    createdAt: nowIso()
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
    createdAt: nowIso()
  }
];
