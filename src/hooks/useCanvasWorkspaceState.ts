import {
  addEdge,
  useEdgesState,
  useNodesState,
  useReactFlow,
  type Connection,
  type EdgeChange,
  type NodeChange,
  type OnSelectionChangeParams,
  type Viewport
} from "@xyflow/react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PanelId } from "../components/CanvasPanels";
import { nodeFootprints } from "../constants";
import { createNode, nowIso, uid } from "../fixtures";
import type { CanvasNodeData, LibEdge, LibNode, NodeKind, Project } from "../types";

type UpdateProject = (id: string, patch: Partial<Project> | ((project: Project) => Project)) => void;

export function useCanvasWorkspaceState({
  project,
  updateProject
}: {
  project: Project;
  updateProject: UpdateProject;
}) {
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
        setNodes((items) => items.map((node) => ({ ...node, selected: false })));
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
  }, [deleteSelected, nodes, organizeCanvas, pasteNode, selectedNode, setNodes, undo]);

  const fitView = useCallback(() => {
    flow.fitView({ padding: 0.18, duration: 240 });
  }, [flow]);

  return {
    nodes,
    edges,
    setEdges,
    activePanel,
    setActivePanel,
    configOpen,
    setConfigOpen,
    selectedId,
    navigatorCollapsed,
    setNavigatorCollapsed,
    zoom,
    readonlyProject,
    updateNodeData,
    addCanvasNode,
    addNodeNear,
    onNodesChange,
    onEdgesChange,
    onConnect,
    onSelectionChange,
    locateNode,
    persistViewport,
    organizeCanvas,
    fitView
  };
}
