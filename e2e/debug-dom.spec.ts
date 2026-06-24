import { test } from "@playwright/test";

test.describe("Debug - DOM & Console Inspection", () => {
  test("capture DOM tree and console logs", async ({ page }) => {
    const consoleLogs: string[] = [];
    const consoleErrors: string[] = [];

    page.on("console", (msg) => {
      const entry = `[${msg.type()}] ${msg.text()}`;
      consoleLogs.push(entry);
      if (msg.type() === "error") consoleErrors.push(entry);
    });

    page.on("pageerror", (err) => {
      consoleErrors.push(`[pageerror] ${err.message}`);
    });

    await page.goto("http://localhost:1420");
    await page.waitForLoadState("networkidle");
    await page.waitForTimeout(1000);

    // Capture page title and URL
    console.log("\n=== PAGE INFO ===");
    console.log("Title:", await page.title());
    console.log("URL:", page.url());

    // Capture key DOM structure
    const headerText = await page.locator("header").textContent();
    console.log("\n=== HEADER ===");
    console.log(headerText);

    const sidebarText = await page.locator("aside").textContent();
    console.log("\n=== SIDEBAR ===");
    console.log(sidebarText);

    const footerText = await page.locator("footer").textContent();
    console.log("\n=== FOOTER ===");
    console.log(footerText);

    // Capture editor content
    const editorContent = await page.locator(".cm-content").textContent();
    console.log("\n=== EDITOR CONTENT ===");
    console.log(editorContent?.substring(0, 500));

    // Check all visible buttons/tabs
    const buttons = await page.locator("button").allTextContents();
    console.log("\n=== ALL BUTTONS ===");
    console.log(buttons);

    // Check for any error states
    console.log("\n=== CONSOLE LOGS ===");
    consoleLogs.forEach((l) => console.log(l));

    if (consoleErrors.length > 0) {
      console.log("\n=== CONSOLE ERRORS ===");
      consoleErrors.forEach((e) => console.log(e));
    } else {
      console.log("\n=== NO CONSOLE ERRORS ===");
    }
  });

  test("inspect graph and diff views", async ({ page }) => {
    await page.goto("http://localhost:1420");
    await page.waitForLoadState("networkidle");

    // Switch to Graph
    await page.locator('button[aria-controls="panel-graph"]').click();
    await page.waitForTimeout(500);

    const graphPanel = page.locator("#panel-graph");
    const canvasVisible = await graphPanel.isVisible();
    console.log("\n=== GRAPH VIEW ===");
    console.log("Graph panel visible:", canvasVisible);
    if (canvasVisible) {
      const box = await graphPanel.boundingBox();
      console.log("Graph panel size:", box);
    }

    // Switch to Diff on the human face.
    await page.locator('button[aria-label="切换到人类面"]').click();
    await page.locator('button[aria-controls="panel-diff"]').click();
    await page.waitForTimeout(500);

    const diffContent = await page.locator("main").textContent();
    console.log("\n=== DIFF VIEW ===");
    console.log(diffContent?.substring(0, 300));
  });
});
