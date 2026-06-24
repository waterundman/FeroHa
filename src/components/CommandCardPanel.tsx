import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import CommandCard from "./CommandCard";
import FeroHaIcon from "./FeroHaIcon";
import type { LegacyCommandCardDefinition, CommandCategory, ParamValue } from "../types/command-card";
import { useCommandCardStore } from "../store/commandCardStore";
import type { CommandCardDefinition } from "../types/command-card";

type CommandParams = Record<string, ParamValue>;

function toLegacyFormat(card: CommandCardDefinition): LegacyCommandCardDefinition {
  const params: Record<string, ParamValue> = {};
  for (const p of card.params) {
    params[p.name] = (p.defaultValue ?? "") as ParamValue;
  }
  return {
    id: card.meta.id,
    type: card.meta.type,
    category: card.meta.category,
    label: card.meta.name,
    description: card.meta.description,
    icon: card.meta.icon,
    params,
    promptTemplate: card.prompt.template,
    version: card.meta.version,
    tags: card.meta.tags,
    isCustom: card.meta.isCustom,
  };
}

function toInputValue(value: ParamValue) {
  if (Array.isArray(value)) return value;
  if (typeof value === "boolean") return String(value);
  return value;
}

interface CommandCardPanelProps {
  onExecute: (card: LegacyCommandCardDefinition, params: CommandParams) => void;
  isTauri: boolean;
  isOpen: boolean;
  onClose: () => void;
}

export default function CommandCardPanel({
  onExecute,
  isTauri: _isTauri,
  isOpen,
  onClose,
}: CommandCardPanelProps) {
  const [selectedCard, setSelectedCard] = useState<LegacyCommandCardDefinition | null>(null);
  const [params, setParams] = useState<CommandParams>({});
  const [searchQuery, setSearchQuery] = useState("");
  const [activeCategory, setActiveCategory] = useState<CommandCategory | "all">("all");
  const [hoveredCard, setHoveredCard] = useState<LegacyCommandCardDefinition | null>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const customCards = useCommandCardStore((s) => s.customCards);
  const getAllCards = useCommandCardStore((s) => s.getAllCards);

  // Derive card list from registry
  const registryCards: LegacyCommandCardDefinition[] = useMemo(
    () => getAllCards().map(toLegacyFormat),
    [customCards, getAllCards]
  );

  // Filter cards based on search query and category
  const filteredCards = registryCards.filter((card) => {
    const matchesSearch =
      searchQuery === "" ||
      card.label.toLowerCase().includes(searchQuery.toLowerCase()) ||
      card.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
      card.tags.some((tag) => tag.toLowerCase().includes(searchQuery.toLowerCase()));

    const matchesCategory =
      activeCategory === "all" || card.category === activeCategory;

    return matchesSearch && matchesCategory;
  });

  // Group cards by category
  const groupedCards = filteredCards.reduce(
    (acc, card) => {
      if (!acc[card.category]) {
        acc[card.category] = [];
      }
      acc[card.category].push(card);
      return acc;
    },
    {} as Record<CommandCategory, LegacyCommandCardDefinition[]>
  );

  // Focus search input when panel opens
  useEffect(() => {
    if (isOpen && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isOpen]);

  // Handle keyboard shortcuts
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  const handleCardClick = useCallback((card: LegacyCommandCardDefinition) => {
    setSelectedCard(card);
    setParams({ ...card.params });
  }, []);

  const handleExecute = useCallback(() => {
    if (selectedCard) {
      onExecute(selectedCard, params);
      setSelectedCard(null);
      setParams({});
      setSearchQuery("");
    }
  }, [selectedCard, params, onExecute]);

  const handleParamChange = useCallback(
    (key: string, value: ParamValue) => {
      setParams((prev) => ({
        ...prev,
        [key]: value,
      }));
    },
    []
  );

  const getCategoryLabel = (category: CommandCategory): string => {
    switch (category) {
      case "content":
        return "内容操作";
      case "analysis":
        return "分析";
      case "format":
        return "格式转换";
      case "system":
        return "系统";
      default:
        return "其他";
    }
  };

  if (!isOpen) return null;

  return (
    <div className="command-card-panel">
      <div className="panel-overlay" onClick={onClose} />
      <div className="panel-content" role="dialog" aria-modal="true" aria-label="指令卡">
        <div className="panel-header">
          <h3>指令卡</h3>
          <button className="close-btn" onClick={onClose} aria-label="关闭指令卡">
            <FeroHaIcon name="X" size={18} />
          </button>
        </div>

        <div className="search-bar">
          <span className="search-icon"><FeroHaIcon name="Search" size={16} /></span>
          <input
            ref={searchInputRef}
            type="text"
            placeholder="搜索指令..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="search-input"
          />
          {searchQuery && (
            <button
              className="clear-search"
              onClick={() => setSearchQuery("")}
              aria-label="清除搜索"
            >
              <FeroHaIcon name="X" size={12} />
            </button>
          )}
        </div>

        <div className="category-tabs">
          <button
            className={`category-tab ${activeCategory === "all" ? "active" : ""}`}
            onClick={() => setActiveCategory("all")}
          >
            全部
          </button>
          {(["content", "analysis", "format", "system"] as CommandCategory[]).map(
            (category) => (
              <button
                key={category}
                className={`category-tab ${activeCategory === category ? "active" : ""}`}
                onClick={() => setActiveCategory(category)}
              >
                {getCategoryLabel(category)}
              </button>
            )
          )}
        </div>

        <div className="cards-container">
          {Object.entries(groupedCards).map(([category, cards]) => (
            <div key={category} className="category-section">
              <h4 className="category-title">
                {getCategoryLabel(category as CommandCategory)}
              </h4>
              <div className="cards-grid">
                {cards.map((card) => (
                  <CommandCard
                    key={card.id}
                    card={card}
                    isSelected={selectedCard?.id === card.id}
                    onClick={handleCardClick}
                    onHover={setHoveredCard}
                  />
                ))}
              </div>
            </div>
          ))}

          {filteredCards.length === 0 && (
            <div className="no-results">
              <span className="no-results-icon"><FeroHaIcon name="Search" size={48} /></span>
              <p>没有找到指令</p>
              <p className="no-results-hint">换一个关键词试试</p>
            </div>
          )}
        </div>

        {selectedCard && (
          <div className="card-detail-panel">
            <div className="detail-header">
              <span className="detail-icon"><FeroHaIcon name={selectedCard.icon} size={32} /></span>
              <div>
                <h4 className="detail-title">{selectedCard.label}</h4>
                <p className="detail-description">{selectedCard.description}</p>
              </div>
            </div>

            <div className="params-form">
              <h5 className="params-title">参数</h5>
              {Object.entries(selectedCard.params).map(([key, defaultValue]) => (
                <div key={key} className="param-row">
                  <label className="param-label">{key}:</label>
                  <input
                    type={typeof defaultValue === "number" ? "number" : "text"}
                    value={toInputValue(params[key] ?? defaultValue)}
                    onChange={(e) =>
                      handleParamChange(
                        key,
                        typeof defaultValue === "number"
                          ? Number(e.target.value)
                          : e.target.value
                      )
                    }
                    className="param-input"
                    placeholder={`输入 ${key}...`}
                  />
                </div>
              ))}
            </div>

            <button className="execute-btn" onClick={handleExecute}>
              执行指令
            </button>
          </div>
        )}

        {hoveredCard && !selectedCard && (
          <div className="hovered-card-info">
            <span className="hovered-label"><FeroHaIcon name={hoveredCard.icon} size={24} /></span>
            <span className="hovered-label">{hoveredCard.label}</span>
            <span className="hovered-description">{hoveredCard.description}</span>
          </div>
        )}
      </div>

      <style>{`
        .command-card-panel {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          z-index: 1000;
          display: flex;
          align-items: center;
          justify-content: center;
        }

        .panel-overlay {
          position: absolute;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background: rgba(0, 0, 0, 0.6);
          backdrop-filter: blur(4px);
        }

        .panel-content {
          position: relative;
          background: #1e1e2e;
          border-radius: 8px;
          width: 90%;
          max-width: 1000px;
          max-height: 80vh;
          display: flex;
          flex-direction: column;
          box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
          border: 1px solid #313244;
          animation: panelSlideIn 0.2s ease;
        }

        @keyframes panelSlideIn {
          from {
            opacity: 0;
            transform: translateY(20px) scale(0.95);
          }
          to {
            opacity: 1;
            transform: translateY(0) scale(1);
          }
        }

        .panel-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 16px 20px;
          border-bottom: 1px solid #313244;
        }

        .panel-header h3 {
          margin: 0;
          font-size: 18px;
          font-weight: 600;
          color: #cdd6f4;
        }

        .close-btn {
          background: transparent;
          border: none;
          color: #6c7086;
          font-size: 18px;
          cursor: pointer;
          padding: 4px 8px;
          border-radius: 4px;
          transition: all 0.15s;
        }

        .close-btn:hover {
          background: #45475a;
          color: #cdd6f4;
        }

        .search-bar {
          display: flex;
          align-items: center;
          padding: 12px 20px;
          border-bottom: 1px solid #313244;
          gap: 8px;
        }

        .search-icon {
          font-size: 16px;
          color: #6c7086;
        }

        .search-input {
          flex: 1;
          background: transparent;
          border: none;
          outline: none;
          color: #cdd6f4;
          font-size: 14px;
          font-family: inherit;
        }

        .search-input::placeholder {
          color: #6c7086;
        }

        .clear-search {
          background: transparent;
          border: none;
          color: #6c7086;
          cursor: pointer;
          padding: 4px;
          border-radius: 4px;
          font-size: 12px;
        }

        .clear-search:hover {
          background: #45475a;
          color: #cdd6f4;
        }

        .category-tabs {
          display: flex;
          gap: 4px;
          padding: 12px 20px;
          border-bottom: 1px solid #313244;
          overflow-x: auto;
        }

        .category-tab {
          background: transparent;
          border: none;
          color: #6c7086;
          padding: 6px 12px;
          border-radius: 6px;
          cursor: pointer;
          font-size: 12px;
          white-space: nowrap;
          transition: all 0.15s;
        }

        .category-tab:hover {
          background: #313244;
          color: #cdd6f4;
        }

        .category-tab.active {
          background: #45475a;
          color: #cdd6f4;
        }

        .cards-container {
          flex: 1;
          overflow-y: auto;
          padding: 16px 20px;
        }

        .category-section {
          margin-bottom: 24px;
        }

        .category-title {
          margin: 0 0 12px 0;
          font-size: 14px;
          font-weight: 600;
          color: #a6adc8;
          display: flex;
          align-items: center;
          gap: 8px;
        }

        .cards-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
          gap: 12px;
        }

        .no-results {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          padding: 40px;
          color: #6c7086;
        }

        .no-results-icon {
          font-size: 48px;
          margin-bottom: 16px;
        }

        .no-results p {
          margin: 0 0 4px 0;
          font-size: 16px;
        }

        .no-results-hint {
          font-size: 12px;
          color: #45475a;
        }

        .card-detail-panel {
          padding: 16px 20px;
          border-top: 1px solid #313244;
          background: #181825;
          border-radius: 0 0 12px 12px;
        }

        .detail-header {
          display: flex;
          align-items: flex-start;
          gap: 12px;
          margin-bottom: 16px;
        }

        .detail-icon {
          font-size: 32px;
        }

        .detail-title {
          margin: 0 0 4px 0;
          font-size: 18px;
          font-weight: 600;
          color: #cdd6f4;
        }

        .detail-description {
          margin: 0;
          font-size: 14px;
          color: #a6adc8;
        }

        .params-form {
          margin-bottom: 16px;
        }

        .params-title {
          margin: 0 0 12px 0;
          font-size: 14px;
          font-weight: 600;
          color: #a6adc8;
        }

        .param-row {
          display: flex;
          align-items: center;
          gap: 12px;
          margin-bottom: 8px;
        }

        .param-label {
          font-size: 12px;
          color: #89b4fa;
          font-family: 'JetBrains Mono', monospace;
          min-width: 100px;
        }

        .param-input {
          flex: 1;
          padding: 8px 12px;
          background: #313244;
          border: 1px solid #45475a;
          border-radius: 6px;
          color: #cdd6f4;
          font-size: 13px;
          font-family: inherit;
          transition: all 0.15s;
        }

        .param-input:focus {
          outline: none;
          border-color: #89b4fa;
          box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.2);
        }

        .param-input::placeholder {
          color: #45475a;
        }

        .execute-btn {
          width: 100%;
          padding: 12px;
          background: #89b4fa;
          color: #1e1e2e;
          border: none;
          border-radius: 8px;
          cursor: pointer;
          font-weight: 600;
          font-size: 14px;
          transition: all 0.15s;
        }

        .execute-btn:hover {
          background: #74c7ec;
          transform: translateY(-1px);
        }

        .execute-btn:active {
          transform: translateY(0);
        }

        .hovered-card-info {
          display: flex;
          align-items: center;
          gap: 12px;
          padding: 12px 20px;
          border-top: 1px solid #313244;
          background: #181825;
          border-radius: 0 0 12px 12px;
        }

        .hovered-icon {
          font-size: 24px;
        }

        .hovered-label {
          font-size: 14px;
          font-weight: 600;
          color: #cdd6f4;
        }

        .hovered-description {
          font-size: 12px;
          color: #a6adc8;
          flex: 1;
        }
      `}</style>
    </div>
  );
}
