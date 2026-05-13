// Two-Surface Interaction Protocol
// Defines the communication protocol between Human Surface and AI Surface

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Events from Human Surface to AI Surface
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum HumanToAiEvent {
    /// Note was created
    NoteCreated {
        path: String,
        content: String,
        timestamp: u64,
    },
    /// Note was modified
    NoteModified {
        path: String,
        content: String,
        old_content: Option<String>,
        timestamp: u64,
    },
    /// Note was deleted
    NoteDeleted {
        path: String,
        timestamp: u64,
    },
    /// Link was created
    LinkCreated {
        from: String,
        to: String,
        timestamp: u64,
    },
    /// Link was deleted
    LinkDeleted {
        from: String,
        to: String,
        timestamp: u64,
    },
    /// User requested AI action via instruction card
    InstructionCardExecuted {
        card_type: String,
        params: HashMap<String, String>,
        timestamp: u64,
    },
    /// User requested dream cycle
    DreamRequested {
        mode: String,
        timestamp: u64,
    },
}

/// Actions from AI Surface to Human Surface
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AiToHumanAction {
    /// Write a note to AI surface
    WriteNote {
        path: String,
        content: String,
        source: String,
        timestamp: u64,
    },
    /// Create a link between notes
    CreateLink {
        from: String,
        to: String,
        link_type: String,
        timestamp: u64,
    },
    /// Send notification to user
    Notify {
        title: String,
        message: String,
        notification_type: NotificationType,
        timestamp: u64,
    },
    /// Show suggestion to user
    Suggest {
        suggestion: String,
        context: String,
        confidence: f32,
        timestamp: u64,
    },
    /// Dream cycle completed
    DreamCompleted {
        stats: DreamStatsSummary,
        timestamp: u64,
    },
    /// Research result
    ResearchResult {
        topic: String,
        summary: String,
        sources: Vec<String>,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamStatsSummary {
    pub memories_processed: usize,
    pub connections_strengthened: usize,
    pub bridges_created: usize,
    pub communities_found: usize,
    pub duration_ms: u64,
}

/// Interaction mode between surfaces
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InteractionMode {
    /// Manual mode: only respond to explicit user actions
    Manual,
    /// Semi-auto mode: suggest actions but require user approval
    SemiAuto,
    /// Full auto mode: automatically execute actions
    FullAuto,
}

/// Two-Surface Protocol handler
pub struct TwoSurfaceProtocol {
    mode: InteractionMode,
    event_log: Vec<HumanToAiEvent>,
    action_log: Vec<AiToHumanAction>,
}

impl TwoSurfaceProtocol {
    pub fn new() -> Self {
        TwoSurfaceProtocol {
            mode: InteractionMode::Manual,
            event_log: Vec::new(),
            action_log: Vec::new(),
        }
    }

    pub fn with_mode(mode: InteractionMode) -> Self {
        TwoSurfaceProtocol {
            mode,
            event_log: Vec::new(),
            action_log: Vec::new(),
        }
    }

    /// Process an event from Human Surface
    pub fn process_event(&mut self, event: HumanToAiEvent) -> Vec<AiToHumanAction> {
        self.event_log.push(event.clone());

        match &self.mode {
            InteractionMode::Manual => {
                // Only respond to explicit instruction card executions
                match &event {
                    HumanToAiEvent::InstructionCardExecuted { .. } => {
                        self.handle_instruction_card(event)
                    }
                    HumanToAiEvent::DreamRequested { .. } => {
                        self.handle_dream_request(event)
                    }
                    _ => Vec::new(), // Ignore other events in manual mode
                }
            }
            InteractionMode::SemiAuto => {
                // Suggest actions for all events
                let mut actions = Vec::new();
                match &event {
                    HumanToAiEvent::NoteCreated { .. } => {
                        actions.push(self.suggest_analysis(&event));
                    }
                    HumanToAiEvent::NoteModified { .. } => {
                        actions.push(self.suggest_update(&event));
                    }
                    HumanToAiEvent::InstructionCardExecuted { .. } => {
                        actions.extend(self.handle_instruction_card(event));
                    }
                    HumanToAiEvent::DreamRequested { .. } => {
                        actions.extend(self.handle_dream_request(event));
                    }
                    _ => {}
                }
                actions
            }
            InteractionMode::FullAuto => {
                // Automatically execute all actions
                match &event {
                    HumanToAiEvent::NoteCreated { .. } => {
                        vec![self.auto_analyze(&event)]
                    }
                    HumanToAiEvent::NoteModified { .. } => {
                        vec![self.auto_update(&event)]
                    }
                    HumanToAiEvent::InstructionCardExecuted { .. } => {
                        self.handle_instruction_card(event)
                    }
                    HumanToAiEvent::DreamRequested { .. } => {
                        self.handle_dream_request(event)
                    }
                    _ => Vec::new(),
                }
            }
        }
    }

    /// Handle instruction card execution
    fn handle_instruction_card(&self, event: HumanToAiEvent) -> Vec<AiToHumanAction> {
        if let HumanToAiEvent::InstructionCardExecuted { card_type, params, timestamp } = event {
            match card_type.as_str() {
                "search" => {
                    let query = params.get("query").cloned().unwrap_or_default();
                    vec![AiToHumanAction::ResearchResult {
                        topic: query,
                        summary: "Search results placeholder".to_string(),
                        sources: Vec::new(),
                        timestamp,
                    }]
                }
                "summarize" => {
                    let target = params.get("target").cloned().unwrap_or_default();
                    vec![AiToHumanAction::WriteNote {
                        path: format!("ai-notes/summaries/{}.md", target),
                        content: "Summary placeholder".to_string(),
                        source: "summarize-card".to_string(),
                        timestamp,
                    }]
                }
                "dream" => {
                    vec![AiToHumanAction::DreamCompleted {
                        stats: DreamStatsSummary {
                            memories_processed: 0,
                            connections_strengthened: 0,
                            bridges_created: 0,
                            communities_found: 0,
                            duration_ms: 0,
                        },
                        timestamp,
                    }]
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }

    /// Handle dream request
    fn handle_dream_request(&self, event: HumanToAiEvent) -> Vec<AiToHumanAction> {
        if let HumanToAiEvent::DreamRequested { mode: _, timestamp } = event {
            vec![AiToHumanAction::DreamCompleted {
                stats: DreamStatsSummary {
                    memories_processed: 0,
                    connections_strengthened: 0,
                    bridges_created: 0,
                    communities_found: 0,
                    duration_ms: 0,
                },
                timestamp,
            }]
        } else {
            Vec::new()
        }
    }

    /// Suggest analysis for a new note
    fn suggest_analysis(&self, event: &HumanToAiEvent) -> AiToHumanAction {
        let timestamp = match event {
            HumanToAiEvent::NoteCreated { timestamp, .. } => *timestamp,
            _ => 0,
        };
        AiToHumanAction::Suggest {
            suggestion: "Analyze this note for connections and insights?".to_string(),
            context: "New note detected".to_string(),
            confidence: 0.8,
            timestamp,
        }
    }

    /// Suggest update for a modified note
    fn suggest_update(&self, event: &HumanToAiEvent) -> AiToHumanAction {
        let timestamp = match event {
            HumanToAiEvent::NoteModified { timestamp, .. } => *timestamp,
            _ => 0,
        };
        AiToHumanAction::Suggest {
            suggestion: "Update AI index for this note?".to_string(),
            context: "Note modified".to_string(),
            confidence: 0.7,
            timestamp,
        }
    }

    /// Auto-analyze a new note
    fn auto_analyze(&self, event: &HumanToAiEvent) -> AiToHumanAction {
        let timestamp = match event {
            HumanToAiEvent::NoteCreated { timestamp, .. } => *timestamp,
            _ => 0,
        };
        AiToHumanAction::Notify {
            title: "AI Analysis".to_string(),
            message: "Note analyzed and indexed".to_string(),
            notification_type: NotificationType::Info,
            timestamp,
        }
    }

    /// Auto-update for a modified note
    fn auto_update(&self, event: &HumanToAiEvent) -> AiToHumanAction {
        let timestamp = match event {
            HumanToAiEvent::NoteModified { timestamp, .. } => *timestamp,
            _ => 0,
        };
        AiToHumanAction::Notify {
            title: "AI Update".to_string(),
            message: "Note index updated".to_string(),
            notification_type: NotificationType::Info,
            timestamp,
        }
    }

    /// Get interaction mode
    pub fn get_mode(&self) -> &InteractionMode {
        &self.mode
    }

    /// Set interaction mode
    pub fn set_mode(&mut self, mode: InteractionMode) {
        self.mode = mode;
    }

    /// Get event log
    pub fn get_event_log(&self) -> &[HumanToAiEvent] {
        &self.event_log
    }

    /// Get action log
    pub fn get_action_log(&self) -> &[AiToHumanAction] {
        &self.action_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_creation() {
        let protocol = TwoSurfaceProtocol::new();
        assert!(matches!(protocol.get_mode(), InteractionMode::Manual));
        assert_eq!(protocol.get_event_log().len(), 0);
        assert_eq!(protocol.get_action_log().len(), 0);
    }

    #[test]
    fn test_manual_mode_ignores_note_events() {
        let mut protocol = TwoSurfaceProtocol::new();
        let event = HumanToAiEvent::NoteCreated {
            path: "test.md".to_string(),
            content: "# Test".to_string(),
            timestamp: 0,
        };
        let actions = protocol.process_event(event);
        assert_eq!(actions.len(), 0);
    }

    #[test]
    fn test_manual_mode_handles_instruction_card() {
        let mut protocol = TwoSurfaceProtocol::new();
        let mut params = HashMap::new();
        params.insert("query".to_string(), "test query".to_string());

        let event = HumanToAiEvent::InstructionCardExecuted {
            card_type: "search".to_string(),
            params,
            timestamp: 0,
        };
        let actions = protocol.process_event(event);
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn test_semi_auto_mode_suggests() {
        let mut protocol = TwoSurfaceProtocol::with_mode(InteractionMode::SemiAuto);
        let event = HumanToAiEvent::NoteCreated {
            path: "test.md".to_string(),
            content: "# Test".to_string(),
            timestamp: 0,
        };
        let actions = protocol.process_event(event);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AiToHumanAction::Suggest { .. }));
    }

    #[test]
    fn test_full_auto_mode_analyzes() {
        let mut protocol = TwoSurfaceProtocol::with_mode(InteractionMode::FullAuto);
        let event = HumanToAiEvent::NoteCreated {
            path: "test.md".to_string(),
            content: "# Test".to_string(),
            timestamp: 0,
        };
        let actions = protocol.process_event(event);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AiToHumanAction::Notify { .. }));
    }
}
