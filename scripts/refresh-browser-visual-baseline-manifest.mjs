#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function pngDimensions(buffer) {
  if (buffer.length < 24 || buffer.toString("ascii", 12, 16) !== "IHDR") {
    throw new Error("Not a PNG with an IHDR chunk");
  }
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}

export function refreshManifest({ baselineDir, manifestPath, screenshots }) {
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const artifacts = {};
  for (const name of screenshots) {
    const path = join(baselineDir, name);
    if (!existsSync(path)) throw new Error(`Missing baseline screenshot ${path}`);
    const buffer = readFileSync(path);
    artifacts[name] = {
      ...pngDimensions(buffer),
      bytes: buffer.length,
      sha256: createHash("sha256").update(buffer).digest("hex"),
    };
  }
  const next = { ...manifest, screenshots: [...screenshots], artifacts };
  writeFileSync(manifestPath, `${JSON.stringify(next, null, 2)}\n`);
  return next;
}

async function main() {
  const args = process.argv.slice(2).filter((value) => value !== "--");
  const read = (flag) => {
    const index = args.indexOf(flag);
    return index >= 0 ? args[index + 1] : undefined;
  };
  const baselineDir = read("--baseline");
  const manifestPath = read("--manifest");
  if (!baselineDir || !manifestPath) {
    console.error("Usage: refresh-browser-visual-baseline-manifest.mjs --baseline <dir> --manifest <file>");
    process.exit(2);
  }
  const { visualQaScenarios, scenarioScreenshotName } = await import("./browser-visual-qa.mjs");
  const screenshots = visualQaScenarios.map(scenarioScreenshotName);
  refreshManifest({ baselineDir: resolve(baselineDir), manifestPath: resolve(manifestPath), screenshots });
  console.log(`Refreshed ${screenshots.length} baseline artifacts in ${manifestPath}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
