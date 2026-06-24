import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPanel from "../SettingsPanel";
import { useAppStore } from "../../hooks/useAppStore";
import { useSettingsStore } from "../../hooks/useSettings";

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

describe("SettingsPanel", () => {
  beforeEach(() => {
    localStorageMock.clear();
    useAppStore.setState({ mode: "ai", activePanel: "settings" });
    useSettingsStore.setState({
      settings: {
        language: "zh",
        theme: "classic",
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

  it("lists shortcut help without registering panel-switch shortcuts", () => {
    render(<SettingsPanel />);

    fireEvent.keyDown(window, { key: "2", ctrlKey: true });

    expect(screen.getByText("快捷键")).toBeDefined();
    expect(useAppStore.getState().mode).toBe("ai");
    expect(useAppStore.getState().activePanel).toBe("settings");
  });

  it("offers a real classic theme value instead of the legacy mocha alias", () => {
    render(<SettingsPanel />);

    fireEvent.click(screen.getByTitle("Classic"));

    expect(useSettingsStore.getState().settings.theme).toBe("classic");
  });

  it("keeps product and framework labels out of the visible settings surface", () => {
    render(<SettingsPanel />);

    expect(screen.queryByText(/FeroHa - Dual-Track/)).toBeNull();
    expect(screen.queryByText(/Tauri 2\.0/)).toBeNull();
    expect(screen.queryByTitle("FeroHa")).toBeNull();
    expect(screen.getByTitle("深绿")).toBeDefined();
  });

  it("offers an explicit save-and-debug action for API settings", () => {
    render(<SettingsPanel />);

    expect(screen.getByRole("button", { name: /保存并调试 API|Save and debug API/ })).toBeDefined();
  });
});
