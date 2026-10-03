import { createHash } from "node:crypto";
import { readFileSync, readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

export function assessRustAudit(report) {
  if (!report?.database?.["last-commit"] || !report?.vulnerabilities?.list
    || report.settings?.ignore?.length) throw new Error("Incomplete or ignored Rust security findings.");
  if (report.vulnerabilities.count !== report.vulnerabilities.list.length
    || report.vulnerabilities.found || report.vulnerabilities.count) throw new Error("Unresolved Rust vulnerability.");
  for (const [kind, findings] of Object.entries(report.warnings ?? {})) {
    if (kind !== "unmaintained" && findings.length) throw new Error(`Unresolved Rust ${kind} advisory.`);
  }
  return { vulnerabilities: 0, unmaintained: report.warnings?.unmaintained?.length ?? 0 };
}

export function verifyGlibBackport(root = "src-tauri/vendor/glib") {
  const manifest = JSON.parse(readFileSync(`${root}/SECURITY-PATCH.json`, "utf8"));
  if (manifest.advisory !== "RUSTSEC-2024-0429" || manifest.version !== "0.18.5"
    || manifest.upstreamCrateSha256 !== "233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5") throw new Error("Unreviewed GTK3 backport source.");
  const actual = {};
  const visit = directory => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = `${directory}/${entry.name}`;
      if (entry.isSymbolicLink()) throw new Error("Symlink in GTK3 backport source.");
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile() && entry.name !== "SECURITY-PATCH.json") {
        actual[relative(root, path)] = createHash("sha256").update(readFileSync(path)).digest("hex");
      }
    }
  };
  visit(root);
  if (Object.keys(actual).length !== Object.keys(manifest.files).length
    || Object.entries(manifest.files).some(([path, hash]) => actual[path] !== hash)) throw new Error("GTK3 backport source changed.");
  const source = readFileSync(`${root}/src/variant_iter.rs`, "utf8");
  if (!source.includes("let mut p: *mut libc::c_char") || !source.includes("                &mut p,")) throw new Error("GTK3 iterator fix is missing.");
  return true;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  verifyGlibBackport();
  const manifest = readFileSync("src-tauri/Cargo.toml", "utf8");
  if (!manifest.includes('glib = { path = "vendor/glib" }')) throw new Error("GTK3 backport is not selected.");
  const audit = spawnSync("cargo", ["audit", "--file", "src-tauri/Cargo.lock", "--json"], { encoding: "utf8", timeout: 120_000 });
  if (audit.error || ![0, 1].includes(audit.status)) throw new Error("Rust audit failed. Install cargo-audit to run this gate.");
  const result = assessRustAudit(JSON.parse(audit.stdout));
  console.log(`Rust security: ${result.vulnerabilities} unresolved vulnerabilities or unsound advisories; GTK3 iterator backport verified; ${result.unmaintained} upstream maintenance warnings remain visible.`);
}
