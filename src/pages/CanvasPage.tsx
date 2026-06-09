import {
  addEdge,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  useReactFlow,
  type Connection,
  type EdgeChange,
  type NodeChange,
  type NodeProps,
  type OnSelectionChangeParams,
  type Viewport
} from "@xyflow/react";
import {
  ArrowLeft,
  Copy,
  Grid2X2,
  Home,
  Maximize2,
  PanelLeft,
  Rows3,
  Save,
  Settings
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { AppShell, ConfigModal } from "../components/AppShell";
import {
  AddNodePanel,
  AssetsPanel,
  BottomToolbar,
  CanvasDrawer,
  HelpPanel,
  HistoryPanel,
  ShortcutsModal,
  ToolboxPanel,
  panelTitle,
  type PanelId
} from "../components/CanvasPanels";
import { CanvasNavigator } from "../components/CanvasNavigator";
import { LibNodeComponent, type LibNodeComponentProps } from "../components/LibNode";
import { Button, IconButton } from "../components/ui";
import { nodeFootprints, nodeLabels } from "../constants";
import {
  createNode,
  imageCovers,
  nowIso,
  sampleAudio,
  sampleVideo,
  toolboxPresets,
  uid
} from "../fixtures";
import {
  extractImageResult,
  extractResponseText,
  historyDisplayText,
  historyToNodeKind,
  numberParam,
  stringParam
} from "../services/generation";
import { useStore } from "../storage";
import type {
  Asset,
  AssetKind,
  CanvasNodeData,
  GenerationHistory,
  GenerationParams,
  LibEdge,
  LibNode,
  NodeKind,
  Project
} from "../types";

function fileToAssetKind(file: File): AssetKind {
  if (file.type.startsWith("video")) {
    return "video";
  }
  if (file.type.startsWith("audio")) {
    return "audio";
  }
  return "image";
}

export function CanvasPage() {
  const { projectId } = useParams();
  const { projects } = useStore();
  const project = projects.find((item) => item.id === projectId);

  if (!project) {
    return (
      <AppShell>
        <main className="not-found">
          <h1>项目不存在</h1>
          <Link to="/project" className="text-link">
            返回项目
          </Link>
        </main>
      </AppShell>
    );
  }

  return (
    <ReactFlowProvider>
      <CanvasWorkspace key={project.id} project={project} />
    </ReactFlowProvider>
  );
}

function CanvasWorkspace({ project }: { project: Project }) {
  const {
    updateProject,
    duplicateProject,
    config,
    ui,
    setUi,
    assets,
    setAssets,
    history,
    setHistory
  } = useStore();
  const navigate = useNavigate();
  const flow = useReactFlow();
  const [nodes, setNodes, onNodesChangeBase] = useNodesState<LibNode>(project.nodes);
  const [edges, setEdges, onEdgesChangeBase] = useEdgesState<LibEdge>(project.edges);
  const [activePanel, setActivePanel] = useState<PanelId>(null);
  const [configOpen, setConfigOpen] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [navigatorCollapsed, setNavigatorCollapsed] = useState(false);
  const [zoom, setZoom] = useState(project.viewport?.zoom ?? 1);
  const copiedNodeRef = useRef<LibNode | null>(null);
  const undoStackRef = useRef<{ nodes: LibNode[]; edges: LibEdge[] }[]>([]);
  const restoringRef = useRef(false);
  const snapshotRef = useRef(JSON.stringify({ nodes, edges }));
  const readonlyProject = Boolean(project.readonly);

  const selectedNode = nodes.find((node) => node.id === selectedId);

  const updateNodeData = useCallback(
    (id: string, patch: Partial<CanvasNodeData>) => {
      if (readonlyProject) {
        return;
      }
      setNodes((items) =>
        items.map((node) =>
          node.id === id ? { ...node, data: { ...node.data, ...patch } } : node
        )
      );
    },
    [readonlyProject, setNodes]
  );

  const addCanvasNode = useCallback(
    (
      kind: NodeKind,
      name: string,
      extra: Partial<CanvasNodeData> = {},
      position?: { x: number; y: number }
    ) => {
      if (readonlyProject) {
        return undefined;
      }
      const column = nodes.length % 3;
      const row = Math.floor(nodes.length / 3);
      const node = createNode(
        kind,
        name,
        position?.x ?? 120 + column * 720,
        position?.y ?? 120 + row * 440,
        extra
      );
      setNodes((items) => [...items, node]);
      return node;
    },
    [nodes.length, readonlyProject, setNodes]
  );

  const addNodeNear = useCallback(
    (source: LibNode | undefined, kind: NodeKind, name: string, extra: Partial<CanvasNodeData> = {}) => {
      if (readonlyProject) {
        return undefined;
      }
      const targetSize = nodeFootprints[kind];
      const sourceWidth = Number(source?.data.contentWidth ?? 380);
      const x = (source?.position.x ?? 120) + sourceWidth + 170;
      let y = source?.position.y ?? 120;
      let guard = 0;
      while (
        guard < 10 &&
        nodes.some((node) => {
          const size = nodeFootprints[node.data.kind];
          const width = Number(node.data.contentWidth ?? size.width);
          const height = Number(node.data.contentHeight ?? size.height);
          return (
            x < node.position.x + width + 72 &&
            x + targetSize.width + 72 > node.position.x &&
            y < node.position.y + height + 72 &&
            y + targetSize.height + 72 > node.position.y
          );
        })
      ) {
        y += targetSize.height + 88;
        guard += 1;
      }
      const node = addCanvasNode(kind, name, extra, { x, y });
      if (source && node) {
        setEdges((items) => [...items, { id: uid("edge"), source: source.id, target: node.id }]);
      }
      return node;
    },
    [addCanvasNode, nodes, readonlyProject, setEdges]
  );

  const addHistory = useCallback(
    (item: Omit<GenerationHistory, "id" | "createdAt">) => {
      const record: GenerationHistory = { id: uid("history"), createdAt: nowIso(), ...item };
      setHistory((items) => [record, ...items]);
      return record;
    },
    [setHistory]
  );

  const onNodesChange = useCallback(
    (changes: NodeChange<LibNode>[]) => {
      if (readonlyProject) {
        const selectionChanges = changes.filter((change) => change.type === "select");
        if (selectionChanges.length > 0) {
          onNodesChangeBase(selectionChanges);
        }
        return;
      }
      onNodesChangeBase(changes);
    },
    [onNodesChangeBase, readonlyProject]
  );
  const onEdgesChange = useCallback(
    (changes: EdgeChange<LibEdge>[]) => {
      if (readonlyProject) {
        const selectionChanges = changes.filter((change) => change.type === "select");
        if (selectionChanges.length > 0) {
          onEdgesChangeBase(selectionChanges);
        }
        return;
      }
      onEdgesChangeBase(changes);
    },
    [onEdgesChangeBase, readonlyProject]
  );

  const onConnect = useCallback(
    (connection: Connection) => {
      if (readonlyProject) {
        return;
      }
      setEdges((items) => addEdge({ ...connection, id: uid("edge") }, items));
    },
    [readonlyProject, setEdges]
  );

  const onSelectionChange = useCallback((params: OnSelectionChangeParams) => {
    setSelectedId(params.nodes[0]?.id ?? null);
  }, []);

  const locateNode = useCallback(
    (node: LibNode) => {
      const size = nodeFootprints[node.data.kind];
      const width = Number(node.data.contentWidth ?? size.width);
      const height = Number(node.data.contentHeight ?? size.height);
      setSelectedId(node.id);
      setNodes((items) => items.map((item) => ({ ...item, selected: item.id === node.id })));
      const currentZoom = flow.getViewport().zoom;
      flow.setCenter(node.position.x + width / 2, node.position.y + height / 2, {
        zoom: Math.max(currentZoom, 0.42),
        duration: 260
      });
    },
    [flow, setNodes]
  );

  const persistViewport = useCallback(
    (_event: MouseEvent | TouchEvent | null, viewport: Viewport) => {
      setZoom(viewport.zoom);
      if (readonlyProject) {
        return;
      }
      updateProject(project.id, { viewport });
    },
    [project.id, readonlyProject, updateProject]
  );

  useEffect(() => {
    if (readonlyProject) {
      return;
    }
    updateProject(project.id, { nodes, edges, updatedAt: nowIso() });
    const current = JSON.stringify({ nodes, edges });
    if (!restoringRef.current && snapshotRef.current !== current) {
      try {
        undoStackRef.current = [
          ...undoStackRef.current.slice(-18),
          JSON.parse(snapshotRef.current) as { nodes: LibNode[]; edges: LibEdge[] }
        ];
      } catch {
        undoStackRef.current = [];
      }
      snapshotRef.current = current;
    }
    restoringRef.current = false;
  }, [edges, nodes, project.id, readonlyProject, updateProject]);

  const organizeCanvas = useCallback(() => {
    if (readonlyProject) {
      return;
    }
    setNodes((items) =>
      items.map((node, index) => ({
        ...node,
        position: {
          x: 80 + (index % 3) * 720,
          y: 110 + Math.floor(index / 3) * 430
        }
      }))
    );
    window.requestAnimationFrame(() => flow.fitView({ padding: 0.18, duration: 260 }));
  }, [flow, readonlyProject, setNodes]);

  const deleteSelected = useCallback(() => {
    if (readonlyProject) {
      return;
    }
    const selectedNodeIds = new Set(nodes.filter((node) => node.selected).map((node) => node.id));
    const selectedEdgeIds = new Set(edges.filter((edge) => edge.selected).map((edge) => edge.id));
    if (selectedNodeIds.size === 0 && selectedEdgeIds.size === 0 && selectedId) {
      selectedNodeIds.add(selectedId);
    }
    setNodes((items) => items.filter((node) => !selectedNodeIds.has(node.id)));
    setEdges((items) =>
      items.filter(
        (edge) =>
          !selectedEdgeIds.has(edge.id) &&
          !selectedNodeIds.has(edge.source) &&
          !selectedNodeIds.has(edge.target)
      )
    );
    setSelectedId(null);
  }, [edges, nodes, readonlyProject, selectedId, setEdges, setNodes]);

  const pasteNode = useCallback(() => {
    if (readonlyProject || !copiedNodeRef.current) {
      return;
    }
    const copy: LibNode = {
      ...copiedNodeRef.current,
      id: uid(copiedNodeRef.current.data.kind),
      selected: true,
      position: {
        x: copiedNodeRef.current.position.x + 60,
        y: copiedNodeRef.current.position.y + 60
      },
      data: { ...copiedNodeRef.current.data, name: `${copiedNodeRef.current.data.name} 副本` }
    };
    setNodes((items) => [
      ...items.map((node) => ({ ...node, selected: false })),
      copy
    ]);
    setSelectedId(copy.id);
  }, [readonlyProject, setNodes]);

  const undo = useCallback(() => {
    if (readonlyProject) {
      return;
    }
    const previous = undoStackRef.current.pop();
    if (!previous) {
      return;
    }
    restoringRef.current = true;
    setNodes(previous.nodes);
    setEdges(previous.edges);
  }, [readonlyProject, setEdges, setNodes]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const editing =
        target?.tagName === "INPUT" || target?.tagName === "TEXTAREA" || target?.tagName === "SELECT";
      if (event.key === "Escape") {
        setActivePanel(null);
        flow.setNodes((items) => items.map((node) => ({ ...node, selected: false })));
        setSelectedId(null);
      }
      if (editing) {
        return;
      }
      if (event.key === "Delete" || event.key === "Backspace") {
        event.preventDefault();
        deleteSelected();
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "c") {
        copiedNodeRef.current = nodes.find((node) => node.selected) ?? selectedNode ?? null;
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "v") {
        event.preventDefault();
        pasteNode();
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "z") {
        event.preventDefault();
        undo();
      }
      if (event.altKey && event.shiftKey && event.key.toLowerCase() === "f") {
        event.preventDefault();
        organizeCanvas();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [deleteSelected, flow, nodes, organizeCanvas, pasteNode, selectedNode, undo]);

  const importAsset = useCallback(
    (asset: Asset) => {
      addCanvasNode(asset.kind, asset.name, {
        url: asset.url,
        prompt: asset.prompt ?? `从我的素材导入：${asset.name}`,
        params: {
          ...(asset.params ?? {}),
          ...(asset.model ? { model: asset.model } : {})
        },
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
        url: item.resultUrl,
        prompt: item.kind === "text" ? text : item.revisedPrompt ?? item.prompt,
        text: item.kind === "text" ? text : undefined,
        params: item.params,
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
            createdAt: nowIso()
          };
          setAssets((items) => [asset, ...items]);
          importAsset(asset);
        };
        reader.readAsDataURL(file);
      });
    },
    [importAsset, setAssets]
  );

  const runOpenAIText = useCallback(
    async (id: string) => {
      const node = nodes.find((item) => item.id === id);
      const prompt = node?.data.prompt?.trim() || node?.data.text?.trim() || "生成一段创意脚本。";
      if (!config.openai.enabled || !config.openai.apiKey.trim()) {
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 0, message: "请先配置 OpenAI" }
        });
        return;
      }
      updateNodeData(id, { taskInfo: { status: "running", progress: 25, message: "OpenAI 生成中" } });
      const started = addHistory({
        kind: "text",
        provider: "openai",
        model: config.openai.textModel,
        prompt,
        status: "running",
        progress: 25
      });
      try {
        const response = await fetch(`${config.openai.baseUrl.replace(/\/$/, "")}/responses`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${config.openai.apiKey}`
          },
          body: JSON.stringify({
            model: config.openai.textModel,
            input: prompt
          })
        });
        if (!response.ok) {
          throw new Error(`OpenAI 请求失败：${response.status}`);
        }
        const payload = await response.json();
        const text = extractResponseText(payload) || "生成成功，但响应中没有可显示文本。";
        updateNodeData(id, {
          text,
          prompt: text,
          taskInfo: { status: "done", progress: 100, message: "已写回节点" }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id
              ? { ...item, resultText: text, status: "done", progress: 100, resultUrl: undefined }
              : item
          )
        );
      } catch (error) {
        const message = error instanceof Error ? error.message : "生成失败";
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 100, message }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id ? { ...item, status: "failed", progress: 100, error: message } : item
          )
        );
      }
    },
    [addHistory, config.openai, nodes, setHistory, updateNodeData]
  );

  const runOpenAIImage = useCallback(
    async (id: string) => {
      const node = nodes.find((item) => item.id === id);
      const prompt = node?.data.prompt?.trim() || node?.data.name || "生成一张电影感创意视觉图。";
      if (!config.openai.enabled || !config.openai.apiKey.trim()) {
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 0, message: "请先配置 OpenAI" }
        });
        return;
      }
      updateNodeData(id, { taskInfo: { status: "running", progress: 20, message: "OpenAI 图像生成中" } });
      const requestParams: GenerationParams = {
        size: "1024x1024",
        quality: "medium",
        output_format: "png"
      };
      const started = addHistory({
        kind: "image",
        provider: "openai",
        model: config.openai.imageModel,
        prompt,
        status: "running",
        progress: 20,
        params: requestParams
      });
      try {
        const response = await fetch(`${config.openai.baseUrl.replace(/\/$/, "")}/images/generations`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${config.openai.apiKey}`
          },
          body: JSON.stringify({
            model: config.openai.imageModel,
            prompt,
            size: requestParams.size,
            quality: requestParams.quality,
            output_format: requestParams.output_format
          })
        });
        if (!response.ok) {
          throw new Error(`OpenAI 图像请求失败：${response.status}`);
        }
        const payload = await response.json();
        const { url: resultUrl, revisedPrompt } = extractImageResult(payload);
        if (!resultUrl) {
          throw new Error("OpenAI 响应中没有可显示图像");
        }
        updateNodeData(id, {
          url: resultUrl,
          taskInfo: { status: "done", progress: 100, message: "图像已写回节点" }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id
              ? { ...item, status: "done", progress: 100, resultUrl, revisedPrompt }
              : item
          )
        );
        setAssets((items) => [
          {
            id: uid("asset"),
            kind: "image",
            name: `${node?.data.name ?? "OpenAI 图像"}`,
            url: resultUrl,
            category: "project",
            provider: "openai",
            model: config.openai.imageModel,
            prompt: revisedPrompt ?? prompt,
            params: requestParams,
            createdAt: nowIso()
          },
          ...items
        ]);
      } catch (error) {
        const message = error instanceof Error ? error.message : "图像生成失败";
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 100, message }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === started.id ? { ...item, status: "failed", progress: 100, error: message } : item
          )
        );
      }
    },
    [addHistory, config.openai, nodes, setAssets, setHistory, updateNodeData]
  );

  const runSeedanceMock = useCallback(
    (id: string, kind: "video" | "audio" | "compose" = "video") => {
      if (!config.seedance.enabled) {
        updateNodeData(id, {
          taskInfo: { status: "failed", progress: 0, message: "Seedance mock 未启用" }
        });
        return;
      }
      const node = nodes.find((item) => item.id === id);
      const mediaKind: AssetKind = kind === "audio" ? "audio" : "video";
      const nodeParams = (node?.data.params ?? {}) as GenerationParams;
      const model = stringParam(
        nodeParams,
        "model",
        mediaKind === "audio" ? config.seedance.audioModel : config.seedance.videoModel
      );
      const prompt = node?.data.prompt || `${node?.data.name ?? "Seedance"} mock 生成`;
      const resultUrl = mediaKind === "audio" ? sampleAudio : sampleVideo;
      const generationParams: GenerationParams = {
        model,
        duration: numberParam(nodeParams, "duration", config.seedance.duration),
        ...(mediaKind === "video"
          ? {
              modeType: stringParam(nodeParams, "modeType", "text2video"),
              ratio: stringParam(nodeParams, "ratio", "16:9"),
              resolution: stringParam(nodeParams, "resolution", config.seedance.resolution),
              ...(kind === "compose" ? { transition: stringParam(nodeParams, "transition", "crossfade") } : {})
            }
          : {})
      };
      const record = addHistory({
        kind: mediaKind,
        provider: "seedance-mock",
        model,
        prompt,
        status: "queued",
        progress: 0,
        params: generationParams
      });
      updateNodeData(id, {
        taskInfo: { status: "queued", progress: 0, message: "已加入队列" }
      });
      let progress = 0;
      const steps = Math.max(4, Math.round(config.seedance.mockLatencyMs / 320));
      const timer = window.setInterval(() => {
        progress = Math.min(100, progress + Math.ceil(100 / steps));
        const running = progress < 100;
        updateNodeData(id, {
          url: running ? node?.data.url : resultUrl,
          taskInfo: {
            status: running ? "running" : "done",
            progress,
            message: running ? "Seedance mock 生成中" : "生成完成"
          }
        });
        setHistory((items) =>
          items.map((item) =>
            item.id === record.id
              ? {
                  ...item,
                  status: running ? "running" : "done",
                  progress,
                  resultUrl: running ? item.resultUrl : resultUrl
                }
              : item
          )
        );
        if (!running) {
          window.clearInterval(timer);
          setAssets((items) => [
            {
              id: uid("asset"),
              kind: mediaKind,
              name: `${node?.data.name ?? "生成结果"}`,
              url: resultUrl,
              category: mediaKind === "audio" ? "sound" : "project",
              provider: "seedance-mock",
              model,
              prompt,
              params: generationParams,
              createdAt: nowIso()
            },
            ...items
          ]);
        }
      }, Math.max(220, config.seedance.mockLatencyMs / steps));
    },
    [addHistory, config.seedance, nodes, setAssets, setHistory, updateNodeData]
  );

  const runImageTool = useCallback(
    (id: string, label: string) => {
      const source = nodes.find((item) => item.id === id);
      const cover = imageCovers[Math.floor(Math.random() * imageCovers.length)];
      const node = addNodeNear(source, "image", `${label}结果`, {
        url: cover,
        prompt: `${label} mock：${source?.data.prompt ?? source?.data.name ?? ""}`,
        taskInfo: { status: "done", progress: 100, message: "mock 完成" }
      });
      if (!node) {
        return;
      }
      addHistory({
        kind: "image",
        provider: "local",
        model: `image-${label}`,
        prompt: node.data.prompt ?? label,
        status: "done",
        progress: 100,
        resultUrl: cover
      });
    },
    [addHistory, addNodeNear, nodes]
  );

  const runDirectorShot = useCallback(
    (id: string) => {
      const source = nodes.find((item) => item.id === id);
      const cover = imageCovers[1];
      const node = addNodeNear(source, "image", "导演台截图", {
        url: cover,
        prompt: `导演台截图：${source?.data.prompt ?? "场景参考"}`,
        taskInfo: { status: "done", progress: 100, message: "截图已生成" }
      });
      if (!node) {
        return;
      }
      addHistory({
        kind: "image",
        provider: "local",
        model: "director-stage-screenshot",
        prompt: node.data.prompt ?? "导演台截图",
        status: "done",
        progress: 100,
        resultUrl: cover
      });
    },
    [addHistory, addNodeNear, nodes]
  );

  const generateStoryboard = useCallback(
    (id: string) => {
      const source = nodes.find((item) => item.id === id);
      let previous = source;
      [0, 1, 2].forEach((index) => {
        const node = addNodeNear(previous, index === 2 ? "video" : "image", `故事板 ${index + 1}`, {
          url: index === 2 ? sampleVideo : imageCovers[(index + 2) % imageCovers.length],
          prompt: `故事板镜头 ${index + 1}`,
          taskInfo: { status: "done", progress: 100, message: "故事板已生成" }
        });
        if (node) {
          previous = node;
        }
      });
    },
    [addNodeNear, nodes]
  );

  const quickAction = useCallback(
    (id: string, action: string) => {
      const source = nodes.find((node) => node.id === id);
      if (action === "text2video") {
        addNodeNear(source, "video", "文生视频", {
          prompt: source?.data.prompt ?? source?.data.text ?? "",
          params: {
            model: config.seedance.videoModel,
            modeType: "text2video",
            ratio: "16:9",
            resolution: config.seedance.resolution,
            duration: config.seedance.duration
          }
        });
      }
      if (action === "imagePrompt") {
        addNodeNear(source, "text", "图片反推提示词", {
          prompt: "请基于输入图片反推提示词。",
          text: "画面主体、风格、镜头、光线、色彩、构图。"
        });
      }
      if (action === "music") {
        addNodeNear(source, "audio", "文字生音乐", {
          prompt: source?.data.prompt ?? source?.data.text ?? "",
          params: { model: config.seedance.audioModel, duration: config.seedance.duration }
        });
      }
    },
    [addNodeNear, config.seedance, nodes]
  );

  const insertToolboxPreset = useCallback(
    (presetId: string) => {
      const preset = toolboxPresets.find((item) => item.id === presetId) ?? toolboxPresets[0];
      let previous: LibNode | undefined;
      preset.kinds.forEach((kind, index) => {
        const node = addCanvasNode(kind, `${preset.name} ${nodeLabels[kind]}`, {
          prompt: `${preset.description} 第 ${index + 1} 步`,
          url: kind === "image" ? preset.thumb : kind === "video" ? sampleVideo : undefined,
          params: kind === "video" ? { model: config.seedance.videoModel, duration: config.seedance.duration } : {}
        }, {
          x: 120 + index * 460,
          y: 160 + (nodes.length % 2) * 120
        });
        if (!node) {
          return;
        }
        if (previous) {
          setEdges((items) => [...items, { id: uid("edge"), source: previous!.id, target: node.id }]);
        }
        previous = node;
      });
    },
    [addCanvasNode, config.seedance.duration, config.seedance.videoModel, nodes.length, setEdges]
  );

  const nodeHandlersRef = useRef<Pick<
    LibNodeComponentProps,
    | "onUpdate"
    | "onOpenAIText"
    | "onOpenAIImage"
    | "onSeedance"
    | "onImageTool"
    | "onDirectorShot"
    | "onStoryboard"
    | "onQuickAction"
  > | null>(null);
  nodeHandlersRef.current = {
    onUpdate: updateNodeData,
    onOpenAIText: runOpenAIText,
    onOpenAIImage: runOpenAIImage,
    onSeedance: runSeedanceMock,
    onImageTool: runImageTool,
    onDirectorShot: runDirectorShot,
    onStoryboard: generateStoryboard,
    onQuickAction: quickAction
  };

  const nodeTypes = useMemo(
    () => ({
      libNode: (props: NodeProps<LibNode>) => {
        const handlers = nodeHandlersRef.current;
        if (!handlers) {
          return null;
        }
        return <LibNodeComponent {...props} {...handlers} />;
      }
    }),
    []
  );

  const createEditableCopy = useCallback(() => {
    const copy = duplicateProject(project.id);
    if (copy) {
      navigate(`/canvas/${copy.id}`);
    }
  }, [duplicateProject, navigate, project.id]);

  return (
    <div className={`canvas-page ${navigatorCollapsed ? "navigator-collapsed" : "with-navigator"}`}>
      <CanvasNavigator
        project={project}
        nodes={nodes}
        assets={assets}
        selectedId={selectedId}
        collapsed={navigatorCollapsed}
        onToggle={() => setNavigatorCollapsed((value) => !value)}
        onLocateNode={locateNode}
      />
      <header className="canvas-topbar">
        <div className="canvas-nav">
          <IconButton label="返回首页" onClick={() => navigate("/")}>
            <Home size={18} />
          </IconButton>
          <IconButton label="返回项目" onClick={() => navigate("/project")}>
            <ArrowLeft size={18} />
          </IconButton>
          <input
            className="project-name-input"
            aria-label="项目名称"
            name="projectName"
            value={project.name}
            readOnly={project.readonly}
            onChange={(event) => updateProject(project.id, { name: event.target.value })}
          />
          {project.readonly && <span className="pill">只读预览</span>}
          {project.readonly && (
            <Button className="canvas-copy-btn" size="sm" onClick={createEditableCopy}>
              <Copy size={14} />
              创建副本
            </Button>
          )}
        </div>
        <div className="canvas-top-actions">
          {!readonlyProject && (
            <IconButton label="保存状态">
              <Save size={18} />
            </IconButton>
          )}
          <IconButton label="系统配置" onClick={() => setConfigOpen(true)}>
            <Settings size={18} />
          </IconButton>
        </div>
      </header>

      <ReactFlow
        className={ui.snapToGrid ? "snap-grid" : ""}
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onSelectionChange={onSelectionChange}
        onMoveEnd={persistViewport}
        defaultViewport={project.viewport}
        minZoom={0.08}
        maxZoom={2}
        fitView={nodes.length === 0}
        nodesDraggable={!readonlyProject}
        nodesConnectable={!readonlyProject}
        nodesFocusable={!readonlyProject}
        edgesFocusable={!readonlyProject}
        elementsSelectable={!readonlyProject}
        edgesReconnectable={!readonlyProject}
        snapToGrid={ui.snapToGrid}
        snapGrid={[20, 20]}
        deleteKeyCode={null}
        proOptions={{ hideAttribution: true }}
      >
        <Background
          color={ui.snapToGrid ? "#006666" : "#b9c1c9"}
          gap={ui.snapToGrid ? 20 : 32}
          size={1}
          variant={BackgroundVariant.Dots}
        />
        <Controls showInteractive={false} position="bottom-left" />
        {ui.minimap && <MiniMap pannable zoomable position="bottom-right" />}
      </ReactFlow>

      <div className="canvas-left-controls" aria-label="画布控制">
        {!readonlyProject && (
          <IconButton label="整理画布" onClick={organizeCanvas}>
            <Rows3 size={16} />
          </IconButton>
        )}
        <IconButton
          label="切换小地图"
          className={ui.minimap ? "active" : ""}
          onClick={() => setUi((value) => ({ ...value, minimap: !value.minimap }))}
        >
          <PanelLeft size={16} />
        </IconButton>
        {!readonlyProject && (
          <IconButton
            label="网格吸附"
            className={ui.snapToGrid ? "active" : ""}
            onClick={() => setUi((value) => ({ ...value, snapToGrid: !value.snapToGrid }))}
          >
            <Grid2X2 size={16} />
          </IconButton>
        )}
        <span className="zoom-pill">{Math.round(zoom * 100)}%</span>
        <IconButton label="适配视图" onClick={() => flow.fitView({ padding: 0.18, duration: 240 })}>
          <Maximize2 size={16} />
        </IconButton>
      </div>

      {!readonlyProject && <BottomToolbar activePanel={activePanel} setActivePanel={setActivePanel} />}

      {!readonlyProject && activePanel && activePanel !== "shortcuts" && (
        <CanvasDrawer panel={activePanel} title={panelTitle(activePanel)} onClose={() => setActivePanel(null)}>
          {activePanel === "add" && (
            <AddNodePanel
              onAdd={addCanvasNode}
              onUpload={handleUpload}
              history={history}
              onImportHistory={importHistory}
            />
          )}
          {activePanel === "toolbox" && <ToolboxPanel onUse={insertToolboxPreset} />}
          {activePanel === "assets" && (
            <AssetsPanel assets={assets} onUpload={handleUpload} onImport={importAsset} />
          )}
          {activePanel === "history" && (
            <HistoryPanel
              history={history}
              setHistory={setHistory}
              onImport={importHistory}
            />
          )}
          {activePanel === "help" && <HelpPanel />}
        </CanvasDrawer>
      )}
      {!readonlyProject && activePanel === "shortcuts" && <ShortcutsModal onClose={() => setActivePanel(null)} />}
      {configOpen && <ConfigModal onClose={() => setConfigOpen(false)} />}
    </div>
  );
}

