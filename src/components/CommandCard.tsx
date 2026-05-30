import { useState, useCallback } from "react";
import FeroHaIcon from "./FeroHaIcon";
import type { LegacyCommandCardDefinition, CommandCategory } from "../types/command-card";

export interface CommandCardProps {
  card: LegacyCommandCardDefinition;
  isSelected: boolean;
  onClick: (card: LegacyCommandCardDefinition) => void;
  onHover?: (card: LegacyCommandCardDefinition | null) => void;
}

export default function CommandCard({
  card,
  isSelected,
  onClick,
  onHover,
}: CommandCardProps) {
  const [isHovered, setIsHovered] = useState(false);

  const handleClick = useCallback(() => {
    onClick(card);
  }, [card, onClick]);

  const handleMouseEnter = useCallback(() => {
    setIsHovered(true);
    onHover?.(card);
  }, [card, onHover]);

  const handleMouseLeave = useCallback(() => {
    setIsHovered(false);
    onHover?.(null);
  }, [onHover]);

  const getCategoryColor = (category: CommandCategory): string => {
    switch (category) {
      case "content":
        return "#89b4fa";
      case "analysis":
        return "#a6e3a1";
      case "format":
        return "#f9e2af";
      case "system":
        return "#f38ba8";
      default:
        return "#6c7086";
    }
  };

  const getCategoryLabel = (category: CommandCategory): string => {
    switch (category) {
      case "content":
        return "Content";
      case "analysis":
        return "Analysis";
      case "format":
        return "Format";
      case "system":
        return "System";
      default:
        return "Other";
    }
  };

  return (
    <div
      className={`command-card ${isSelected ? "selected" : ""} ${isHovered ? "hovered" : ""}`}
      onClick={handleClick}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          handleClick();
        }
      }}
      aria-label={`${card.label}: ${card.description}`}
      aria-pressed={isSelected}
    >
      <div className="card-header">
        <span className="card-icon"><FeroHaIcon name={card.icon} size={24} /></span>
        <span
          className="card-category"
          style={{ backgroundColor: getCategoryColor(card.category) }}
        >
          {getCategoryLabel(card.category)}
        </span>
      </div>

      <div className="card-body">
        <h4 className="card-title">{card.label}</h4>
        <p className="card-description">{card.description}</p>
      </div>

      <div className="card-footer">
        {card.tags.slice(0, 3).map((tag) => (
          <span key={tag} className="card-tag">
            {tag}
          </span>
        ))}
        {card.isCustom && <span className="card-custom-badge">Custom</span>}
      </div>

      {isHovered && (
        <div className="card-tooltip">
          <div className="tooltip-header">
            <span className="tooltip-icon"><FeroHaIcon name={card.icon} size={20} /></span>
            <span className="tooltip-title">{card.label}</span>
          </div>
          <p className="tooltip-description">{card.description}</p>
          <div className="tooltip-params">
            {Object.entries(card.params).map(([key, defaultValue]) => (
              <div key={key} className="tooltip-param">
                <span className="param-name">{key}:</span>
                <span className="param-default">{String(defaultValue)}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      <style>{`
        .command-card {
          display: flex;
          flex-direction: column;
          padding: 12px;
          background: var(--bg-input);
          border-radius: 8px;
          cursor: pointer;
          transition: all 0.2s ease;
          border: 2px solid transparent;
          position: relative;
          user-select: none;
        }

        .command-card:hover {
          background: var(--bg-hover);
          transform: translateY(-2px);
          box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }

        .command-card.selected {
          border-color: var(--accent-primary);
          background: var(--bg-hover);
        }

        .command-card:focus {
          outline: none;
          border-color: var(--accent-primary);
          box-shadow: 0 0 0 2px var(--accent-primary-glow);
        }

        .card-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 8px;
        }

        .card-icon {
          font-size: 24px;
          line-height: 1;
        }

        .card-category {
          font-size: 10px;
          padding: 2px 6px;
          border-radius: 4px;
          color: var(--bg-primary);
          font-weight: 600;
          text-transform: uppercase;
          letter-spacing: 0.5px;
        }

        .card-body {
          flex: 1;
          margin-bottom: 8px;
        }

        .card-title {
          margin: 0 0 4px 0;
          font-size: 14px;
          font-weight: 600;
          color: var(--text-primary);
        }

        .card-description {
          margin: 0;
          font-size: 11px;
          color: var(--text-secondary);
          line-height: 1.4;
          display: -webkit-box;
          -webkit-line-clamp: 2;
          -webkit-box-orient: vertical;
          overflow: hidden;
        }

        .card-footer {
          display: flex;
          flex-wrap: wrap;
          gap: 4px;
          align-items: center;
        }

        .card-tag {
          font-size: 10px;
          padding: 2px 6px;
          background: var(--bg-hover);
          border-radius: 4px;
          color: var(--text-secondary);
        }

        .card-custom-badge {
          font-size: 10px;
          padding: 2px 6px;
          background: var(--diff-warn);
          border-radius: 4px;
          color: var(--bg-primary);
          font-weight: 600;
          margin-left: auto;
        }

        .command-card .card-tooltip {
          position: absolute;
          bottom: 100%;
          left: 50%;
          transform: translateX(-50%);
          background: var(--bg-primary);
          border: 1px solid var(--bg-hover);
          border-radius: 8px;
          padding: 12px;
          min-width: 250px;
          max-width: 300px;
          z-index: 1000;
          box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
          pointer-events: none;
          animation: tooltipFadeIn 0.2s ease;
        }

        @keyframes tooltipFadeIn {
          from {
            opacity: 0;
            transform: translateX(-50%) translateY(4px);
          }
          to {
            opacity: 1;
            transform: translateX(-50%) translateY(0);
          }
        }

        .tooltip-header {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 8px;
        }

        .tooltip-icon {
          font-size: 20px;
        }

        .tooltip-title {
          font-size: 14px;
          font-weight: 600;
          color: var(--text-primary);
        }

        .tooltip-description {
          margin: 0 0 12px 0;
          font-size: 12px;
          color: var(--text-secondary);
          line-height: 1.5;
        }

        .tooltip-params {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }

        .tooltip-param {
          display: flex;
          align-items: center;
          gap: 8px;
          font-size: 11px;
        }

        .param-name {
          color: var(--accent-primary);
          font-family: 'JetBrains Mono', monospace;
          min-width: 60px;
        }

        .param-default {
          color: var(--text-muted);
          font-family: 'JetBrains Mono', monospace;
        }
      `}</style>
    </div>
  );
}
