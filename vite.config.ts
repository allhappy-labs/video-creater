import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

function productionManualChunks(moduleId: string): string | undefined {
  const id = moduleId.replace(/\\/g, "/");
  if (!id.includes("/node_modules/")) return undefined;
  if (
    id.includes("/node_modules/react/") ||
    id.includes("/node_modules/react-dom/") ||
    id.includes("/node_modules/scheduler/")
  ) {
    return "vendor-react";
  }
  if (id.includes("/node_modules/lucide-react/")) return "vendor-icons";
  if (id.includes("/node_modules/@tauri-apps/")) return "vendor-tauri";
  return "vendor";
}

export default defineConfig({
  plugins: [react()],
  publicDir: path.resolve(__dirname, "./src-tauri/resources/sample-project"),
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    // Cargo's build output holds hundreds of thousands of files; watching it exhausts inotify watchers
    // and crashes the dev server (and the Playwright web server) mid-run.
    watch: { ignored: ["**/src-tauri/target/**"] },
  },
  envPrefix: ["VITE_", "TAURI_"],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks: productionManualChunks,
      },
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    setupFiles: ["src/test-utils/jsdom-setup.ts"],
  },
});
