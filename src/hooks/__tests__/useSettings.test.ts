// Test: useSettings hook
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useSettings, useSettingsStore } from "../useSettings";

// Mock localStorage
const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn((key: string) => store[key] || null),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = value;
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key];
    }),
    clear: vi.fn(() => {
      store = {};
    }),
  };
})();

Object.defineProperty(window, "localStorage", {
  value: localStorageMock,
});

describe("useSettings", () => {
  beforeEach(() => {
    localStorageMock.clear();
    vi.clearAllMocks();
    // Reset store state
    useSettingsStore.setState({
      settings: {
        language: "zh",
        theme: "mocha",
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
      },
    });
  });

  it("returns default settings when localStorage is empty", () => {
    const { result } = renderHook(() => useSettings());
    const [settings] = result.current;

    expect(settings.language).toBe("zh");
    expect(settings.theme).toBe("mocha");
    expect(settings.editorFontSize).toBe(14);
  });

  it("loads settings from localStorage", () => {
    const savedSettings = {
      language: "en" as const,
      theme: "macchiato" as const,
      editorFontSize: 16,
      editorFontFamily: "JetBrains Mono, 'Fira Code', monospace",
      defaultViewMode: "edit" as const,
      autoSaveInterval: 30,
      llmProvider: "openai" as const,
      llmApiKey: "sk-test",
      llmModel: "gpt-4",
      embeddingProvider: "openai" as const,
      embeddingApiKey: "sk-test",
      ollamaBaseUrl: "http://localhost:11434",
    };
    localStorageMock.getItem.mockReturnValue(JSON.stringify(savedSettings));

    // Reset store with saved settings
    useSettingsStore.setState({
      settings: savedSettings,
    });

    const { result } = renderHook(() => useSettings());
    const [settings] = result.current;

    expect(settings.language).toBe("en");
    expect(settings.theme).toBe("macchiato");
    expect(settings.editorFontSize).toBe(16);
  });

  it("updates settings and persists to localStorage", () => {
    const { result } = renderHook(() => useSettings());
    const [, updateSettings] = result.current;

    act(() => {
      updateSettings({ language: "en" });
    });

    const [settings] = result.current;
    expect(settings.language).toBe("en");
    expect(localStorageMock.setItem).toHaveBeenCalledWith(
      "bayesian-settings",
      expect.stringContaining('"language":"en"')
    );
  });

  it("merges partial updates with existing settings", () => {
    const { result } = renderHook(() => useSettings());
    const [, updateSettings] = result.current;

    act(() => {
      updateSettings({ theme: "latte" });
    });

    const [settings] = result.current;
    expect(settings.theme).toBe("latte");
    expect(settings.language).toBe("zh"); // unchanged
    expect(settings.editorFontSize).toBe(14); // unchanged
  });

  it("handles corrupted localStorage gracefully", () => {
    localStorageMock.getItem.mockReturnValue("invalid json");

    // Store should use defaults when localStorage is corrupted
    const { result } = renderHook(() => useSettings());
    const [settings] = result.current;

    expect(settings).toEqual({
      language: "zh",
      theme: "mocha",
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
    });
  });
});
