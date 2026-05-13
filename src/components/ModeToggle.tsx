import { useAppStore } from "../hooks/useAppStore";

export default function ModeToggle() {
  const mode = useAppStore((s) => s.mode);
  const setMode = useAppStore((s) => s.setMode);

  const toggle = () => {
    setMode(mode === "human" ? "ai" : "human");
  };

  return (
    <button onClick={toggle} style={styles.button} title={mode === "human" ? "Switch to AI mode" : "Switch to Human mode"}>
      <span style={styles.icon}>{mode === "human" ? "👤" : "🤖"}</span>
    </button>
  );
}

const styles: Record<string, React.CSSProperties> = {
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
  icon: {
    fontSize: "16px",
  },
};