import { useState, useRef, useEffect, useCallback } from "react";

interface EnhancedTooltipProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  children: React.ReactNode;
  position?: "top" | "bottom" | "left" | "right";
  delay?: number;
}

export default function EnhancedTooltip({
  icon,
  title,
  description,
  children,
  position = "top",
  delay = 300,
}: EnhancedTooltipProps) {
  const [isVisible, setIsVisible] = useState(false);
  const [coords, setCoords] = useState({ x: 0, y: 0 });
  const triggerRef = useRef<HTMLDivElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  const showTooltip = useCallback(() => {
    timerRef.current = setTimeout(() => {
      setIsVisible(true);
    }, delay);
  }, [delay]);

  const hideTooltip = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
    }
    setIsVisible(false);
  }, []);

  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, []);

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

  return (
    <div
      ref={triggerRef}
      onMouseEnter={showTooltip}
      onMouseLeave={hideTooltip}
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
          <div style={styles.content}>
            {icon && <div style={styles.icon}>{icon}</div>}
            <div style={styles.text}>
              <div style={styles.title}>{title}</div>
              {description && (
                <div style={styles.description}>{description}</div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export function InfoTooltip({
  info,
  children,
}: {
  info: { icon?: React.ReactNode; title: string; description?: string };
  children: React.ReactNode;
}) {
  return (
    <EnhancedTooltip
      icon={info.icon}
      title={info.title}
      description={info.description}
    >
      {children}
    </EnhancedTooltip>
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
    padding: "10px 14px",
    boxShadow: "0 4px 12px rgba(0, 0, 0, 0.3)",
    maxWidth: "300px",
    transition: "all 0.15s ease-in-out",
    pointerEvents: "none",
  },
  content: {
    display: "flex",
    gap: "10px",
    alignItems: "flex-start",
  },
  icon: {
    fontSize: "16px",
    flexShrink: 0,
    marginTop: "2px",
  },
  text: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
  },
  title: {
    fontSize: "13px",
    fontWeight: 600,
    color: "#cdd6f4",
    lineHeight: "1.4",
  },
  description: {
    fontSize: "12px",
    color: "#a6adc8",
    lineHeight: "1.4",
  },
};
