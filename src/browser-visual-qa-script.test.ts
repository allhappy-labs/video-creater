import { createServer } from "node:net";
import { execFile, spawn, type ChildProcess } from "node:child_process";
import { join } from "node:path";
import { existsSync } from "node:fs";
import { once } from "node:events";
import { pathToFileURL } from "node:url";
import { describe, expect, it, vi } from "vitest";

import {
  startViteServer,
  stopViteServer,
  settingsVisualQaScenarios,
  visualQaScenarios,
// @ts-expect-error The visual harness is an executable ESM script with test-only exports.
} from "../scripts/browser-visual-qa.mjs";
import { settingsVisualQaFixtures } from "./lib/settings-visual-qa-fixtures";

const repoRoot = process.cwd();

async function reapTestChild(child?: ChildProcess): Promise<void> {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  const closed = once(child, "close");
  child.kill("SIGKILL");
  await closed;
}

describe("browser visual QA automation", () => {
  it("refuses an occupied default target instead of using another server", async () => {
    const occupiedServer = createServer();
    await new Promise<void>((resolve, reject) => {
      occupiedServer.once("error", reject);
      occupiedServer.listen(0, "127.0.0.1", resolve);
    });
    try {
      const address = occupiedServer.address();
      if (!address || typeof address === "string") throw new Error("Expected TCP address");
      const visualScriptUrl = pathToFileURL(join(repoRoot, "scripts/browser-visual-qa.mjs")).href;
      const failure = await new Promise<{ error: Error | null; stderr: string }>((resolve) => {
        execFile(process.execPath, [
          "--input-type=module", "-e",
          "const { startViteServer } = await import(process.argv[1]); await startViteServer(process.argv[2])",
          visualScriptUrl, `http://127.0.0.1:${address.port}`,
        ], { timeout: 2_000 }, (error, _stdout, stderr) => resolve({ error, stderr }));
      });
      expect(failure.error).toBeTruthy();
      expect(failure.stderr).toMatch(/already in use/i);
    } finally {
      await new Promise<void>((resolve, reject) => occupiedServer.close((error) => error ? reject(error) : resolve()));
    }
  });

  it("rejects an occupied target before launching the Vite child", async () => {
    const occupiedServer = createServer();
    await new Promise<void>((resolve, reject) => {
      occupiedServer.once("error", reject);
      occupiedServer.listen(0, "127.0.0.1", resolve);
    });
    const spawnChild = vi.fn(() => { throw new Error("Vite child must not be launched"); });
    try {
      const address = occupiedServer.address();
      if (!address || typeof address === "string") throw new Error("Expected TCP address");
      await expect(startViteServer(`http://127.0.0.1:${address.port}`, { spawnChild })).rejects.toThrow(/already in use/i);
      expect(spawnChild).not.toHaveBeenCalled();
    } finally {
      await new Promise<void>((resolve, reject) => occupiedServer.close((error) => error ? reject(error) : resolve()));
    }
  });

  it("reaps a starting child before reporting its readiness timeout", async () => {
    let child: ChildProcess | undefined;
    try {
      await expect(startViteServer("http://127.0.0.1:0", {
        startupTimeoutMs: 100,
        spawnChild: () => (child = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { stdio: "ignore" })),
      })).rejects.toThrow("Timed out starting");
      expect(child).toBeDefined();
      expect(child!.exitCode !== null || child!.signalCode !== null).toBe(true);
    } finally { await reapTestChild(child); }
  });

  it("waits for forced termination of a child that ignores graceful shutdown", async () => {
    let child: ChildProcess | undefined;
    let server: Awaited<ReturnType<typeof startViteServer>> | undefined;
    try {
      server = await startViteServer("http://127.0.0.1:0", {
        shutdownTimeoutMs: 25,
        spawnChild: (_command: string, args: string[]) => (child = spawn(process.execPath, ["-e",
          "process.on('SIGTERM',()=>{}); require('node:fs').writeFileSync(process.argv[1],'ready'); setInterval(()=>{},1000)",
          args[3]!,
        ], { stdio: "ignore" })),
      });
      await stopViteServer(server);
      expect(child?.signalCode).toBe("SIGKILL");
      expect(existsSync(server.readyPath)).toBe(false);
    } finally { await reapTestChild(child); await stopViteServer(server); }
  });

  it("handles child spawn failure without leaving an unhandled process error", async () => {
    await expect(startViteServer("http://127.0.0.1:0", {
      spawnChild: () => spawn("/nonexistent-video-creater-visual-qa-child", [], { stdio: "ignore" }),
    })).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("maps every default Settings scenario to a current fixture and surface", () => {
    const defaultSettings = visualQaScenarios.filter(
      (scenario: { surface: string }) =>
        scenario.surface === "settings" || scenario.surface === "settings-state",
    );
    expect(defaultSettings.map((scenario: { id?: string }) => scenario.id)).toEqual([
      "settings-desktop",
      "settings-narrow",
    ]);
    expect(visualQaScenarios).not.toContainEqual(
      expect.objectContaining({ fixtureId: "settings-model-missing" }),
    );
    expect(defaultSettings).not.toContainEqual(
      expect.objectContaining({ surface: "settings" }),
    );
    for (const scenario of defaultSettings) {
      const fixture = settingsVisualQaFixtures[
        scenario.fixtureId as keyof typeof settingsVisualQaFixtures
      ];
      expect(fixture).toBeDefined();
      expect(scenario.fixtureSurface).toBe(fixture.surface);
      expect(scenario.category).toBe(fixture.category);
      expect(scenario.projectContext).toBe(fixture.projectContext);
      expect(settingsVisualQaScenarios).toContainEqual(
        expect.objectContaining({
          fixtureId: scenario.fixtureId,
          surface: scenario.surface,
          viewport: scenario.viewport,
        }),
      );
    }
  });
});
