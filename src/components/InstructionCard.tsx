import { useState, useCallback } from "react";

// Instruction Card Types
export type CardType = 
  | "search"
  | "summarize"
  | "organize"
  | "connect"
  | "dream"
  | "research"
  | "custom";

export interface InstructionCard {
  id: string;
  type: CardType;
  label: string;
  description: string;
  params: Record<string, any>;
  icon: string;
}

export interface ComboCard {
  id: string;
  name: string;
  cards: InstructionCard[];
  description: string;
}

// Predefined single cards
const SINGLE_CARDS: InstructionCard[] = [
  {
    id: "search",
    type: "search",
    label: "Search",
    description: "Search notes by keyword or semantic similarity",
    params: { query: "", top_k: 5 },
    icon: "🔍",
  },
  {
    id: "summarize",
    type: "summarize",
    label: "Summarize",
    description: "Generate a summary of selected notes",
    params: { target: "", style: "bullet" },
    icon: "📝",
  },
  {
    id: "organize",
    type: "organize",
    label: "Organize",
    description: "Organize and structure notes",
    params: { target: "", method: "auto" },
    icon: "📂",
  },
  {
    id: "connect",
    type: "connect",
    label: "Connect",
    description: "Find and create connections between notes",
    params: { source: "", target: "" },
    icon: "🔗",
  },
  {
    id: "dream",
    type: "dream",
    label: "Dream",
    description: "Run memory consolidation (NREM/REM/Insight)",
    params: { mode: "full" },
    icon: "💤",
  },
  {
    id: "research",
    type: "research",
    label: "Research",
    description: "Deep research on a topic using AI",
    params: { topic: "", depth: "standard" },
    icon: "🔬",
  },
];

// Predefined combo cards
const COMBO_CARDS: ComboCard[] = [
  {
    id: "search-summarize",
    name: "Search & Summarize",
    cards: [
      SINGLE_CARDS.find((c) => c.id === "search")!,
      SINGLE_CARDS.find((c) => c.id === "summarize")!,
    ],
    description: "Search for notes and generate a summary",
  },
  {
    id: "research-organize",
    name: "Research & Organize",
    cards: [
      SINGLE_CARDS.find((c) => c.id === "research")!,
      SINGLE_CARDS.find((c) => c.id === "organize")!,
    ],
    description: "Research a topic and organize findings",
  },
  {
    id: "dream-connect",
    name: "Dream & Connect",
    cards: [
      SINGLE_CARDS.find((c) => c.id === "dream")!,
      SINGLE_CARDS.find((c) => c.id === "connect")!,
    ],
    description: "Run dream cycle and find connections",
  },
];

interface InstructionCardPanelProps {
  onExecute: (card: InstructionCard, params: Record<string, any>) => void;
  onExecuteCombo: (combo: ComboCard) => void;
  isTauri: boolean;
}

export default function InstructionCardPanel({
  onExecute,
  onExecuteCombo,
  isTauri: _isTauri,
}: InstructionCardPanelProps) {
  const [selectedCard, setSelectedCard] = useState<InstructionCard | null>(null);
  const [params, setParams] = useState<Record<string, any>>({});
  const [activeTab, setActiveTab] = useState<"single" | "combo">("single");

  const handleCardClick = useCallback((card: InstructionCard) => {
    setSelectedCard(card);
    setParams({ ...card.params });
  }, []);

  const handleExecute = useCallback(() => {
    if (selectedCard) {
      onExecute(selectedCard, params);
      setSelectedCard(null);
      setParams({});
    }
  }, [selectedCard, params, onExecute]);

  const handleComboClick = useCallback(
    (combo: ComboCard) => {
      onExecuteCombo(combo);
    },
    [onExecuteCombo]
  );

  return (
    <div className="instruction-card-panel">
      <div className="panel-header">
        <h3>Instruction Cards</h3>
        <div className="tab-buttons">
          <button
            className={`tab-btn ${activeTab === "single" ? "active" : ""}`}
            onClick={() => setActiveTab("single")}
          >
            Single
          </button>
          <button
            className={`tab-btn ${activeTab === "combo" ? "active" : ""}`}
            onClick={() => setActiveTab("combo")}
          >
            Combo
          </button>
        </div>
      </div>

      <div className="cards-grid">
        {activeTab === "single"
          ? SINGLE_CARDS.map((card) => (
              <div
                key={card.id}
                className={`card-item ${selectedCard?.id === card.id ? "selected" : ""}`}
                onClick={() => handleCardClick(card)}
              >
                <span className="card-icon">{card.icon}</span>
                <span className="card-label">{card.label}</span>
              </div>
            ))
          : COMBO_CARDS.map((combo) => (
              <div
                key={combo.id}
                className="card-item combo"
                onClick={() => handleComboClick(combo)}
              >
                <span className="card-icon">
                  {combo.cards.map((c) => c.icon).join("→")}
                </span>
                <span className="card-label">{combo.name}</span>
              </div>
            ))}
      </div>

      {selectedCard && (
        <div className="card-detail">
          <h4>
            {selectedCard.icon} {selectedCard.label}
          </h4>
          <p>{selectedCard.description}</p>
          <div className="params-form">
            {Object.entries(selectedCard.params).map(([key, defaultValue]) => (
              <div key={key} className="param-row">
                <label>{key}:</label>
                <input
                  type={typeof defaultValue === "number" ? "number" : "text"}
                  value={params[key] ?? defaultValue}
                  onChange={(e) =>
                    setParams((prev) => ({
                      ...prev,
                      [key]:
                        typeof defaultValue === "number"
                          ? Number(e.target.value)
                          : e.target.value,
                    }))
                  }
                />
              </div>
            ))}
          </div>
          <button className="execute-btn" onClick={handleExecute}>
            Execute
          </button>
        </div>
      )}

      <style>{`
        .instruction-card-panel {
          padding: 16px;
          background: #1e1e2e;
          border-radius: 8px;
          color: #cdd6f4;
        }
        .panel-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 16px;
        }
        .panel-header h3 {
          margin: 0;
          font-size: 16px;
        }
        .tab-buttons {
          display: flex;
          gap: 4px;
          background: #313244;
          border-radius: 6px;
          padding: 2px;
        }
        .tab-btn {
          padding: 6px 12px;
          border: none;
          background: transparent;
          color: #cdd6f4;
          cursor: pointer;
          border-radius: 4px;
          font-size: 12px;
        }
        .tab-btn.active {
          background: #45475a;
        }
        .cards-grid {
          display: grid;
          grid-template-columns: repeat(3, 1fr);
          gap: 8px;
          margin-bottom: 16px;
        }
        .card-item {
          display: flex;
          flex-direction: column;
          align-items: center;
          padding: 12px;
          background: #313244;
          border-radius: 8px;
          cursor: pointer;
          transition: all 0.2s;
          border: 2px solid transparent;
        }
        .card-item:hover {
          background: #45475a;
        }
        .card-item.selected {
          border-color: #89b4fa;
          background: #45475a;
        }
        .card-icon {
          font-size: 24px;
          margin-bottom: 4px;
        }
        .card-label {
          font-size: 12px;
          text-align: center;
        }
        .card-detail {
          background: #313244;
          border-radius: 8px;
          padding: 16px;
        }
        .card-detail h4 {
          margin: 0 0 8px 0;
          font-size: 14px;
        }
        .card-detail p {
          margin: 0 0 12px 0;
          font-size: 12px;
          color: #a6adc8;
        }
        .params-form {
          display: flex;
          flex-direction: column;
          gap: 8px;
          margin-bottom: 12px;
        }
        .param-row {
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .param-row label {
          font-size: 12px;
          min-width: 80px;
          color: #a6adc8;
        }
        .param-row input {
          flex: 1;
          padding: 6px 8px;
          background: #45475a;
          border: 1px solid #585b70;
          border-radius: 4px;
          color: #cdd6f4;
          font-size: 12px;
        }
        .execute-btn {
          width: 100%;
          padding: 8px;
          background: #89b4fa;
          color: #1e1e2e;
          border: none;
          border-radius: 6px;
          cursor: pointer;
          font-weight: 600;
          font-size: 13px;
        }
        .execute-btn:hover {
          background: #74c7ec;
        }
      `}</style>
    </div>
  );
}
