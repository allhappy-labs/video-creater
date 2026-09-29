import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";

const repoRoot = resolve(import.meta.dirname, "..");

describe("AVFoundation exporter package verification", () => {
  it("accepts one ARM64 system-linked fail-closed exporter", () => {
    const temporary = mkdtempSync(join(tmpdir(), "avfoundation-exporter-"));
    const exporter = join(temporary, "exporter");
    executable(
      exporter,
      "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"event\":\"failed\",\"protocol\":\"video-creater.avfoundation-export\",\"schemaVersion\":5}'\nexit 1\n",
    );
    executable(join(temporary, "file"), "#!/bin/sh\necho 'exporter: Mach-O 64-bit executable arm64'\n");
    executable(join(temporary, "lipo"), "#!/bin/sh\necho arm64\n");
    executable(
      join(temporary, "otool"),
      "#!/bin/sh\necho exporter:\necho '/System/Library/Frameworks/AVFoundation.framework/AVFoundation'\n",
    );
    executable(join(temporary, "codesign"), "#!/bin/sh\nexit 0\n");

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/verify-avfoundation-exporter.mjs"),
        "--target",
        "aarch64-apple-darwin",
        "--path",
        exporter,
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
      protocolProbe: { exitCode: 1, finalEvent: "failed" },
      failures: [],
    });
  });

  it("rejects non-system codec dependencies", () => {
    const temporary = mkdtempSync(join(tmpdir(), "avfoundation-exporter-"));
    const exporter = join(temporary, "exporter");
    executable(
      exporter,
      "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"event\":\"failed\",\"protocol\":\"video-creater.avfoundation-export\",\"schemaVersion\":5}'\nexit 1\n",
    );
    executable(join(temporary, "file"), "#!/bin/sh\necho 'exporter: Mach-O 64-bit executable arm64'\n");
    executable(join(temporary, "lipo"), "#!/bin/sh\necho arm64\n");
    executable(
      join(temporary, "otool"),
      "#!/bin/sh\necho exporter:\necho '/opt/homebrew/lib/libavcodec.dylib'\n",
    );
    executable(join(temporary, "codesign"), "#!/bin/sh\nexit 0\n");

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/verify-avfoundation-exporter.mjs"),
        "--target",
        "aarch64-apple-darwin",
        "--path",
        exporter,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: { ...process.env, PATH: `${temporary}:${process.env.PATH ?? ""}` },
      },
    );

    expect(result.status).toBe(1);
    expect(JSON.parse(result.stdout).failures).toContain(
      "exporter links an unreviewed non-system dynamic dependency",
    );
  });
});

function executable(path: string, source: string) {
  writeFileSync(path, source);
  chmodSync(path, 0o755);
}
