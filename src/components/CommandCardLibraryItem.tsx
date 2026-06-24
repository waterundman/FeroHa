import FeroHaIcon from "./FeroHaIcon";
import { commandCardSkillDescriptor } from "../lib/commandCardSkill";
import type { CommandCardDefinition } from "../types/command-card";

interface CommandCardLibraryItemProps {
  card: CommandCardDefinition;
  selected: boolean;
  favorite: boolean;
  llmReady: boolean;
  onOpen: (card: CommandCardDefinition) => void;
  onToggleFavorite: (id: string) => void;
  onEdit: (card: CommandCardDefinition) => void;
  onDelete: (id: string) => void;
  onDuplicate: (id: string) => void;
}

export default function CommandCardLibraryItem({
  card,
  selected,
  favorite,
  llmReady,
  onOpen,
  onToggleFavorite,
  onEdit,
  onDelete,
  onDuplicate,
}: CommandCardLibraryItemProps) {
  const skill = commandCardSkillDescriptor(
    {
      id: card.meta.id,
      type: card.meta.type,
      category: card.meta.category,
      promptTemplate: card.prompt.template,
    },
    { llmReady },
  );

  return (
    <div
      className={`card-item ${favorite ? "favorite" : ""} ${
        card.meta.isCustom ? "custom" : ""
      } ${selected ? "selected" : ""}`}
      onClick={() => onOpen(card)}
    >
      <div className="card-header">
        <span className="card-icon">
          <FeroHaIcon name={card.meta.icon} size={24} />
        </span>
        <div className="card-actions">
          <button
            className={`action-btn favorite-btn ${favorite ? "active" : ""}`}
            onClick={(event) => {
              event.stopPropagation();
              onToggleFavorite(card.meta.id);
            }}
            title={favorite ? "Remove favorite" : "Favorite"}
          >
            <FeroHaIcon name="Star" size={14} />
          </button>
          {card.meta.isCustom && (
            <>
              <button
                className="action-btn edit-btn"
                onClick={(event) => {
                  event.stopPropagation();
                  onEdit(card);
                }}
                title="Edit"
              >
                <FeroHaIcon name="Pencil" size={14} />
              </button>
              <button
                className="action-btn delete-btn"
                onClick={(event) => {
                  event.stopPropagation();
                  onDelete(card.meta.id);
                }}
                title="Delete"
              >
                <FeroHaIcon name="X" size={14} />
              </button>
            </>
          )}
          <button
            className="action-btn duplicate-btn"
            onClick={(event) => {
              event.stopPropagation();
              onDuplicate(card.meta.id);
            }}
            title="Duplicate"
          >
            <FeroHaIcon name="Copy" size={14} />
          </button>
        </div>
      </div>

      <div className="card-body">
        <h5 className="card-title">{card.meta.name}</h5>
        <p className="card-description">{card.meta.description}</p>
        <div
          className="card-skill-line"
          title={`${skill.skillId} path ${skill.capabilities.join(" / ")}`}
        >
          <span className={`card-skill-dot ${skill.status}`} />
          <span>{skill.skillId}</span>
          <span>{skill.statusLabel}</span>
        </div>
      </div>

      <div className="card-footer">
        <div className="card-tags">
          {card.meta.tags.slice(0, 3).map((tag) => (
            <span key={tag} className="card-tag">
              {tag}
            </span>
          ))}
          {card.meta.tags.length > 3 && (
            <span className="card-tag-more">+{card.meta.tags.length - 3}</span>
          )}
        </div>
        {card.meta.isCustom && <span className="custom-badge">Custom</span>}
      </div>

      <div className="card-meta">
        <span className="meta-version">v{card.meta.version}</span>
        {card.meta.author && <span className="meta-author">{card.meta.author}</span>}
      </div>
    </div>
  );
}
