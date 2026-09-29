import { defineConfig } from "@playwright/test";

const port = 4178;

export default defineConfig({
  testDir: "./e2e",
  testIgnore: "remote-host*.spec.ts",
  outputDir: "output/playwright/app-smoke",
  fullyParallel: false,
  workers: 1,
  reporter: [["line"]],
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: `pnpm exec vite --host 127.0.0.1 --port ${port} --strictPort`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
