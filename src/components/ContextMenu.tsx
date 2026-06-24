import { useEffect, useRef } from "react";
import FeroHaIcon from "./FeroHaIcon";

export interface ContextMenuActionItem {
  id: string;
  label: string;
  icon?: string;
  disabled?: boolean;
  shortcut?: string;
  variant?: "default" | "danger";
  onSelect: () => void;
}

export interface ContextMenuSeparatorItem {
  id: string;
  type: "separator";
}

export type ContextMenuItem = ContextMenuActionItem | ContextMenuSeparatorItem;

function isContextMenuSeparator(item: ContextMenuItem): item is ContextMenuSeparatorItem {
  return "type" in item && item.type === "separator";
}

interface ContextMenuProps {
  point: { x: number; y: number };
  items: ContextMenuItem[];
  onClose: () => void;
}

export default function ContextMenu({ point, items, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target;
      if (target instanceof Node && menuRef.current?.contains(target)) return;
      onClose();
    };

    document.addEventListener("keydown", handleKeyDown);
    document.addEventListener("mousedown", handlePointerDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      document.removeEventListener("mousedown", handlePointerDown);
    };
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      className="feroha-context-menu"
      role="menu"
      style={{
        position: "fixed",
        left: point.x,
        top: point.y,
        zIndex: 1200,
      }}
    >
      {items.map((item) => {
        if (isContextMenuSeparator(item)) {
          return <div key={item.id} role="separator" className="feroha-context-menu-separator" />;
        }

        const variantClass = item.variant === "danger" ? " is-danger" : "";
        return (
          <button
            key={item.id}
            type="button"
            role="menuitem"
            className={`feroha-context-menu-item${variantClass}`}
            disabled={item.disabled}
            onClick={() => {
              if (item.disabled) return;
              item.onSelect();
              onClose();
            }}
          >
            {item.icon && <FeroHaIcon name={item.icon} size={13} />}
            <span className="feroha-context-menu-label">{item.label}</span>
            {item.shortcut && <span className="feroha-context-menu-shortcut">{item.shortcut}</span>}
          </button>
        );
      })}
    </div>
  );
}
