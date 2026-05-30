export interface OrchestratorStatus {
  active_agents: number;
  degraded_agents: string[];
  epoch_count: number;
  track_count: number;
  last_event: OrchestratorEvent | null;
  recent_events: OrchestratorEvent[];
  agent_states: AgentState[];
  track_details: TrackInfo[];
}

export interface OrchestratorEvent {
  epoch: number;
  agent_id: string;
  event_type: string;
  timestamp: number;
  detail: string | null;
}

export interface AgentState {
  agent_id: string;
  status: string;
  regression_count: number;
  last_epoch: number;
}

export interface TrackInfo {
  track_id: string;
  focus: string;
  status: string;
  parent_agent: string;
}
