import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const fixtureModulePattern = /(?:^|\/)(?:fixtures(?:\/|$)|fixture-bootstrap$)/;
const staticImportPattern = /^\s*(?:import|export)\b[^;]*?\bfrom\s+["']([^"']+)["']/gm;
const dynamicImportPattern = /\bimport\(\s*["']([^"']+)["']\s*\)/g;

function productionSourceFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productionSourceFiles(path);
    return /\.(?:ts|tsx)$/.test(entry.name) && !/\.(?:test|spec)\.(?:ts|tsx)$/.test(entry.name) ? [path] : [];
  });
}

/** Files that only load inside the DEV-gated fixture chunk. */
function insideFixtureChunk(path: string): boolean {
  return path.startsWith("src/lib/runtime/fixtures/") || path === "src/lib/runtime/fixture-bootstrap.ts";
}

describe("production chunk policy", () => {
  it("does not manually split mutually dependent first-party editor modules", () => {
    const config = readFileSync("vite.config.ts", "utf8");
    expect(config).not.toContain('return "app-timeline"');
    expect(config).not.toContain('return "app-editor"');
    expect(config).toContain('return "vendor-react"');
    expect(config).toContain('return "vendor-tauri"');
    expect(config).toContain("manualChunks: productionManualChunks");
    expect(config).not.toContain("chunkSizeWarningLimit");
    expect(config).not.toContain("onlyExplicitManualChunks");
  });

  it("keeps fixture handlers out of production bundles behind the DEV-only dynamic import", () => {
    const staticFixtureImports = productionSourceFiles("src")
      .filter((path) => !insideFixtureChunk(path))
      .flatMap((path) =>
        [...readFileSync(path, "utf8").matchAll(staticImportPattern)]
          .map((match) => match[1] ?? "")
          .filter((specifier) => fixtureModulePattern.test(specifier))
          .map((specifier) => `${path} -> ${specifier}`),
      );
    expect(staticFixtureImports).toEqual([]);

    const dynamicFixtureImports = productionSourceFiles("src")
      .filter((path) => !insideFixtureChunk(path))
      .flatMap((path) =>
        [...readFileSync(path, "utf8").matchAll(dynamicImportPattern)]
          .filter((match) => fixtureModulePattern.test(match[1] ?? ""))
          .map((match) => ({ path, index: match.index ?? 0 })),
      );
    expect(dynamicFixtureImports.map(({ path }) => path)).toEqual(["src/lib/runtime/bootstrap.ts"]);

    // Vite replaces import.meta.env.DEV with false, so the early return makes the import unreachable.
    const bootstrap = readFileSync("src/lib/runtime/bootstrap.ts", "utf8");
    const importIndex = dynamicFixtureImports[0]?.index ?? -1;
    const functionStart = bootstrap.lastIndexOf("async function", importIndex);
    const guardIndex = bootstrap.indexOf("if (!import.meta.env.DEV", functionStart);
    expect(functionStart).toBeGreaterThanOrEqual(0);
    expect(guardIndex).toBeGreaterThan(functionStart);
    expect(guardIndex).toBeLessThan(importIndex);
    expect(bootstrap.slice(guardIndex, bootstrap.indexOf("\n", guardIndex))).toMatch(/\)\s*return null;$/);
  });
});
