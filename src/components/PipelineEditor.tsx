import { useState, useCallback, useRef, useEffect, useMemo } from "react";
import FeroHaIcon from "./FeroHaIcon";
import type {
  PipelineDefinition,
  PipelineNode,
  PipelineEdge,
  PipelineExecution,
} from "../lib/commandCardPipeline";
import { pipelineManager, pipelineEngine } from "../lib/commandCardPipeline";
import { commandCardRegistry } from "../store/commandCardRegistry";
import type { CommandCardDefinition, CommandCategory } from "../types/command-card";
import "./PipelineEditor.css";

interface PipelineEditorProps {
  pipelineId?: string;
  onSave?: (pipeline: PipelineDefinition) => void;
  onRun?: (pipeline: PipelineDefinition) => void;
}

const NODE_WIDTH = 160;
const NODE_MIN_HEIGHT = 52;
const PORT_OFFSET_Y_IN = -4;
const PORT_OFFSET_Y_OUT = 4;
const PORT_RADIUS = 5;
const CATEGORY_LABELS: Record<CommandCategory | string, string> = {
  content: "内容",
  analysis: "分析",
  format: "格式",
  system: "系统",
  agent: "Agent",
  all: "全部",
};

const CATEGORY_ICONS: Record<CommandCategory | string, string> = {
  content: "FileText",
  analysis: "Microscope",
  format: "Ruler",
  system: "Settings",
  agent: "Bot",
  all: "Layers",
};

function loadPipeline(pipelineId?: string): PipelineDefinition {
  if (pipelineId) {
    const existing = pipelineManager.get(pipelineId);
    if (existing) return existing;
  }
  return pipelineManager.create("新流程", "新的指令卡流程");
}

const EXEC_STATUS_LABELS: Record<NodeExecStatus, string> = {
  idle: "空闲",
  running: "运行中",
  completed: "已完成",
  failed: "失败",
};

const PIPELINE_STATUS_LABELS: Record<NodeExecStatus, string> = {
  idle: "就绪",
  running: "运行中...",
  completed: "完成",
  failed: "失败",
};

function getDefaultParams(card?: CommandCardDefinition): Record<string, unknown> | undefined {
  if (!card) return undefined;
  return Object.fromEntries(
    card.params.map((param) => [
      param.templateVar || param.name,
      param.defaultValue ?? "",
    ])
  );
}

type NodeExecStatus = "idle" | "running" | "completed" | "failed";

export default function PipelineEditor({
  pipelineId,
  onSave,
  onRun,
}: PipelineEditorProps) {
  const [pipeline, setPipeline] = useState<PipelineDefinition>(() => loadPipeline(pipelineId));

  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [draggingNodeId, setDraggingNodeId] = useState<string | null>(null);
  const [connectingFromNodeId, setConnectingFromNodeId] = useState<string | null>(null);
  const [tempEdgeEnd, setTempEdgeEnd] = useState<{ x: number; y: number } | null>(null);
  const [paramsEditingNodeId, setParamsEditingNodeId] = useState<string | null>(null);
  const [paramsEditorPos, setParamsEditorPos] = useState<{ x: number; y: number }>({ x: 400, y: 200 });

  const [execStatus, setExecStatus] = useState<"idle" | "running" | "completed" | "failed">("idle");
  const [execProgress, setExecProgress] = useState(0);
  const [execCurrentNodeId, setExecCurrentNodeId] = useState<string | null>(null);
  const [completedNodeIds, setCompletedNodeIds] = useState<Set<string>>(new Set());
  const [failedNodeIds, setFailedNodeIds] = useState<Set<string>>(new Set());

  const [undoStack, setUndoStack] = useState<PipelineDefinition[]>([]);
  const [redoStack, setRedoStack] = useState<PipelineDefinition[]>([]);
  const [collapsedCategories, setCollapsedCategories] = useState<Set<string>>(new Set());
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    nodeId?: string;
    edgeId?: string;
  } | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  const canvasRef = useRef<HTMLDivElement>(null);
  const loadedPipelineIdRef = useRef(pipelineId);
  const dragOffsetRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const pendingPipelineRef = useRef<PipelineDefinition>(pipeline);

  useEffect(() => {
    pendingPipelineRef.current = pipeline;
  }, [pipeline]);

  useEffect(() => {
    if (loadedPipelineIdRef.current === pipelineId) return;
    loadedPipelineIdRef.current = pipelineId;
    setPipeline(loadPipeline(pipelineId));
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
    setUndoStack([]);
    setRedoStack([]);
    setExecStatus("idle");
  }, [pipelineId]);

  const pushUndo = useCallback((p: PipelineDefinition) => {
    setUndoStack((prev) => [...prev.slice(-49), p]);
    setRedoStack([]);
  }, []);

  const updatePipeline = useCallback((updater: (prev: PipelineDefinition) => PipelineDefinition) => {
    setPipeline((prev) => {
      const next = updater({ ...prev, nodes: prev.nodes.map((n) => ({ ...n })) });
      pushUndo(prev);
      pipelineManager.update(next.id, next);
      return next;
    });
  }, [pushUndo]);

  const handleUndo = useCallback(() => {
    if (undoStack.length === 0) return;
    const prev = undoStack[undoStack.length - 1];
    setUndoStack((s) => s.slice(0, -1));
    setRedoStack((s) => [...s, pipeline]);
    setPipeline(prev);
    pipelineManager.update(prev.id, prev);
  }, [undoStack, pipeline]);

  const handleRedo = useCallback(() => {
    if (redoStack.length === 0) return;
    const next = redoStack[redoStack.length - 1];
    setRedoStack((s) => s.slice(0, -1));
    setUndoStack((s) => [...s, pipeline]);
    setPipeline(next);
    pipelineManager.update(next.id, next);
  }, [redoStack, pipeline]);

  const getNodeExecStatus = useCallback((nodeId: string): NodeExecStatus => {
    if (execCurrentNodeId === nodeId) return "running";
    if (failedNodeIds.has(nodeId)) return "failed";
    if (completedNodeIds.has(nodeId)) return "completed";
    return "idle";
  }, [execCurrentNodeId, failedNodeIds, completedNodeIds]);

  // ── Canvas event helpers ──
  const canvasToLocal = useCallback((clientX: number, clientY: number) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return { x: 0, y: 0 };
    const el = canvasRef.current;
    return {
      x: clientX - rect.left + (el?.scrollLeft ?? 0),
      y: clientY - rect.top + (el?.scrollTop ?? 0),
    };
  }, []);

  // ── Node manipulation ──
  const addNodeAt = useCallback((card: CommandCardDefinition, x: number, y: number) => {
    updatePipeline((prev) => {
      const newNode: Omit<PipelineNode, "id"> = {
        type: "command",
        card,
        params: getDefaultParams(card),
        position: { x, y },
      };
      const result = pipelineManager.addNode(prev.id, newNode);
      if (result) {
        return { ...prev, nodes: [...prev.nodes, result] };
      }
      return prev;
    });
  }, [updatePipeline]);

  const removeNode = useCallback((nodeId: string) => {
    updatePipeline((prev) => {
      pipelineManager.removeNode(prev.id, nodeId);
      const updated = pipelineManager.get(prev.id);
      return updated ?? prev;
    });
    if (selectedNodeId === nodeId) setSelectedNodeId(null);
    if (paramsEditingNodeId === nodeId) setParamsEditingNodeId(null);
  }, [updatePipeline, selectedNodeId, paramsEditingNodeId]);

  const updateNodePosition = useCallback((nodeId: string, x: number, y: number) => {
    setPipeline((prev) => ({
      ...prev,
      nodes: prev.nodes.map((n) => (n.id === nodeId ? { ...n, position: { x, y } } : n)),
    }));
  }, []);

  const commitNodePosition = useCallback((nodeId: string, x: number, y: number) => {
    updatePipeline((prev) => ({
      ...prev,
      nodes: prev.nodes.map((n) => (n.id === nodeId ? { ...n, position: { x, y } } : n)),
    }));
  }, [updatePipeline]);

  const addEdge = useCallback((source: string, target: string) => {
    if (source === target) return;
    const alreadyExists = pipeline.edges.some(
      (e) => e.source === source && e.target === target
    );
    if (alreadyExists) return;
    updatePipeline((prev) => {
      const result = pipelineManager.addEdge(prev.id, { source, target });
      if (result) {
        return { ...prev, edges: [...prev.edges, result] };
      }
      return prev;
    });
  }, [pipeline.edges, updatePipeline]);

  const removeEdge = useCallback((edgeId: string) => {
    updatePipeline((prev) => {
      pipelineManager.removeEdge(prev.id, edgeId);
      const updated = pipelineManager.get(prev.id);
      return updated ?? prev;
    });
    if (selectedEdgeId === edgeId) setSelectedEdgeId(null);
  }, [updatePipeline, selectedEdgeId]);

  const updateNodeParams = useCallback((nodeId: string, params: Record<string, unknown>) => {
    updatePipeline((prev) => ({
      ...prev,
      nodes: prev.nodes.map((n) => (n.id === nodeId ? { ...n, params } : n)),
    }));
  }, [updatePipeline]);

  // ── Drag & drop: palette → canvas ──
  const handlePaletteDragStart = useCallback((e: React.DragEvent, card: CommandCardDefinition) => {
    e.dataTransfer.setData("application/feroha-card", card.meta.id);
    e.dataTransfer.effectAllowed = "copy";
  }, []);

  const handleCanvasDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    if (e.dataTransfer.types.includes("application/feroha-card")) {
      e.dataTransfer.dropEffect = "copy";
    }
  }, []);

  const handleCanvasDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    const cardId = e.dataTransfer.getData("application/feroha-card");
    if (!cardId) return;
    const card = commandCardRegistry.get(cardId);
    if (!card) return;
    const pos = canvasToLocal(e.clientX, e.clientY);
    addNodeAt(card, pos.x - NODE_WIDTH / 2, pos.y - 30);
  }, [canvasToLocal, addNodeAt]);

  // ── Node dragging on canvas ──
  const handleNodeMouseDown = useCallback((e: React.MouseEvent, nodeId: string) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    e.preventDefault();
    setSelectedNodeId(nodeId);
    setSelectedEdgeId(null);

    const node = pipeline.nodes.find((n) => n.id === nodeId);
    if (!node) return;
    const local = canvasToLocal(e.clientX, e.clientY);
    dragOffsetRef.current = {
      x: local.x - node.position.x,
      y: local.y - node.position.y,
    };
    setDraggingNodeId(nodeId);

    const handleMouseMove = (ev: MouseEvent) => {
      const mlocal = canvasToLocal(ev.clientX, ev.clientY);
      updateNodePosition(
        nodeId,
        mlocal.x - dragOffsetRef.current.x,
        mlocal.y - dragOffsetRef.current.y
      );
    };

    const handleMouseUp = (ev: MouseEvent) => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      setDraggingNodeId(null);
      const mlocal = canvasToLocal(ev.clientX, ev.clientY);
      commitNodePosition(
        nodeId,
        mlocal.x - dragOffsetRef.current.x,
        mlocal.y - dragOffsetRef.current.y
      );
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }, [pipeline.nodes, canvasToLocal, updateNodePosition, commitNodePosition]);

  // ── Edge connection via ports ──
  const handlePortMouseDown = useCallback((e: React.MouseEvent, nodeId: string) => {
    e.stopPropagation();
    e.preventDefault();
    setConnectingFromNodeId(nodeId);
    const local = canvasToLocal(e.clientX, e.clientY);
    setTempEdgeEnd(local);

    const handleMouseMove = (ev: MouseEvent) => {
      const mlocal = canvasToLocal(ev.clientX, ev.clientY);
      setTempEdgeEnd(mlocal);
    };

    const handleMouseUp = (ev: MouseEvent) => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      setConnectingFromNodeId(null);
      setTempEdgeEnd(null);

      const targetEl = document.elementFromPoint(ev.clientX, ev.clientY);
      if (targetEl) {
        const portEl = (targetEl as HTMLElement).closest("[data-port-node-id]");
        if (portEl) {
          const targetNodeId = portEl.getAttribute("data-port-node-id");
          if (targetNodeId && targetNodeId !== nodeId) {
            addEdge(nodeId, targetNodeId);
          }
        }
      }
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }, [canvasToLocal, addEdge]);

  // ── Canvas click ──
  const handleCanvasClick = useCallback(() => {
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
    setParamsEditingNodeId(null);
    setContextMenu(null);
  }, []);

  // ── Context menu ──
  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY });
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
  }, []);

  const handleNodeContextMenu = useCallback((e: React.MouseEvent, nodeId: string) => {
    e.preventDefault();
    e.stopPropagation();
    setSelectedNodeId(nodeId);
    setContextMenu({ x: e.clientX, y: e.clientY, nodeId });
  }, []);

  const handleEdgeContextMenu = useCallback((e: React.MouseEvent, edgeId: string) => {
    e.preventDefault();
    e.stopPropagation();
    setSelectedEdgeId(edgeId);
    setContextMenu({ x: e.clientX, y: e.clientY, edgeId });
  }, []);

  useEffect(() => {
    if (!contextMenu) return;
    const close = () => setContextMenu(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [contextMenu]);

  // ── Double-click → param editor ──
  const handleNodeDoubleClick = useCallback((e: React.MouseEvent, node: PipelineNode) => {
    e.stopPropagation();
    if (node.type !== "command" || !node.card) return;
    setParamsEditingNodeId(node.id);
    setSelectedNodeId(node.id);
    const nodeEl = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setParamsEditorPos({ x: nodeEl.right + 10, y: nodeEl.top });
  }, []);

  // ── Execution ──
  const handleRun = useCallback(async () => {
    if (execStatus === "running") return;
    setExecStatus("running");
    setExecProgress(0);
    setExecCurrentNodeId(null);
    setCompletedNodeIds(new Set());
    setFailedNodeIds(new Set());

    const current = pendingPipelineRef.current;
    try {
      await pipelineEngine.execute(
        current,
        {},
        (exec: PipelineExecution) => {
          setExecProgress(exec.progress);
          if (exec.currentNodeId) {
            setExecCurrentNodeId(exec.currentNodeId);
          }
        },
        (node: PipelineNode) => {
          setCompletedNodeIds((prev) => new Set(prev).add(node.id));
        }
      );
      setExecStatus("completed");
      setExecProgress(100);
      onRun?.(current);
    } catch {
      setExecStatus("failed");
      setFailedNodeIds((prev) => {
        const next = new Set(prev);
        if (execCurrentNodeId) next.add(execCurrentNodeId);
        return next;
      });
    }
  }, [execStatus, onRun, execCurrentNodeId]);

  // ── Export / Import ──
  const handleExport = useCallback(() => {
    const json = pipelineManager.export(pipeline.id);
    if (!json) return;
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${pipeline.name.replace(/\s+/g, "_")}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [pipeline]);

  const handleImport = useCallback(() => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = (e: Event) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      const reader = new FileReader();
      reader.onload = () => {
        const json = reader.result as string;
        const imported = pipelineManager.import(json);
        if (imported) {
          setPipeline(imported);
          setUndoStack([]);
          setRedoStack([]);
          setSelectedNodeId(null);
          setSelectedEdgeId(null);
        }
      };
      reader.readAsText(file);
    };
    input.click();
  }, []);

  // ── Derived data ──
  const availableCards = useMemo(() => {
    let cards = commandCardRegistry.getAll();
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      cards = cards.filter(
        (c) =>
          c.meta.name.toLowerCase().includes(q) ||
          c.meta.description.toLowerCase().includes(q) ||
          c.meta.tags.some((t) => t.toLowerCase().includes(q))
      );
    }
    const grouped: Map<string, CommandCardDefinition[]> = new Map();
    for (const card of cards) {
      const cat = card.meta.category;
      if (!grouped.has(cat)) grouped.set(cat, []);
      grouped.get(cat)!.push(card);
    }
    return grouped;
  }, [searchQuery]);

  const selectedNode = useMemo(
    () => pipeline.nodes.find((n) => n.id === selectedNodeId),
    [pipeline.nodes, selectedNodeId]
  );

  const selectedEdge = useMemo(
    () => pipeline.edges.find((e) => e.id === selectedEdgeId),
    [pipeline.edges, selectedEdgeId]
  );

  const toggleCategory = useCallback((cat: string) => {
    setCollapsedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(cat)) next.delete(cat);
      else next.add(cat);
      return next;
    });
  }, []);

  // ── Compute edge coordinates ──
  const getEdgeCoords = useCallback((edge: PipelineEdge) => {
    const src = pipeline.nodes.find((n) => n.id === edge.source);
    const tgt = pipeline.nodes.find((n) => n.id === edge.target);
    if (!src || !tgt) return null;
    const x1 = src.position.x + NODE_WIDTH / 2;
    const y1 = src.position.y + NODE_MIN_HEIGHT + PORT_OFFSET_Y_OUT;
    const x2 = tgt.position.x + NODE_WIDTH / 2;
    const y2 = tgt.position.y + PORT_OFFSET_Y_IN;
    return { x1, y1, x2, y2 };
  }, [pipeline.nodes]);

  // ── Render ──
  return (
    <div className="pipeline-editor-v2">
      {/* ── Toolbar ── */}
      <div className="pipeline-toolbar">
        <div className="pipeline-toolbar-group">
          <button
            className="pipeline-toolbar-btn run-btn"
            onClick={handleRun}
            disabled={execStatus === "running"}
            title="运行流程"
          >
            <FeroHaIcon name="Play" size={14} />
            运行
          </button>
          <button
            className="pipeline-toolbar-btn"
            onClick={() => onSave?.(pipeline)}
            title="保存流程"
          >
            <FeroHaIcon name="Save" size={14} />
            保存
          </button>
        </div>

        <div className="pipeline-toolbar-group">
          <button
            className="pipeline-toolbar-btn"
            onClick={handleUndo}
            disabled={undoStack.length === 0}
            title="撤销"
          >
            <FeroHaIcon name="Undo" size={14} />
          </button>
          <button
            className="pipeline-toolbar-btn"
            onClick={handleRedo}
            disabled={redoStack.length === 0}
            title="重做"
          >
            <FeroHaIcon name="Redo" size={14} />
          </button>
        </div>

        <div className="pipeline-toolbar-name">
          <input
            type="text"
            value={pipeline.name}
            onChange={(e) =>
              updatePipeline((prev) => ({ ...prev, name: e.target.value }))
            }
          />
        </div>

        <div className="pipeline-toolbar-group">
          {execStatus !== "idle" && (
            <>
              <span className={`pipeline-run-status ${execStatus}`}>
                {EXEC_STATUS_LABELS[execStatus]}
              </span>
              <div className="pipeline-progress-bar">
                <div
                  className="pipeline-progress-bar-fill"
                  style={{ width: `${execProgress}%` }}
                />
              </div>
            </>
          )}
          <button
            className="pipeline-toolbar-btn"
            onClick={handleExport}
            title="导出 JSON"
          >
            <FeroHaIcon name="Download" size={14} />
            导出
          </button>
          <button
            className="pipeline-toolbar-btn"
            onClick={handleImport}
            title="导入 JSON"
          >
            <FeroHaIcon name="Upload" size={14} />
            导入
          </button>
          {selectedNode && (
            <button
              className="pipeline-toolbar-btn danger-btn"
              onClick={() => removeNode(selectedNode.id)}
              title="删除选中节点"
            >
              <FeroHaIcon name="Trash2" size={14} />
              删除
            </button>
          )}
        </div>
      </div>

      {/* ── Content ── */}
      <div className="pipeline-editor-content">
        {/* ── Card Palette ── */}
        <div className="pipeline-card-palette">
          <div style={{ padding: "8px" }}>
            <input
              type="text"
              placeholder="搜索指令卡..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pipeline-card-search-input feroha-search"
              style={{ width: "100%", margin: 0 }}
            />
          </div>
          {Array.from(availableCards.entries()).map(([category, cards]) => (
            <div key={category} className="palette-category">
              <div
                className="palette-category-header"
                onClick={() => toggleCategory(category)}
              >
                <span
                  className={`chevron ${collapsedCategories.has(category) ? "collapsed" : ""}`}
                >
                  ▼
                </span>
                <FeroHaIcon
                  name={CATEGORY_ICONS[category] || "Folder"}
                  size={12}
                />
                <span>{CATEGORY_LABELS[category] || category}</span>
                <span style={{ color: "var(--text-muted)", fontSize: 10 }}>
                  ({cards.length})
                </span>
              </div>
              {!collapsedCategories.has(category) &&
                cards.map((card) => (
                  <div
                    key={card.meta.id}
                    className="palette-card-item"
                    draggable
                    onDragStart={(e) => handlePaletteDragStart(e, card)}
                    title={card.meta.description}
                  >
                    <span className="palette-card-item-icon">
                      <FeroHaIcon name={card.meta.icon} size={14} />
                    </span>
                    <span className="palette-card-item-name">
                      {card.meta.name}
                    </span>
                  </div>
                ))}
            </div>
          ))}
        </div>

        {/* ── Canvas ── */}
        <div className="pipeline-canvas-container">
          <div
            ref={canvasRef}
            className="pipeline-canvas"
            onClick={handleCanvasClick}
            onDragOver={handleCanvasDragOver}
            onDrop={handleCanvasDrop}
            onContextMenu={handleContextMenu}
          >
            {/* ── SVG Layer (edges + ports + temp edge) ── */}
            <svg className="pipeline-svg-layer">
              <defs>
                <marker
                  id="pipe-arrowhead"
                  markerWidth="8"
                  markerHeight="6"
                  refX="8"
                  refY="3"
                  orient="auto"
                >
                  <polygon points="0 0, 8 3, 0 6" fill="#3d5568" />
                </marker>
              </defs>

              {/* Edges */}
              {pipeline.edges.map((edge) => {
                const coords = getEdgeCoords(edge);
                if (!coords) return null;
                const { x1, y1, x2, y2 } = coords;
                const isSelected = selectedEdgeId === edge.id;
                return (
                  <g key={edge.id}>
                    <line
                      className="edge-hit-area"
                      x1={x1}
                      y1={y1}
                      x2={x2}
                      y2={y2}
                      onClick={(e) => {
                        e.stopPropagation();
                        setSelectedEdgeId(edge.id);
                        setSelectedNodeId(null);
                      }}
                      onContextMenu={(e) => handleEdgeContextMenu(e, edge.id)}
                    />
                    <line
                      className={`edge-line ${isSelected ? "selected" : ""}`}
                      x1={x1}
                      y1={y1}
                      x2={x2}
                      y2={y2}
                      markerEnd="url(#pipe-arrowhead)"
                    />
                  </g>
                );
              })}

              {/* Temporary edge while connecting */}
              {connectingFromNodeId && tempEdgeEnd && (() => {
                const srcNode = pipeline.nodes.find(
                  (n) => n.id === connectingFromNodeId
                );
                if (!srcNode) return null;
                const x1 = srcNode.position.x + NODE_WIDTH / 2;
                const y1 = srcNode.position.y + NODE_MIN_HEIGHT + PORT_OFFSET_Y_OUT;
                return (
                  <line
                    className="temp-edge"
                    x1={x1}
                    y1={y1}
                    x2={tempEdgeEnd.x}
                    y2={tempEdgeEnd.y}
                  />
                );
              })()}

              {/* Port circles (output ports) */}
              {pipeline.nodes.map((node) => {
                const ox = node.position.x + NODE_WIDTH / 2;
                const oy = node.position.y + NODE_MIN_HEIGHT + PORT_OFFSET_Y_OUT;
                const ix = node.position.x + NODE_WIDTH / 2;
                const iy = node.position.y + PORT_OFFSET_Y_IN;
                return (
                  <g key={`ports-${node.id}`}>
                    <circle
                      className="port-circle"
                      cx={ix}
                      cy={iy}
                      r={PORT_RADIUS}
                      data-port-node-id={node.id}
                    >
                      <title>输入端口</title>
                    </circle>
                    <circle
                      className="port-circle"
                      cx={ox}
                      cy={oy}
                      r={PORT_RADIUS}
                      onMouseDown={(e) => handlePortMouseDown(e, node.id)}
                      data-port-node-id={node.id}
                    >
                      <title>输出端口，拖动以连接</title>
                    </circle>
                  </g>
                );
              })}
            </svg>

            {/* ── Nodes ── */}
            {pipeline.nodes.map((node) => {
              const execState = getNodeExecStatus(node.id);
              let nodeClass = "pipeline-node";
              if (node.id === selectedNodeId) nodeClass += " selected";
              if (execState === "running") nodeClass += " executing";
              if (execState === "completed") nodeClass += " completed";
              if (execState === "failed") nodeClass += " failed";
              if (node.type === "start") nodeClass += " node-start";
              if (node.type === "end") nodeClass += " node-end";
              if (node.type === "condition") nodeClass += " node-condition";
              if (node.type === "parallel") nodeClass += " node-parallel";

              return (
                <div
                  key={node.id}
                  className={nodeClass}
                  style={{
                    left: node.position.x,
                    top: node.position.y,
                    width: NODE_WIDTH,
                    cursor: draggingNodeId === node.id ? "grabbing" : "grab",
                  }}
                  onMouseDown={(e) => handleNodeMouseDown(e, node.id)}
                  onDoubleClick={(e) => handleNodeDoubleClick(e, node)}
                  onContextMenu={(e) => handleNodeContextMenu(e, node.id)}
                >
                  <div className="pipeline-node-header">
                    <FeroHaIcon
                      name={
                        node.type === "start"
                          ? "Play"
                          : node.type === "end"
                          ? "Square"
                          : node.type === "condition"
                          ? "GitBranch"
                          : node.type === "parallel"
                          ? "Zap"
                          : node.card?.meta.icon || "Settings"
                      }
                      size={13}
                    />
                    <span className="pipeline-node-title">
                      {node.type === "start"
                        ? "开始"
                        : node.type === "end"
                        ? "结束"
                        : node.type === "condition"
                        ? "条件"
                        : node.type === "parallel"
                        ? "并行"
                        : node.card?.meta.name || "指令"}
                    </span>
                  </div>
                  {node.type === "command" && node.card && (
                    <div className="pipeline-node-body">
                      <p>{node.card.meta.description}</p>
                    </div>
                  )}
                  {node.type === "condition" && (
                    <div className="pipeline-node-body">
                      <p>{node.condition || "未设置条件"}</p>
                    </div>
                  )}
                  {(execState === "completed" || execState === "failed") && (
                    <div
                      className={`pipeline-node-badge ${execState}`}
                      title={execState}
                    >
                      {execState === "completed" ? "\u2713" : "!"}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        {/* ── Properties Panel ── */}
        {(selectedNode || selectedEdge) && (
          <div className="pipeline-properties-panel">
            <div className="pipeline-properties-header">
              <span>
                {selectedNode ? "节点属性" : "连线属性"}
              </span>
              <button
                className="pipeline-properties-close"
                onClick={() => {
                  setSelectedNodeId(null);
                  setSelectedEdgeId(null);
                }}
              >
                ×
              </button>
            </div>

            {selectedNode && (
              <>
                <div className="pipeline-property-section">
                  <div className="pipeline-property-label">基础</div>
                  <div className="pipeline-property-row">
                    <label>类型</label>
                    <span style={{ fontSize: 12, color: "var(--text-primary)" }}>
                      {selectedNode.type}
                    </span>
                  </div>
                  {selectedNode.card && (
                    <>
                      <div className="pipeline-property-row">
                        <label>指令卡</label>
                        <span
                          style={{ fontSize: 12, color: "var(--text-primary)" }}
                        >
                          {selectedNode.card.meta.name}
                        </span>
                      </div>
                      <div className="pipeline-property-row">
                        <label>说明</label>
                        <p
                          style={{
                            fontSize: 11,
                            color: "var(--text-muted)",
                            margin: 0,
                          }}
                        >
                          {selectedNode.card.meta.description}
                        </p>
                      </div>
                    </>
                  )}
                  {selectedNode.type === "condition" && (
                    <div className="pipeline-property-row">
                      <label>条件</label>
                      <input
                        type="text"
                        value={selectedNode.condition || ""}
                        onChange={(e) =>
                          updatePipeline((prev) => ({
                            ...prev,
                            nodes: prev.nodes.map((n) =>
                              n.id === selectedNode.id
                                ? { ...n, condition: e.target.value }
                                : n
                            ),
                          }))
                        }
                      />
                    </div>
                  )}
                </div>
                {selectedNode.type === "command" &&
                  selectedNode.params &&
                  selectedNode.card && (
                    <div className="pipeline-property-section">
                      <div className="pipeline-property-label">
                        参数
                      </div>
                      {selectedNode.card.params.map((param) => (
                        <div
                          key={param.name}
                          className="pipeline-property-row"
                        >
                          <label>{param.label || param.name}</label>
                          {param.type === "textarea" ? (
                            <textarea
                              rows={2}
                              value={String(
                                selectedNode.params?.[
                                  param.templateVar || param.name
                                ] ?? ""
                              )}
                              onChange={(e) => {
                                const newParams = {
                                  ...selectedNode.params,
                                  [param.templateVar || param.name]:
                                    e.target.value,
                                };
                                updateNodeParams(selectedNode.id, newParams);
                              }}
                            />
                          ) : param.type === "select" ? (
                            <select
                              value={String(
                                selectedNode.params?.[
                                  param.templateVar || param.name
                                ] ?? param.defaultValue ?? ""
                              )}
                              onChange={(e) => {
                                const newParams = {
                                  ...selectedNode.params,
                                  [param.templateVar || param.name]:
                                    e.target.value,
                                };
                                updateNodeParams(selectedNode.id, newParams);
                              }}
                            >
                              {(param.options || []).map((opt) => (
                                <option key={opt.value} value={opt.value}>
                                  {opt.label}
                                </option>
                              ))}
                            </select>
                          ) : param.type === "number" ? (
                            <input
                              type="number"
                              value={
                                selectedNode.params?.[
                                  param.templateVar || param.name
                                ] as number | string ?? param.defaultValue ?? ""
                              }
                              min={param.validation?.min}
                              max={param.validation?.max}
                              onChange={(e) => {
                                const newParams = {
                                  ...selectedNode.params,
                                  [param.templateVar || param.name]:
                                    e.target.value,
                                };
                                updateNodeParams(selectedNode.id, newParams);
                              }}
                            />
                          ) : (
                            <input
                              type="text"
                              value={String(
                                selectedNode.params?.[
                                  param.templateVar || param.name
                                ] ?? ""
                              )}
                              onChange={(e) => {
                                const newParams = {
                                  ...selectedNode.params,
                                  [param.templateVar || param.name]:
                                    e.target.value,
                                };
                                updateNodeParams(selectedNode.id, newParams);
                              }}
                            />
                          )}
                        </div>
                      ))}
                    </div>
                  )}
              </>
            )}

            {selectedEdge && (
              <div className="pipeline-edge-info">
                <span>
                  来源：{" "}
                  {pipeline.nodes.find((n) => n.id === selectedEdge.source)
                    ?.card?.meta.name || selectedEdge.source}
                </span>
                <br />
                <span>
                  目标：{" "}
                  {pipeline.nodes.find((n) => n.id === selectedEdge.target)
                    ?.card?.meta.name || selectedEdge.target}
                </span>
                <div style={{ marginTop: 10 }}>
                  <button
                    className="pipeline-toolbar-btn danger-btn"
                    onClick={() => removeEdge(selectedEdge.id)}
                    style={{ fontSize: 11 }}
                  >
                    <FeroHaIcon name="Trash2" size={12} />
                    删除连线
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

        {/* ── Inline Param Editor ── */}
        {paramsEditingNodeId &&
          pipeline.nodes.find((n) => n.id === paramsEditingNodeId)?.card && (
            <>
              <div
                className="pipeline-inline-editor-overlay"
                onClick={() => setParamsEditingNodeId(null)}
              />
              <div
                className="pipeline-inline-editor"
                style={{
                  left: paramsEditorPos.x,
                  top: paramsEditorPos.y,
                }}
              >
                <h4>
                  编辑：{" "}
                  {
                    pipeline.nodes.find((n) => n.id === paramsEditingNodeId)!
                      .card!.meta.name
                  }
                </h4>
                {pipeline.nodes
                  .find((n) => n.id === paramsEditingNodeId)!
                  .card!.params.map((param) => {
                    const node = pipeline.nodes.find(
                      (n) => n.id === paramsEditingNodeId
                    )!;
                    const value =
                      node.params?.[param.templateVar || param.name] ?? "";
                    return (
                      <div key={param.name} className="pipeline-inline-editor-param">
                        <label>{param.label || param.name}</label>
                        {param.type === "textarea" ? (
                          <textarea
                            rows={2}
                            value={String(value)}
                            onChange={(e) => {
                              const newParams = {
                                ...node.params,
                                [param.templateVar || param.name]:
                                  e.target.value,
                              };
                              updateNodeParams(node.id, newParams);
                            }}
                          />
                        ) : param.type === "select" ? (
                          <select
                            value={String(value)}
                            onChange={(e) => {
                              const newParams = {
                                ...node.params,
                                [param.templateVar || param.name]:
                                  e.target.value,
                              };
                              updateNodeParams(node.id, newParams);
                            }}
                          >
                            {(param.options || []).map((opt) => (
                              <option key={opt.value} value={opt.value}>
                                {opt.label}
                              </option>
                            ))}
                          </select>
                        ) : (
                          <input
                            type={param.type === "number" ? "number" : "text"}
                            value={String(value)}
                            min={param.validation?.min}
                            max={param.validation?.max}
                            onChange={(e) => {
                              const newParams = {
                                ...node.params,
                                [param.templateVar || param.name]:
                                  e.target.value,
                              };
                              updateNodeParams(node.id, newParams);
                            }}
                          />
                        )}
                      </div>
                    );
                  })}
                <div className="pipeline-inline-editor-actions">
                  <button
                    className="btn-cancel"
                    onClick={() => setParamsEditingNodeId(null)}
                  >
                    关闭
                  </button>
                </div>
              </div>
            </>
          )}
      </div>

      {/* ── Context Menu ── */}
      {contextMenu && (
        <div
          className="pipeline-context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          {contextMenu.nodeId && (
            <>
              <div
                className="pipeline-context-menu-item"
                onClick={() => {
                  const node = pipeline.nodes.find(
                    (n) => n.id === contextMenu.nodeId
                  );
                  if (node?.type === "command" && node.card) {
                    const nodeEl = document.querySelector(
                      `[data-node-id="${node.id}"]`
                    );
                    const rect = nodeEl?.getBoundingClientRect();
                    if (rect) setParamsEditorPos({ x: rect.right + 10, y: rect.top });
                    setParamsEditingNodeId(node.id);
                    setSelectedNodeId(node.id);
                  }
                  setContextMenu(null);
                }}
              >
                <FeroHaIcon name="Settings" size={12} />
                编辑参数
              </div>
              <div className="pipeline-context-menu-divider" />
              <div
                className="pipeline-context-menu-item danger"
                onClick={() => {
                  if (contextMenu.nodeId) removeNode(contextMenu.nodeId);
                  setContextMenu(null);
                }}
              >
                <FeroHaIcon name="Trash2" size={12} />
                删除节点
              </div>
            </>
          )}
          {contextMenu.edgeId && (
            <div
              className="pipeline-context-menu-item danger"
              onClick={() => {
                removeEdge(contextMenu.edgeId!);
                setContextMenu(null);
              }}
            >
              <FeroHaIcon name="Trash2" size={12} />
              删除连线
            </div>
          )}
          {!contextMenu.nodeId && !contextMenu.edgeId && (
            <div
              className="pipeline-context-menu-item"
              onClick={() => setContextMenu(null)}
            >
              暂无可用操作
            </div>
          )}
        </div>
      )}

      {/* ── Status Bar ── */}
      <div className="pipeline-status-bar">
        <span className="status-item">
          <span className="status-dot" />
          节点：{pipeline.nodes.length}
        </span>
        <span className="status-item">
          <span
            className={`status-dot ${pipeline.edges.length > 0 ? "connected" : ""}`}
          />
          连线：{pipeline.edges.length}
        </span>
        <span className="status-item">
          状态：{PIPELINE_STATUS_LABELS[execStatus]}
        </span>
        {execStatus === "running" && (
          <span className="status-item">
            进度：{execProgress}%
          </span>
        )}
      </div>
    </div>
  );
}
