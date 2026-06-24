import { useCallback, useEffect } from "react";
import { useAppStore, type AppMode, type ActivePanel } from "./useAppStore";

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

export interface ShortcutHelpRow {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  description: string;
}

export function shortcutPanelsForMode(mode: AppMode): { secondPanel: ActivePanel; thirdPanel: ActivePanel } {
  return {
    secondPanel: mode === "ai" ? "graph" : "inspiration",
    thirdPanel: mode === "ai" ? "tasks" : "diff",
  };
}

export function shortcutHelpRows(mode: AppMode): ShortcutHelpRow[] {
  return [
    { key: "N", ctrl: true, description: "新建笔记" },
    { key: "P", ctrl: true, description: "快速搜索" },
    { key: "B", ctrl: true, description: "切换侧栏" },
    { key: "1", ctrl: true, description: "编辑器" },
    { key: "2", ctrl: true, description: mode === "ai" ? "知识图谱" : "灵感画布" },
    { key: "3", ctrl: true, description: mode === "ai" ? "Agent 任务" : "差异审查" },
    { key: "/", ctrl: true, description: "快捷键帮助" },
    { key: "←", alt: true, description: "后退导航" },
    { key: "→", alt: true, description: "前进导航" },
    { key: "`", ctrl: true, description: "打开 CLI 浮窗" },
  ];
}

export function useKeyboardShortcuts({
  onNewNote,
  onToggleSidebar,
  onSearch,
  onShowHelp,
  onToggleCli,
}: KeyboardShortcutProps = {}) {
  const setActivePanel = useAppStore((s) => s.setActivePanel);
  const mode = useAppStore((s) => s.mode);
  const goBack = useAppStore((s) => s.goBack);
  const goForward = useAppStore((s) => s.goForward);

  const { secondPanel, thirdPanel } = shortcutPanelsForMode(mode);
  const shortcutRows = shortcutHelpRows(mode);

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
        setActivePanel(secondPanel);
        return;
      }

      if (ctrl && !shift && !alt && e.key === "3") {
        e.preventDefault();
        setActivePanel(thirdPanel);
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
      }
    },
    [
      setActivePanel,
      secondPanel,
      thirdPanel,
      goBack,
      goForward,
      onNewNote,
      onToggleSidebar,
      onSearch,
      onShowHelp,
      onToggleCli,
    ],
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleKeyDown]);

  return {
    shortcuts: [
      { ...shortcutRows[0], action: () => onNewNote?.() },
      { ...shortcutRows[1], action: () => onSearch?.() },
      { ...shortcutRows[2], action: () => onToggleSidebar?.() },
      { ...shortcutRows[3], action: () => setActivePanel("editor") },
      {
        ...shortcutRows[4],
        action: () => setActivePanel(secondPanel),
      },
      {
        ...shortcutRows[5],
        action: () => setActivePanel(thirdPanel),
      },
      { ...shortcutRows[6], action: () => onShowHelp?.() },
      { ...shortcutRows[7], action: () => goBack() },
      { ...shortcutRows[8], action: () => goForward() },
      { ...shortcutRows[9], action: () => onToggleCli?.() },
    ] as ShortcutConfig[],
  };
}
