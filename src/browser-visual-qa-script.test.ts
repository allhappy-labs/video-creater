import { createServer } from "node:net";
import { execFile } from "node:child_process";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";

import {
  settingsVisualQaScenarios,
  visualQaScenarios,
// @ts-expect-error The visual harness is an executable ESM script with test-only exports.
} from "../scripts/browser-visual-qa.mjs";
import { settingsVisualQaFixtures } from "./lib/settings-visual-qa-fixtures";

const repoRoot = process.cwd();

describe("browser visual QA automation", () => {
  it("refuses an occupied default target instead of using another server", async () => {
    const occupiedServer = createServer();
    await new Promise<void>((resolve, reject) => {
      occupiedServer.once("error", reject);
      occupiedServer.listen(0, "127.0.0.1", resolve);
    });
    const address = occupiedServer.address();
    if (!address || typeof address === "string") {
      occupiedServer.close();
      throw new Error("Test server did not bind a TCP port");
    }

    const visualScriptUrl = pathToFileURL(
      join(repoRoot, "scripts/browser-visual-qa.mjs"),
    ).href;
    const failure = await new Promise<{ error: Error | null; stderr: string }>((resolve) => {
      execFile(
        process.execPath,
        [
          "--input-type=module",
          "-e",
          "const { startViteServer } = await import(process.argv[1]); await startViteServer(process.argv[2])",
          visualScriptUrl,
          `http://127.0.0.1:${address.port}`,
        ],
        (error, _stdout, stderr) => resolve({ error, stderr }),
      );
    });
    expect(failure.error).toBeTruthy();
    expect(failure.stderr).toMatch(/already in use/i);
    await new Promise<void>((resolve, reject) => {
      occupiedServer.close((error) => error ? reject(error) : resolve());
    });
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
