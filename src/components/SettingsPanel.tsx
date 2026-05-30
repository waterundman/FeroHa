import { useState } from "react";
import FeroHaIcon from "./FeroHaIcon";
import { useSettings, Language, ThemeName, LLMProvider, EmbeddingProvider, EditorViewMode } from "../hooks/useSettings";
import { useKeyboardShortcuts } from "../hooks/useKeyboardShortcuts";

const LANGUAGES: { value: Language; label: string }[] = [
  { value: "zh", label: "中文" },
  { value: "en", label: "English" },
];

const THEMES: { value: ThemeName; label: string; color: string }[] = [
  { value: "feroha", label: "FeroHa", color: "#030d08" },
  { value: "mocha", label: "Classic", color: "#1e1e2e" },
  { value: "macchiato", label: "Macchiato", color: "#24273a" },
  { value: "frappe", label: "Frappé", color: "#303446" },
  { value: "latte", label: "Latte", color: "#eff1f5" },
];

const LLM_PROVIDERS: { value: LLMProvider; label: string }[] = [
  { value: "gemini", label: "Google Gemini" },
  { value: "openai", label: "OpenAI" },
  { value: "deepseek", label: "DeepSeek" },
  { value: "anthropic", label: "Anthropic Claude" },
  { value: "ollama", label: "Ollama (Local)" },
];

const EMBEDDING_PROVIDERS: { value: EmbeddingProvider; label: string }[] = [
  { value: "gemini", label: "Google Gemini" },
  { value: "openai", label: "OpenAI" },
  { value: "none", label: "None" },
];

const VIEW_MODES = [
  { value: "edit", label: "Edit" },
  { value: "preview", label: "Preview" },
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
    <div style={styles.section}>
      <div
        style={styles.sectionHeader}
        onClick={() => setOpen(!open)}
        onMouseEnter={(e) => (e.currentTarget.style.background = "var(--bg-hover)")}
        onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
      >
        <FeroHaIcon name={open ? "ChevronDown" : "ChevronRight"} size={12} />
        <FeroHaIcon name={icon} size={14} />
        <span style={styles.sectionTitle}>{title}</span>
      </div>
      <div
        style={{
          ...styles.sectionContentWrapper,
          maxHeight: open ? "2000px" : "0px",
        }}
      >
        <div style={styles.sectionContent}>{children}</div>
      </div>
    </div>
  );
}

export default function SettingsPanel() {
  const [settings, update] = useSettings();
  const { shortcuts } = useKeyboardShortcuts({});
  const t = settings.language === "zh";

  return (
    <div style={styles.container}>
      <h2 style={styles.title}>
        {t ? "设置" : "Settings"}
      </h2>

      <CollapsibleSection icon="Palette" title={t ? "外观" : "Appearance"}>
        <div style={styles.configGroup}>
          <div style={styles.configRow}>
            <label style={styles.configLabel}>
              {t ? "语言" : "Language"}
            </label>
            <select
              style={styles.select}
              value={settings.language}
              onChange={(e) => update({ language: e.target.value as Language })}
            >
              {LANGUAGES.map((l) => (
                <option key={l.value} value={l.value}>
                  {l.label}
                </option>
              ))}
            </select>
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>
              {t ? "主题" : "Theme"}
            </label>
            <div style={styles.themeGrid}>
              {THEMES.map((theme) => (
                <button
                  key={theme.value}
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
                  <span style={styles.themeLabel}>
                    {theme.label}
                  </span>
                </button>
              ))}
            </div>
          </div>
        </div>
      </CollapsibleSection>

      <CollapsibleSection icon="Monitor" title={t ? "编辑器" : "Editor"}>
        <div style={styles.configGroup}>
          <div style={styles.configRow}>
            <label style={styles.configLabel}>
              {t ? "编辑器字号" : "Editor Font Size"}
            </label>
            <div style={styles.sliderRow}>
              <input
                type="range"
                min={10}
                max={24}
                step={1}
                value={settings.editorFontSize}
                onChange={(e) =>
                  update({ editorFontSize: Number(e.target.value) })
                }
                style={styles.slider}
              />
              <span style={styles.sliderValue}>{settings.editorFontSize}px</span>
            </div>
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>
              {t ? "字体族" : "Font Family"}
            </label>
            <input
              type="text"
              style={styles.input}
              value={settings.editorFontFamily}
              onChange={(e) => update({ editorFontFamily: e.target.value })}
              placeholder="JetBrains Mono, 'Fira Code', monospace"
            />
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>
              {t ? "默认视图模式" : "Default View Mode"}
            </label>
            <select
              style={styles.select}
              value={settings.defaultViewMode}
              onChange={(e) => update({ defaultViewMode: e.target.value as EditorViewMode })}
            >
              {VIEW_MODES.map((m) => (
                <option key={m.value} value={m.value}>
                  {m.label}
                </option>
              ))}
            </select>
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>
              {t ? "自动保存间隔（秒）" : "Auto-save Interval (s)"}
            </label>
            <input
              type="number"
              style={styles.input}
              min={5}
              max={300}
              value={settings.autoSaveInterval}
              onChange={(e) => update({ autoSaveInterval: Number(e.target.value) })}
              placeholder="30"
            />
          </div>
        </div>
      </CollapsibleSection>

      <CollapsibleSection icon="Keyboard" title={t ? "键盘快捷键" : "Keyboard Shortcuts"} defaultOpen={false}>
        <div className="shortcuts-table">
          {shortcuts.map((sc, i) => (
            <div key={i} className="shortcuts-row">
              <kbd>{sc.ctrl ? "Ctrl+" : ""}{sc.alt ? "Alt+" : ""}{sc.shift ? "Shift+" : ""}{sc.key}</kbd>
              <span>{sc.description}</span>
            </div>
          ))}
        </div>
      </CollapsibleSection>

      <CollapsibleSection icon="Settings" title={t ? "AI 提供商" : "AI Providers"}>
        <div style={styles.configGroup}>
          <div style={styles.configRow}>
            <label style={styles.configLabel}>
              {t ? "LLM 提供商" : "LLM Provider"}
            </label>
            <select
              style={styles.select}
              value={settings.llmProvider}
              onChange={(e) => update({ llmProvider: e.target.value as LLMProvider })}
            >
              {LLM_PROVIDERS.map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>
              LLM API Key
            </label>
            <input
              type="password"
              style={styles.input}
              value={settings.llmApiKey}
              onChange={(e) => update({ llmApiKey: e.target.value })}
              placeholder={t ? "输入 API Key" : "Enter API Key"}
            />
          </div>

          <div style={styles.configRow}>
            <label style={styles.configLabel}>
              {t ? "LLM 模型" : "LLM Model"}
            </label>
            <input
              type="text"
              style={styles.input}
              value={settings.llmModel}
              onChange={(e) => update({ llmModel: e.target.value })}
              placeholder={t ? "输入模型名称" : "Enter model name"}
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
            <label style={styles.configLabel}>
              {t ? "Embedding 提供商" : "Embedding Provider"}
            </label>
            <select
              style={styles.select}
              value={settings.embeddingProvider}
              onChange={(e) => update({ embeddingProvider: e.target.value as EmbeddingProvider })}
            >
              {EMBEDDING_PROVIDERS.map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>

          {settings.embeddingProvider !== "none" && (
            <div style={styles.configRow}>
              <label style={styles.configLabel}>
                Embedding API Key
              </label>
              <input
                type="password"
                style={styles.input}
                value={settings.embeddingApiKey}
                onChange={(e) => update({ embeddingApiKey: e.target.value })}
                placeholder={t ? "输入 API Key" : "Enter API Key"}
              />
            </div>
          )}
        </div>
      </CollapsibleSection>

      <CollapsibleSection icon="Info" title={t ? "关于" : "About"}>
        <div style={styles.aboutSection}>
          <span style={styles.aboutName}>FeroHa — Dual-Track AI Note IDE</span>
          <span style={styles.aboutMeta}>v3.0.0</span>
          <span style={styles.aboutMeta}>Tauri 2.0 · React 18 · Rust</span>
        </div>
      </CollapsibleSection>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    padding: "16px",
    height: "100%",
    overflowY: "auto",
    color: "var(--text-primary)",
  },
  title: {
    fontSize: "16px",
    fontWeight: 600,
    color: "var(--text-primary)",
    margin: 0,
    paddingBottom: "12px",
    borderBottom: "1px solid var(--border-color)",
  },
  section: {
    borderBottom: "1px solid var(--border-muted)",
  },
  sectionHeader: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "8px 0",
    cursor: "pointer",
    borderBottom: "1px solid var(--border-color)",
    transition: "background var(--transition-fast) var(--easing-smooth)",
    userSelect: "none" as const,
  },
  sectionTitle: {
    fontSize: "13px",
    fontWeight: 600,
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
    padding: "12px 0",
  },
  configGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "12px",
  },
  configRow: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
  },
  configLabel: {
    fontSize: "11px",
    color: "var(--text-secondary)",
    fontWeight: 500,
  },
  select: {
    padding: "6px 10px",
    backgroundColor: "var(--bg-input)",
    border: "1px solid var(--border-color)",
    borderRadius: "4px",
    color: "var(--text-primary)",
    fontSize: "13px",
    outline: "none",
    cursor: "pointer",
    width: "100%",
  },
  input: {
    padding: "6px 10px",
    backgroundColor: "var(--bg-input)",
    border: "1px solid var(--border-color)",
    borderRadius: "4px",
    color: "var(--text-primary)",
    fontSize: "13px",
    outline: "none",
    width: "100%",
    boxSizing: "border-box" as const,
  },
  themeGrid: {
    display: "flex",
    gap: "8px",
    flexWrap: "wrap" as const,
  },
  themeBtn: {
    width: "64px",
    height: "48px",
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
    fontWeight: 500,
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
  aboutSection: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    padding: "8px 0",
  },
  aboutName: {
    fontSize: "14px",
    fontWeight: 600,
    color: "var(--text-primary)",
  },
  aboutMeta: {
    fontSize: "11px",
    color: "var(--text-muted)",
  },
};
