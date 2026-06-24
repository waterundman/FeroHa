import React from "react";
import { icons, type LucideProps } from "lucide-react";

interface FeroHaIconProps {
  name: string;
  size?: number;
  className?: string;
}

const iconAliases: Record<string, string> = {
  HelpCircle: "CircleHelp",
  BarChart3: "ChartBar",
  Edit: "Pencil",
};

export default function FeroHaIcon({ name, size = 20, className = "" }: FeroHaIconProps) {
  const normalized = name.charAt(0).toUpperCase() + name.slice(1).replace(/[^a-zA-Z0-9]/g, "");
  const iconName = iconAliases[normalized] ?? normalized;

  const IconComponent = (icons as Record<string, React.ComponentType<LucideProps>>)[iconName];

  if (!IconComponent) {
    console.warn(`FeroHaIcon: icon "${name}" (→"${normalized}") not found in lucide-react`);
    return (
      <span
        className={`feroha-icon ${className}`}
        aria-hidden="true"
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
      aria-hidden="true"
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: size,
        height: size,
      }}
    >
      <IconComponent size={size} strokeWidth={1.5} />
    </span>
  );
}
