import assert from "node:assert/strict";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const repoRoot = resolve(import.meta.dirname, "..");
const scriptPath = resolve(repoRoot, "scripts/prepare-fluidaudio-checkout.mjs");

test("FluidAudio checkout preparation explicitly excludes upstream benchmark documentation", () => {
  const temp = mkdtempSync(join(tmpdir(), "video-creater-fluidaudio-checkout-"));
  const benchmarkPath = join(
    temp,
    "Sources/FluidAudio/ASR/Parakeet/Unified/benchmark.md",
  );
  const manifestPath = join(temp, "Package.swift");

  try {
    mkdirSync(dirname(benchmarkPath), { recursive: true });
    writeFileSync(benchmarkPath, "# Benchmark\n", "utf8");
    writeFileSync(
      manifestPath,
      `// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "FluidAudio",
    targets: [
        .target(
            name: "FluidAudio",
            dependencies: ["FastClusterWrapper"],
            path: "Sources/FluidAudio"
        )
    ]
)
`,
      "utf8",
    );
    chmodSync(manifestPath, 0o444);

    for (let attempt = 0; attempt < 2; attempt += 1) {
      const result = spawnSync(
        process.execPath,
        [scriptPath, "--checkout", temp],
        { cwd: repoRoot, encoding: "utf8" },
      );
      assert.equal(
        result.status,
        0,
        `checkout preparation failed: ${result.stderr || result.stdout}`,
      );
    }

    const manifest = readFileSync(manifestPath, "utf8");
    assert.match(
      manifest,
      /path: "Sources\/FluidAudio",\n\s+exclude: \["ASR\/Parakeet\/Unified\/benchmark\.md"\]/,
    );
    assert.equal(
      manifest.match(/ASR\/Parakeet\/Unified\/benchmark\.md/g)?.length,
      1,
      "preparation must be idempotent",
    );
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
});
