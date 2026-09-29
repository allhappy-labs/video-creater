import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const defaultRoot = resolve("output/parity-audit-2026-07-13");
const defaultPairFiles = [
  ["home", "Project home", "01-home.png", "home-palmier-desktop.png"],
];

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function imageData(file) {
  const extension = file.toLowerCase().endsWith(".jpg") || file.toLowerCase().endsWith(".jpeg")
    ? "jpeg"
    : "png";
  return `data:image/${extension};base64,${readFileSync(file).toString("base64")}`;
}

function renderBoard(pairs) {
  const rows = pairs.map((pair) => `
    <article class="pair" id="${escapeHtml(pair.id)}">
      <header><span>${escapeHtml(pair.id)}</span><h2>${escapeHtml(pair.label)}</h2></header>
      <div class="screens">
        <figure><figcaption>Palmier reference</figcaption><img src="${pair.referenceData}" alt="${escapeHtml(pair.label)} Palmier reference"><small>${escapeHtml(pair.reference)}</small></figure>
        <figure><figcaption>Video Creater implementation</figcaption><img src="${pair.implementationData}" alt="${escapeHtml(pair.label)} Video Creater implementation"><small>${escapeHtml(pair.implementation)}</small></figure>
      </div>
    </article>`).join("\n");

  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Palmier visual parity review</title>
<style>
:root{color-scheme:dark;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text",sans-serif}*{box-sizing:border-box}body{margin:0;background:#0a0a0a;color:#f4f4f4}main{width:min(1760px,calc(100vw - 32px));margin:0 auto;padding:32px 0 64px}h1{margin:0 0 8px;font-size:30px;font-weight:500;letter-spacing:-.03em}.intro{margin:0 0 28px;color:#9b9b9b}.pair{margin-top:18px;overflow:hidden;border:1px solid #292929;border-radius:18px;background:#111}.pair>header{display:flex;align-items:baseline;gap:12px;padding:14px 16px;border-bottom:1px solid #292929}.pair>header span{color:#777;font:600 11px/1 ui-monospace,SFMono-Regular,Menlo,monospace;text-transform:uppercase;letter-spacing:.12em}h2{margin:0;font-size:16px;font-weight:600}.screens{display:grid;grid-template-columns:repeat(2,minmax(0,1fr))}figure{min-width:0;margin:0;padding:14px}figure+figure{border-left:1px solid #292929}figcaption{margin-bottom:10px;color:#c9c9c9;font-size:12px;font-weight:600}img{display:block;width:100%;height:auto;max-height:78vh;object-fit:contain;object-position:top center;background:#050505;border:1px solid #242424}small{display:block;margin-top:8px;overflow:hidden;color:#676767;font:10px/1.4 ui-monospace,SFMono-Regular,Menlo,monospace;text-overflow:ellipsis;white-space:nowrap}@media(max-width:900px){.screens{grid-template-columns:1fr}figure+figure{border-left:0;border-top:1px solid #292929}}
</style></head><body><main><h1>Palmier visual parity review</h1><p class="intro">Unfiltered references and implementation captures shown together for design review.</p>${rows}</main></body></html>`;
}

export function buildComparisonBoard({ pairs, htmlOut, manifestOut }) {
  if (!Array.isArray(pairs) || pairs.length === 0) throw new Error("At least one visual comparison pair is required.");
  const normalized = pairs.map((pair) => {
    for (const file of [pair.reference, pair.implementation]) {
      if (!existsSync(file) || statSync(file).size === 0) throw new Error(`Missing visual comparison input: ${file}`);
    }
    return { ...pair, referenceData: imageData(pair.reference), implementationData: imageData(pair.implementation) };
  });
  mkdirSync(dirname(htmlOut), { recursive: true });
  mkdirSync(dirname(manifestOut), { recursive: true });
  writeFileSync(htmlOut, renderBoard(normalized));
  writeFileSync(manifestOut, JSON.stringify({
    generatedAt: new Date().toISOString(),
    excludedReferenceFiles: [{ file: "10-ai-edit-duplicate.png", reason: "Pixel-identical duplicate coverage of reference 09." }],
    pairs: normalized.map(({ referenceData: _reference, implementationData: _implementation, ...pair }) => pair),
  }, null, 2));
  return { pairs: normalized };
}

function optionValue(args, name, fallback) {
  const index = args.indexOf(name);
  if (index === -1) return fallback;
  const value = args[index + 1];
  if (!value || value.startsWith("--")) throw new Error(`${name} requires a path.`);
  return resolve(value);
}

function printHelp() {
  process.stdout.write(`Usage: node scripts/palmier-visual-comparison.mjs [options]\n\nOptions:\n  --reference <dir>       Palmier PNG directory\n  --implementation <dir>  Video Creater PNG directory\n  --html-out <file>       Self-contained comparison board\n  --manifest-out <file>   JSON pair manifest\n  --help                  Show this help\n`);
}

function main() {
  const args = process.argv.slice(2);
  if (args.includes("--help")) { printHelp(); return; }
  const referenceRoot = optionValue(args, "--reference", resolve(defaultRoot, "reference"));
  const implementationRoot = optionValue(args, "--implementation", resolve(defaultRoot, "implementation"));
  const htmlOut = optionValue(args, "--html-out", resolve(defaultRoot, "comparison/index.html"));
  const manifestOut = optionValue(args, "--manifest-out", resolve(defaultRoot, "comparison/manifest.json"));
  const pairs = defaultPairFiles.map(([id, label, reference, implementation]) => ({ id, label, reference: resolve(referenceRoot, reference), implementation: resolve(implementationRoot, implementation) }));
  const result = buildComparisonBoard({ pairs, htmlOut, manifestOut });
  process.stdout.write(`Wrote ${result.pairs.length} comparison pairs to ${htmlOut}\n`);
}

const isDirectRun = process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url;
if (isDirectRun) main();
