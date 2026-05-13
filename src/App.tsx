import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore, type NoteMeta, type GraphData } from "./hooks/useAppStore";
import Editor from "./components/Editor";
import VaultBrowser from "./components/VaultBrowser";
import GraphView from "./components/GraphView";
import DiffView from "./components/DiffView";
import CliBar from "./components/CliBar";
import ModeToggle from "./components/ModeToggle";
import InstructionCardPanel from "./components/InstructionCard";
import SettingsPanel from "./components/SettingsPanel";

export default function App() {
  const [isTauri, setIsTauri] = useState(false);
  const [backendStatus, setBackendStatus] = useState("Initializing...");
  const [showSettings, setShowSettings] = useState(false);
  const setVaultPath = useAppStore((s) => s.setVaultPath);
  const setNotes = useAppStore((s) => s.setNotes);
  const setGraph = useAppStore((s) => s.setGraph);
  const vaultPath = useAppStore((s) => s.vaultPath);
  const activePanel = useAppStore((s) => s.activePanel);
  const mode = useAppStore((s) => s.mode);

  useEffect(() => {
    if (window.__TAURI_INTERNALS__) {
      setIsTauri(true);
      invoke<string>("ping")
        .then((res) => {
          setBackendStatus(`Backend ready: ${res}`);
          loadVaultState();
        })
        .catch(() => setBackendStatus("Backend connection failed"));
    } else {
      setBackendStatus("Browser mode — Tauri not detected");
    }
  }, []);

  const loadVaultState = async () => {
    try {
      const path = await invoke<string>("get_vault_path");
      if (path) {
        setVaultPath(path);
        const notes = await invoke<NoteMeta[]>("list_notes");
        setNotes(notes);
        const graph = await invoke<GraphData>("get_graph");
        setGraph(graph);
      }
    } catch {
      // No vault open yet — normal
    }
  };

  return (
    <div style={styles.app}>
      <header style={styles.header}>
        <span style={styles.status}>{backendStatus}</span>
      </header>

      <div style={styles.main}>
        <aside style={styles.sidebar}>
          <div style={styles.sidebarNav}>
            <ModeToggle />
            <TabBtn d="M12 2l2 2L6 12H4v-2l6-8z" panel="editor" title="Editor" />
            <TabBtn d="M4 4h0M12 4h0M8 13h0M4 4l4 9M12 4l-4 9" panel="graph" title="Graph" />
            <TabBtn d="M3 2v12M13 2v12M6 5h4M6 8h4M6 11h4" panel="diff" title="Diff" />
            <button
              onClick={() => setShowSettings(!showSettings)}
              title="Settings"
              style={{
                ...styles.sidebarTabBtn,
                ...(showSettings ? styles.sidebarTabBtnActive : {}),
                marginLeft: "auto",
              }}
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M8 10a2 2 0 100-4 2 2 0 000 4z" />
                <path d="M13 8c0-.4-.2-.7-.5-1l.3-1.6-1.2-.7-.8 1.3c-.3-.1-.6-.2-.9-.2L9.2 4.2H7.8L7.1 5.8c-.3.1-.6.1-.9.2L5.4 4.7l-1.2.7.3 1.6c-.3.3-.5.6-.5 1s.2.7.5 1l-.3 1.6 1.2.7.8-1.3c.3.1.6.2.9.2l.7 1.6h1.4l.7-1.6c.3-.1.6-.1.9-.2l.8 1.3 1.2-.7-.3-1.6c.3-.3.5-.6.5-1z" />
              </svg>
            </button>
          </div>
          {showSettings ? (
            <SettingsPanel />
          ) : (
            <>
              <VaultBrowser vaultPath={vaultPath} onSelectVault={setVaultPath} isTauri={isTauri} />
              {mode === "ai" && (
                <InstructionCardPanel
                  onExecute={(card, params) => console.log("Execute", card, params)}
                  onExecuteCombo={(combo) => console.log("Combo", combo)}
                  isTauri={isTauri}
                />
              )}
            </>
          )}
        </aside>

        <main style={styles.content}>
          {activePanel === "editor" && <Editor isTauri={isTauri} />}
          {activePanel === "graph" && <GraphView />}
          {activePanel === "diff" && <DiffView />}
        </main>
      </div>

      <footer style={styles.footer}>
        {mode === "ai" && <CliBar isTauri={isTauri} />}
      </footer>
    </div>
  );
}

function TabBtn({ d, panel, title: t }: { d: string; panel: "editor" | "graph" | "diff"; title: string }) {
  const activePanel = useAppStore((s) => s.activePanel);
  const setActivePanel = useAppStore((s) => s.setActivePanel);
  const isActive = activePanel === panel;

  return (
    <button
      onClick={() => setActivePanel(panel)}
      title={t}
      style={{
        ...styles.sidebarTabBtn,
        ...(isActive ? styles.sidebarTabBtnActive : {}),
      }}
    >
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <path d={d} />
      </svg>
    </button>
  );
}

const styles: Record<string, React.CSSProperties> = {
  app: {
    display: "flex",
    flexDirection: "column",
    height: "100vh",
    fontFamily: "system-ui, -apple-system, sans-serif",
    backgroundColor: "#1e1e2e",
    color: "#cdd6f4",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "6px 16px",
    backgroundColor: "#181825",
    borderBottom: "1px solid #313244",
    fontSize: "13px",
    userSelect: "none",
  },
  logo: {
    fontWeight: 700,
    color: "#cba6f7",
    letterSpacing: "0.5px",
  },
  nav: {
    display: "flex",
    gap: "4px",
  },
  tab: {
    padding: "3px 12px",
    backgroundColor: "transparent",
    color: "#6c7086",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "12px",
    transition: "all 0.15s",
  },
  tabActive: {
    backgroundColor: "#313244",
    color: "#cdd6f4",
  },
  status: {
    fontSize: "11px",
    color: "#a6adc8",
    maxWidth: "240px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  main: {
    display: "flex",
    flex: 1,
    overflow: "hidden",
  },
  sidebar: {
    width: "260px",
    minWidth: "200px",
    backgroundColor: "#181825",
    borderRight: "1px solid #313244",
    overflow: "auto",
  },
  content: {
    flex: 1,
    overflow: "auto",
    padding: "24px",
  },
  footer: {
    borderTop: "1px solid #313244",
    backgroundColor: "#181825",
  },
  sidebarNav: {
    display: "flex",
    gap: "2px",
    padding: "6px 8px",
    borderBottom: "1px solid #313244",
  },
  sidebarTabBtn: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "30px",
    height: "28px",
    backgroundColor: "transparent",
    color: "#6c7086",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    transition: "all 0.15s",
  },
  sidebarTabBtnActive: {
    backgroundColor: "#313244",
    color: "#cdd6f4",
  },
};
