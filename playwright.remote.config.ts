import { defineConfig } from "@playwright/test";

const port = Number(process.env.VIDEO_CREATER_REMOTE_E2E_PORT ?? 4788);

export default defineConfig({
  testDir: "./e2e",
  testMatch: "remote-host*.spec.ts",
  outputDir: "output/playwright/remote-host",
  fullyParallel: false,
  workers: 1,
  reporter: [["line"]],
  timeout: 120_000,
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "node scripts/remote-host-e2e.mjs",
    url: `http://127.0.0.1:${port}/healthz`,
    reuseExistingServer: false,
    timeout: 180_000,
  },
});
