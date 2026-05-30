export interface CanvasCard {
  id: string;
  notePath: string | null;
  title: string;
  preview: string;
  x: number;
  y: number;
  width: number;
  height: number;
  color: string;
}

export interface CanvasEdge {
  id: string;
  fromCardId: string;
  toCardId: string;
  fromSide: 'top' | 'right' | 'bottom' | 'left';
  toSide: 'top' | 'right' | 'bottom' | 'left';
  label: string;
  color: string;
}

export interface CanvasState {
  cards: CanvasCard[];
  edges: CanvasEdge[];
  viewport: { x: number; y: number; zoom: number };
}
