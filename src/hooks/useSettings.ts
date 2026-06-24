import { create } from "zustand";

export type Language = "zh" | "en";
export type ThemeName = "feroha" | "classic" | "macchiato" | "frappe" | "latte";
export type LLMProvider = "gemini" | "openai" | "deepseek" | "anthropic" | "ollama";
export type EmbeddingProvider = "gemini" | "openai" | "none";
export type EditorViewMode = "edit" | "preview";

export interface Settings {
  language: Language;
  theme: ThemeName;
  editorFontSize: number;
  editorFontFamily: string;
  defaultViewMode: EditorViewMode;
  autoSaveInterval: number;
  llmProvider: LLMProvider;
  llmApiKey: string;
  llmModel: string;
  embeddingProvider: EmbeddingProvider;
  embeddingApiKey: string;
  ollamaBaseUrl: string;
}

export interface ApiDebugSuccessDetail {
  provider: string;
  model: string;
  latencyMs: number;
}

export interface ApiDebugResult {
  ok: boolean;
  provider: string;
  model: string;
  latency_ms?: number;
  message?: string;
  error?: string;
}

interface SettingsStore {
  settings: Settings;
  updateSettings: (patch: Partial<Settings>) => void;
}

const DEFAULT_SETTINGS: Settings = {
  language: "zh",
  theme: "feroha",
  editorFontSize: 14,
  editorFontFamily: "JetBrains Mono, 'Fira Code', monospace",
  defaultViewMode: "edit",
  autoSaveInterval: 30,
  llmProvider: "gemini",
  llmApiKey: "",
  llmModel: "gemini-pro",
  embeddingProvider: "none",
  embeddingApiKey: "",
  ollamaBaseUrl: "http://localhost:11434",
};

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem("bayesian-settings");
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<Settings>;
      return { ...DEFAULT_SETTINGS, ...parsed, theme: normalizeTheme(parsed.theme) };
    }
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

function settingsToBackendConfig(settings: Settings): Record<string, unknown> {
  return {
    llm_provider: settings.llmProvider,
    llm_api_key: settings.llmApiKey,
    llm_model: settings.llmModel,
    embedding_provider: settings.embeddingProvider,
    embedding_api_key: settings.embeddingApiKey,
    theme: settings.theme,
    ollama_base_url: settings.ollamaBaseUrl,
  };
}

function backendConfigToSettingsPatch(config: Record<string, unknown>): Partial<Settings> {
  const patch: Partial<Settings> = {};
  if (typeof config.llm_provider === "string") patch.llmProvider = config.llm_provider as LLMProvider;
  if (typeof config.llm_api_key === "string") patch.llmApiKey = config.llm_api_key;
  if (typeof config.llm_model === "string") patch.llmModel = config.llm_model;
  if (typeof config.embedding_provider === "string") patch.embeddingProvider = config.embedding_provider as EmbeddingProvider;
  if (typeof config.embedding_api_key === "string") patch.embeddingApiKey = config.embedding_api_key;
  if (typeof config.theme === "string") patch.theme = normalizeTheme(config.theme);
  return patch;
}

export function shouldAutoDebugApiPatch(patch: Partial<Settings>): boolean {
  return Boolean(
    patch.llmProvider !== undefined ||
    patch.llmApiKey !== undefined ||
    patch.llmModel !== undefined ||
    patch.ollamaBaseUrl !== undefined
  );
}

export function emitApiDebugSuccessEffect(detail: ApiDebugSuccessDetail): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(
    new CustomEvent<ApiDebugSuccessDetail>("feroha:api-debug-success", {
      detail: {
        provider: detail.provider,
        model: detail.model,
        latencyMs: detail.latencyMs,
      },
    })
  );
}

function isThemeName(value: string): value is ThemeName {
  return ["feroha", "classic", "macchiato", "frappe", "latte"].includes(value);
}

function normalizeTheme(value: unknown): ThemeName {
  if (value === "mocha") return "classic";
  if (typeof value === "string" && isThemeName(value)) return value;
  return DEFAULT_SETTINGS.theme;
}

export async function saveSettingsToBackend(settings: Settings): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("set_config", { config: settingsToBackendConfig(settings) });
}

export async function saveAndDebugApiSettings(settings: Settings): Promise<ApiDebugResult> {
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("set_config", { config: settingsToBackendConfig(settings) });
  const result = await invoke<ApiDebugResult>("debug_llm_config");
  if (result.ok) {
    emitApiDebugSuccessEffect({
      provider: result.provider,
      model: result.model,
      latencyMs: result.latency_ms ?? 0,
    });
  }
  return result;
}

function syncToBackend(settings: Settings) {
  saveSettingsToBackend(settings).catch(() => {});
}

export async function loadSettingsFromBackend(): Promise<void> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const rawConfig = await invoke("get_config");
    const config = rawConfig as Record<string, unknown>;
    const patch = backendConfigToSettingsPatch(config);
    if (Object.keys(patch).length > 0) {
      const state = useSettingsStore.getState();
      const merged = { ...state.settings, ...patch };
      useSettingsStore.setState({ settings: merged });
      persistSettings(merged);
    }
  } catch {
    // best-effort
  }
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  settings: loadSettings(),
  updateSettings: (patch) =>
    set((state) => {
      const next = { ...state.settings, ...patch };
      persistSettings(next);
      syncToBackend(next);
      return { settings: next };
    }),
}));

export function useSettings(): [Settings, (patch: Partial<Settings>) => void] {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  return [settings, updateSettings];
}
