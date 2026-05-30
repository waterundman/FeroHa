import React from "react";
import { icons, type LucideProps } from "lucide-react";

interface FeroHaIconProps {
  name: string;
  size?: number;
  className?: string;
}

export default function FeroHaIcon({ name, size = 20, className = "" }: FeroHaIconProps) {
  const normalized = name.charAt(0).toUpperCase() + name.slice(1).replace(/[^a-zA-Z0-9]/g, "");

  const IconComponent = (icons as Record<string, React.ComponentType<LucideProps>>)[normalized];

  if (!IconComponent) {
    console.warn(`FeroHaIcon: icon "${name}" (→"${normalized}") not found in lucide-react`);
    return (
      <span
        className={`feroha-icon ${className}`}
        style={{
          display: "inline-flex",
          width: size,
          height: size,
          borderRadius: "50%",
          border: "1px solid var(--icon-default)",
          opacity: 0.5,
        }}
      />
    );
  }

  return (
    <span
      className={`feroha-icon ${className}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: size,
        height: size,
      }}
    >
      <style>{`
        .feroha-icon svg {
          stroke: var(--icon-default);
          transition: all var(--transition-normal) var(--easing-smooth);
        }
        .feroha-icon:hover svg {
          stroke: var(--icon-hover);
          filter: drop-shadow(0 0 8px rgba(42, 224, 154, 0.6));
          transform: scale(1.05);
        }
      `}</style>
      <IconComponent size={size} strokeWidth={1.5} />
    </span>
  );
}
