import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const repoRoot = process.cwd();

describe("precompose vendored dependency provenance", () => {
  it("pins reviewed permissive sources and excludes forbidden build paths", () => {
    const result = spawnSync(
      process.execPath,
      [join(repoRoot, "scripts/check-precompose-vendor-provenance.mjs")],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status, result.stderr).toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({ status: "passed", failures: [] });
  });

  it("exposes the provenance gate through the package scripts", () => {
    const packageJson = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8")) as {
      scripts?: Record<string, string>;
    };
    expect(packageJson.scripts?.["check:precompose-vendor"]).toBe(
      "node scripts/check-precompose-vendor-provenance.mjs",
    );
    expect(packageJson.scripts?.["check:precompose-licenses"]).toBe(
      "node scripts/check-precompose-cargo-licenses.mjs",
    );
  });
});
