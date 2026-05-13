import { create } from "zustand";

export type Language = "zh" | "en";
export type ThemeName = "mocha" | "macchiato" | "frappe" | "latte";

export interface Settings {
  language: Language;
  theme: ThemeName;
  editorFontSize: number;
}

interface SettingsStore {
  settings: Settings;
  updateSettings: (patch: Partial<Settings>) => void;
}

const DEFAULT_SETTINGS: Settings = {
  language: "zh",
  theme: "mocha",
  editorFontSize: 14,
};

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem("bayesian-settings");
    if (raw) return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
    // corrupted storage — fall back to defaults
  }
  return DEFAULT_SETTINGS;
}

function persistSettings(settings: Settings) {
  try {
    localStorage.setItem("bayesian-settings", JSON.stringify(settings));
  } catch {
    // storage full or blocked — silent fail
  }
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  settings: loadSettings(),
  updateSettings: (patch) =>
    set((state) => {
      const next = { ...state.settings, ...patch };
      persistSettings(next);
      return { settings: next };
    }),
}));

export function useSettings(): [Settings, (patch: Partial<Settings>) => void] {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  return [settings, updateSettings];
}
