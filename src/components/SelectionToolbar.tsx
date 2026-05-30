import { useState, useEffect, useRef } from "react";
import FeroHaIcon from "./FeroHaIcon";

interface SelectionToolbarProps {
  visible: boolean;
  x: number;
  y: number;
  onAction: (action: string) => void;
  onDismiss: () => void;
}

export default function SelectionToolbar({
  visible,
  x,
  y,
  onAction,
  onDismiss,
}: SelectionToolbarProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [adjustedY, setAdjustedY] = useState(y);

  useEffect(() => {
    if (!visible || !ref.current) {
      setAdjustedY(y);
      return;
    }
    const rect = ref.current.getBoundingClientRect();
    const viewportH = window.innerHeight;
    if (y + rect.height + 12 > viewportH) {
      setAdjustedY(y - rect.height - 8);
    } else {
      setAdjustedY(y + 8);
    }
  }, [visible, y]);

  useEffect(() => {
    if (!visible) return;
    const handle = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onDismiss();
      }
    };
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onDismiss();
    };
    setTimeout(() => {
      document.addEventListener("mousedown", handle);
      document.addEventListener("keydown", handleEsc);
    }, 0);
    return () => {
      document.removeEventListener("mousedown", handle);
      document.removeEventListener("keydown", handleEsc);
    };
  }, [visible, onDismiss]);

  const actions = [
    { id: "analyze", label: "Analyze", icon: "FileSearch" },
    { id: "expand", label: "Expand", icon: "Maximize2" },
    { id: "rewrite", label: "Rewrite", icon: "PenLine" },
    { id: "correct", label: "Correct", icon: "SpellCheck" },
  ];

  return (
    <div
      ref={ref}
      className={`selection-toolbar${visible ? " visible" : ""}`}
      style={{ left: x, top: adjustedY }}
      role="toolbar"
      aria-label="Selection actions"
    >
      {actions.map((a) => (
        <button
          key={a.id}
          className="sel-toolbar-btn"
          onClick={() => onAction(a.id)}
          title={a.label}
        >
          <FeroHaIcon name={a.icon} size={14} />
          <span>{a.label}</span>
        </button>
      ))}
    </div>
  );
}
