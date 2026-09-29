import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import test from "node:test";

const repoRoot = resolve(import.meta.dirname, "..");
const smokeModules = readdirSync(resolve(repoRoot, "scripts"))
  .filter((name) => /^linux-desktop-smoke.*\.mjs$/.test(name))
  .sort()
  .map((name) => ({ name: `scripts/${name}`, source: readFileSync(resolve(repoRoot, "scripts", name), "utf8") }));
const smokeScript = smokeModules.map((module) => module.source).join("\n");

/** Accessible names the Linux desktop smoke run looks up in XPath and CSS selectors. */
const labelPatterns = [
  /@aria-label='([^']+)'/g,
  /\[aria-label='([^']+)'\]/g,
  /normalize-space\(\)='([^'$]+)'/g,
  /starts-with\(@aria-label, '([^']+)'\)/g,
  /\b(?:settingsTab|editorTab|exportChoice)\("([^"]+)"\)/g,
  /\[data-testid='([^']+)'\]/g,
  /@data-testid='([^']+)'/g,
];

function smokeLabels(): string[] {
  const labels = labelPatterns.flatMap((pattern) => [...smokeScript.matchAll(pattern)].map((match) => match[1]));
  return [...new Set(labels)].sort();
}

function productSources(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productSources(path);
    return /\.tsx?$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name) ? [path] : [];
  });
}

test("the Linux desktop smoke run only looks up accessible names that the app renders", () => {
  const labels = smokeLabels();
  assert.ok(labels.length >= 20, `expected the smoke selectors to be extracted, found ${labels.join(", ")}`);
  const sources = productSources(resolve(repoRoot, "src")).map((path) => readFileSync(path, "utf8")).join("\n");
  const missing = labels.filter((label) => !sources.includes(label));
  assert.deepEqual(missing, [], "smoke selectors name UI text that no longer exists in src/");
});

test("the Linux desktop smoke run no longer drives the pre-redesign editor", () => {
  for (const retired of ["Export options", "More export options", "Export project", "Transcribe selected media", "ancestor::footer"]) {
    for (const module of smokeModules) {
      assert.ok(!module.source.includes(retired), `${module.name} still references "${retired}"`);
    }
  }
});

test("every Linux desktop smoke module stays under 600 lines", () => {
  const oversized = smokeModules
    .map((module) => ({ name: module.name, lines: (module.source.match(/\n/g) ?? []).length }))
    .filter((module) => module.lines >= 600);
  assert.deepEqual(oversized, [], "split the smoke module so each file stays under 600 lines");
});
