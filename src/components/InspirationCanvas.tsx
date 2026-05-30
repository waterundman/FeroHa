import { useEffect, useRef, useState, useCallback } from "react";
import { useAppStore, type NoteMeta } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";
import type { CanvasCard, CanvasEdge, CanvasState } from "../types/canvas";

const CARD_WIDTH = 240;
const CARD_HEIGHT = 160;

const COLORS: Record<string, string> = {
  red: '#ef4444',
  orange: '#f97316',
  yellow: '#eab308',
  green: '#22c55e',
  blue: '#3b82f6',
  purple: '#a855f7',
  gray: '#6b7280',
};

const COLOR_KEYS = ['red', 'orange', 'yellow', 'green', 'blue', 'purple', 'gray'] as const;

type Mode = 'select' | 'connect' | 'delete';

interface ConnectDrag {
  fromCardId: string;
  fromSide: 'top' | 'right' | 'bottom' | 'left';
  mouseX: number;
  mouseY: number;
}

interface ContextMenu {
  x: number;
  y: number;
  type: 'card' | 'edge';
  targetId: string;
}

interface DragState {
  cardId: string;
  startMouseX: number;
  startMouseY: number;
  startCardX: number;
  startCardY: number;
  isDragging: boolean;
}

function loadCanvasState(vaultPath: string | null): { cards: CanvasCard[]; edges: CanvasEdge[]; viewport: { x: number; y: number; zoom: number } } {
  const key = vaultPath ? `feroha-canvas-${vaultPath}` : 'feroha-canvas-default';
  try {
    const raw = localStorage.getItem(key);
    if (raw) {
      const data = JSON.parse(raw) as CanvasState;
      return {
        cards: data.cards || [],
        edges: data.edges || [],
        viewport: data.viewport || { x: 0, y: 0, zoom: 1 },
      };
    }
  } catch {
    // ignore parse errors
  }
  return { cards: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };
}

function useCanvasPersistence(
  storageKey: string,
  cards: CanvasCard[],
  edges: CanvasEdge[],
  viewport: { x: number; y: number; zoom: number },
) {
  const vaultPath = useAppStore((s) => s.vaultPath);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isInitialMount = useRef(true);

  useEffect(() => {
    if (isInitialMount.current) {
      isInitialMount.current = false;
      return;
    }
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(() => {
      if (!vaultPath) return;
      const state: CanvasState = { cards, edges, viewport };
      try {
        localStorage.setItem(storageKey, JSON.stringify(state));
      } catch {
        // ignore storage errors
      }
    }, 1000);
    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    };
  }, [cards, edges, viewport, storageKey, vaultPath]);
}

function generateId(): string {
  return crypto.randomUUID();
}

function getSidePoint(
  card: CanvasCard,
  side: 'top' | 'right' | 'bottom' | 'left',
): { x: number; y: number } {
  switch (side) {
    case 'top': return { x: card.x + card.width / 2, y: card.y };
    case 'bottom': return { x: card.x + card.width / 2, y: card.y + card.height };
    case 'left': return { x: card.x, y: card.y + card.height / 2 };
    case 'right': return { x: card.x + card.width, y: card.y + card.height / 2 };
  }
}

function closestSide(
  card: CanvasCard,
  px: number,
  py: number,
): 'top' | 'right' | 'bottom' | 'left' {
  const cx = card.x + card.width / 2;
  const cy = card.y + card.height / 2;
  const dx = px - cx;
  const dy = py - cy;
  const absDx = Math.abs(dx) / (card.width / 2);
  const absDy = Math.abs(dy) / (card.height / 2);
  if (absDx > absDy) return dx > 0 ? 'right' : 'left';
  return dy > 0 ? 'bottom' : 'top';
}

function hitTestCard(
  card: CanvasCard,
  px: number,
  py: number,
): boolean {
  return px >= card.x && px <= card.x + card.width && py >= card.y && py <= card.y + card.height;
}

function hitTestHandle(
  card: CanvasCard,
  side: 'top' | 'right' | 'bottom' | 'left',
  px: number,
  py: number,
): boolean {
  const pt = getSidePoint(card, side);
  const dist = Math.sqrt((px - pt.x) ** 2 + (py - pt.y) ** 2);
  return dist < 8;
}

function getCSSVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

export default function InspirationCanvas() {
  const vaultPath = useAppStore((s) => s.vaultPath);
  const notes = useAppStore((s) => s.notes);
  const openNote = useAppStore((s) => s.openNote);
  const setActivePanel = useAppStore((s) => s.setActivePanel);

  const storageKey = vaultPath ? `feroha-canvas-${vaultPath}` : 'feroha-canvas-default';

  const [cards, setCards] = useState<CanvasCard[]>(() => loadCanvasState(useAppStore.getState().vaultPath).cards);
  const [edges, setEdges] = useState<CanvasEdge[]>(() => loadCanvasState(useAppStore.getState().vaultPath).edges);
  const [viewport, setViewport] = useState(() => loadCanvasState(useAppStore.getState().vaultPath).viewport);
  const [selectedCardId, setSelectedCardId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>('select');
  const [connectDrag, setConnectDrag] = useState<ConnectDrag | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenu | null>(null);
  const [showColorSub, setShowColorSub] = useState(false);
  const [showNoteList, setShowNoteList] = useState(false);
  const [noteSearch, setNoteSearch] = useState('');
  const [hoveredCardId, setHoveredCardId] = useState<string | null>(null);
  const [hoveredSide, setHoveredSide] = useState<'top' | 'right' | 'bottom' | 'left' | null>(null);

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<DragState | null>(null);
  const panRef = useRef<{ startX: number; startY: number; startVpX: number; startVpY: number } | null>(null);

  const selectedCard = selectedCardId ? cards.find(c => c.id === selectedCardId) ?? null : null;
  const selectedEdge = selectedEdgeId ? edges.find(e => e.id === selectedEdgeId) ?? null : null;

  useCanvasPersistence(storageKey, cards, edges, viewport);

  // --- Canvas Rendering ---
  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio;
    const rect = container.getBoundingClientRect();
    const w = rect.width;
    const h = rect.height;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const bgPrimary = getCSSVar('--bg-primary');
    const textPrimary = getCSSVar('--text-primary');
    const textMuted = getCSSVar('--text-muted');
    const borderColor = getCSSVar('--border-color');
    const accentPrimary = getCSSVar('--accent-primary');
    const bgSecondary = getCSSVar('--bg-secondary');
    const bgInput = getCSSVar('--bg-input');

    // Background
    ctx.fillStyle = bgPrimary;
    ctx.fillRect(0, 0, w, h);

    // Dot grid
    const dotColor = textMuted;
    ctx.fillStyle = dotColor;
    const dotSize = 1.5 * viewport.zoom;
    if (dotSize >= 0.5) {
      const gridSpacing = 20;
      const left = -viewport.x / viewport.zoom;
      const top = -viewport.y / viewport.zoom;
      const right = left + w / viewport.zoom;
      const bottom = top + h / viewport.zoom;
      const startX = Math.floor(left / gridSpacing) * gridSpacing;
      const startY = Math.floor(top / gridSpacing) * gridSpacing;
      ctx.globalAlpha = 0.15;
      for (let gx = startX; gx <= right; gx += gridSpacing) {
        for (let gy = startY; gy <= bottom; gy += gridSpacing) {
          const sx = viewport.x + gx * viewport.zoom;
          const sy = viewport.y + gy * viewport.zoom;
          ctx.beginPath();
          ctx.arc(sx, sy, dotSize, 0, Math.PI * 2);
          ctx.fill();
        }
      }
      ctx.globalAlpha = 1;
    }

    // Apply viewport transform
    ctx.save();
    ctx.translate(viewport.x, viewport.y);
    ctx.scale(viewport.zoom, viewport.zoom);

    // Draw edges
    for (const edge of edges) {
      const fromCard = cards.find(c => c.id === edge.fromCardId);
      const toCard = cards.find(c => c.id === edge.toCardId);
      if (!fromCard || !toCard) continue;

      const fromPt = getSidePoint(fromCard, edge.fromSide);
      const toPt = getSidePoint(toCard, edge.toSide);

      const edgeColor = COLORS[edge.color] || COLORS.gray;
      const isSelected = selectedEdgeId === edge.id;

      ctx.strokeStyle = isSelected ? accentPrimary : edgeColor;
      ctx.lineWidth = isSelected ? 3 : 2;
      ctx.globalAlpha = isSelected ? 1 : 0.8;
      ctx.setLineDash([]);
      ctx.beginPath();

      const cpDist = 80;
      const cp1 = getControlPoint(fromPt, edge.fromSide, cpDist);
      const cp2 = getControlPoint(toPt, edge.toSide, cpDist);

      ctx.moveTo(fromPt.x, fromPt.y);
      ctx.bezierCurveTo(cp1.x, cp1.y, cp2.x, cp2.y, toPt.x, toPt.y);
      ctx.stroke();
      ctx.globalAlpha = 1;

      // Label
      if (edge.label) {
        const mx = (fromPt.x + toPt.x) / 2 + (cp1.x + cp2.x) / 4;
        const my = (fromPt.y + toPt.y) / 2 + (cp1.y + cp2.y) / 4;
        ctx.font = '11px system-ui, sans-serif';
        ctx.fillStyle = textPrimary;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(edge.label, mx, my);
      }
    }

    // Draw connect-mode drag line
    if (connectDrag) {
      const fromCard = cards.find(c => c.id === connectDrag.fromCardId);
      if (fromCard) {
        const fromPt = getSidePoint(fromCard, connectDrag.fromSide);
        const mx = (connectDrag.mouseX - viewport.x) / viewport.zoom;
        const my = (connectDrag.mouseY - viewport.y) / viewport.zoom;
        ctx.strokeStyle = accentPrimary;
        ctx.lineWidth = 2;
        ctx.globalAlpha = 0.6;
        ctx.setLineDash([6, 4]);
        ctx.beginPath();
        const cp = getControlPoint(fromPt, connectDrag.fromSide, 60);
        ctx.moveTo(fromPt.x, fromPt.y);
        ctx.bezierCurveTo(cp.x, cp.y, mx, my, mx, my);
        ctx.stroke();
        ctx.setLineDash([]);
        ctx.globalAlpha = 1;
      }
    }

    // Draw cards
    for (const card of cards) {
      const isSelected = selectedCardId === card.id;
      const isHovered = hoveredCardId === card.id;

      // Card shadow / glow
      if (isSelected || isHovered) {
        ctx.shadowColor = accentPrimary;
        ctx.shadowBlur = isSelected ? 12 : 6;
      }

      // Card body
      const radius = 6;
      ctx.beginPath();
      roundRect(ctx, card.x, card.y, card.width, card.height, radius);
      ctx.fillStyle = bgSecondary;
      ctx.fill();

      // Border
      ctx.strokeStyle = isSelected ? accentPrimary : borderColor;
      ctx.lineWidth = isSelected ? 2 : 1;
      ctx.stroke();
      ctx.shadowColor = 'transparent';
      ctx.shadowBlur = 0;

      // Color strip on top
      const stripColor = COLORS[card.color] || COLORS.gray;
      ctx.fillStyle = stripColor;
      ctx.beginPath();
      ctx.moveTo(card.x + radius, card.y);
      ctx.lineTo(card.x + card.width - radius, card.y);
      ctx.arcTo(card.x + card.width, card.y, card.x + card.width, card.y + radius, radius);
      ctx.lineTo(card.x + card.width, card.y + 4);
      ctx.lineTo(card.x, card.y + 4);
      ctx.lineTo(card.x, card.y + radius);
      ctx.arcTo(card.x, card.y, card.x + radius, card.y, radius);
      ctx.fill();

      // Title
      ctx.font = 'bold 13px system-ui, sans-serif';
      ctx.fillStyle = textPrimary;
      ctx.textAlign = 'left';
      ctx.textBaseline = 'top';
      clipText(ctx, card.title, card.x + 10, card.y + 10, card.width - 20);

      // Path subtitle (if vault note)
      if (card.notePath) {
        ctx.font = '9px system-ui, sans-serif';
        ctx.fillStyle = textMuted;
        clipText(ctx, card.notePath, card.x + 10, card.y + 30, card.width - 20);
      }

      // Preview text
      if (card.preview) {
        ctx.font = '11px system-ui, sans-serif';
        ctx.fillStyle = textPrimary;
        ctx.globalAlpha = 0.7;
        const previewY = card.notePath ? card.y + 44 : card.y + 30;
        wrapText(ctx, card.preview, card.x + 10, previewY, card.width - 20, 15, 3);
        ctx.globalAlpha = 1;
      }

      // Connection handles on hover
      if (isHovered && mode === 'select') {
        const sides: Array<'top' | 'right' | 'bottom' | 'left'> = ['top', 'right', 'bottom', 'left'];
        for (const side of sides) {
          const pt = getSidePoint(card, side);
          ctx.beginPath();
          ctx.arc(pt.x, pt.y, 5, 0, Math.PI * 2);
          ctx.fillStyle = bgInput;
          ctx.fill();
          ctx.strokeStyle = accentPrimary;
          ctx.lineWidth = 1.5;
          ctx.stroke();
        }
      }
    }

    ctx.restore();
  }, [cards, edges, viewport, selectedCardId, selectedEdgeId, mode, connectDrag, hoveredCardId]);

  // --- Event Handlers ---
  const screenToWorld = useCallback(
    (sx: number, sy: number): { x: number; y: number } => {
      return {
        x: (sx - viewport.x) / viewport.zoom,
        y: (sy - viewport.y) / viewport.zoom,
      };
    },
    [viewport],
  );

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      setContextMenu(null);
      setShowColorSub(false);

      const container = containerRef.current;
      if (!container) return;
      const rect = container.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const world = screenToWorld(mx, my);

      // Check handles first (in connect mode or select mode)
      if (mode === 'connect' || (mode === 'select' && hoveredCardId && hoveredSide)) {
        if (hoveredCardId && hoveredSide) {
          // If connect drag is ongoing and clicking handle of another card, finish connection
          if (connectDrag && hoveredCardId !== connectDrag.fromCardId) {
            const destSide = hoveredSide;
            const newEdge: CanvasEdge = {
              id: generateId(),
              fromCardId: connectDrag.fromCardId,
              toCardId: hoveredCardId,
              fromSide: connectDrag.fromSide,
              toSide: destSide,
              label: '',
              color: 'gray',
            };
            setEdges(prev => [...prev, newEdge]);
            setConnectDrag(null);
            setMode('select');
            return;
          }
          // Start connect drag from handle
          if (!connectDrag) {
            setConnectDrag({
              fromCardId: hoveredCardId,
              fromSide: hoveredSide,
              mouseX: mx,
              mouseY: my,
            });
            return;
          }
        }
      }

      // Check card hit
      for (let i = cards.length - 1; i >= 0; i--) {
        const card = cards[i];
        if (hitTestCard(card, world.x, world.y)) {
          if (mode === 'delete') {
            setCards(prev => prev.filter(c => c.id !== card.id));
            setEdges(prev => prev.filter(e => e.fromCardId !== card.id && e.toCardId !== card.id));
            if (selectedCardId === card.id) setSelectedCardId(null);
            return;
          }

          if (mode === 'connect' && connectDrag) {
            // Finish connect
            if (connectDrag.fromCardId !== card.id) {
              const destSide = closestSide(card, world.x, world.y);
              const newEdge: CanvasEdge = {
                id: generateId(),
                fromCardId: connectDrag.fromCardId,
                toCardId: card.id,
                fromSide: connectDrag.fromSide,
                toSide: destSide,
                label: '',
                color: 'gray',
              };
              setEdges(prev => [...prev, newEdge]);
            }
            setConnectDrag(null);
            setMode('select');
            return;
          }

          // Select mode: begin drag
          setSelectedCardId(card.id);
          setSelectedEdgeId(null);
          dragRef.current = {
            cardId: card.id,
            startMouseX: e.clientX,
            startMouseY: e.clientY,
            startCardX: card.x,
            startCardY: card.y,
            isDragging: false,
          };
          return;
        }
      }

      // Check edge hit (click near edge midpoint)
      if ((mode === 'select' && !connectDrag) || mode === 'delete') {
        for (const edge of edges) {
          const fromCard = cards.find(c => c.id === edge.fromCardId);
          const toCard = cards.find(c => c.id === edge.toCardId);
          if (!fromCard || !toCard) continue;
          const fromPt = getSidePoint(fromCard, edge.fromSide);
          const toPt = getSidePoint(toCard, edge.toSide);
          const cp1 = getControlPoint(fromPt, edge.fromSide, 80);
          const cp2 = getControlPoint(toPt, edge.toSide, 80);
          const mid = bezierMidpoint(fromPt, cp1, cp2, toPt);
          const dist = Math.sqrt((world.x - mid.x) ** 2 + (world.y - mid.y) ** 2);
          if (dist < 12) {
            if (mode === 'delete') {
              setEdges(prev => prev.filter(e => e.id !== edge.id));
              if (selectedEdgeId === edge.id) setSelectedEdgeId(null);
              return;
            }
            setSelectedEdgeId(edge.id);
            setSelectedCardId(null);
            return;
          }
        }
      }

      // Click on empty space
      if (mode === 'connect' && connectDrag) {
        setConnectDrag(null);
        return;
      }

      // Start panning
      setSelectedCardId(null);
      setSelectedEdgeId(null);
      panRef.current = {
        startX: e.clientX,
        startY: e.clientY,
        startVpX: viewport.x,
        startVpY: viewport.y,
      };
    },
    [cards, edges, viewport, mode, connectDrag, hoveredCardId, hoveredSide, selectedCardId, selectedEdgeId, screenToWorld],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      const container = containerRef.current;
      const canvas = canvasRef.current;
      if (!container || !canvas) return;
      const rect = container.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;

      // Connect drag
      if (connectDrag) {
        setConnectDrag(prev => prev ? { ...prev, mouseX: mx, mouseY: my } : null);
        canvas.style.cursor = 'crosshair';
        return;
      }

      // Card drag
      if (dragRef.current) {
        const drag = dragRef.current;
        const dx = (e.clientX - drag.startMouseX) / viewport.zoom;
        const dy = (e.clientY - drag.startMouseY) / viewport.zoom;
        if (Math.abs(dx) > 1 || Math.abs(dy) > 1) {
          drag.isDragging = true;
        }
        if (drag.isDragging) {
          setCards(prev =>
            prev.map(c =>
              c.id === drag.cardId
                ? { ...c, x: drag.startCardX + dx, y: drag.startCardY + dy }
                : c,
            ),
          );
        }
        canvas.style.cursor = drag.isDragging ? 'grabbing' : 'default';
        return;
      }

      // Pan
      if (panRef.current) {
        const pan = panRef.current;
        setViewport(prev => ({
          ...prev,
          x: pan.startVpX + (e.clientX - pan.startX),
          y: pan.startVpY + (e.clientY - pan.startY),
        }));
        canvas.style.cursor = 'grabbing';
        return;
      }

      // Hover detection
      const world = screenToWorld(mx, my);
      let foundCard: CanvasCard | null = null;
      for (let i = cards.length - 1; i >= 0; i--) {
        if (hitTestCard(cards[i], world.x, world.y)) {
          foundCard = cards[i];
          break;
        }
      }

      if (foundCard) {
        setHoveredCardId(foundCard.id);
        const sides: Array<'top' | 'right' | 'bottom' | 'left'> = ['top', 'right', 'bottom', 'left'];
        let foundSide: 'top' | 'right' | 'bottom' | 'left' | null = null;
        for (const side of sides) {
          if (hitTestHandle(foundCard, side, world.x, world.y)) {
            foundSide = side;
            break;
          }
        }
        setHoveredSide(foundSide);
        canvas.style.cursor = foundSide ? 'pointer' : 'grab';
      } else {
        setHoveredCardId(null);
        setHoveredSide(null);
        canvas.style.cursor = 'default';
      }
    },
    [cards, viewport, connectDrag, screenToWorld],
  );

  const handleMouseUp = useCallback(() => {
    if (dragRef.current) {
      dragRef.current = null;
    }
    if (panRef.current) {
      panRef.current = null;
    }
    const canvas = canvasRef.current;
    if (canvas) {
      canvas.style.cursor = 'default';
    }
  }, []);

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();
      const container = containerRef.current;
      if (!container) return;
      const rect = container.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const world = screenToWorld(mx, my);

      const delta = e.deltaY > 0 ? 0.9 : 1.1;
      const newZoom = Math.max(0.3, Math.min(3.0, viewport.zoom * delta));
      const newX = mx - world.x * newZoom;
      const newY = my - world.y * newZoom;
      setViewport({ x: newX, y: newY, zoom: newZoom });
    },
    [viewport, screenToWorld],
  );

  const handleDoubleClick = useCallback(
    async (e: React.MouseEvent) => {
      const container = containerRef.current;
      if (!container) return;
      const rect = container.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const world = screenToWorld(mx, my);

      for (let i = cards.length - 1; i >= 0; i--) {
        const card = cards[i];
        if (hitTestCard(card, world.x, world.y)) {
          if (card.notePath) {
            const name = card.notePath;
            setActivePanel('editor');
            const { invoke } = await import("@tauri-apps/api/core");
            try {
              const content = await invoke<string>("read_note", { path: name });
              openNote(name, content);
            } catch {
              openNote(name, `# ${card.title}\n\n${card.preview}`);
            }
          }
          return;
        }
      }
    },
    [cards, screenToWorld, openNote, setActivePanel],
  );

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const container = containerRef.current;
      if (!container) return;
      const rect = container.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const world = screenToWorld(mx, my);

      // Check card hit
      for (let i = cards.length - 1; i >= 0; i--) {
        if (hitTestCard(cards[i], world.x, world.y)) {
          setContextMenu({
            x: e.clientX - rect.left,
            y: e.clientY - rect.top,
            type: 'card',
            targetId: cards[i].id,
          });
          setSelectedCardId(cards[i].id);
          setSelectedEdgeId(null);
          return;
        }
      }

      // Check edge hit
      for (const edge of edges) {
        const fromCard = cards.find(c => c.id === edge.fromCardId);
        const toCard = cards.find(c => c.id === edge.toCardId);
        if (!fromCard || !toCard) continue;
        const fromPt = getSidePoint(fromCard, edge.fromSide);
        const toPt = getSidePoint(toCard, edge.toSide);
        const cp1 = getControlPoint(fromPt, edge.fromSide, 80);
        const cp2 = getControlPoint(toPt, edge.toSide, 80);
        const mid = bezierMidpoint(fromPt, cp1, cp2, toPt);
        const dist = Math.sqrt((world.x - mid.x) ** 2 + (world.y - mid.y) ** 2);
        if (dist < 12) {
          setContextMenu({
            x: e.clientX - rect.left,
            y: e.clientY - rect.top,
            type: 'edge',
            targetId: edge.id,
          });
          setSelectedEdgeId(edge.id);
          setSelectedCardId(null);
          return;
        }
      }

      setContextMenu(null);
    },
    [cards, edges, screenToWorld],
  );

  // --- Actions ---
  const addCard = useCallback(
    (notePath: string | null, title: string, preview: string) => {
      const centerX = -viewport.x / viewport.zoom + 400 / viewport.zoom;
      const centerY = -viewport.y / viewport.zoom + 300 / viewport.zoom;
      const newCard: CanvasCard = {
        id: generateId(),
        notePath,
        title: title || 'Untitled',
        preview,
        x: centerX + (Math.random() - 0.5) * 100,
        y: centerY + (Math.random() - 0.5) * 100,
        width: CARD_WIDTH,
        height: CARD_HEIGHT,
        color: 'blue',
      };
      setCards(prev => [...prev, newCard]);
    },
    [viewport],
  );

  const addNoteCard = useCallback(
    async (note: NoteMeta) => {
      let preview = '';
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const content = await invoke<string>("read_note", { path: note.path });
        preview = content.replace(/^#.*$/gm, '').trim().slice(0, 150);
      } catch {
        preview = '';
      }
      addCard(note.path, note.title, preview);
      setShowNoteList(false);
      setNoteSearch('');
    },
    [addCard],
  );

  const addStickyNote = useCallback(() => {
    addCard(null, 'Sticky Note', '');
    setShowNoteList(false);
    setNoteSearch('');
  }, [addCard]);

  const deleteCard = useCallback(
    (cardId: string) => {
      setCards(prev => prev.filter(c => c.id !== cardId));
      setEdges(prev => prev.filter(e => e.fromCardId !== cardId && e.toCardId !== cardId));
      if (selectedCardId === cardId) setSelectedCardId(null);
      setContextMenu(null);
    },
    [selectedCardId],
  );

  const deleteEdge = useCallback(
    (edgeId: string) => {
      setEdges(prev => prev.filter(e => e.id !== edgeId));
      if (selectedEdgeId === edgeId) setSelectedEdgeId(null);
      setContextMenu(null);
    },
    [selectedEdgeId],
  );

  const changeCardColor = useCallback(
    (cardId: string, color: string) => {
      setCards(prev => prev.map(c => (c.id === cardId ? { ...c, color } : c)));
      setContextMenu(null);
      setShowColorSub(false);
    },
    [],
  );

  const changeEdgeColor = useCallback(
    (edgeId: string, color: string) => {
      setEdges(prev => prev.map(e => (e.id === edgeId ? { ...e, color } : e)));
      setContextMenu(null);
      setShowColorSub(false);
    },
    [],
  );

  const handleExport = useCallback(async () => {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const vaultCards = cards.filter(c => c.notePath);
    const noteLines = vaultCards.length > 0
      ? vaultCards.map(c => `- [[${c.title}]]`).join('\n')
      : '(none)';
    const edgeLines = edges.length > 0
      ? edges
          .map(e => {
            const from = cards.find(c => c.id === e.fromCardId);
            const to = cards.find(c => c.id === e.toCardId);
            if (!from || !to) return '';
            const label = e.label ? ` (${e.label})` : '';
            return `- [[${from.title}]] --> [[${to.title}]]${label}`;
          })
          .filter(Boolean)
          .join('\n')
      : '(none)';

    const markdown = `# Canvas Export — ${new Date().toLocaleString()}\n\n## Notes\n${noteLines}\n\n## Connections\n${edgeLines}\n`;

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_note", { path: `canvas-export-${timestamp}.md`, content: markdown });
    } catch (e) {
      console.error("Export failed:", e);
    }
  }, [cards, edges]);

  const handleFit = useCallback(() => {
    if (cards.length === 0) {
      setViewport({ x: 0, y: 0, zoom: 1 });
      return;
    }
    const container = containerRef.current;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const padding = 60;
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const c of cards) {
      if (c.x < minX) minX = c.x;
      if (c.y < minY) minY = c.y;
      if (c.x + c.width > maxX) maxX = c.x + c.width;
      if (c.y + c.height > maxY) maxY = c.y + c.height;
    }
    const contentW = maxX - minX + padding * 2;
    const contentH = maxY - minY + padding * 2;
    const zoomW = rect.width / contentW;
    const zoomH = rect.height / contentH;
    const zoom = Math.min(zoomW, zoomH, 2);
    const centerX = (minX + maxX) / 2;
    const centerY = (minY + maxY) / 2;
    setViewport({
      x: rect.width / 2 - centerX * zoom,
      y: rect.height / 2 - centerY * zoom,
      zoom,
    });
  }, [cards]);

  const filteredNotes = notes.filter(
    n =>
      !noteSearch ||
      n.title.toLowerCase().includes(noteSearch.toLowerCase()) ||
      n.path.toLowerCase().includes(noteSearch.toLowerCase()),
  );

  const containerStyle: React.CSSProperties = {
    position: 'relative',
    width: '100%',
    height: '100%',
    overflow: 'hidden',
    backgroundColor: 'var(--bg-primary)',
  };

  const toolbarStyle: React.CSSProperties = {
    position: 'absolute',
    left: 12,
    top: 12,
    display: 'flex',
    flexDirection: 'column',
    gap: 4,
    zIndex: 10,
  };

  const toolBtnBase: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: 34,
    height: 34,
    backgroundColor: 'var(--bg-secondary)',
    border: '1px solid var(--border-color)',
    borderRadius: 6,
    cursor: 'pointer',
    color: 'var(--text-muted)',
    transition: 'all 0.15s',
    outline: 'none',
  };

  const toolBtnActive: React.CSSProperties = {
    ...toolBtnBase,
    backgroundColor: 'var(--bg-input)',
    borderColor: 'var(--accent-primary)',
    color: 'var(--accent-primary)',
  };

  return (
    <div ref={containerRef} style={containerStyle}>
      <canvas
        ref={canvasRef}
        style={{ display: 'block', width: '100%', height: '100%', cursor: 'default' }}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onWheel={handleWheel}
        onDoubleClick={handleDoubleClick}
        onContextMenu={handleContextMenu}
      />

      {/* Floating Toolbar */}
      <div style={toolbarStyle}>
        <button
          style={toolBtnBase}
          onClick={() => setShowNoteList(v => !v)}
          title="Add Note"
          aria-label="Add Note"
        >
          <FeroHaIcon name="Plus" size={16} />
        </button>
        <button
          style={mode === 'connect' ? toolBtnActive : toolBtnBase}
          onClick={() => {
            setMode(m => (m === 'connect' ? 'select' : 'connect'));
            setConnectDrag(null);
          }}
          title="Connect Mode"
          aria-label="Connect Mode"
          aria-pressed={mode === 'connect'}
        >
          <FeroHaIcon name="Link" size={16} />
        </button>
        <button
          style={mode === 'delete' ? toolBtnActive : toolBtnBase}
          onClick={() => setMode(m => (m === 'delete' ? 'select' : 'delete'))}
          title="Delete Mode"
          aria-label="Delete Mode"
          aria-pressed={mode === 'delete'}
        >
          <FeroHaIcon name="Trash2" size={16} />
        </button>
        <button
          style={toolBtnBase}
          onClick={handleExport}
          title="Export to Markdown"
          aria-label="Export"
        >
          <FeroHaIcon name="FileDown" size={16} />
        </button>
        <button
          style={toolBtnBase}
          onClick={handleFit}
          title="Fit All Cards"
          aria-label="Fit"
        >
          <FeroHaIcon name="Maximize" size={16} />
        </button>
      </div>

      {/* Mode indicator */}
      {mode !== 'select' && (
        <div
          style={{
            position: 'absolute',
            left: 54,
            top: 12,
            padding: '4px 10px',
            backgroundColor: 'var(--bg-secondary)',
            border: '1px solid var(--accent-primary)',
            borderRadius: 4,
            fontSize: 11,
            color: 'var(--accent-primary)',
            zIndex: 10,
            pointerEvents: 'none',
          }}
        >
          {mode === 'connect' ? 'Connect Mode — drag from handle to card' : 'Delete Mode — click cards/edges to delete'}
        </div>
      )}

      {/* Context Menu */}
      {contextMenu && (
        <div
          style={{
            position: 'absolute',
            left: contextMenu.x,
            top: contextMenu.y,
            backgroundColor: 'var(--bg-secondary)',
            border: '1px solid var(--border-color)',
            borderRadius: 6,
            padding: '4px 0',
            minWidth: 140,
            zIndex: 100,
            boxShadow: '0 4px 16px rgba(0,0,0,0.3)',
          }}
          onClick={() => setContextMenu(null)}
        >
          {contextMenu.type === 'card' && (
            <>
              <ContextMenuItem
                label="Change Color"
                onClick={(e) => { e.stopPropagation(); setShowColorSub(v => !v); }}
              />
              {showColorSub && (
                <div style={{ display: 'flex', gap: 4, padding: '4px 8px', flexWrap: 'wrap', maxWidth: 180 }}>
                  {COLOR_KEYS.map(colorKey => (
                    <button
                      key={colorKey}
                      onClick={(e) => { e.stopPropagation(); changeCardColor(contextMenu.targetId, colorKey); }}
                      style={{
                        width: 20,
                        height: 20,
                        borderRadius: '50%',
                        backgroundColor: COLORS[colorKey],
                        border: selectedCard?.color === colorKey ? '2px solid var(--text-primary)' : '2px solid transparent',
                        cursor: 'pointer',
                      }}
                      title={colorKey}
                    />
                  ))}
                </div>
              )}
              <ContextMenuItem
                label="Copy Note Link"
                onClick={(e) => {
                  e.stopPropagation();
                  const card = cards.find(c => c.id === contextMenu.targetId);
                  if (card?.notePath) {
                    navigator.clipboard.writeText(`[[${card.title}]]`).catch(() => {});
                  }
                  setContextMenu(null);
                }}
              />
              <div style={{ height: 1, backgroundColor: 'var(--border-color)', margin: '4px 0' }} />
              <ContextMenuItem
                label="Delete Card"
                onClick={(e) => { e.stopPropagation(); deleteCard(contextMenu.targetId); }}
                danger
              />
            </>
          )}
          {contextMenu.type === 'edge' && (
            <>
              <ContextMenuItem
                label="Change Color"
                onClick={(e) => { e.stopPropagation(); setShowColorSub(v => !v); }}
              />
              {showColorSub && (
                <div style={{ display: 'flex', gap: 4, padding: '4px 8px', flexWrap: 'wrap', maxWidth: 180 }}>
                  {COLOR_KEYS.map(colorKey => (
                    <button
                      key={colorKey}
                      onClick={(e) => { e.stopPropagation(); changeEdgeColor(contextMenu.targetId, colorKey); }}
                      style={{
                        width: 20,
                        height: 20,
                        borderRadius: '50%',
                        backgroundColor: COLORS[colorKey],
                        border: selectedEdge?.color === colorKey ? '2px solid var(--text-primary)' : '2px solid transparent',
                        cursor: 'pointer',
                      }}
                      title={colorKey}
                    />
                  ))}
                </div>
              )}
              <div style={{ height: 1, backgroundColor: 'var(--border-color)', margin: '4px 0' }} />
              <ContextMenuItem
                label="Delete Edge"
                onClick={(e) => { e.stopPropagation(); deleteEdge(contextMenu.targetId); }}
                danger
              />
            </>
          )}
        </div>
      )}

      {/* Note List Overlay */}
      {showNoteList && (
        <div
          style={{
            position: 'absolute',
            left: 54,
            top: 12,
            width: 260,
            maxHeight: 400,
            backgroundColor: 'var(--bg-secondary)',
            border: '1px solid var(--border-color)',
            borderRadius: 8,
            zIndex: 100,
            boxShadow: '0 4px 20px rgba(0,0,0,0.4)',
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
          }}
        >
          <div style={{ padding: '8px 10px', borderBottom: '1px solid var(--border-color)', display: 'flex', gap: 6 }}>
            <input
              type="text"
              placeholder="Search notes..."
              value={noteSearch}
              onChange={(e) => setNoteSearch(e.target.value)}
              style={{
                flex: 1,
                backgroundColor: 'var(--bg-input)',
                border: '1px solid var(--border-color)',
                borderRadius: 4,
                padding: '4px 8px',
                fontSize: 12,
                color: 'var(--text-primary)',
                outline: 'none',
              }}
              autoFocus
            />
            <button
              onClick={addStickyNote}
              style={{
                padding: '2px 8px',
                backgroundColor: 'var(--bg-input)',
                border: '1px solid var(--border-color)',
                borderRadius: 4,
                fontSize: 11,
                color: 'var(--text-muted)',
                cursor: 'pointer',
                whiteSpace: 'nowrap',
              }}
              title="Add sticky note (not linked to a file)"
            >
              Sticky
            </button>
          </div>
          <div style={{ overflow: 'auto', flex: 1, maxHeight: 330 }}>
            {filteredNotes.length === 0 && (
              <div style={{ padding: '12px 10px', fontSize: 12, color: 'var(--text-muted)' }}>
                {noteSearch ? 'No matching notes' : 'No notes in vault'}
              </div>
            )}
            {filteredNotes.map(note => (
              <div
                key={note.path}
                onClick={() => addNoteCard(note)}
                style={{
                  padding: '8px 10px',
                  cursor: 'pointer',
                  fontSize: 12,
                  color: 'var(--text-primary)',
                  borderBottom: '1px solid var(--border-muted)',
                }}
                onMouseEnter={(e) => {
                  (e.currentTarget as HTMLElement).style.backgroundColor = 'var(--bg-hover)';
                }}
                onMouseLeave={(e) => {
                  (e.currentTarget as HTMLElement).style.backgroundColor = 'transparent';
                }}
              >
                <div style={{ fontWeight: 600 }}>{note.title}</div>
                <div style={{ fontSize: 10, color: 'var(--text-muted)' }}>{note.path}</div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// --- Context menu item ---
function ContextMenuItem({
  label,
  onClick,
  danger,
}: {
  label: string;
  onClick: (e: React.MouseEvent) => void;
  danger?: boolean;
}) {
  return (
    <div
      onClick={onClick}
      style={{
        padding: '6px 12px',
        fontSize: 12,
        color: danger ? '#F38BA8' : 'var(--text-primary)',
        cursor: 'pointer',
        transition: 'background 0.1s',
      }}
      onMouseEnter={(e) => {
        (e.currentTarget as HTMLElement).style.backgroundColor = 'var(--bg-hover)';
      }}
      onMouseLeave={(e) => {
        (e.currentTarget as HTMLElement).style.backgroundColor = 'transparent';
      }}
    >
      {label}
    </div>
  );
}

// --- Canvas helper functions ---
function getControlPoint(
  pt: { x: number; y: number },
  side: 'top' | 'right' | 'bottom' | 'left',
  dist: number,
): { x: number; y: number } {
  switch (side) {
    case 'top': return { x: pt.x, y: pt.y - dist };
    case 'bottom': return { x: pt.x, y: pt.y + dist };
    case 'left': return { x: pt.x - dist, y: pt.y };
    case 'right': return { x: pt.x + dist, y: pt.y };
  }
}

function bezierMidpoint(
  p0: { x: number; y: number },
  p1: { x: number; y: number },
  p2: { x: number; y: number },
  p3: { x: number; y: number },
): { x: number; y: number } {
  const t = 0.5;
  const u = 1 - t;
  return {
    x: u * u * u * p0.x + 3 * u * u * t * p1.x + 3 * u * t * t * p2.x + t * t * t * p3.x,
    y: u * u * u * p0.y + 3 * u * u * t * p1.y + 3 * u * t * t * p2.y + t * t * t * p3.y,
  };
}

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y);
  ctx.arcTo(x + w, y, x + w, y + r, r);
  ctx.lineTo(x + w, y + h - r);
  ctx.arcTo(x + w, y + h, x + w - r, y + h, r);
  ctx.lineTo(x + r, y + h);
  ctx.arcTo(x, y + h, x, y + h - r, r);
  ctx.lineTo(x, y + r);
  ctx.arcTo(x, y, x + r, y, r);
  ctx.closePath();
}

function clipText(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  maxWidth: number,
) {
  let currentText = text;
  while (ctx.measureText(currentText).width > maxWidth && currentText.length > 0) {
    currentText = currentText.slice(0, -1);
  }
  if (currentText.length < text.length && currentText.length > 2) {
    currentText = currentText.slice(0, -2) + '..';
  }
  ctx.fillText(currentText, x, y);
}

function wrapText(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  maxWidth: number,
  lineHeight: number,
  maxLines: number,
) {
  const words = text.split(' ');
  let line = '';
  let lines = 0;
  for (const word of words) {
    const testLine = line ? line + ' ' + word : word;
    const metrics = ctx.measureText(testLine);
    if (metrics.width > maxWidth && line) {
      ctx.fillText(line, x, y + lines * lineHeight);
      lines++;
      line = word;
      if (lines >= maxLines) {
        if (lines === maxLines) {
          ctx.fillText(line + '...', x, y + (lines) * lineHeight);
        }
        return;
      }
    } else {
      line = testLine;
    }
  }
  if (line && lines < maxLines) {
    ctx.fillText(line, x, y + lines * lineHeight);
  }
}
