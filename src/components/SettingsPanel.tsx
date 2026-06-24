import { useState } from "react";
import FeroHaIcon from "./FeroHaIcon";
import {
  useSettings,
  Language,
  ThemeName,
  LLMProvider,
  EmbeddingProvider,
  EditorViewMode,
  saveAndDebugApiSettings,
} from "../hooks/useSettings";
import { shortcutHelpRows } from "../hooks/useKeyboardShortcuts";
import { useAppStore } from "../hooks/useAppStore";

const LANGUAGES: { value: Language; label: string }[] = [
  { value: "zh", label: "中文" },
  { value: "en", label: "English" },
];

const THEMES: { value: ThemeName; label: string; color: string }[] = [
  { value: "feroha", label: "深绿", color: "#030d08" },
  { value: "classic", label: "Classic", color: "#1e1e2e" },
  { value: "macchiato", label: "Macchiato", color: "#24273a" },
  { value: "frappe", label: "Frappe", color: "#303446" },
  { value: "latte", label: "Latte", color: "#eff1f5" },
];

const LLM_PROVIDERS: { value: LLMProvider; label: string }[] = [
  { value: "gemini", label: "Google Gemini" },
  { value: "openai", label: "OpenAI" },
  { value: "deepseek", label: "DeepSeek" },
  { value: "anthropic", label: "Anthropic Claude" },
  { value: "ollama", label: "Ollama 本地" },
];

const EMBEDDING_PROVIDERS: { value: EmbeddingProvider; label: string }[] = [
  { value: "gemini", label: "Google Gemini" },
  { value: "openai", label: "OpenAI" },
  { value: "none", label: "关闭" },
];

const VIEW_MODES = [
  { value: "edit", label: "编辑" },
  { value: "preview", label: "预览" },
];

interface CollapsibleSectionProps {
  icon: string;
  title: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}

function CollapsibleSection({ icon, title, defaultOpen = true, children }: CollapsibleSectionProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <section style={styles.section}>
      <button
        type="button"
        style={styles.sectionHeader}
        onClick={() => setOpen(!open)}
        aria-expanded={open}
      >
        <FeroHaIcon name={open ? "ChevronDown" : "ChevronRight"} size={12} />
        <FeroHaIcon name={icon} size={14} />
        <span style={styles.sectionTitle}>{title}</span>
      </button>
      <div style={{ ...styles.sectionContentWrapper, maxHeight: open ? "2000px" : "0px" }}>
        <div style={styles.sectionContent}>{children}</div>
      </div>
    </section>
  );
}

export default function SettingsPanel() {
  const [settings, update] = useSettings();
  const mode = useAppStore((s) => s.mode);
  const shortcuts = shortcutHelpRows(mode);
  const [apiDebugStatus, setApiDebugStatus] = useState("");
  const [isDebuggingApi, setIsDebuggingApi] = useState(false);

  const handleSaveAndDebugApi = async () => {
    setIsDebuggingApi(true);
    setApiDebugStatus("正在保存并调试 API...");
    try {
      const result = await saveAndDebugApiSettings(settings);
      if (result.ok) {
        setApiDebugStatus(`调试成功：${result.provider} / ${result.model}`);
      } else {
        setApiDebugStatus(result.error || "API 调试未通过");
      }
    } catch (error) {
      setApiDebugStatus(`调试失败：${String(error)}`);
    } finally {
      setIsDebuggingApi(false);
    }
  };

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <div>
          <h2 style={styles.title}>设置</h2>
          <p style={styles.subtitle}>配置界面、编辑器、模型和快捷键。</p>
        </div>
      </header>

      <CollapsibleSection icon="Palette" title="外观">
        <div style={styles.configGroup}>
          <div style={styles.configRow}>
            <label style={styles.configLabel}>语言</label>
            <select
              style={styles.select}
              value={settings.language}
              onChange={(e) => update({ language: e.target.value as Language })}
            >
              {LANGUAGES.map((language) => (
                <option key={language.value} value={language.value}>
                  {language.label}
                </option>
              ))}
            </select>
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>主题</label>
            <div style={styles.themeGrid}>
              {THEMES.map((theme) => (
                <button
                  key={theme.value}
                  type="button"
                  style={{
                    ...styles.themeBtn,
                    backgroundColor: theme.color,
                    borderColor:
                      settings.theme === theme.value
                        ? "var(--accent-primary)"
                        : "var(--border-color)",
                  }}
                  onClick={() => update({ theme: theme.value })}
                  title={theme.label}
                >
                  <span style={styles.themeLabel}>{theme.label}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      </CollapsibleSection>

      <CollapsibleSection icon="Monitor" title="编辑器">
        <div style={styles.configGroup}>
          <div style={styles.configRow}>
            <label style={styles.configLabel}>编辑器字号</label>
            <div style={styles.sliderRow}>
              <input
                type="range"
                min={10}
                max={24}
                step={1}
                value={settings.editorFontSize}
                onChange={(e) => update({ editorFontSize: Number(e.target.value) })}
                style={styles.slider}
              />
              <span style={styles.sliderValue}>{settings.editorFontSize}px</span>
            </div>
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>字体族</label>
            <input
              type="text"
              style={styles.input}
              value={settings.editorFontFamily}
              onChange={(e) => update({ editorFontFamily: e.target.value })}
              placeholder="JetBrains Mono, Fira Code, monospace"
            />
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>默认视图</label>
            <select
              style={styles.select}
              value={settings.defaultViewMode}
              onChange={(e) => update({ defaultViewMode: e.target.value as EditorViewMode })}
            >
              {VIEW_MODES.map((mode) => (
                <option key={mode.value} value={mode.value}>
                  {mode.label}
                </option>
              ))}
            </select>
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>自动保存间隔（秒）</label>
            <input
              type="number"
              style={styles.input}
              min={5}
              max={300}
              value={settings.autoSaveInterval}
              onChange={(e) => update({ autoSaveInterval: Number(e.target.value) })}
            />
          </div>
        </div>
      </CollapsibleSection>

      <CollapsibleSection icon="Keyboard" title="快捷键" defaultOpen={false}>
        <div className="shortcuts-table">
          {shortcuts.map((shortcut, i) => (
            <div key={`${shortcut.key}-${i}`} className="shortcuts-row">
              <kbd>
                {shortcut.ctrl ? "Ctrl+" : ""}
                {shortcut.alt ? "Alt+" : ""}
                {shortcut.shift ? "Shift+" : ""}
                {shortcut.key}
              </kbd>
              <span>{shortcut.description}</span>
            </div>
          ))}
        </div>
      </CollapsibleSection>

      <CollapsibleSection icon="Settings" title="AI 提供商">
        <div style={styles.configGroup}>
          <div style={styles.configRow}>
            <label style={styles.configLabel}>LLM 提供商</label>
            <select
              style={styles.select}
              value={settings.llmProvider}
              onChange={(e) => update({ llmProvider: e.target.value as LLMProvider })}
            >
              {LLM_PROVIDERS.map((provider) => (
                <option key={provider.value} value={provider.value}>
                  {provider.label}
                </option>
              ))}
            </select>
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>LLM API Key</label>
            <input
              type="password"
              style={styles.input}
              value={settings.llmApiKey}
              onChange={(e) => update({ llmApiKey: e.target.value })}
              placeholder="输入 API Key"
            />
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>LLM 模型</label>
            <input
              type="text"
              style={styles.input}
              value={settings.llmModel}
              onChange={(e) => update({ llmModel: e.target.value })}
              placeholder="输入模型名称"
            />
          </div>

          {settings.llmProvider === "ollama" && (
            <div style={styles.configRow}>
              <label style={styles.configLabel}>Ollama URL</label>
              <input
                type="text"
                style={styles.input}
                value={settings.ollamaBaseUrl}
                onChange={(e) => update({ ollamaBaseUrl: e.target.value })}
                placeholder="http://localhost:11434"
              />
            </div>
          )}

          <div style={styles.configRow}>
            <label style={styles.configLabel}>Embedding 提供商</label>
            <select
              style={styles.select}
              value={settings.embeddingProvider}
              onChange={(e) => update({ embeddingProvider: e.target.value as EmbeddingProvider })}
            >
              {EMBEDDING_PROVIDERS.map((provider) => (
                <option key={provider.value} value={provider.value}>
                  {provider.label}
                </option>
              ))}
            </select>
          </div>

          {settings.embeddingProvider !== "none" && (
            <div style={styles.configRow}>
              <label style={styles.configLabel}>Embedding API Key</label>
              <input
                type="password"
                style={styles.input}
                value={settings.embeddingApiKey}
                onChange={(e) => update({ embeddingApiKey: e.target.value })}
                placeholder="输入 API Key"
              />
            </div>
          )}

          <div style={styles.apiDebugRow}>
            <button
              type="button"
              style={styles.apiDebugButton}
              onClick={handleSaveAndDebugApi}
              disabled={isDebuggingApi}
              aria-label="保存并调试 API"
            >
              <FeroHaIcon name={isDebuggingApi ? "Loader" : "Sparkles"} size={14} />
              <span>{isDebuggingApi ? "调试中..." : "保存并调试 API"}</span>
            </button>
            {apiDebugStatus && <span role="status" style={styles.apiDebugStatus}>{apiDebugStatus}</span>}
          </div>
        </div>
      </CollapsibleSection>

    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    padding: "22px 28px",
    height: "100%",
    overflowY: "auto",
    color: "var(--text-primary)",
    boxSizing: "border-box",
    maxWidth: "960px",
    width: "100%",
    margin: "0 auto",
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    paddingBottom: "14px",
    borderBottom: "1px solid var(--border-color)",
  },
  title: {
    fontSize: "20px",
    fontWeight: 700,
    color: "var(--text-primary)",
    margin: 0,
    letterSpacing: 0,
  },
  subtitle: {
    margin: "6px 0 0",
    fontSize: "12px",
    color: "var(--text-secondary)",
  },
  section: {
    borderBottom: "1px solid var(--border-muted)",
  },
  sectionHeader: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    width: "100%",
    padding: "10px 0",
    cursor: "pointer",
    border: "none",
    borderBottom: "1px solid var(--border-color)",
    backgroundColor: "transparent",
    color: "var(--text-primary)",
    transition: "background var(--transition-fast) var(--easing-smooth)",
    userSelect: "none",
    textAlign: "left",
  },
  sectionTitle: {
    fontSize: "13px",
    fontWeight: 700,
    color: "var(--text-primary)",
  },
  sectionContentWrapper: {
    overflow: "hidden",
    transition: "max-height var(--transition-normal) var(--easing-smooth)",
  },
  sectionContent: {
    display: "flex",
    flexDirection: "column",
    gap: "12px",
    padding: "14px 0",
  },
  configGroup: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
    gap: "14px 18px",
  },
  configRow: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    minWidth: 0,
  },
  configLabel: {
    fontSize: "12px",
    color: "var(--text-secondary)",
    fontWeight: 600,
  },
  select: {
    padding: "8px 10px",
    backgroundColor: "var(--bg-input)",
    border: "1px solid var(--border-color)",
    borderRadius: "6px",
    color: "var(--text-primary)",
    fontSize: "13px",
    outline: "none",
    cursor: "pointer",
    width: "100%",
  },
  input: {
    padding: "8px 10px",
    backgroundColor: "var(--bg-input)",
    border: "1px solid var(--border-color)",
    borderRadius: "6px",
    color: "var(--text-primary)",
    fontSize: "13px",
    outline: "none",
    width: "100%",
    boxSizing: "border-box",
  },
  apiDebugRow: {
    gridColumn: "1 / -1",
    display: "flex",
    alignItems: "center",
    gap: "10px",
    flexWrap: "wrap",
  },
  apiDebugButton: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "7px",
    minHeight: "32px",
    padding: "7px 12px",
    border: "1px solid var(--control-border-strong)",
    borderRadius: "6px",
    background: "var(--control-bg)",
    color: "var(--text-primary)",
    cursor: "pointer",
    fontSize: "12px",
    fontWeight: 700,
  },
  apiDebugStatus: {
    color: "var(--text-secondary)",
    fontSize: "12px",
  },
  themeGrid: {
    display: "flex",
    gap: "8px",
    flexWrap: "wrap",
  },
  themeBtn: {
    width: "74px",
    height: "52px",
    borderRadius: "6px",
    cursor: "pointer",
    display: "flex",
    alignItems: "flex-end",
    justifyContent: "center",
    padding: "4px",
    border: "2px solid var(--border-color)",
    transition: "border-color var(--transition-fast) var(--easing-smooth)",
  },
  themeLabel: {
    fontSize: "10px",
    fontWeight: 600,
    color: "var(--text-primary)",
  },
  sliderRow: {
    display: "flex",
    alignItems: "center",
    gap: "12px",
  },
  slider: {
    flex: 1,
    accentColor: "var(--accent-primary)",
    cursor: "pointer",
  },
  sliderValue: {
    fontSize: "12px",
    color: "var(--text-secondary)",
    fontFamily: "var(--font-mono)",
    minWidth: "36px",
  },
};
