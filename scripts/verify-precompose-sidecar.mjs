#!/usr/bin/env node

import { createHash } from "node:crypto";
import { lstatSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dirname, "..");
const options = parseArgs(process.argv.slice(2));
const sidecar = resolve(
  repoRoot,
  options.path ??
    `src-tauri/binaries/video-creater-precompose-worker-${options.target}`,
);
const failures = [];

if (options.target !== "aarch64-apple-darwin") {
  failures.push(`unsupported initial release target: ${options.target}`);
}

let stat;
try {
  stat = lstatSync(sidecar);
  if (stat.isSymbolicLink() || !stat.isFile()) {
    failures.push("sidecar must be a regular non-symlink file");
  }
  if ((stat.mode & 0o111) === 0) failures.push("sidecar must be executable");
} catch (error) {
  failures.push(`sidecar could not be inspected: ${error.message}`);
}

const file = run("file", [sidecar]);
if (file.status !== 0 || !/Mach-O 64-bit executable arm64/.test(file.stdout)) {
  failures.push("sidecar is not a macOS ARM64 Mach-O executable");
}

const architectures = run("lipo", ["-archs", sidecar]);
if (architectures.status !== 0 || architectures.stdout.trim() !== "arm64") {
  failures.push(`sidecar architecture must be exactly arm64, got ${architectures.stdout.trim() || "unknown"}`);
}

const dependencies = run("otool", ["-L", sidecar]);
if (dependencies.status !== 0) {
  failures.push("sidecar dynamic dependencies could not be inspected");
} else if (/\/opt\/homebrew|\/usr\/local|libav|x264|ffmpeg/i.test(dependencies.stdout)) {
  failures.push("sidecar links an unreviewed non-system dynamic dependency");
}

const protocolProbe = spawnSync(sidecar, [], {
  cwd: repoRoot,
  encoding: "utf8",
  input: "{}\n",
  env: { PATH: process.env.PATH ?? "/usr/bin:/bin" },
  timeout: 5_000,
});
const protocolEvents = (protocolProbe.stdout ?? "")
  .split(/\r?\n/)
  .filter(Boolean)
  .flatMap((line) => {
    try {
      return [JSON.parse(line)];
    } catch {
      return [];
    }
  });
const finalEvents = protocolEvents.filter(
  (event) => event.event === "completed" || event.event === "failed",
);
if (
  protocolProbe.error ||
  finalEvents.length !== 1 ||
  finalEvents[0]?.event !== "failed" ||
  finalEvents[0]?.protocol !== "video-creater.precompose"
) {
  failures.push("sidecar did not emit one versioned fail-closed protocol event");
}

const signature = run("codesign", ["--verify", "--strict", "--verbose=2", sidecar]);
const signed = signature.status === 0;
if (options.requireSignature && !signed) {
  failures.push("sidecar signature verification failed");
}

let sha256 = null;
if (stat?.isFile()) {
  sha256 = createHash("sha256").update(readFileSync(sidecar)).digest("hex");
}
const report = {
  schemaVersion: 1,
  status: failures.length === 0 ? "passed" : "failed",
  target: options.target,
  sidecar,
  fileName: basename(sidecar),
  bytes: stat?.size ?? null,
  sha256,
  architectures: architectures.stdout.trim().split(/\s+/).filter(Boolean),
  signed,
  signatureRequired: options.requireSignature,
  protocolProbe: {
    exitCode: protocolProbe.status,
    finalEvent: finalEvents[0]?.event ?? null,
  },
  dynamicDependencies: dependencies.stdout
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.trim())
    .filter(Boolean),
  failures,
};

if (options.report) {
  writeFileSync(resolve(repoRoot, options.report), `${JSON.stringify(report, null, 2)}\n`);
}
console.log(JSON.stringify(report, null, 2));
if (failures.length > 0) process.exitCode = 1;

function run(command, args) {
  const result = spawnSync(command, args, { cwd: repoRoot, encoding: "utf8" });
  return { ...result, stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

function parseArgs(args) {
  const parsed = {
    target: "aarch64-apple-darwin",
    path: null,
    report: null,
    requireSignature: false,
  };
  for (let index = 0; index < args.length; index += 1) {
    const value = args[index];
    if (value === "--target") parsed.target = required(value, args[++index]);
    else if (value === "--path") parsed.path = required(value, args[++index]);
    else if (value === "--report") parsed.report = required(value, args[++index]);
    else if (value === "--require-signature") parsed.requireSignature = true;
    else throw new Error(`unknown argument: ${value}`);
  }
  return parsed;
}

function required(flag, value) {
  if (!value || value.startsWith("--")) throw new Error(`missing value for ${flag}`);
  return value;
}
