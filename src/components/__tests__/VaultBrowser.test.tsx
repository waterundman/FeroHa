// Test: VaultBrowser component
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import VaultBrowser from "../VaultBrowser";

// Mock window.prompt
const mockPrompt = vi.spyOn(window, "prompt");

describe("VaultBrowser", () => {
  it("renders demo notes in browser mode", () => {
    render(<VaultBrowser vaultPath={null} onSelectVault={vi.fn()} isTauri={false} />);
    
    expect(screen.getByText("Welcome")).toBeDefined();
    expect(screen.getByText("Architecture")).toBeDefined();
    expect(screen.getByText("Dual Track")).toBeDefined();
    expect(screen.getByText("LLM Internals")).toBeDefined();
  });

  it("shows Vault title and icon buttons", () => {
    render(<VaultBrowser vaultPath={null} onSelectVault={vi.fn()} isTauri={false} />);
    
    expect(screen.getByText("Vault")).toBeDefined();
    expect(screen.getByTitle("Open Vault")).toBeDefined();
  });

  it("displays file tree when vault is open", () => {
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);
    
    expect(screen.getByText("/test-vault")).toBeDefined();
    expect(screen.getByTitle("New Note")).toBeDefined();
    expect(screen.getByTitle("New Folder")).toBeDefined();
    expect(screen.getByTitle("Sort")).toBeDefined();
  });

  it("creates a new note when prompted", () => {
    mockPrompt.mockReturnValue("test-note.md");
    const onSelectVault = vi.fn();
    
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={onSelectVault} isTauri={false} />);
    
    fireEvent.click(screen.getByTitle("New Note"));
    expect(mockPrompt).toHaveBeenCalledWith("New note name (e.g., my-note.md):");
  });

  it("creates a new folder when prompted", () => {
    mockPrompt.mockReturnValue("test-folder");
    
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);
    
    fireEvent.click(screen.getByTitle("New Folder"));
    expect(mockPrompt).toHaveBeenCalledWith("New folder name:");
  });

  it("sorts notes when sort button is clicked", () => {
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);
    
    const sortBtn = screen.getByTitle("Sort");
    fireEvent.click(sortBtn);
    
    // Verify notes are sorted (Welcome should be first after sort)
    const notes = screen.getAllByText(/Welcome|Architecture|Dual Track|LLM Internals/);
    expect(notes[0].textContent).toBe("Architecture");
  });
});
