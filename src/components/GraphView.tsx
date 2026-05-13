import { useEffect, useRef } from "react";
import * as d3 from "d3-force";
import { useAppStore } from "../hooks/useAppStore";

interface SimNode extends d3.SimulationNodeDatum {
  id: string;
  title: string;
  outgoing: number;
  incoming: number;
  radius: number;
}

/**
 * GraphView — Force-directed knowledge graph visualization
 * Uses D3 force simulation for layout.
 * Click a node to navigate to that note.
 */
export default function GraphView() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const graph = useAppStore((s) => s.graph);
  const openNote = useAppStore((s) => s.openNote);

  useEffect(() => {
    if (!canvasRef.current || graph.nodes.length === 0) return;

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

    // Build simulation nodes
    const nodeMap = new Map<string, SimNode>();
    const nodes: SimNode[] = graph.nodes.map((n) => {
      const simNode: SimNode = {
        id: n.id,
        title: n.title,
        outgoing: n.outgoing,
        incoming: n.incoming,
        radius: 6 + Math.min(n.incoming * 2, 20),
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
    let links: d3.SimulationLinkDatum<SimNode>[] = [];
    if (graph.edges.length > 0) {
      links = graph.edges
        .map((e) => {
          const source = nodeMap.get(e.from);
          const target = nodeMap.get(e.to);
          return source && target ? { source, target } : null;
        })
        .filter(Boolean) as d3.SimulationLinkDatum<SimNode>[];
    } else {
      // Demo links
      if (nodeMap.has("welcome") && nodeMap.has("architecture")) {
        links.push({ source: nodeMap.get("welcome")!, target: nodeMap.get("architecture")! });
      }
      if (nodeMap.has("architecture") && nodeMap.has("dual-track")) {
        links.push({ source: nodeMap.get("architecture")!, target: nodeMap.get("dual-track")! });
      }
      if (nodeMap.has("dual-track") && nodeMap.has("llm")) {
        links.push({ source: nodeMap.get("dual-track")!, target: nodeMap.get("llm")! });
      }
      if (nodeMap.has("welcome") && nodeMap.has("rust")) {
        links.push({ source: nodeMap.get("welcome")!, target: nodeMap.get("rust")! });
      }
    }

    // Simulation
    const sim = d3
      .forceSimulation<SimNode>(nodes)
      .force(
        "link",
        d3
          .forceLink<SimNode, d3.SimulationLinkDatum<SimNode>>(links)
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

    // Track pan/zoom state
    let transform = { x: 0, y: 0, scale: 1 };
    let isDragging = false;
    let dragStartX = 0;
    let dragStartY = 0;
    let dragStartTransform = { x: 0, y: 0 };

    const handleMouseDown = (e: MouseEvent) => {
      if (!hoveredNode) {
        isDragging = true;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        dragStartTransform = { x: transform.x, y: transform.y };
      }
    };
    const handleMouseMove = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      mouseX = e.clientX - rect.left;
      mouseY = e.clientY - rect.top;
      if (isDragging) {
        transform.x = dragStartTransform.x + (e.clientX - dragStartX);
        transform.y = dragStartTransform.y + (e.clientY - dragStartY);
      }
    };
    const handleMouseUp = () => { isDragging = false; };
    const handleWheel = (e: WheelEvent) => {
      e.preventDefault();
      const delta = e.deltaY > 0 ? 0.9 : 1.1;
      transform.scale = Math.max(0.2, Math.min(5, transform.scale * delta));
    };
    const handleClick = () => {
      if (hoveredNode && hoveredNode.id) {
        openNote(hoveredNode.id, `# ${hoveredNode.title}\n\n`);
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
      ctx.fillStyle = "#1e1e2e";
      ctx.fillRect(0, 0, w, h);

      // Apply pan/zoom transform
      ctx.save();
      ctx.translate(transform.x, transform.y);
      ctx.scale(transform.scale, transform.scale);

      // Edges
      ctx.strokeStyle = "#45475a";
      ctx.lineWidth = 1;
      ctx.beginPath();
      links.forEach((l) => {
        const s = l.source as SimNode;
        const t = l.target as SimNode;
        if (s.x == null || s.y == null || t.x == null || t.y == null) return;
        ctx.moveTo(s.x, s.y);
        ctx.lineTo(t.x, t.y);
      });
      ctx.stroke();

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

      // Nodes
      for (const node of nodes) {
        if (node.x == null || node.y == null) continue;
        const isHovered = node === hoveredNode;

        ctx.beginPath();
        ctx.arc(node.x, node.y, isHovered ? node.radius + 2 : node.radius, 0, 2 * Math.PI);
        ctx.fillStyle = isHovered ? "#cba6f7" : "#89b4fa";
        ctx.fill();

        // Border
        ctx.strokeStyle = isHovered ? "#f5c2e7" : "#1e1e2e";
        ctx.lineWidth = 2;
        ctx.stroke();

        // Label
        const fontSize = isHovered ? 13 : 10;
        ctx.font = `${fontSize}px system-ui, sans-serif`;
        ctx.fillStyle = isHovered ? "#cdd6f4" : "#a6adc8";
        ctx.textAlign = "center";
        ctx.fillText(node.title, node.x, node.y + node.radius + 16);
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
  }, [graph]);

  if (graph.nodes.length === 0) {
    // Show demo graph for development preview
  }

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <span style={styles.title}>Knowledge Graph</span>
        <span style={styles.stats}>
          {graph.nodes.length} nodes · {graph.edges.length} edges
        </span>
      </div>
      <div style={styles.canvasWrapper}>
        <canvas ref={canvasRef} style={styles.canvas} />
      </div>
      <div style={styles.legend}>
        <span>● Blue = Notes</span>
        <span>Click node to open</span>
        <span>Drag to rearrange</span>
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
    borderBottom: "1px solid #313244",
    marginBottom: "12px",
  },
  title: {
    fontSize: "14px",
    fontWeight: 600,
    color: "#cdd6f4",
  },
  stats: {
    fontSize: "11px",
    color: "#6c7086",
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
    color: "#585b70",
  },
};
