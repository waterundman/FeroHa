import { useCallback, useEffect, useRef, useState } from "react";
import * as d3 from "d3-force";
import { useAppStore, type GraphData, type GraphEdgeType } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";
import "./GraphView.css";

interface SimNode extends d3.SimulationNodeDatum {
  id: string;
  title: string;
  outgoing: number;
  incoming: number;
  radius: number;
  activation?: number;
}

interface SimLink extends d3.SimulationLinkDatum<SimNode> {
  edgeType: GraphEdgeType;
  origin: string;
  confidence: number;
  memoryRegion: GraphMemoryRegion;
}

export interface GraphEdgeStyle {
  dash: number[];
  width: number;
  alpha: number;
  tone: "default" | "accent" | "muted";
  memoryRegion: GraphMemoryRegion;
  color: string;
  animationSpeed: number;
}

type GraphEdgeLike = {
  from?: string;
  to?: string;
  edge_type?: GraphEdgeType;
  origin?: string;
  memory_region?: string;
};

export type GraphEdgeCategory = "wikilink" | "structure" | "dream";
export type GraphMemoryRegion = "working" | "semantic" | "long_term" | "bridge";
export type GraphViewMode = "focus" | "three-zone";

export type GraphEdgeCategoryState = Record<GraphEdgeCategory, boolean>;

export interface MemoryRegionVisualPlan {
  label: string;
  color: string;
  animationSpeed: number;
  role: "memory_zone" | "cross_zone_link";
}

type DemoGraphNode = Pick<SimNode, "id" | "title" | "outgoing" | "incoming">;
type DemoGraphEdge = {
  from: string;
  to: string;
  edgeType: GraphEdgeType;
  origin: string;
  confidence: number;
  memoryRegion: GraphMemoryRegion;
};

const defaultEdgeFilters: GraphEdgeCategoryState = {
  wikilink: true,
  structure: true,
  dream: true,
};

const demoGraphNodes: DemoGraphNode[] = [
  { id: "welcome", title: "欢迎", outgoing: 2, incoming: 0 },
  { id: "architecture", title: "系统架构", outgoing: 1, incoming: 1 },
  { id: "dual-track", title: "本地工作流", outgoing: 2, incoming: 2 },
  { id: "llm", title: "LLM 内部", outgoing: 0, incoming: 1 },
  { id: "rust", title: "Rust 后端", outgoing: 1, incoming: 0 },
];

const demoGraphEdges: DemoGraphEdge[] = [
  { from: "welcome", to: "architecture", edgeType: "reference", origin: "demo", confidence: 1, memoryRegion: "working" },
  { from: "architecture", to: "dual-track", edgeType: "parent", origin: "demo", confidence: 1, memoryRegion: "semantic" },
  { from: "dual-track", to: "llm", edgeType: "related", origin: "demo", confidence: 1, memoryRegion: "long_term" },
  { from: "welcome", to: "rust", edgeType: "bridge", origin: "demo", confidence: 1, memoryRegion: "bridge" },
];

export function graphManagerCaption(): string {
  return "AI Manager 知识图谱";
}

export function graphViewModeOptions(): Array<{ value: GraphViewMode; label: string; title: string }> {
  return [
    { value: "focus", label: "聚焦", title: "只显示焦点、直接邻居和跨区桥" },
    { value: "three-zone", label: "Dream 三区", title: "显示工作、结构语义、长期三类记忆" },
  ];
}

export function graphMemoryLegendRegions(): GraphMemoryRegion[] {
  return ["working", "semantic", "long_term", "bridge"];
}

export function graphDisplaySummaryCounts(graph: Pick<GraphData, "nodes" | "edges">): { nodes: number; edges: number } {
  const edgePlan = graphEdgeDisplayPlan(graph.edges, defaultEdgeFilters);
  return {
    nodes: graph.nodes.length === 0 ? demoGraphNodes.length : graph.nodes.length,
    edges: edgePlan.useDemo ? demoGraphEdges.length : graph.edges.length,
  };
}

const dreamZoneVisualPlans: Record<GraphMemoryRegion, MemoryRegionVisualPlan> = {
  working: {
    label: "工作记忆",
    color: "#94e2d5",
    animationSpeed: 0.65,
    role: "memory_zone",
  },
  semantic: {
    label: "结构语义",
    color: "#cba6f7",
    animationSpeed: 0.22,
    role: "memory_zone",
  },
  long_term: {
    label: "长期记忆",
    color: "#a6adc8",
    animationSpeed: 0.08,
    role: "memory_zone",
  },
  bridge: {
    label: "跨区连接",
    color: "#f9e2af",
    animationSpeed: 1.05,
    role: "cross_zone_link",
  },
};

export function graphEdgeCategoryLabel(category: GraphEdgeCategory): string {
  switch (category) {
    case "wikilink":
      return "Wiki 双链";
    case "structure":
      return "结构语义";
    case "dream":
      return "Dream 记忆";
  }
}

export function graphEdgeCategoryTitle(category: GraphEdgeCategory, enabled: boolean): string {
  return `${enabled ? "隐藏" : "显示"} ${graphEdgeCategoryLabel(category)}连接`;
}

export function memoryRegionVisualPlan(region: GraphMemoryRegion): MemoryRegionVisualPlan {
  return dreamZoneVisualPlans[region];
}

function normalizeMemoryRegion(value?: string): GraphMemoryRegion | null {
  const normalized = value?.trim().toLowerCase().replace(/[-\s]+/g, "_");
  if (!normalized) return null;
  if (["core", "structure", "structural", "jsonld", "tree", "semantic_core"].includes(normalized)) {
    return "semantic";
  }
  if (["working", "work", "hot", "wikilink", "human", "scratch", "temporal"].includes(normalized)) {
    return "working";
  }
  if (["dream", "rem", "semantic", "memory_dream"].includes(normalized)) {
    return "semantic";
  }
  if (["archive", "cold", "cold_archive", "storage", "source", "long_term", "longterm"].includes(normalized)) {
    return "long_term";
  }
  if (["bridge", "dream_bridge", "cross_region", "cross"].includes(normalized)) {
    return "bridge";
  }
  return null;
}

export function graphMemoryRegion(edge: GraphEdgeLike): GraphMemoryRegion {
  const explicit = normalizeMemoryRegion(edge.memory_region);
  if (explicit) return explicit;

  const origin = edge.origin?.toLowerCase();
  const edgeType = edge.edge_type ?? "reference";
  if (edgeType === "bridge") return "bridge";
  if (origin === "dream" && edgeType === "temporal") return "working";
  if (edgeType === "temporal") return "working";
  if (origin === "dream" || edgeType === "semantic") return "semantic";
  if (origin === "archive" || origin === "cold" || edgeType === "source") return "long_term";
  if (
    origin === "frontmatter" ||
    origin === "jsonld" ||
    origin === "legacy_frontmatter" ||
    origin === "mdt" ||
    edgeType === "parent" ||
    edgeType === "related" ||
    edgeType === "sequence"
  ) {
    return "semantic";
  }
  return "working";
}

export function graphEdgeCategory(edge: GraphEdgeLike): GraphEdgeCategory {
  const origin = edge.origin?.toLowerCase();
  const edgeType = edge.edge_type ?? "reference";
  if (origin === "dream" || edgeType === "semantic" || edgeType === "temporal" || edgeType === "bridge") {
    return "dream";
  }
  if (
    origin === "frontmatter" ||
    origin === "jsonld" ||
    origin === "legacy_frontmatter" ||
    origin === "mdt" ||
    edgeType === "parent" ||
    edgeType === "related" ||
    edgeType === "source" ||
    edgeType === "sequence"
  ) {
    return "structure";
  }
  return "wikilink";
}

export function filterGraphEdgesByCategory<T extends GraphEdgeLike>(
  edges: T[],
  enabled: GraphEdgeCategoryState,
): T[] {
  return edges.filter((edge) => enabled[graphEdgeCategory(edge)]);
}

export function graphEdgeDisplayPlan<T extends GraphEdgeLike>(
  edges: T[],
  enabled: GraphEdgeCategoryState,
): { edges: T[]; useDemo: boolean } {
  return {
    edges: filterGraphEdgesByCategory(edges, enabled),
    useDemo: edges.length === 0,
  };
}

export function filterGraphEdgesByViewMode<T extends GraphEdgeLike>(
  edges: T[],
  mode: GraphViewMode,
  focusNodeId?: string,
): T[] {
  if (mode !== "focus" || !focusNodeId) return edges;
  return edges.filter((edge) => edge.from === focusNodeId || edge.to === focusNodeId || graphMemoryRegion(edge) === "bridge");
}

export function styleForGraphEdge(edgeType?: GraphEdgeType, memoryRegion?: string): GraphEdgeStyle {
  const region = normalizeMemoryRegion(memoryRegion) ?? graphMemoryRegion({ edge_type: edgeType });
  const visual = memoryRegionVisualPlan(region);
  switch (edgeType ?? "reference") {
    case "parent":
      return { dash: [], width: 1.8, alpha: 0.85, tone: "default", memoryRegion: region, color: visual.color, animationSpeed: visual.animationSpeed };
    case "reference":
      return { dash: [6, 4], width: 1.2, alpha: 0.7, tone: "default", memoryRegion: region, color: visual.color, animationSpeed: visual.animationSpeed };
    case "related":
      return { dash: [2, 5], width: 1, alpha: 0.55, tone: "muted", memoryRegion: region, color: visual.color, animationSpeed: visual.animationSpeed };
    case "source":
      return { dash: [8, 4], width: 1.4, alpha: 0.8, tone: "default", memoryRegion: region, color: visual.color, animationSpeed: visual.animationSpeed };
    case "sequence":
      return { dash: [10, 4], width: 1.2, alpha: 0.75, tone: "default", memoryRegion: region, color: visual.color, animationSpeed: visual.animationSpeed };
    case "semantic":
      return { dash: [], width: 0.8, alpha: 0.32, tone: "muted", memoryRegion: region, color: visual.color, animationSpeed: visual.animationSpeed };
    case "temporal":
      return { dash: [3, 3], width: 0.9, alpha: 0.42, tone: "muted", memoryRegion: region, color: visual.color, animationSpeed: visual.animationSpeed };
    case "bridge":
      return { dash: [], width: 2.2, alpha: 0.9, tone: "accent", memoryRegion: region, color: visual.color, animationSpeed: visual.animationSpeed };
  }
}

function hexToRgba(hex: string, alpha: number): string {
  const m = hex.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (!m) return hex;
  return `rgba(${parseInt(m[1], 16)}, ${parseInt(m[2], 16)}, ${parseInt(m[3], 16)}, ${alpha})`;
}

function getCSSVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function hasTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

/**
 * GraphView — Force-directed knowledge graph visualization
 * Uses D3 force simulation for layout. Supports pan/zoom and node dragging.
 * Click a node to navigate to that note.
 */
export default function GraphView({ focusNotePath }: { focusNotePath?: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const graph = useAppStore((s) => s.graph);
  const openNote = useAppStore((s) => s.openNote);
  const setActivePanel = useAppStore((s) => s.setActivePanel);
  const addToNavigationPath = useAppStore((s) => s.addToNavigationPath);
  const clearNavigationPath = useAppStore((s) => s.clearNavigationPath);
  const navigationPath = useAppStore((s) => s.navigationPath);
  const [searchQuery, setSearchQuery] = useState("");
  const [edgeFilters] = useState<GraphEdgeCategoryState>(defaultEdgeFilters);
  const [graphViewMode, setGraphViewMode] = useState<GraphViewMode>("focus");

  // Reload graph with focus data when focusNotePath changes
  useEffect(() => {
    if (!hasTauriRuntime()) return;
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke<GraphData>(
        "get_graph_with_focus",
        { focusPath: focusNotePath || null }
      ).then((data) => {
        useAppStore.getState().setGraph(data);
        if (focusNotePath) {
          addToNavigationPath(focusNotePath);
        }
      }).catch(() => {});
    });
  }, [focusNotePath]);

  const exportPng = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    canvas.toBlob((blob) => {
      if (!blob) return;
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `feroha-graph-${Date.now()}.png`;
      a.click();
    }, "image/png");
  }, []);

  useEffect(() => {
    if (!canvasRef.current) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Resize to container
    const dpr = window.devicePixelRatio;
    const resize = () => {
      const rect = canvas.parentElement!.getBoundingClientRect();
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      canvas.style.width = `${rect.width}px`;
      canvas.style.height = `${rect.height}px`;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    resize();
    window.addEventListener("resize", resize);

    // Cache CSS variable values for canvas rendering
    const bgPrimary = getCSSVar("--bg-primary");
    const borderColor = getCSSVar("--border-color");
    const accentPrimary = getCSSVar("--accent-primary");
    const accentGlow = getCSSVar("--accent-glow");
    const bgSecondary = getCSSVar("--bg-secondary");
    const textPrimary = getCSSVar("--text-primary");
    const textSecondary = getCSSVar("--text-secondary");
    const nodeDefaultFill = hexToRgba(accentPrimary, 0.7);

    // Build simulation nodes
    const nodeMap = new Map<string, SimNode>();
    const nodes: SimNode[] = graph.nodes.map((n) => {
      const simNode: SimNode = {
        id: n.id,
        title: n.title,
        outgoing: n.outgoing,
        incoming: n.incoming,
        radius: 6 + Math.min(n.incoming * 2, 20),
        activation: n.activation,
      };
      nodeMap.set(n.id, simNode);
      return simNode;
    });

    // If only demo data, add more visible nodes
    if (nodes.length === 0) {
      demoGraphNodes.forEach((n) => {
        const simNode: SimNode = { ...n, radius: 8 };
        nodeMap.set(n.id, simNode);
        nodes.push(simNode);
      });
    }

    // Build links
    let links: SimLink[] = [];
    const edgePlan = graphEdgeDisplayPlan(graph.edges, edgeFilters);
    const displayEdges = filterGraphEdgesByViewMode(edgePlan.edges, graphViewMode, focusNotePath);
    if (!edgePlan.useDemo) {
      links = displayEdges
        .map((e) => {
          const source = nodeMap.get(e.from);
          const target = nodeMap.get(e.to);
          return source && target
            ? {
                source,
                target,
                edgeType: e.edge_type ?? "reference",
                origin: e.origin ?? "wikilink",
                confidence: e.confidence ?? 1,
                memoryRegion: graphMemoryRegion(e),
              }
            : null;
        })
        .filter(Boolean) as SimLink[];
    } else {
      demoGraphEdges.forEach((edge) => {
        const source = nodeMap.get(edge.from);
        const target = nodeMap.get(edge.to);
        if (source && target) {
          links.push({
            source,
            target,
            edgeType: edge.edgeType,
            origin: edge.origin,
            confidence: edge.confidence,
            memoryRegion: edge.memoryRegion,
          });
        }
      });
    }

    // Simulation
    const sim = d3
      .forceSimulation<SimNode>(nodes)
      .force(
        "link",
        d3
          .forceLink<SimNode, SimLink>(links)
          .distance(100)
          .strength(0.5)
      )
      .force("charge", d3.forceManyBody().strength(-200))
      .force("center", d3.forceCenter((canvas.width / dpr) / 2, (canvas.height / dpr) / 2))
      .force("collision", d3.forceCollide<SimNode>().radius((d) => d.radius + 10));

    // Track mouse for hover detection
    let hoveredNode: SimNode | null = null;
    let mouseX = 0;
    let mouseY = 0;

    // Track canvas pan/zoom state
    const transform = { x: 0, y: 0, scale: 1 };
    let isPanning = false;
    let panStartX = 0;
    let panStartY = 0;
    let panStartTransform = { x: 0, y: 0 };

    // Track node dragging state
    let isNodeDragging = false;
    let dragNode: SimNode | null = null;
    let dragStartMouseX = 0;
    let dragStartMouseY = 0;
    let dragTotalDistance = 0;
    let requestRender: () => void = () => undefined;

    const handleMouseDown = (e: MouseEvent) => {
      if (hoveredNode) {
        isNodeDragging = true;
        dragNode = hoveredNode;
        dragStartMouseX = e.clientX;
        dragStartMouseY = e.clientY;
        dragTotalDistance = 0;
        dragNode.fx = dragNode.x ?? 0;
        dragNode.fy = dragNode.y ?? 0;
      } else {
        isPanning = true;
        panStartX = e.clientX;
        panStartY = e.clientY;
        panStartTransform = { x: transform.x, y: transform.y };
      }
      requestRender();
    };

    const handleMouseMove = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      mouseX = e.clientX - rect.left;
      mouseY = e.clientY - rect.top;

      if (isNodeDragging && dragNode) {
        const graphX = (e.clientX - rect.left - transform.x) / transform.scale;
        const graphY = (e.clientY - rect.top - transform.y) / transform.scale;
        dragNode.fx = graphX;
        dragNode.fy = graphY;
        dragTotalDistance = Math.sqrt(
          (e.clientX - dragStartMouseX) ** 2 + (e.clientY - dragStartMouseY) ** 2
        );
        sim.alpha(0.3).restart();
      } else if (isPanning) {
        transform.x = panStartTransform.x + (e.clientX - panStartX);
        transform.y = panStartTransform.y + (e.clientY - panStartY);
      }

      // Cursor feedback
      if (isNodeDragging) {
        canvas.style.cursor = "grabbing";
      } else if (hoveredNode) {
        canvas.style.cursor = "grab";
      } else if (isPanning) {
        canvas.style.cursor = "grabbing";
      } else {
        canvas.style.cursor = "default";
      }
      requestRender();
    };

    const handleMouseUp = () => {
      if (isNodeDragging && dragNode) {
        dragNode.fx = null;
        dragNode.fy = null;
        sim.alpha(0.3).restart();
      }
      isNodeDragging = false;
      dragNode = null;
      isPanning = false;
      requestRender();
    };

    const handleWheel = (e: WheelEvent) => {
      e.preventDefault();
      const delta = e.deltaY > 0 ? 0.9 : 1.1;
      transform.scale = Math.max(0.2, Math.min(5, transform.scale * delta));
      requestRender();
    };

    const handleClick = () => {
      if (dragTotalDistance < 5 && hoveredNode && hoveredNode.id) {
        const node = hoveredNode;
        addToNavigationPath(node.id);
        if (hasTauriRuntime()) {
          import("@tauri-apps/api/core")
            .then(({ invoke }) => invoke<string>("read_note", { path: node.id }))
            .then((content) => openNote(node.id, content))
            .catch(() => openNote(node.id, `# ${node.title}\n\n`));
        } else {
          openNote(node.id, `# ${node.title}\n\n`);
        }
        setActivePanel("editor");
      }
    };

    canvas.addEventListener("mousedown", handleMouseDown);
    canvas.addEventListener("mousemove", handleMouseMove);
    canvas.addEventListener("mouseup", handleMouseUp);
    canvas.addEventListener("wheel", handleWheel);
    canvas.addEventListener("click", handleClick);

    let frameId: number | null = null;
    let disposed = false;

    const draw = () => {
      const w = canvas.width / window.devicePixelRatio;
      const h = canvas.height / window.devicePixelRatio;
      const now = Date.now();

      ctx.clearRect(0, 0, w, h);

      // Background
      ctx.fillStyle = bgPrimary;
      ctx.fillRect(0, 0, w, h);

      // Apply pan/zoom transform
      ctx.save();
      ctx.translate(transform.x, transform.y);
      ctx.scale(transform.scale, transform.scale);

      // Edges
      links.forEach((l) => {
        const s = l.source as SimNode;
        const t = l.target as SimNode;
        if (s.x == null || s.y == null || t.x == null || t.y == null) return;
        const edgeStyle = styleForGraphEdge(l.edgeType, l.memoryRegion);
        const confidenceAlpha = edgeStyle.alpha * Math.max(0.1, Math.min(1, l.confidence));
        const dashOffset = -((now / 120) * edgeStyle.animationSpeed);
        ctx.save();
        if (edgeStyle.animationSpeed >= 0.8) {
          ctx.globalAlpha = confidenceAlpha * 0.22;
          ctx.strokeStyle = edgeStyle.color;
          ctx.lineWidth = edgeStyle.width + 5;
          ctx.setLineDash([]);
          ctx.beginPath();
          ctx.moveTo(s.x, s.y);
          ctx.lineTo(t.x, t.y);
          ctx.stroke();
        }
        ctx.globalAlpha = confidenceAlpha;
        ctx.strokeStyle = edgeStyle.tone === "accent"
          ? edgeStyle.color || accentPrimary
          : edgeStyle.tone === "muted"
            ? hexToRgba(edgeStyle.color || textSecondary, 0.78)
            : edgeStyle.color || borderColor;
        ctx.lineWidth = edgeStyle.width;
        ctx.setLineDash(edgeStyle.dash);
        ctx.lineDashOffset = dashOffset;
        ctx.beginPath();
        ctx.moveTo(s.x, s.y);
        ctx.lineTo(t.x, t.y);
        ctx.stroke();
        ctx.restore();
      });
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;

      // Hover detection (convert mouse to graph-space)
      const graphMouseX = (mouseX - transform.x) / transform.scale;
      const graphMouseY = (mouseY - transform.y) / transform.scale;
      hoveredNode = null;
      for (const node of nodes) {
        if (node.x == null || node.y == null) continue;
        const dx = graphMouseX - node.x;
        const dy = graphMouseY - node.y;
        if (Math.sqrt(dx * dx + dy * dy) < node.radius + 4) {
          hoveredNode = node;
          break;
        }
      }

      // Search matching
      const query = searchQuery.toLowerCase().trim();
      const matchingIds = new Set<string>();
      if (query) {
        for (const node of nodes) {
          if (
            node.id.toLowerCase().includes(query) ||
            node.title.toLowerCase().includes(query)
          ) {
            matchingIds.add(node.id);
          }
        }
      }

      // Nodes
      for (const node of nodes) {
        if (node.x == null || node.y == null) continue;
        const isHovered = node === hoveredNode;
        const isBeingDragged = node === dragNode && isNodeDragging;
        const isMatch = query ? matchingIds.has(node.id) : true;

        // Focus-aware activation rendering
        const activation = node.activation;
        let nodeAlpha = 1;
        let nodeScale = 1;
        let fillColor = accentPrimary;
        if (activation !== undefined) {
          if (activation >= 0.99) {
            // Focus node: pulse animation via oscillating radius
            nodeAlpha = 1;
            nodeScale = 1.15 + 0.05 * Math.sin(now * 0.004);
          } else if (activation > 0.7) {
            nodeAlpha = 1;
            nodeScale = 1.15;
          } else if (activation >= 0.4) {
            nodeAlpha = 0.65;
            nodeScale = 1;
          } else {
            nodeAlpha = 0.35;
            nodeScale = 1;
            fillColor = getCSSVar("--text-muted") || "#888";
          }
        }

        let r = node.radius * nodeScale;
        if (isBeingDragged) r += 4;
        else if (isHovered) r += 2;
        if (isMatch && query) r *= 1.5;

        const alpha = query && !isMatch ? 0.15 : nodeAlpha;
        ctx.globalAlpha = alpha;

        // Glow ring for dragged or matched node
        if (isBeingDragged || (isMatch && query)) {
          ctx.beginPath();
          ctx.arc(node.x, node.y, r + 6, 0, 2 * Math.PI);
          ctx.strokeStyle = isMatch && query ? accentPrimary : accentGlow;
          ctx.lineWidth = 3;
          ctx.stroke();
        }

        ctx.beginPath();
        ctx.arc(node.x, node.y, r, 0, 2 * Math.PI);
        ctx.fillStyle = isHovered || isBeingDragged || (isMatch && query) ? accentPrimary
          : (activation !== undefined ? fillColor : nodeDefaultFill);
        ctx.fill();

        // Border
        ctx.strokeStyle = (isHovered || isBeingDragged || (isMatch && query)) ? accentGlow : bgSecondary;
        ctx.lineWidth = 2;
        ctx.stroke();

        // Label
        const fontSize = isHovered || isBeingDragged || (isMatch && query) ? 13 : 10;
        ctx.font = `${fontSize}px system-ui, sans-serif`;
        ctx.fillStyle = isHovered || isBeingDragged || (isMatch && query) ? textPrimary : textSecondary;
        ctx.textAlign = "center";
        ctx.fillText(node.title, node.x, node.y + node.radius + 16);

        ctx.globalAlpha = 1;
      }

      ctx.restore();
    };

    requestRender = () => {
      if (!disposed) draw();
    };

    const animate = () => {
      if (disposed) return;
      draw();
      frameId = window.requestAnimationFrame(animate);
    };

    // Render loop: keeps animated memory edges and canvas interactions alive after D3 cools.
    sim.on("tick", requestRender);
    frameId = window.requestAnimationFrame(animate);

    return () => {
      disposed = true;
      if (frameId !== null) {
        window.cancelAnimationFrame(frameId);
      }
      sim.stop();
      canvas.removeEventListener("mousedown", handleMouseDown);
      canvas.removeEventListener("mousemove", handleMouseMove);
      canvas.removeEventListener("mouseup", handleMouseUp);
      canvas.removeEventListener("wheel", handleWheel);
      canvas.removeEventListener("click", handleClick);
      window.removeEventListener("resize", resize);
    };
  }, [graph, edgeFilters, graphViewMode, focusNotePath, openNote, searchQuery, addToNavigationPath, setActivePanel]);

  const summaryCounts = graphDisplaySummaryCounts(graph);

  return (
    <div className="graph-view-shell" style={styles.container}>
      <div className="graph-view-header" style={styles.header}>
        <div style={styles.titleGroup}>
          <span style={styles.title}>{graphManagerCaption()}</span>
          <span style={styles.titleMeta}>Dream 三区记忆结构</span>
        </div>
        <div className="graph-view-header-actions" style={styles.headerActions}>
          <div style={styles.searchWrapper}>
            <input
              type="text"
              placeholder="搜索节点..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="graph-search-input feroha-search"
            />
            {searchQuery && (
              <button
                className="graph-icon-button"
                style={styles.clearBtn}
                title="清除搜索"
                onClick={() => setSearchQuery("")}
              >
                <FeroHaIcon name="X" size={12} />
              </button>
            )}
          </div>
          <button className="graph-action-button" style={styles.exportBtn} title="导出为 PNG" onClick={exportPng}>
            <FeroHaIcon name="Download" size={16} />
          </button>
          <div style={styles.modeSwitch} aria-label="图谱视图模式">
            {graphViewModeOptions().map((mode) => (
              <button
                key={mode.value}
                type="button"
                className="graph-legend-toggle"
                aria-pressed={graphViewMode === mode.value}
                title={mode.title}
                style={{
                  ...styles.modeButton,
                  ...(graphViewMode === mode.value ? styles.modeButtonActive : {}),
                }}
                onClick={() => setGraphViewMode(mode.value)}
              >
                {mode.label}
              </button>
            ))}
          </div>
          <span style={styles.stats}>
            {summaryCounts.nodes} 节点 / {summaryCounts.edges} 连线
          </span>
        </div>
      </div>
      {navigationPath.length > 0 && (
        <div style={styles.breadcrumbBar}>
          <button
            className="graph-breadcrumb-clear"
            style={styles.breadcrumbBtn}
            title="清除导航路径"
            onClick={() => clearNavigationPath()}
          >
            <FeroHaIcon name="X" size={12} />
          </button>
          <span style={styles.breadcrumbRoot}>起点</span>
          {navigationPath.map((p, i) => (
            <span key={p + i} style={styles.breadcrumbItem}>
              <span style={styles.breadcrumbSep}>&gt;</span>
              <button
                style={styles.breadcrumbLink}
                title={p}
                onClick={() => {
                  if (hasTauriRuntime()) {
                    import("@tauri-apps/api/core")
                      .then(({ invoke }) =>
                        invoke<string>("read_note", { path: p })
                      )
                      .then((content) => openNote(p, content))
                      .catch(() => openNote(p, `# ${p.split("/").pop()?.replace(".md", "")}\n\n`));
                  } else {
                    openNote(p, `# ${p.split("/").pop()?.replace(".md", "")}\n\n`);
                  }
                  setActivePanel("editor");
                }}
              >
                {p.split("/").pop()?.replace(".md", "") || p}
              </button>
            </span>
          ))}
        </div>
      )}
      <div style={styles.canvasWrapper}>
        <canvas ref={canvasRef} style={styles.canvas} />
      </div>
      <div style={styles.legend}>
        <div className="graph-memory-legend" style={styles.memoryLegend}>
          {graphMemoryLegendRegions().map((region) => {
            const plan = memoryRegionVisualPlan(region);
            return (
              <span key={region} style={styles.memoryLegendItem} title={`运动速率 ${plan.animationSpeed}`}>
                <span style={{ ...styles.memoryLegendDot, backgroundColor: plan.color }} />
                {plan.label}
              </span>
            );
          })}
        </div>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    minWidth: 0,
    overflow: "hidden",
    padding: "12px 14px",
    gap: "10px",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-start",
    flexWrap: "wrap",
    padding: "0 0 10px",
    borderBottom: "1px solid var(--border-color)",
    gap: "12px",
  },
  titleGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "3px",
    minWidth: "160px",
  },
  headerActions: {
    display: "flex",
    alignItems: "center",
    justifyContent: "flex-end",
    flex: "1 1 340px",
    flexWrap: "wrap",
    gap: "8px",
    minWidth: 0,
  },
  searchWrapper: {
    position: "relative" as const,
    display: "flex",
    alignItems: "center",
    flex: "1 1 260px",
    minWidth: "190px",
    maxWidth: "430px",
  },
  clearBtn: {
    position: "absolute" as const,
    right: "6px",
    background: "transparent",
    border: "none",
    cursor: "pointer",
    color: "var(--text-muted)",
    padding: "4px",
    display: "flex",
    borderRadius: "4px",
  },
  exportBtn: {
    background: "var(--control-bg)",
    border: "1px solid var(--control-border)",
    borderRadius: "6px",
    cursor: "pointer",
    color: "var(--text-secondary)",
    padding: "7px 9px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
  title: {
    fontSize: "16px",
    fontWeight: 600,
    color: "var(--text-primary)",
    letterSpacing: 0,
  },
  titleMeta: {
    fontSize: "11px",
    color: "var(--text-muted)",
    letterSpacing: 0,
  },
  stats: {
    fontSize: "11px",
    color: "var(--text-secondary)",
    whiteSpace: "nowrap",
    border: "1px solid var(--border-muted)",
    background: "var(--bg-secondary)",
    borderRadius: "6px",
    padding: "6px 9px",
  },
  canvasWrapper: {
    flex: 1,
    position: "relative",
    minHeight: "380px",
    overflow: "hidden",
    border: "1px solid var(--border-color)",
    borderRadius: "8px",
    background: "var(--bg-primary)",
    boxShadow: "inset 0 0 0 1px var(--border-muted)",
  },
  canvas: {
    width: "100%",
    height: "100%",
  },
  legend: {
    display: "flex",
    justifyContent: "flex-start",
    flexWrap: "wrap",
    gap: "8px",
    padding: "0",
    fontSize: "11px",
    color: "var(--text-muted)",
  },
  memoryLegend: {
    display: "inline-flex",
    alignItems: "center",
    flexWrap: "wrap" as const,
    gap: "6px",
    marginLeft: "2px",
    minWidth: 0,
  },
  memoryLegendItem: {
    display: "inline-flex",
    alignItems: "center",
    gap: "5px",
    minHeight: "26px",
    border: "1px solid var(--border-muted)",
    background: "var(--bg-secondary)",
    borderRadius: "6px",
    color: "var(--text-secondary)",
    padding: "4px 8px",
    fontSize: "11px",
    whiteSpace: "nowrap" as const,
  },
  memoryLegendDot: {
    width: "7px",
    height: "7px",
    borderRadius: "50%",
    boxShadow: "0 0 0 2px var(--bg-primary)",
  },
  legendToggle: {
    border: "1px solid var(--border-color)",
    background: "var(--bg-secondary)",
    color: "var(--text-secondary)",
    borderRadius: "6px",
    padding: "6px 10px",
    cursor: "pointer",
    fontSize: "12px",
    lineHeight: 1,
  },
  legendToggleActive: {
    color: "var(--accent-primary)",
    borderColor: "var(--accent-primary)",
    background: "var(--accent-glow)",
  },
  modeSwitch: {
    display: "inline-flex",
    alignItems: "center",
    gap: "3px",
    padding: "2px",
    border: "1px solid var(--control-border)",
    borderRadius: "7px",
    background: "var(--control-bg)",
  },
  modeButton: {
    border: "1px solid transparent",
    background: "transparent",
    color: "var(--text-secondary)",
    borderRadius: "5px",
    padding: "5px 8px",
    cursor: "pointer",
    fontSize: "11px",
    lineHeight: 1,
  },
  modeButtonActive: {
    color: "var(--accent-primary)",
    borderColor: "var(--accent-primary)",
    background: "var(--accent-glow)",
  },
  breadcrumbBar: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    padding: "6px 8px",
    border: "1px solid var(--border-muted)",
    borderRadius: "6px",
    background: "var(--bg-secondary)",
    overflowX: "auto",
    whiteSpace: "nowrap",
    fontSize: "11px",
    minHeight: "30px",
  },
  breadcrumbBtn: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    background: "var(--control-bg)",
    border: "1px solid var(--control-border)",
    borderRadius: "4px",
    cursor: "pointer",
    color: "var(--text-muted)",
    padding: "3px 5px",
    fontSize: "10px",
  },
  breadcrumbRoot: {
    color: "var(--text-muted)",
    fontSize: "11px",
    flex: "0 0 auto",
  },
  breadcrumbSep: {
    color: "var(--text-muted)",
    padding: "0",
    fontSize: "10px",
  },
  breadcrumbItem: {
    display: "inline-flex",
    alignItems: "center",
    gap: "6px",
    minWidth: 0,
  },
  breadcrumbLink: {
    background: "transparent",
    border: "none",
    cursor: "pointer",
    color: "var(--accent-primary)",
    fontSize: "11px",
    padding: "2px 4px",
    borderRadius: "4px",
    textDecoration: "none",
    maxWidth: "180px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
};
