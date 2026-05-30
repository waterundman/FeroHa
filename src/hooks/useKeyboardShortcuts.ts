import { useEffect, useCallback } from "react";
import { useAppStore } from "./useAppStore";

export interface ShortcutConfig {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  description: string;
  action: () => void;
}

interface KeyboardShortcutProps {
  onNewNote?: () => void;
  onToggleSidebar?: () => void;
  onSearch?: () => void;
  onShowHelp?: () => void;
  onToggleCli?: () => void;
}

export function useKeyboardShortcuts({
  onNewNote,
  onToggleSidebar,
  onSearch,
  onShowHelp,
  onToggleCli,
}: KeyboardShortcutProps = {}) {
  const setActivePanel = useAppStore((s) => s.setActivePanel);
  const goBack = useAppStore((s) => s.goBack);
  const goForward = useAppStore((s) => s.goForward);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      const shift = e.shiftKey;
      const alt = e.altKey;

      if (ctrl && !shift && !alt && e.key === "n") {
        e.preventDefault();
        onNewNote?.();
        return;
      }

      if (ctrl && !shift && !alt && e.key === "p") {
        e.preventDefault();
        onSearch?.();
        return;
      }

      if (ctrl && !shift && !alt && e.key === "b") {
        e.preventDefault();
        onToggleSidebar?.();
        return;
      }

      if (ctrl && !shift && !alt && e.key === "1") {
        e.preventDefault();
        setActivePanel("editor");
        return;
      }

      if (ctrl && !shift && !alt && e.key === "2") {
        e.preventDefault();
        setActivePanel("graph");
        return;
      }

      if (ctrl && !shift && !alt && e.key === "3") {
        e.preventDefault();
        setActivePanel("diff");
        return;
      }

      if (ctrl && !shift && !alt && e.key === "/") {
        e.preventDefault();
        onShowHelp?.();
        return;
      }

      if (ctrl && !shift && !alt && e.key === "`") {
        e.preventDefault();
        onToggleCli?.();
        return;
      }

      if (alt && !ctrl && !shift && e.key === "ArrowLeft") {
        e.preventDefault();
        goBack();
        return;
      }

      if (alt && !ctrl && !shift && e.key === "ArrowRight") {
        e.preventDefault();
        goForward();
        return;
      }
    },
    [setActivePanel, goBack, goForward, onNewNote, onToggleSidebar, onSearch, onShowHelp, onToggleCli]
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleKeyDown]);

  return {
    shortcuts: [
      { key: "N", ctrl: true, description: "新建笔记", action: () => onNewNote?.() },
      { key: "P", ctrl: true, description: "快速搜索", action: () => onSearch?.() },
      { key: "B", ctrl: true, description: "切换侧边栏", action: () => onToggleSidebar?.() },
      { key: "1", ctrl: true, description: "Editor面板", action: () => setActivePanel("editor") },
      { key: "2", ctrl: true, description: "Graph面板", action: () => setActivePanel("graph") },
      { key: "3", ctrl: true, description: "Diff面板", action: () => setActivePanel("diff") },
      { key: "/", ctrl: true, description: "快捷键帮助", action: () => onShowHelp?.() },
      { key: "←", alt: true, description: "后退导航", action: () => goBack() },
      { key: "`", ctrl: true, description: "Toggle CLI window", action: () => onToggleCli?.() },
    ] as ShortcutConfig[],
  };
}
