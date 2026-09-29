import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { relative, resolve, sep } from "node:path";
import test from "node:test";

const repoRoot = resolve(import.meta.dirname, "..");
const sourceRoot = resolve(repoRoot, "src");
const runtimeAdapterDirectory = "src/lib/runtime/adapters/";
// Settings views that predate the dialog adapter; new code opens dialogs through the adapters.
const legacyDialogImports = new Set([
  "src/components/settings/advanced-settings.tsx",
  "src/components/settings/models-settings.tsx",
  "src/components/settings/storage-settings.tsx",
]);
const tauriImportPattern = /from\s+["']@tauri-apps\/(api\/[\w-]+|plugin-[\w-]+)["']/g;

function productionTypeScriptFiles(directory: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...productionTypeScriptFiles(path));
      continue;
    }
    if (
      entry.isFile() &&
      /\.(?:ts|tsx)$/.test(entry.name) &&
      !/\.(?:test|spec)\.(?:ts|tsx)$/.test(entry.name)
    ) {
      files.push(path);
    }
  }
  return files.sort();
}

function repositoryPath(path: string): string {
  return relative(repoRoot, path).split(sep).join("/");
}

test("production Tauri imports stay inside runtime adapters", () => {
  const violations = productionTypeScriptFiles(sourceRoot)
    .map((path) => ({
      path: repositoryPath(path),
      modules: [...readFileSync(path, "utf8").matchAll(tauriImportPattern)].map(
        (match) => match[1],
      ),
    }))
    .filter(({ path }) => !path.startsWith(runtimeAdapterDirectory))
    .filter(({ path, modules }) =>
      modules.some((module) => !(module === "plugin-dialog" && legacyDialogImports.has(path))),
    )
    .map(({ path }) => path);

  assert.deepEqual(
    violations,
    [],
    `Tauri imports escaped the runtime adapters:\n${violations.join("\n")}`,
  );
});

test("React components do not invoke native commands or convert media URLs", () => {
  const componentFiles = productionTypeScriptFiles(resolve(sourceRoot, "components"));
  const appFile = resolve(sourceRoot, "App.tsx");
  const violations = [...componentFiles, appFile]
    .filter((path) => /\binvoke\s*\(|\bconvertFileSrc\s*\(/.test(readFileSync(path, "utf8")))
    .map(repositoryPath);

  assert.deepEqual(
    violations,
    [],
    `React files bypassed typed domain modules:\n${violations.join("\n")}`,
  );
});
