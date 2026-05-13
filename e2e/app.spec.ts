// End-to-end tests with Playwright
// Stage 7: Test the full user workflow
//
// npx playwright test

import { test, expect } from "@playwright/test";

test.describe("Dual-Track Note IDE — E2E Tests", () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the app
    await page.goto("http://localhost:1420");
  });

  test("app loads and shows editor", async ({ page }) => {
    // Verify the app loads
    await expect(page.getByText("◈ Dual-Track Note IDE")).toBeVisible();

    // Editor should be rendered
    const editor = page.locator(".cm-content");
    await expect(editor).toBeVisible();
  });

  test("CLI bar activates on / keypress", async ({ page }) => {
    // Click the editor to focus
    await page.locator(".cm-content").click();

    // Type / to activate CLI
    await page.keyboard.press("/");

    // CLI input should appear
    const cliInput = page.locator("footer input");
    await expect(cliInput).toBeVisible();
  });

  test("can type and view word count in editor", async ({ page }) => {
    const editor = page.locator(".cm-content");
    await editor.click();
    await page.keyboard.type("Hello world! This is a test note.");

    // Word count should update
    const wordCount = page.getByText(/words/);
    await expect(wordCount).toBeVisible();
  });

  test("vault browser shows demo notes", async ({ page }) => {
    // Sidebar should be visible with note entries
    const sidebar = page.locator("aside");
    await expect(sidebar).toBeVisible();

    // Should show demo notes in browser mode
    const noteEntry = sidebar.getByText("Welcome", { exact: true });
    await expect(noteEntry).toBeVisible();
  });

  test("tab navigation works", async ({ page }) => {
    // Click Graph tab
    await page.getByText("Graph", { exact: true }).click();

    // Graph canvas should appear
    const canvas = page.locator("canvas");
    await expect(canvas).toBeVisible();

    // Switch to Diff tab
    await page.getByText("Diff", { exact: true }).click();

    // Diff view should load
    await expect(page.getByText("Pending")).toBeVisible();
  });
});
