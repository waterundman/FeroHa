import { useSettings, Language, ThemeName } from "../hooks/useSettings";

const LANGUAGES: { value: Language; label: string }[] = [
  { value: "zh", label: "中文" },
  { value: "en", label: "English" },
];

const THEMES: { value: ThemeName; label: string; color: string }[] = [
  { value: "mocha", label: "Mocha", color: "#1e1e2e" },
  { value: "macchiato", label: "Macchiato", color: "#24273a" },
  { value: "frappe", label: "Frappé", color: "#303446" },
  { value: "latte", label: "Latte", color: "#eff1f5" },
];

export default function SettingsPanel() {
  const [settings, update] = useSettings();

  return (
    <div style={styles.container}>
      <h2 style={styles.title}>
        {settings.language === "zh" ? "设置" : "Settings"}
      </h2>

      {/* Language */}
      <div style={styles.section}>
        <label style={styles.label}>
          {settings.language === "zh" ? "语言" : "Language"}
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

      {/* Theme */}
      <div style={styles.section}>
        <label style={styles.label}>
          {settings.language === "zh" ? "主题" : "Theme"}
        </label>
        <div style={styles.themeGrid}>
          {THEMES.map((t) => (
            <button
              key={t.value}
              style={{
                ...styles.themeBtn,
                backgroundColor: t.color,
                border:
                  settings.theme === t.value
                    ? "2px solid #89b4fa"
                    : "2px solid #313244",
              }}
              onClick={() => update({ theme: t.value })}
              title={t.label}
            >
              <span
                style={{
                  ...styles.themeLabel,
                  color:
                    t.value === "latte" ? "#4c4f69" : "#cdd6f4",
                }}
              >
                {t.label}
              </span>
            </button>
          ))}
        </div>
      </div>

      {/* Editor font size */}
      <div style={styles.section}>
        <label style={styles.label}>
          {settings.language === "zh" ? "编辑器字号" : "Editor Font Size"}
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
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    gap: "20px",
    padding: "20px",
    height: "100%",
  },
  title: {
    fontSize: "16px",
    fontWeight: 600,
    color: "#cdd6f4",
    margin: 0,
    paddingBottom: "12px",
    borderBottom: "1px solid #313244",
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
  },
  label: {
    fontSize: "12px",
    fontWeight: 500,
    color: "#a6adc8",
    textTransform: "uppercase" as const,
    letterSpacing: "0.5px",
  },
  select: {
    padding: "6px 10px",
    backgroundColor: "#313244",
    border: "1px solid #45475a",
    borderRadius: "4px",
    color: "#cdd6f4",
    fontSize: "13px",
    outline: "none",
    cursor: "pointer",
    width: "180px",
  },
  themeGrid: {
    display: "flex",
    gap: "8px",
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
    transition: "border-color 0.15s",
  },
  themeLabel: {
    fontSize: "10px",
    fontWeight: 500,
  },
  sliderRow: {
    display: "flex",
    alignItems: "center",
    gap: "12px",
  },
  slider: {
    flex: 1,
    maxWidth: "200px",
    accentColor: "#89b4fa",
    cursor: "pointer",
  },
  sliderValue: {
    fontSize: "12px",
    color: "#a6adc8",
    fontFamily: "monospace",
    minWidth: "36px",
  },
};
