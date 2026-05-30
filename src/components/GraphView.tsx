import { useCallback, useEffect, useRef, useState } from "react";
import * as d3 from "d3-force";
import { useAppStore, type GraphData, type GraphEdgeType } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";

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
}

export interface GraphEdgeStyle {
  dash: number[];
  width: number;
  alpha: number;
  tone: "default" | "accent" | "muted";
}

export function styleForGraphEdge(edgeType?: GraphEdgeType): GraphEdgeStyle {
  switch (edgeType ?? "reference") {
    case "parent":
      return { dash: [], width: 1.8, alpha: 0.85, tone: "default" };
    case "reference":
      return { dash: [6, 4], width: 1.2, alpha: 0.7, tone: "default" };
    case "related":
      return { dash: [2, 5], width: 1, alpha: 0.55, tone: "muted" };
    case "source":
      return { dash: [8, 4], width: 1.4, alpha: 0.8, tone: "default" };
    case "sequence":
      return { dash: [10, 4], width: 1.2, alpha: 0.75, tone: "default" };
    case "semantic":
      return { dash: [], width: 0.8, alpha: 0.32, tone: "muted" };
    case "temporal":
      return { dash: [3, 3], width: 0.9, alpha: 0.42, tone: "muted" };
    case "bridge":
      return { dash: [], width: 2.2, alpha: 0.9, tone: "accent" };
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
      const demoNodes = [
        { id: "welcome", title: "Welcome", outgoing: 2, incoming: 0 },
        { id: "architecture", title: "Architecture", outgoing: 1, incoming: 1 },
        { id: "dual-track", title: "Dual Track", outgoing: 2, incoming: 2 },
        { id: "llm", title: "LLM Internals", outgoing: 0, incoming: 1 },
        { id: "rust", title: "Rust", outgoing: 1, incoming: 0 },
      ];
      demoNodes.forEach((n) => {
        const simNode: SimNode = { ...n, radius: 8 };
        nodeMap.set(n.id, simNode);
        nodes.push(simNode);
      });
    }

    // Build links
    let links: SimLink[] = [];
    if (graph.edges.length > 0) {
      links = graph.edges
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
              }
            : null;
        })
        .filter(Boolean) as SimLink[];
    } else {
      // Demo links
      if (nodeMap.has("welcome") && nodeMap.has("architecture")) {
        links.push({ source: nodeMap.get("welcome")!, target: nodeMap.get("architecture")!, edgeType: "reference", origin: "demo", confidence: 1 });
      }
      if (nodeMap.has("architecture") && nodeMap.has("dual-track")) {
        links.push({ source: nodeMap.get("architecture")!, target: nodeMap.get("dual-track")!, edgeType: "parent", origin: "demo", confidence: 1 });
      }
      if (nodeMap.has("dual-track") && nodeMap.has("llm")) {
        links.push({ source: nodeMap.get("dual-track")!, target: nodeMap.get("llm")!, edgeType: "related", origin: "demo", confidence: 1 });
      }
      if (nodeMap.has("welcome") && nodeMap.has("rust")) {
        links.push({ source: nodeMap.get("welcome")!, target: nodeMap.get("rust")!, edgeType: "bridge", origin: "demo", confidence: 1 });
      }
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
    };

    const handleWheel = (e: WheelEvent) => {
      e.preventDefault();
      const delta = e.deltaY > 0 ? 0.9 : 1.1;
      transform.scale = Math.max(0.2, Math.min(5, transform.scale * delta));
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

    // Render loop
    sim.on("tick", () => {
      const w = canvas.width / window.devicePixelRatio;
      const h = canvas.height / window.devicePixelRatio;

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
        const edgeStyle = styleForGraphEdge(l.edgeType);
        ctx.save();
        ctx.globalAlpha = edgeStyle.alpha * Math.max(0.1, Math.min(1, l.confidence));
        ctx.strokeStyle = edgeStyle.tone === "accent"
          ? accentPrimary
          : edgeStyle.tone === "muted"
            ? textSecondary
            : borderColor;
        ctx.lineWidth = edgeStyle.width;
        ctx.setLineDash(edgeStyle.dash);
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
      const now = Date.now();
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
    });

    return () => {
      sim.stop();
      canvas.removeEventListener("mousedown", handleMouseDown);
      canvas.removeEventListener("mousemove", handleMouseMove);
      canvas.removeEventListener("mouseup", handleMouseUp);
      canvas.removeEventListener("wheel", handleWheel);
      canvas.removeEventListener("click", handleClick);
      window.removeEventListener("resize", resize);
    };
  }, [graph, openNote, searchQuery, addToNavigationPath, setActivePanel]);

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <span style={styles.title}>Knowledge Graph</span>
        <div style={styles.headerActions}>
          <div style={styles.searchWrapper}>
            <input
              type="text"
              placeholder="Search nodes..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="graph-search-input"
            />
            {searchQuery && (
              <button
                style={styles.clearBtn}
                title="Clear search"
                onClick={() => setSearchQuery("")}
              >
                <FeroHaIcon name="X" size={12} />
              </button>
            )}
          </div>
          <button style={styles.exportBtn} title="Export as PNG" onClick={exportPng}>
            <FeroHaIcon name="Download" size={16} />
          </button>
          <span style={styles.stats}>
            {graph.nodes.length} nodes / {graph.edges.length} edges
          </span>
        </div>
      </div>
      {navigationPath.length > 0 && (
        <div style={styles.breadcrumbBar}>
          <button
            style={styles.breadcrumbBtn}
            title="Clear navigation path"
            onClick={() => clearNavigationPath()}
          >
            <FeroHaIcon name="X" size={12} />
          </button>
          <span style={styles.breadcrumbSep}>Home</span>
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
        <span>Nodes = Notes</span>
        <span>Click node to open</span>
        <span>Drag node to rearrange</span>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "6px 0",
    borderBottom: "1px solid var(--border-color)",
    marginBottom: "12px",
    gap: "8px",
  },
  headerActions: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
  },
  searchWrapper: {
    position: "relative" as const,
    display: "flex",
    alignItems: "center",
  },
  clearBtn: {
    position: "absolute" as const,
    right: "2px",
    background: "none",
    border: "none",
    cursor: "pointer",
    color: "var(--text-muted)",
    padding: "2px",
    display: "flex",
  },
  exportBtn: {
    background: "none",
    border: "1px solid var(--border-color)",
    borderRadius: "4px",
    cursor: "pointer",
    color: "var(--text-secondary)",
    padding: "4px 6px",
    display: "flex",
    alignItems: "center",
  },
  title: {
    fontSize: "14px",
    fontWeight: 600,
    color: "var(--text-primary)",
  },
  stats: {
    fontSize: "11px",
    color: "var(--text-muted)",
  },
  canvasWrapper: {
    flex: 1,
    position: "relative",
    minHeight: "350px",
  },
  canvas: {
    width: "100%",
    height: "100%",
  },
  legend: {
    display: "flex",
    justifyContent: "center",
    gap: "20px",
    padding: "8px",
    fontSize: "11px",
    color: "var(--text-muted)",
  },
  breadcrumbBar: {
    display: "flex",
    alignItems: "center",
    gap: "2px",
    padding: "4px 0",
    marginBottom: "4px",
    borderBottom: "1px solid var(--border-color)",
    overflowX: "auto",
    whiteSpace: "nowrap",
    fontSize: "11px",
  },
  breadcrumbBtn: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    background: "none",
    border: "1px solid var(--border-color)",
    borderRadius: "3px",
    cursor: "pointer",
    color: "var(--text-muted)",
    padding: "2px 4px",
    marginRight: "6px",
    fontSize: "10px",
  },
  breadcrumbSep: {
    color: "var(--text-muted)",
    padding: "0 2px",
    fontSize: "10px",
  },
  breadcrumbItem: {
    display: "inline-flex",
    alignItems: "center",
    gap: "0",
  },
  breadcrumbLink: {
    background: "none",
    border: "none",
    cursor: "pointer",
    color: "var(--accent-primary)",
    fontSize: "11px",
    padding: "1px 3px",
    borderRadius: "2px",
    textDecoration: "none",
  },
};
