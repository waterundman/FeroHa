import FeroHaIcon from "./FeroHaIcon";
import { commandCardSkillDescriptor } from "../lib/commandCardSkill";
import type { CommandCardDefinition } from "../types/command-card";

interface CommandCardPreviewProps {
  card: CommandCardDefinition | null;
  llmReady: boolean;
  useLabel: string;
  message: string;
  onUse: () => void;
}

export default function CommandCardPreview({
  card,
  llmReady,
  useLabel,
  message,
  onUse,
}: CommandCardPreviewProps) {
  if (!card) {
    return (
      <aside className="command-card-preview empty" aria-label="Command card preview">
        <FeroHaIcon name="MousePointerClick" size={18} />
        <p>Click a command card to inspect its prompt, parameters, and usage path.</p>
      </aside>
    );
  }

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
    <aside className="command-card-preview" aria-label="Command card preview">
      <div className="preview-header">
        <span className="preview-icon"><FeroHaIcon name={card.meta.icon} size={22} /></span>
        <div>
          <h3>{card.meta.name}</h3>
          <p>{card.meta.description}</p>
        </div>
      </div>
      <div className="preview-skill">
        <span className={`card-skill-dot ${skill.status}`} />
        <span>{skill.skillId}</span>
        <strong>{skill.statusLabel}</strong>
      </div>
      <div className="preview-block">
        <span className="preview-label">Prompt</span>
        <pre>{card.prompt.template}</pre>
      </div>
      <div className="preview-block">
        <span className="preview-label">Parameters</span>
        {card.params.length === 0 ? (
          <p className="preview-muted">No parameters required.</p>
        ) : (
          <ul>
            {card.params.map((param) => (
              <li key={param.name}>
                <strong>{param.label || param.name}</strong>
                <span>{param.type}{param.required ? " · required" : ""}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="preview-actions">
        <button type="button" className="use-preview-btn" onClick={onUse}>
          {useLabel}
        </button>
        {message && <span className="preview-message">{message}</span>}
      </div>
    </aside>
  );
}
