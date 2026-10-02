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
    watch: { ignored: ["**/src-tauri/target/**", "**/output/**"] },
  },
  // Only the real app is a scan entry; generated HTML is not executable app source.
  // Eager coverage keeps a valid Home-only cache from adding another React bundle on navigation.
  optimizeDeps: {
    entries: ["index.html"],
    include: [
      "react-dom/client",
      "@radix-ui/react-context-menu", "@radix-ui/react-dialog",
      "@radix-ui/react-dropdown-menu", "@radix-ui/react-popover",
      "@radix-ui/react-select", "@radix-ui/react-slider", "@radix-ui/react-switch",
      "@radix-ui/react-tabs", "@radix-ui/react-toast", "@radix-ui/react-toggle-group",
      "@radix-ui/react-tooltip",
      "@tauri-apps/api/core", "@tauri-apps/api/event", "@tauri-apps/api/webview",
      "@tauri-apps/plugin-dialog", "zustand", "zustand/vanilla",
    ],
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
