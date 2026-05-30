/**
 * PluginSettings — Manage installed plugins
 * Supports install from URL, enable/disable, uninstall
 */
import { useEffect, useState } from "react";

export default function PluginSettings() {
  const [status, setStatus] = useState<string>("Checking plugin system...");
  const [message, setMessage] = useState<string>("Backend plugin infrastructure exists; frontend integration is pending.");

  useEffect(() => {
    let cancelled = false;
    import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke<{ status: string; message: string; available_plugins: number }>("plugin_status"))
      .then((payload) => {
        if (cancelled) return;
        setStatus(payload.status);
        setMessage(`${payload.message} (${payload.available_plugins} available)`);
      })
      .catch(() => {
        if (cancelled) return;
        setStatus("Browser mode");
        setMessage("Plugin backend is only available in the Tauri app.");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div style={styles.container}>
      <div style={styles.comingSoon}>
        <h2 style={styles.title}>Plugin system is under development</h2>
        <p style={styles.subtitle}>
          {status}: {message || "Backend plugin infrastructure exists; frontend integration is pending."}
        </p>
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
  comingSoon: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    flex: 1,
    padding: "60px 20px",
    textAlign: "center",
  },
  title: {
    fontSize: "18px",
    fontWeight: 600,
    color: "#cdd6f4",
    margin: "0 0 12px 0",
  },
  subtitle: {
    fontSize: "13px",
    color: "#6c7086",
    margin: 0,
    lineHeight: "1.6",
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
    backgroundColor: "transparent",
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
