import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  timeout: 45000,
  retries: 1,
  use: {
    baseURL: "http://localhost:1420",
    headless: true,
    screenshot: "only-on-failure",
    trace: "on-first-retry",
  },
  webServer: {
    command: "npm run dev",
    port: 1420,
    reuseExistingServer: !process.env.CI,
    timeout: 180000,
  },
});
