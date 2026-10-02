// @vitest-environment node
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer, optimizeDeps, resolveConfig, type ViteDevServer } from "vite";
import { expect, it, vi } from "vitest";

const configFile = resolve("vite.config.ts");

it("repairs a previously valid Home-only cache before lazy Settings and editor dependencies load", async () => {
  const cacheDir = mkdtempSync(join(tmpdir(), "video-creater-vite-cache-policy-"));
  try {
    const legacy = await resolveConfig({ configFile, cacheDir, logLevel: "silent", optimizeDeps: { entries: ["src/components/home/project-home.tsx"] } }, "serve");
    legacy.optimizeDeps.entries = ["src/components/home/project-home.tsx"];
    legacy.optimizeDeps.include = ["react", "react-dom", "react/jsx-dev-runtime", "react/jsx-runtime"];
    const partial = await optimizeDeps(legacy, true);
    expect(partial.optimized["@radix-ui/react-dialog"]).toBeUndefined();
    const current = await resolveConfig({ configFile, cacheDir, logLevel: "silent" }, "serve");
    const recovered = await optimizeDeps(current, false);
    expect(recovered.optimized["@radix-ui/react-dialog"]).toBeDefined();
    expect(recovered.optimized["react-dom/client"]).toBeDefined();
    expect(recovered.optimized["zustand/vanilla"]).toBeDefined();
  } finally { rmSync(cacheDir, { recursive: true, force: true }); }
});

it("watches the app entry while excluding generated browser trace HTML", async () => {
  const root = mkdtempSync(join(tmpdir(), "video-creater-vite-watch-policy-"));
  mkdirSync(join(root, "output", "traces"), { recursive: true });
  writeFileSync(join(root, "index.html"), "<!doctype html><title>App fixture</title>");
  writeFileSync(join(root, "output", "traces", "fixture-trace.html"), "<!doctype html><title>Generated trace</title>");
  let server: ViteDevServer | undefined;
  let ready!: () => void;
  const watcherReady = new Promise<void>((resolveReady) => { ready = resolveReady; });
  try {
    server = await createServer({
      configFile, root, cacheDir: join(root, "cache"), logLevel: "silent",
      optimizeDeps: { noDiscovery: true, include: [] },
      plugins: [{ name: "watcher-policy-readiness", configureServer(server) { server.watcher.once("ready", ready); } }],
    });
    await watcherReady;
    await vi.waitFor(() => expect(Object.values(server!.watcher.getWatched()).flat()).toContain("index.html"));
    const watched = Object.values(server.watcher.getWatched()).flat();
    expect(watched).toContain("index.html");
    expect(watched).not.toContain("fixture-trace.html");
  } finally { await server?.close(); rmSync(root, { recursive: true, force: true }); }
});
