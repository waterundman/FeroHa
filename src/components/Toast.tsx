import { useState, useEffect, useCallback, useRef } from "react";
import { bindToastStore, type ToastItem as ToastData } from "./toastBus";

interface ToastProps {
  toasts: ToastData[];
  onRemove: (id: string) => void;
}

function Toast({ toasts, onRemove }: ToastProps) {
  return (
    <div style={styles.container}>
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} onRemove={onRemove} />
      ))}
    </div>
  );
}

function ToastItem({
  toast,
  onRemove,
}: {
  toast: ToastData;
  onRemove: (id: string) => void;
}) {
  const [isVisible, setIsVisible] = useState(false);
  const [isLeaving, setIsLeaving] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  const removeToast = useCallback(() => {
    setIsLeaving(true);
    setTimeout(() => {
      onRemove(toast.id);
    }, 300);
  }, [toast.id, onRemove]);

  useEffect(() => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        setIsVisible(true);
      });
    });

    const duration = toast.duration || 3000;
    timerRef.current = setTimeout(() => {
      removeToast();
    }, duration);

    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, [toast.duration, removeToast]);

  const typeStyles = {
    success: { borderLeft: "3px solid #a6e3a1", icon: "OK" },
    error: { borderLeft: "3px solid #f38ba8", icon: "!" },
    warning: { borderLeft: "3px solid #f9e2af", icon: "!" },
    info: { borderLeft: "3px solid #89b4fa", icon: "i" },
  };

  const style = typeStyles[toast.type];

  return (
    <div
      style={{
        ...styles.toast,
        ...style,
        opacity: isVisible && !isLeaving ? 1 : 0,
        transform: isVisible && !isLeaving ? "translateX(0)" : "translateX(100%)",
        transition: "all 0.3s ease-in-out",
      }}
    >
      <span style={styles.icon}>{style.icon}</span>
      <span style={styles.message}>{toast.message}</span>
      <button style={styles.closeBtn} onClick={removeToast} aria-label="Dismiss notification">
        x
      </button>
    </div>
  );
}

export function ToastContainer() {
  const [toasts, setToasts] = useState<ToastData[]>([]);

  useEffect(() => bindToastStore(setToasts), []);

  const handleRemove = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return <Toast toasts={toasts} onRemove={handleRemove} />;
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    position: "fixed",
    top: "16px",
    right: "16px",
    zIndex: 9999,
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    maxWidth: "350px",
  },
  toast: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "10px 14px",
    backgroundColor: "#313244",
    borderRadius: "6px",
    boxShadow: "0 4px 12px rgba(0, 0, 0, 0.3)",
    color: "#cdd6f4",
    fontSize: "13px",
    lineHeight: "1.4",
    backdropFilter: "blur(10px)",
  },
  icon: {
    minWidth: "18px",
    fontSize: "12px",
    fontWeight: 700,
    flexShrink: 0,
    textAlign: "center",
  },
  message: {
    flex: 1,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  closeBtn: {
    padding: "0 4px",
    backgroundColor: "transparent",
    color: "#6c7086",
    border: "none",
    cursor: "pointer",
    fontSize: "12px",
    lineHeight: "1",
    opacity: 0.7,
    transition: "opacity 0.15s",
  },
};

export default Toast;
