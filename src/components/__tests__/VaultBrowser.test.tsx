// Test: VaultBrowser component
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import VaultBrowser from "../VaultBrowser";

// Mock window.prompt (still used by handleCreateFolder)
const mockPrompt = vi.spyOn(window, "prompt");

describe("VaultBrowser", () => {
  it("renders with Vault title and open button", () => {
    render(<VaultBrowser vaultPath={null} onSelectVault={vi.fn()} isTauri={false} />);
    
    expect(screen.getByText("Vault")).toBeDefined();
    expect(screen.getByTitle("Open Vault")).toBeDefined();
  });

  it("shows empty state when no notes in browser mode", () => {
    render(<VaultBrowser vaultPath={null} onSelectVault={vi.fn()} isTauri={false} />);
    
    // In browser mode with null vaultPath, no actions or file list content
    expect(screen.getByText("Vault")).toBeDefined();
    expect(screen.queryByText("No .md files found")).toBeNull(); // file list hidden without vaultPath
  });

  it("displays actions with sort dropdown when vault is open", () => {
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);
    
    expect(screen.getByText("/test-vault")).toBeDefined();
    expect(screen.getByTitle("New Note")).toBeDefined();
    expect(screen.getByTitle("New Folder")).toBeDefined();
    expect(screen.getByTitle("Sort order")).toBeDefined();
    expect(screen.getByTitle("Refresh")).toBeDefined();
  });

  it("opens template picker when new note button clicked", () => {
    const onSelectVault = vi.fn();
    
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={onSelectVault} isTauri={false} />);
    
    fireEvent.click(screen.getByTitle("New Note"));
    expect(screen.getByText("New Note — Choose Template")).toBeDefined();
    expect(screen.getByText(/Blank Note/)).toBeDefined();
  });

  it("creates a new folder when prompted", () => {
    mockPrompt.mockReturnValue("test-folder");
    
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);
    
    fireEvent.click(screen.getByTitle("New Folder"));
    expect(mockPrompt).toHaveBeenCalledWith("New folder name:");
  });

  it("sort dropdown shows all sort options", () => {
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);
    
    const sortSelect = screen.getByTitle("Sort order") as HTMLSelectElement;
    expect(sortSelect.value).toBe("title-asc");
    
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(4);
    expect(options[0].textContent).toBe("Title A-Z");
    expect(options[1].textContent).toBe("Title Z-A");
    expect(options[2].textContent).toBe("Modified (newest)");
    expect(options[3].textContent).toBe("Modified (oldest)");
    
    fireEvent.change(sortSelect, { target: { value: "title-desc" } });
    expect(sortSelect.value).toBe("title-desc");
  });
});
