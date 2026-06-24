import { useCallback, useEffect, useState, type CSSProperties } from "react";
import FeroHaIcon from "./FeroHaIcon";

interface PluginStatusPayload {
  status: string;
  message?: string;
  available_plugins?: number;
  enabled_plugins?: number;
  plugins_dir?: string;
}

type PluginStatusMode = "loading" | "ready" | "preview" | "error";

interface PluginStatusState {
  mode: PluginStatusMode;
  backendStatus: string;
  message: string;
  availablePlugins: number;
  enabledPlugins: number;
  pluginsDir: string;
  error?: string;
}

const initialState: PluginStatusState = {
  mode: "loading",
  backendStatus: "checking",
  message: "正在检查插件后端",
  availablePlugins: 0,
  enabledPlugins: 0,
  pluginsDir: "",
};

function translatePluginMessage(message?: string) {
  if (!message) return "插件管理器已接通";
  if (message === "Plugin manager initialized") return "插件管理器已接通";
  if (message === "Plugin manager initialized; no plugins installed") {
    return "插件管理器已接通，当前未安装插件";
  }
  return message;
}

function statusLabel(mode: PluginStatusMode) {
  switch (mode) {
    case "ready":
      return "已接通";
    case "preview":
      return "浏览器预览";
    case "error":
      return "连接异常";
    default:
      return "检查中";
  }
}

function statusIcon(mode: PluginStatusMode) {
  switch (mode) {
    case "ready":
      return "CircleCheck";
    case "preview":
      return "Monitor";
    case "error":
      return "CircleAlert";
    default:
      return "Loader";
  }
}

export default function PluginSettings() {
  const [pluginStatus, setPluginStatus] = useState<PluginStatusState>(initialState);

  const refreshStatus = useCallback(() => {
    let active = true;
    setPluginStatus((current) => ({
      ...current,
      mode: "loading",
      message: "正在检查插件后端",
      error: undefined,
    }));

    import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke<PluginStatusPayload>("plugin_status"))
      .then((payload) => {
        if (!active) return;
        setPluginStatus({
          mode: payload.status === "ready" ? "ready" : "error",
          backendStatus: payload.status || "unknown",
          message: translatePluginMessage(payload.message),
          availablePlugins: payload.available_plugins ?? 0,
          enabledPlugins: payload.enabled_plugins ?? 0,
          pluginsDir: payload.plugins_dir || ".dualtrack/plugins",
        });
      })
      .catch(() => {
        if (!active) return;
        setPluginStatus({
          mode: "preview",
          backendStatus: "browser",
          message: "插件后端只在 Tauri 应用中可用",
          availablePlugins: 0,
          enabledPlugins: 0,
          pluginsDir: ".dualtrack/plugins",
          error: undefined,
        });
      });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => refreshStatus(), [refreshStatus]);

  const metrics = [
    {
      label: "已发现插件",
      value: pluginStatus.availablePlugins,
      hint: "来自后端插件目录扫描",
      icon: "Package",
    },
    {
      label: "已启用插件",
      value: pluginStatus.enabledPlugins,
      hint: "当前运行时可调用数量",
      icon: "Power",
    },
  ];

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <div style={styles.headerTitleGroup}>
          <span style={styles.eyebrow}>AI 面 · 指令卡扩展</span>
          <h2 style={styles.title}>插件运行状态</h2>
          <p style={styles.subtitle}>
            插件系统作为 AI 面工具与指令卡的扩展底座，只展示当前后端已经确认的运行信息。
          </p>
        </div>
        <button type="button" style={styles.refreshButton} onClick={refreshStatus}>
          <FeroHaIcon name="RefreshCw" size={14} />
          刷新
        </button>
      </header>

      <section style={styles.statusPanel}>
        <div style={styles.statusSummary}>
          <span style={styles.statusIcon}>
            <FeroHaIcon name={statusIcon(pluginStatus.mode)} size={18} />
          </span>
          <div style={styles.statusTextGroup}>
            <div style={styles.statusTitleRow}>
              <span style={styles.statusTitle}>{statusLabel(pluginStatus.mode)}</span>
              <span style={styles.statusPill}>{pluginStatus.backendStatus}</span>
            </div>
            <p style={styles.statusMessage}>{pluginStatus.message}</p>
          </div>
        </div>

        <div style={styles.metricGrid}>
          {metrics.map((metric) => (
            <div key={metric.label} style={styles.metricTile}>
              <div style={styles.metricHeader}>
                <FeroHaIcon name={metric.icon} size={14} />
                <span>{metric.label}</span>
              </div>
              <strong style={styles.metricValue}>{metric.value}</strong>
              <span style={styles.metricHint}>{metric.hint}</span>
            </div>
          ))}
        </div>
      </section>

      <section style={styles.detailBand}>
        <div style={styles.detailItem}>
          <span style={styles.detailLabel}>插件目录</span>
          <code style={styles.detailCode}>{pluginStatus.pluginsDir || ".dualtrack/plugins"}</code>
        </div>
        <div style={styles.detailItem}>
          <span style={styles.detailLabel}>后端命令</span>
          <code style={styles.detailCode}>plugin_status</code>
        </div>
        {pluginStatus.error && (
          <div style={styles.detailItem}>
            <span style={styles.detailLabel}>预览提示</span>
            <span style={styles.detailText}>{pluginStatus.error}</span>
          </div>
        )}
      </section>

      <section style={styles.roleNote}>
        <FeroHaIcon name="ShieldCheck" size={16} />
        <p style={styles.roleText}>
          插件管理目前保留在 AI 面：它影响 agent 可调用能力与指令卡扩展；涉及文件写入或修改的结果仍应进入 Bridge，再由人类面审查。
        </p>
      </section>
    </div>
  );
}

const styles: Record<string, CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    gap: "18px",
    height: "100%",
    width: "100%",
    padding: "24px clamp(18px, 3vw, 34px)",
    overflow: "auto",
    color: "var(--text-primary)",
    backgroundColor: "var(--bg-secondary)",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-start",
    gap: "18px",
    paddingBottom: "16px",
    borderBottom: "1px solid var(--border-color)",
  },
  headerTitleGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    minWidth: 0,
  },
  eyebrow: {
    color: "var(--accent-primary)",
    fontSize: "12px",
    fontWeight: 700,
  },
  title: {
    margin: 0,
    color: "var(--text-primary)",
    fontSize: "22px",
    lineHeight: 1.25,
    letterSpacing: 0,
  },
  subtitle: {
    maxWidth: "680px",
    margin: 0,
    color: "var(--text-secondary)",
    fontSize: "13px",
    lineHeight: 1.65,
  },
  refreshButton: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "8px",
    minHeight: "34px",
    padding: "0 12px",
    border: "1px solid var(--border-color)",
    borderRadius: "6px",
    backgroundColor: "var(--bg-input)",
    color: "var(--text-primary)",
    cursor: "pointer",
    flexShrink: 0,
  },
  statusPanel: {
    display: "grid",
    gridTemplateColumns: "minmax(260px, 1.1fr) minmax(280px, 1fr)",
    gap: "16px",
    alignItems: "stretch",
  },
  statusSummary: {
    display: "flex",
    alignItems: "flex-start",
    gap: "14px",
    padding: "18px",
    border: "1px solid var(--border-color)",
    borderRadius: "8px",
    backgroundColor: "var(--bg-primary)",
    minWidth: 0,
  },
  statusIcon: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "38px",
    height: "38px",
    borderRadius: "8px",
    backgroundColor: "var(--accent-glow)",
    flexShrink: 0,
  },
  statusTextGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    minWidth: 0,
  },
  statusTitleRow: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
    flexWrap: "wrap",
  },
  statusTitle: {
    color: "var(--text-primary)",
    fontSize: "17px",
    fontWeight: 800,
  },
  statusPill: {
    border: "1px solid var(--border-color)",
    borderRadius: "999px",
    padding: "3px 8px",
    color: "var(--accent-primary)",
    backgroundColor: "var(--bg-input)",
    fontFamily: "var(--font-mono)",
    fontSize: "11px",
  },
  statusMessage: {
    margin: 0,
    color: "var(--text-secondary)",
    fontSize: "13px",
    lineHeight: 1.6,
  },
  metricGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(128px, 1fr))",
    gap: "12px",
    minWidth: 0,
  },
  metricTile: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    padding: "16px",
    border: "1px solid var(--border-color)",
    borderRadius: "8px",
    backgroundColor: "var(--bg-primary)",
    minWidth: 0,
  },
  metricHeader: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    color: "var(--text-secondary)",
    fontSize: "12px",
    fontWeight: 700,
  },
  metricValue: {
    color: "var(--text-primary)",
    fontSize: "28px",
    lineHeight: 1,
  },
  metricHint: {
    color: "var(--text-muted)",
    fontSize: "11px",
    lineHeight: 1.45,
  },
  detailBand: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
    gap: "12px",
    padding: "14px 16px",
    border: "1px solid var(--border-muted)",
    borderRadius: "8px",
    backgroundColor: "var(--bg-primary)",
  },
  detailItem: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    minWidth: 0,
  },
  detailLabel: {
    color: "var(--text-muted)",
    fontSize: "11px",
    fontWeight: 700,
  },
  detailCode: {
    display: "block",
    width: "100%",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    color: "var(--text-primary)",
    backgroundColor: "var(--bg-input)",
    border: "1px solid var(--border-color)",
    borderRadius: "6px",
    padding: "8px 10px",
    fontFamily: "var(--font-mono)",
    fontSize: "12px",
  },
  detailText: {
    color: "var(--text-secondary)",
    fontSize: "12px",
    lineHeight: 1.5,
  },
  roleNote: {
    display: "flex",
    alignItems: "flex-start",
    gap: "10px",
    padding: "12px 14px",
    border: "1px solid var(--border-muted)",
    borderRadius: "8px",
    backgroundColor: "var(--bg-primary)",
  },
  roleText: {
    margin: 0,
    color: "var(--text-secondary)",
    fontSize: "12px",
    lineHeight: 1.7,
  },
};
