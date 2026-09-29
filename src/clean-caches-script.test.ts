import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const repoRoot = process.cwd();

describe("clean caches script", () => {
  it("is exposed through package.json", () => {
    const packageJson = JSON.parse(
      readFileSync(join(repoRoot, "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> };

    expect(packageJson.scripts?.["clean:caches"]).toBe(
      "node scripts/clean-caches.mjs",
    );
  });

  it("dry-runs the repo cache cleanup commands", () => {
    const result = spawnSync(
      process.execPath,
      [join(repoRoot, "scripts/clean-caches.mjs"), "--dry-run"],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).toContain(
      "cargo clean --manifest-path src-tauri/Cargo.toml",
    );
    expect(result.stdout).toContain("git clean -ffdX src-tauri/native");
    expect(result.stdout).toContain("git gc --prune=now");
  });
});
