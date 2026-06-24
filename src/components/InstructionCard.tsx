import { useState, useCallback } from "react";
import FeroHaIcon from "./FeroHaIcon";

// 指令卡类型
export type CardType = 
  | "search"
  | "summarize"
  | "organize"
  | "connect"
  | "dream"
  | "research"
  | "analyze"
  | "rewrite"
  | "custom";

type InstructionParamValue = string | number | boolean | string[];
type InstructionParams = Record<string, InstructionParamValue>;

export interface InstructionCard {
  id: string;
  type: CardType;
  label: string;
  description: string;
  params: InstructionParams;
  icon: string;
}

export interface ComboCard {
  id: string;
  name: string;
  cards: InstructionCard[];
  description: string;
}

// 预置单卡
const SINGLE_CARDS: InstructionCard[] = [
  {
    id: "search",
    type: "search",
    label: "搜索",
    description: "按关键词或语义相似度搜索笔记",
    params: { query: "", top_k: 5 },
    icon: "Search",
  },
  {
    id: "summarize",
    type: "summarize",
    label: "总结",
    description: "为选中的笔记生成摘要",
    params: { target: "", style: "bullet" },
    icon: "Pencil",
  },
  {
    id: "organize",
    type: "organize",
    label: "整理",
    description: "整理并结构化笔记",
    params: { target: "", method: "auto" },
    icon: "FolderOpen",
  },
  {
    id: "connect",
    type: "connect",
    label: "连接",
    description: "寻找并创建笔记之间的联系",
    params: { source: "", target: "" },
    icon: "Link",
  },
  {
    id: "dream",
    type: "dream",
    label: "Dream",
    description: "运行记忆巩固（NREM/REM/洞察）",
    params: { mode: "full" },
    icon: "Moon",
  },
  {
    id: "research",
    type: "research",
    label: "研究",
    description: "使用 AI 对主题进行深度研究",
    params: { topic: "", depth: "standard" },
    icon: "Microscope",
  },
  {
    id: "analyze",
    type: "analyze",
    label: "分析",
    description: "分析文本结构、论证逻辑和关键词提取",
    params: { target: "", content: "" },
    icon: "FileSearch",
  },
  {
    id: "rewrite",
    type: "rewrite",
    label: "改写",
    description: "按指定风格改写文本内容",
    params: { target: "", content: "", style: "formal" },
    icon: "PenLine",
  },
];

// 预置组合卡
const COMBO_CARDS: ComboCard[] = [
  {
    id: "search-summarize",
    name: "搜索并总结",
    cards: [
      SINGLE_CARDS.find((c) => c.id === "search")!,
      SINGLE_CARDS.find((c) => c.id === "summarize")!,
    ],
    description: "搜索笔记并生成摘要",
  },
  {
    id: "research-organize",
    name: "研究并整理",
    cards: [
      SINGLE_CARDS.find((c) => c.id === "research")!,
      SINGLE_CARDS.find((c) => c.id === "organize")!,
    ],
    description: "研究主题并整理发现",
  },
  {
    id: "dream-connect",
    name: "Dream 并连接",
    cards: [
      SINGLE_CARDS.find((c) => c.id === "dream")!,
      SINGLE_CARDS.find((c) => c.id === "connect")!,
    ],
    description: "运行 Dream 循环并寻找联系",
  },
];

function toInputValue(value: InstructionParamValue) {
  if (Array.isArray(value)) return value;
  if (typeof value === "boolean") return String(value);
  return value;
}

interface InstructionCardPanelProps {
  onExecute: (card: InstructionCard, params: InstructionParams) => void;
  onExecuteCombo: (combo: ComboCard) => void;
  isTauri: boolean;
}

export default function InstructionCardPanel({
  onExecute,
  onExecuteCombo,
  isTauri: _isTauri,
}: InstructionCardPanelProps) {
  const [selectedCard, setSelectedCard] = useState<InstructionCard | null>(null);
  const [params, setParams] = useState<InstructionParams>({});
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
        <h3>指令卡</h3>
        <div className="tab-buttons">
          <button
            className={`tab-btn ${activeTab === "single" ? "active" : ""}`}
            onClick={() => setActiveTab("single")}
          >
            单卡
          </button>
          <button
            className={`tab-btn ${activeTab === "combo" ? "active" : ""}`}
            onClick={() => setActiveTab("combo")}
          >
            组合
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
                <span className="card-icon"><FeroHaIcon name={card.icon} size={16} /></span>
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
                  {combo.cards.map((c, i) => (
                    <span key={c.id}>
                      <FeroHaIcon name={c.icon} size={16} />
                      {i < combo.cards.length - 1 ? "→" : ""}
                    </span>
                  ))}
                </span>
                <span className="card-label">{combo.name}</span>
              </div>
            ))}
      </div>

      {selectedCard && (
        <div className="card-detail">
          <h4>
            <FeroHaIcon name={selectedCard.icon} size={16} /> {selectedCard.label}
          </h4>
          <p>{selectedCard.description}</p>
          <div className="params-form">
            {Object.entries(selectedCard.params).map(([key, defaultValue]) => (
              <div key={key} className="param-row">
                <label>{key}:</label>
                <input
                  type={typeof defaultValue === "number" ? "number" : "text"}
                  value={toInputValue(params[key] ?? defaultValue)}
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
            执行
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
