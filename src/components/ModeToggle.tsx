import { useEffect, useRef, useState } from "react";
import { useAppStore } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";

type Mode = "human" | "ai";

interface ModeToggleProps {
  collapsed?: boolean;
}

export function modeDisplayLabel(mode: Mode): string {
  return mode === "human" ? "人类面" : "AI 面";
}

export function modeSwitchTitle(mode: Mode): string {
  return mode === "human" ? "切换到 AI 面" : "切换到人类面";
}

export function modeToggleWrapperStyleForState(collapsed: boolean): React.CSSProperties {
  return collapsed ? styles.wrapperCollapsed : styles.wrapper;
}

export default function ModeToggle({ collapsed = false }: ModeToggleProps) {
  const mode = useAppStore((s) => s.mode);
  const setMode = useAppStore((s) => s.setMode);
  const [rotated, setRotated] = useState(false);
  const [label, setLabel] = useState(modeDisplayLabel(mode));
  const [labelVisible, setLabelVisible] = useState(true);
  const prevModeRef = useRef(mode);

  useEffect(() => {
    if (prevModeRef.current === mode) return;
    prevModeRef.current = mode;
    setRotated(true);
    setLabelVisible(false);
    const t1 = setTimeout(() => {
      setLabel(modeDisplayLabel(mode));
      setLabelVisible(true);
    }, 140);
    const t2 = setTimeout(() => setRotated(false), 260);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [mode]);

  return (
    <div style={modeToggleWrapperStyleForState(collapsed)}>
      <button
        onClick={() => setMode(mode === "human" ? "ai" : "human")}
        style={styles.button}
        title={modeSwitchTitle(mode)}
        aria-label={modeSwitchTitle(mode)}
      >
        <span
          style={{
            display: "inline-flex",
            transition: "transform 260ms ease",
            transform: rotated ? "rotate(180deg)" : "rotate(0deg)",
          }}
        >
          <FeroHaIcon name={mode === "human" ? "User" : "Bot"} size={16} />
        </span>
      </button>
      {!collapsed && (
        <span
          style={{
            ...styles.label,
            opacity: labelVisible ? 1 : 0,
            transition: "opacity 260ms ease",
          }}
        >
          {label}
        </span>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  wrapper: {
    display: "inline-flex",
    alignItems: "center",
    gap: "6px",
    width: "100%",
    minWidth: "100%",
    padding: "2px 0 6px",
    borderBottom: "1px solid var(--border-muted)",
  },
  wrapperCollapsed: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "30px",
  },
  button: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "30px",
    height: "28px",
    backgroundColor: "transparent",
    color: "var(--icon-default)",
    border: "1px solid transparent",
    borderRadius: "4px",
    cursor: "pointer",
    transition: "all 0.15s",
    flexShrink: 0,
  },
  label: {
    fontSize: "12px",
    color: "var(--text-secondary)",
    whiteSpace: "nowrap",
    fontWeight: 600,
    letterSpacing: 0,
  },
};
