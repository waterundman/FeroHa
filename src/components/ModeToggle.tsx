import { useState, useEffect, useRef } from "react";
import { useAppStore } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";

export default function ModeToggle() {
  const mode = useAppStore((s) => s.mode);
  const setMode = useAppStore((s) => s.setMode);
  const [rotated, setRotated] = useState(false);
  const [label, setLabel] = useState(mode === "human" ? "人类面" : "AI面");
  const [labelVisible, setLabelVisible] = useState(true);
  const prevModeRef = useRef(mode);

  useEffect(() => {
    if (prevModeRef.current === mode) return;
    prevModeRef.current = mode;
    setRotated(true);
    setLabelVisible(false);
    const t1 = setTimeout(() => {
      setLabel(mode === "human" ? "人类面" : "AI面");
      setLabelVisible(true);
    }, 150);
    const t2 = setTimeout(() => setRotated(false), 300);
    return () => { clearTimeout(t1); clearTimeout(t2); };
  }, [mode]);

  const toggle = () => {
    setMode(mode === "human" ? "ai" : "human");
  };

  return (
    <div style={styles.wrapper}>
      <button onClick={toggle} style={styles.button} title={mode === "human" ? "Switch to AI mode" : "Switch to Human mode"}>
        <span
          style={{
            display: "inline-flex",
            transition: "transform 300ms ease",
            transform: rotated ? "rotate(180deg)" : "rotate(0deg)",
          }}
        >
          <FeroHaIcon name={mode === "human" ? "User" : "Bot"} size={16} />
        </span>
      </button>
      <span
        style={{
          ...styles.label,
          opacity: labelVisible ? 1 : 0,
          transition: "opacity 300ms ease",
        }}
      >
        {label}
      </span>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  wrapper: {
    display: "inline-flex",
    alignItems: "center",
  },
  button: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "30px",
    height: "28px",
    backgroundColor: "transparent",
    color: "#cdd6f4",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    transition: "all 0.15s",
  },
  label: {
    fontSize: "11px",
    color: "var(--text-muted)",
    marginLeft: "4px",
    whiteSpace: "nowrap",
  },
};