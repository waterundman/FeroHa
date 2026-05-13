import { useState } from "react";

interface PluginEntry {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  enabled: boolean;
  installedAt: string;
}

/**
 * PluginSettings — Manage installed plugins
 * Supports install from URL, enable/disable, uninstall
 */
export default function PluginSettings() {
  const [plugins, setPlugins] = useState<PluginEntry[]>(() => [
    {
      id: "demo-search",
      name: "Search Enhancer",
      version: "1.0.0",
      description: "Enhanced full-text search with regex support",
      author: "community",
      enabled: true,
      installedAt: "2026-05-01",
    },
    {
      id: "demo-export",
      name: "Export to PDF",
      version: "0.5.0",
      description: "Export notes as styled PDF documents",
      author: "community",
      enabled: false,
      installedAt: "2026-05-02",
    },
  ]);
  const [searchQuery, setSearchQuery] = useState("");
  const [showMarketplace, setShowMarketplace] = useState(false);

  // Marketplace mock data
  const marketplacePlugins: PluginEntry[] = [
    {
      id: "arxiv-agent",
      name: "Arxiv Agent",
      version: "1.2.0",
      description: "Auto-fetch papers from Arxiv and link to vault",
      author: "research-team",
      enabled: false,
      installedAt: "",
    },
    {
      id: "git-sync",
      name: "Git Sync",
      version: "2.0.1",
      description: "Auto-commit and push vault changes to Git",
      author: "core-team",
      enabled: false,
      installedAt: "",
    },
    {
      id: "mindmap",
      name: "Mind Map View",
      version: "0.8.0",
      description: "Visualize note hierarchy as a mind map",
      author: "design-team",
      enabled: false,
      installedAt: "",
    },
  ];

  const togglePlugin = (id: string) => {
    setPlugins((prev) =>
      prev.map((p) => (p.id === id ? { ...p, enabled: !p.enabled } : p))
    );
  };

  const uninstallPlugin = (id: string) => {
    if (!confirm(`Uninstall "${id}"?`)) return;
    setPlugins((prev) => prev.filter((p) => p.id !== id));
  };

  const installFromMarketplace = (plugin: PluginEntry) => {
    if (plugins.find((p) => p.id === plugin.id)) {
      alert(`Plugin "${plugin.name}" is already installed.`);
      return;
    }
    setPlugins((prev) => [
      ...prev,
      {
        ...plugin,
        enabled: false,
        installedAt: new Date().toISOString().split("T")[0],
      },
    ]);
  };

  const filteredPlugins = plugins.filter(
    (p) =>
      p.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      p.description.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const filteredMarketplace = marketplacePlugins.filter(
    (p) =>
      p.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      p.description.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <div style={styles.tabs}>
          <button
            style={{ ...styles.tabBtn, ...(!showMarketplace ? styles.tabActive : {}) }}
            onClick={() => setShowMarketplace(false)}
          >
            Installed ({plugins.length})
          </button>
          <button
            style={{ ...styles.tabBtn, ...(showMarketplace ? styles.tabActive : {}) }}
            onClick={() => setShowMarketplace(true)}
          >
            Marketplace
          </button>
        </div>
        <input
          style={styles.searchInput}
          placeholder="Search plugins..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
      </div>

      {/* Installed plugins */}
      {!showMarketplace && (
        <div style={styles.pluginList}>
          {filteredPlugins.length === 0 && (
            <div style={styles.empty}>
              <p>No plugins installed</p>
              <p style={styles.hint}>Browse the Marketplace to discover plugins</p>
            </div>
          )}
          {filteredPlugins.map((plugin) => (
            <div key={plugin.id} style={styles.pluginCard}>
              <div style={styles.pluginInfo}>
                <div style={styles.pluginHeader}>
                  <span style={styles.pluginName}>{plugin.name}</span>
                  <span style={styles.pluginVersion}>v{plugin.version}</span>
                  {plugin.enabled && (
                    <span style={styles.enabledBadge}>enabled</span>
                  )}
                </div>
                <p style={styles.pluginDesc}>{plugin.description}</p>
                <div style={styles.pluginMeta}>
                  <span>by {plugin.author}</span>
                  <span>·</span>
                  <span>Installed {plugin.installedAt}</span>
                </div>
              </div>
              <div style={styles.pluginActions}>
                <button
                  style={{
                    ...styles.actionBtn,
                    backgroundColor: plugin.enabled ? "#45475a" : "#a6e3a1",
                    color: plugin.enabled ? "#cdd6f4" : "#1e1e2e",
                  }}
                  onClick={() => togglePlugin(plugin.id)}
                >
                  {plugin.enabled ? "Disable" : "Enable"}
                </button>
                <button
                  style={{ ...styles.actionBtn, color: "#f38ba8" }}
                  onClick={() => uninstallPlugin(plugin.id)}
                >
                  Uninstall
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Marketplace */}
      {showMarketplace && (
        <div style={styles.pluginList}>
          {filteredMarketplace.length === 0 && (
            <div style={styles.empty}>No plugins match your search</div>
          )}
          {filteredMarketplace.map((plugin) => (
            <div key={plugin.id} style={styles.pluginCard}>
              <div style={styles.pluginInfo}>
                <div style={styles.pluginHeader}>
                  <span style={styles.pluginName}>{plugin.name}</span>
                  <span style={styles.pluginVersion}>v{plugin.version}</span>
                </div>
                <p style={styles.pluginDesc}>{plugin.description}</p>
                <div style={styles.pluginMeta}>
                  <span>by {plugin.author}</span>
                </div>
              </div>
              <div style={styles.pluginActions}>
                <button
                  style={{
                    ...styles.actionBtn,
                    backgroundColor: "#89b4fa",
                    color: "#1e1e2e",
                  }}
                  onClick={() => installFromMarketplace(plugin)}
                >
                  Install
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <div style={styles.footer}>
        <span style={styles.footerText}>
          {plugins.filter((p) => p.enabled).length} enabled · {plugins.length} installed
        </span>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "6px 0",
    borderBottom: "1px solid #313244",
    marginBottom: "16px",
  },
  tabs: { display: "flex", gap: "4px" },
  tabBtn: {
    padding: "3px 12px",
    background: "transparent",
    color: "#6c7086",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "12px",
  },
  tabActive: { backgroundColor: "#313244", color: "#cdd6f4" },
  searchInput: {
    padding: "4px 10px",
    backgroundColor: "#313244",
    border: "1px solid #45475a",
    borderRadius: "4px",
    color: "#cdd6f4",
    fontSize: "12px",
    width: "200px",
    outline: "none",
  },
  pluginList: {
    flex: 1,
    overflow: "auto",
    display: "flex",
    flexDirection: "column",
    gap: "10px",
  },
  pluginCard: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-start",
    padding: "12px",
    backgroundColor: "#181825",
    border: "1px solid #313244",
    borderRadius: "8px",
  },
  pluginInfo: { flex: 1 },
  pluginHeader: { display: "flex", alignItems: "center", gap: "8px", marginBottom: "4px" },
  pluginName: { fontSize: "14px", fontWeight: 600, color: "#cdd6f4" },
  pluginVersion: { fontSize: "10px", color: "#6c7086", fontFamily: "monospace" },
  enabledBadge: {
    fontSize: "9px",
    padding: "1px 6px",
    backgroundColor: "#a6e3a133",
    color: "#a6e3a1",
    borderRadius: "4px",
    textTransform: "uppercase",
  },
  pluginDesc: { fontSize: "12px", color: "#a6adc8", margin: "4px 0", lineHeight: "1.5" },
  pluginMeta: { display: "flex", gap: "6px", fontSize: "10px", color: "#585b70" },
  pluginActions: { display: "flex", gap: "6px", flexShrink: 0 },
  actionBtn: {
    padding: "4px 12px",
    backgroundColor: "#313244",
    color: "#cdd6f4",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "11px",
    fontWeight: 500,
  },
  empty: { textAlign: "center", padding: "60px 20px", color: "#6c7086" },
  hint: { fontSize: "12px", marginTop: "8px", color: "#585b70" },
  footer: { padding: "8px 0", borderTop: "1px solid #313244", marginTop: "8px" },
  footerText: { fontSize: "11px", color: "#585b70" },
};
