export enum ContextLayer {
  System = 'System',
  Note = 'Note',
  Session = 'Session',
  Project = 'Project',
  Transient = 'Transient',
}

export enum ContextSource {
  User = 'User',
  Note = 'Note',
  System = 'System',
  RAG = 'RAG',
  Agent = 'Agent',
  Pipeline = 'Pipeline',
}

export interface ContextFragment {
  id: string;
  key: string;
  value: unknown;
  source: ContextSource;
  layer: ContextLayer;
  created_at: number;
  ttl: number | null;
  hash: string;
}

export interface ContextRef {
  key: string;
  layer: ContextLayer;
  hash: string;
}
