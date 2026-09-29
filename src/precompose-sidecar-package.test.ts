import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";

const repoRoot = resolve(import.meta.dirname, "..");

describe("precompose sidecar package verification", () => {
  it("accepts one ARM64 system-linked fail-closed worker", () => {
    const temporary = mkdtempSync(join(tmpdir(), "precompose-sidecar-"));
    const worker = join(temporary, "worker");
    executable(
      worker,
      "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"event\":\"failed\",\"protocol\":\"video-creater.precompose\"}'\nexit 2\n",
    );
    executable(join(temporary, "file"), "#!/bin/sh\necho 'worker: Mach-O 64-bit executable arm64'\n");
    executable(join(temporary, "lipo"), "#!/bin/sh\necho arm64\n");
    executable(
      join(temporary, "otool"),
      "#!/bin/sh\necho worker:\necho '/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)'\n",
    );
    executable(join(temporary, "codesign"), "#!/bin/sh\nexit 0\n");

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/verify-precompose-sidecar.mjs"),
        "--target",
        "aarch64-apple-darwin",
        "--path",
        worker,
        "--require-signature",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: { ...process.env, PATH: `${temporary}:${process.env.PATH ?? ""}` },
      },
    );

    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({
      status: "passed",
      target: "aarch64-apple-darwin",
      signed: true,
      protocolProbe: { exitCode: 2, finalEvent: "failed" },
      failures: [],
    });
  });

  it("rejects targets outside the initial release matrix", () => {
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/verify-precompose-sidecar.mjs"),
        "--target",
        "x86_64-apple-darwin",
        "--path",
        join(repoRoot, "missing-worker"),
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );
    expect(result.status).toBe(1);
    expect(JSON.parse(result.stdout).failures).toContain(
      "unsupported initial release target: x86_64-apple-darwin",
    );
  });
});

function executable(path: string, source: string) {
  writeFileSync(path, source);
  chmodSync(path, 0o755);
}
