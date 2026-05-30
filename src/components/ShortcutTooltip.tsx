import { useState, useRef, useEffect } from "react";
import FeroHaIcon from "./FeroHaIcon";

interface ShortcutTooltipProps {
  shortcut: string;
  description: string;
  children: React.ReactNode;
  position?: "top" | "bottom" | "left" | "right";
}

export default function ShortcutTooltip({
  shortcut,
  description,
  children,
  position = "top",
}: ShortcutTooltipProps) {
  const [isVisible, setIsVisible] = useState(false);
  const [coords, setCoords] = useState({ x: 0, y: 0 });
  const triggerRef = useRef<HTMLDivElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isVisible && triggerRef.current && tooltipRef.current) {
      const triggerRect = triggerRef.current.getBoundingClientRect();
      const tooltipRect = tooltipRef.current.getBoundingClientRect();

      let x = 0;
      let y = 0;

      switch (position) {
        case "top":
          x = triggerRect.left + triggerRect.width / 2 - tooltipRect.width / 2;
          y = triggerRect.top - tooltipRect.height - 8;
          break;
        case "bottom":
          x = triggerRect.left + triggerRect.width / 2 - tooltipRect.width / 2;
          y = triggerRect.bottom + 8;
          break;
        case "left":
          x = triggerRect.left - tooltipRect.width - 8;
          y = triggerRect.top + triggerRect.height / 2 - tooltipRect.height / 2;
          break;
        case "right":
          x = triggerRect.right + 8;
          y = triggerRect.top + triggerRect.height / 2 - tooltipRect.height / 2;
          break;
      }

      setCoords({ x, y });
    }
  }, [isVisible, position]);

  const formatShortcut = (shortcut: string) => {
    return shortcut
      .replace("Ctrl", "Ctrl")
      .replace("Shift", "Shift")
      .replace("Alt", "Alt")
      .replace("Meta", "Cmd/Win");
  };

  return (
    <div
      ref={triggerRef}
      onMouseEnter={() => setIsVisible(true)}
      onMouseLeave={() => setIsVisible(false)}
      style={styles.trigger}
    >
      {children}
      {isVisible && (
        <div
          ref={tooltipRef}
          style={{
            ...styles.tooltip,
            left: coords.x,
            top: coords.y,
            opacity: isVisible ? 1 : 0,
            transform: isVisible ? "translateY(0)" : "translateY(4px)",
          }}
        >
          <div style={styles.description}>{description}</div>
          <div style={styles.shortcut}>
            {formatShortcut(shortcut)}
          </div>
        </div>
      )}
    </div>
  );
}

export function ShortcutHelpModal({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  const shortcuts = [
    { key: "Ctrl+N", description: "新建笔记" },
    { key: "Ctrl+P", description: "快速搜索" },
    { key: "Ctrl+B", description: "切换侧边栏" },
    { key: "Ctrl+1", description: "Editor面板" },
    { key: "Ctrl+2", description: "Graph面板" },
    { key: "Ctrl+3", description: "Diff面板" },
    { key: "Ctrl+/", description: "快捷键帮助" },
    { key: "Ctrl+S", description: "保存笔记" },
  ];

  if (!isOpen) return null;

  return (
    <div style={styles.overlay} onClick={onClose}>
      <div style={styles.modal} onClick={(e) => e.stopPropagation()}>
        <div style={styles.modalHeader}>
          <h3 style={styles.modalTitle}>快捷键列表</h3>
          <button style={styles.closeBtn} onClick={onClose}>
            <FeroHaIcon name="X" size={14} />
          </button>
        </div>
        <div style={styles.shortcutList}>
          {shortcuts.map((s) => (
            <div key={s.key} style={styles.shortcutItem}>
              <span style={styles.shortcutDesc}>{s.description}</span>
              <kbd style={styles.kbd}>
                {s.key
                  .replace("Ctrl", "Ctrl")
                  .replace("Shift", "Shift")
                  .replace("Alt", "Alt")}
              </kbd>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  trigger: {
    position: "relative",
    display: "inline-flex",
  },
  tooltip: {
    position: "fixed",
    zIndex: 10000,
    backgroundColor: "#313244",
    border: "1px solid #45475a",
    borderRadius: "6px",
    padding: "8px 12px",
    boxShadow: "0 4px 12px rgba(0, 0, 0, 0.3)",
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    transition: "all 0.15s ease-in-out",
    pointerEvents: "none",
  },
  description: {
    fontSize: "12px",
    color: "#cdd6f4",
    whiteSpace: "nowrap",
  },
  shortcut: {
    fontSize: "11px",
    color: "#a6adc8",
    display: "flex",
    alignItems: "center",
    gap: "4px",
  },
  overlay: {
    position: "fixed",
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: "rgba(0, 0, 0, 0.5)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    zIndex: 10001,
  },
  modal: {
    backgroundColor: "#1e1e2e",
    borderRadius: "8px",
    border: "1px solid #313244",
    boxShadow: "0 8px 32px rgba(0, 0, 0, 0.4)",
    width: "400px",
    maxHeight: "500px",
    overflow: "auto",
  },
  modalHeader: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "16px 20px",
    borderBottom: "1px solid #313244",
  },
  modalTitle: {
    margin: 0,
    fontSize: "16px",
    fontWeight: 600,
    color: "#cdd6f4",
  },
  closeBtn: {
    padding: "4px 8px",
    backgroundColor: "transparent",
    color: "#6c7086",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "14px",
    transition: "all 0.15s",
  },
  shortcutList: {
    padding: "12px 20px",
  },
  shortcutItem: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "8px 0",
    borderBottom: "1px solid #313244",
  },
  shortcutDesc: {
    fontSize: "13px",
    color: "#cdd6f4",
  },
  kbd: {
    display: "inline-flex",
    alignItems: "center",
    padding: "2px 8px",
    backgroundColor: "#45475a",
    borderRadius: "4px",
    fontSize: "12px",
    fontFamily: "monospace",
    color: "#cdd6f4",
    border: "1px solid #585b70",
  },
};
