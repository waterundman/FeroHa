import { test, expect } from "@playwright/test";

test.describe("Debug - Visual Inspection", () => {
  test("capture full app state", async ({ page }) => {
    await page.goto("http://localhost:1420");
    await page.waitForLoadState("networkidle");

    // Screenshot 1: Initial load
    await page.screenshot({ path: "debug/01-initial-load.png", fullPage: true });

    // Check header
    const header = page.locator("header");
    await expect(header).toBeVisible();
    console.log("Header is visible");

    // Check sidebar
    const sidebar = page.locator("aside");
    await expect(sidebar).toBeVisible();
    console.log("Sidebar is visible");

    // Screenshot 2: Editor panel
    const editor = page.locator(".cm-content");
    await expect(editor).toBeVisible();
    await editor.click();
    await page.screenshot({ path: "debug/02-editor-focused.png", fullPage: true });
    console.log("Editor is visible and focused");

    // Type some text
    await page.keyboard.type("# Hello Bayes\n\nThis is a test note with **bold** and _italic_ text.\n\n- Item 1\n- Item 2\n- Item 3");
    await page.waitForTimeout(500);
    await page.screenshot({ path: "debug/03-editor-with-content.png", fullPage: true });
    console.log("Content typed into editor");

    // Screenshot 4: CLI bar activation
    await page.keyboard.press("/");
    await page.waitForTimeout(300);
    const cliInput = page.locator("footer input");
    if (await cliInput.isVisible()) {
      await page.screenshot({ path: "debug/04-cli-bar-active.png", fullPage: true });
      console.log("CLI bar activated");
    }

    // Close CLI
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);

    // Screenshot 5: Graph tab
    await page.getByText("Graph", { exact: true }).click();
    await page.waitForTimeout(500);
    await page.screenshot({ path: "debug/05-graph-view.png", fullPage: true });
    console.log("Graph view rendered");

    // Screenshot 6: Diff tab
    await page.getByText("Diff", { exact: true }).click();
    await page.waitForTimeout(500);
    await page.screenshot({ path: "debug/06-diff-view.png", fullPage: true });
    console.log("Diff view rendered");

    // Check sidebar content
    const sidebarText = await sidebar.textContent();
    console.log("Sidebar content:", sidebarText?.substring(0, 200));
  });

  test("check editor functionality", async ({ page }) => {
    await page.goto("http://localhost:1420");
    await page.waitForLoadState("networkidle");

    const editor = page.locator(".cm-content");
    await editor.click();

    // Test typing
    await page.keyboard.type("Testing word count");
    await page.waitForTimeout(300);

    // Check word count
    const wordCount = page.getByText(/words/);
    if (await wordCount.isVisible()) {
      const countText = await wordCount.textContent();
      console.log("Word count:", countText);
    }

    await page.screenshot({ path: "debug/07-word-count.png", fullPage: true });
  });
});
