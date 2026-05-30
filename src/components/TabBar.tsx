import { useState, useRef, useEffect, useCallback } from "react";
import { useAppStore } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";
import "./TabBar.css";

export default function TabBar() {
  const tabs = useAppStore((s) => s.tabs);
  const activeTabIndex = useAppStore((s) => s.activeTabIndex);
  const splitActive = useAppStore((s) => s.splitActive);
  const switchTab = useAppStore((s) => s.switchTab);
  const closeTab = useAppStore((s) => s.closeTab);
  const toggleSplit = useAppStore((s) => s.toggleSplit);
  const closeAllTabs = useAppStore((s) => s.closeAllTabs);
  const closeOtherTabs = useAppStore((s) => s.closeOtherTabs);

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; tabIndex: number } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const close = () => setContextMenu(null);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, []);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      const shift = e.shiftKey;
      const store = useAppStore.getState();

      if (ctrl && !shift && e.key.toLowerCase() === "w") {
        e.preventDefault();
        if (store.activeTabIndex >= 0) store.closeTab(store.activeTabIndex);
        return;
      }

      if (ctrl && shift && !e.altKey && e.key === "Tab") {
        e.preventDefault();
        if (store.tabs.length === 0) return;
        const newIndex = (store.activeTabIndex - 1 + store.tabs.length) % store.tabs.length;
        store.switchTab(newIndex);
        return;
      }

      if (ctrl && !shift && !e.altKey && e.key === "Tab") {
        e.preventDefault();
        if (store.tabs.length === 0) return;
        const newIndex = (store.activeTabIndex + 1) % store.tabs.length;
        store.switchTab(newIndex);
        return;
      }

      if (ctrl && shift && e.key.toLowerCase() === "t") {
        e.preventDefault();
        store.restoreTab();
        return;
      }

      if (ctrl && !shift && !e.altKey && e.key === "\\") {
        e.preventDefault();
        store.toggleSplit();
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const handleContextMenu = (e: React.MouseEvent, tabIndex: number) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, tabIndex });
  };

  const handleClose = (e: React.MouseEvent, index: number) => {
    e.stopPropagation();
    closeTab(index);
  };

  const scrollToActive = useCallback(() => {
    if (!scrollRef.current) return;
    const activeEl = scrollRef.current.querySelector(".tab-bar-item.active") as HTMLElement;
    if (activeEl) {
      activeEl.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
    }
  }, []);

  useEffect(() => {
    scrollToActive();
  }, [activeTabIndex, scrollToActive]);

  if (tabs.length === 0) return null;

  return (
    <div className="tab-bar" ref={scrollRef}>
      {tabs.map((tab, i) => {
        const isActive = i === activeTabIndex;
        const isSplit = splitActive && i === useAppStore.getState().splitTabIndex;
        return (
          <div
            key={tab.path}
            className={`tab-bar-item${isActive ? " active" : ""}${isSplit ? " split" : ""}`}
            onClick={() => switchTab(i)}
            onContextMenu={(e) => handleContextMenu(e, i)}
            onMouseDown={(e) => {
              if (e.button === 1) {
                e.preventDefault();
                closeTab(i);
              }
            }}
            title={tab.path}
          >
            <span className="tab-bar-title">{tab.title}</span>
            {tab.isDirty && <span className="tab-bar-dirty" />}
            <button
              className="tab-bar-close"
              onClick={(e) => handleClose(e, i)}
              title="Close tab"
            >
              <FeroHaIcon name="X" size={12} />
            </button>
          </div>
        );
      })}

      {contextMenu && (
        <div
          className="feroha-context-menu"
          style={{
            position: "fixed",
            left: contextMenu.x,
            top: contextMenu.y,
            zIndex: 1001,
          }}
        >
          <div
            className="feroha-context-menu-item"
            onClick={() => {
              closeTab(contextMenu.tabIndex);
              setContextMenu(null);
            }}
          >
            Close
          </div>
          <div
            className="feroha-context-menu-item"
            onClick={() => {
              closeOtherTabs(contextMenu.tabIndex);
              setContextMenu(null);
            }}
          >
            Close Others
          </div>
          <div
            className="feroha-context-menu-item"
            onClick={() => {
              closeAllTabs();
              setContextMenu(null);
            }}
          >
            Close All
          </div>
          {!splitActive && tabs.length >= 2 && (
            <>
              <div style={{ height: "1px", backgroundColor: "var(--border-color)", margin: "4px 0" }} />
              <div
                className="feroha-context-menu-item"
                onClick={() => {
                  toggleSplit("right");
                  setContextMenu(null);
                }}
              >
                Split Right
              </div>
              <div
                className="feroha-context-menu-item"
                onClick={() => {
                  toggleSplit("down");
                  setContextMenu(null);
                }}
              >
                Split Down
              </div>
            </>
          )}
          {splitActive && (
            <div
              className="feroha-context-menu-item"
              onClick={() => {
                toggleSplit();
                setContextMenu(null);
              }}
            >
              Close Split
            </div>
          )}
        </div>
      )}
    </div>
  );
}
