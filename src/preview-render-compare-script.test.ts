import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import { requiredAt } from "@/test-utils/required";

const repoRoot = process.cwd();

describe("preview/render frame comparison automation", () => {
  it("writes previewComparison JSON and diff frames for mismatched PNG pairs", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-preview-compare-"));
    const previewMatch = join(dir, "preview-match.png");
    const renderedMatch = join(dir, "rendered-match.png");
    const previewMismatch = join(dir, "preview-mismatch.png");
    const renderedMismatch = join(dir, "rendered-mismatch.png");
    const outPath = join(dir, "preview-comparison.json");
    const diffDir = join(dir, "diffs");

    writeRgbaPng(previewMatch, 1, 1, [[20, 40, 60, 255]]);
    writeRgbaPng(renderedMatch, 1, 1, [[20, 40, 60, 255]]);
    writeRgbaPng(previewMismatch, 1, 1, [[20, 40, 60, 255]]);
    writeRgbaPng(renderedMismatch, 1, 1, [[200, 30, 10, 255]]);

    execFileSync(
      "node",
      [
        "scripts/compare-preview-render-frames.mjs",
        "--threshold",
        "0.01",
        "--out",
        outPath,
        "--diff-dir",
        diffDir,
        "--frame",
        `1.25:${previewMatch}:${renderedMatch}`,
        "--frame",
        `2.5:${previewMismatch}:${renderedMismatch}`,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    const comparison = JSON.parse(readFileSync(outPath, "utf8")) as {
      status: string;
      comparedFrames: Array<{
        timelineSeconds: number;
        previewFrame: string;
        renderedFrame: string;
        diffFrame: string | null;
        mismatchRatio: number;
        passed: boolean;
      }>;
    };

    expect(comparison.status).toBe("failed");
    expect(comparison.comparedFrames).toHaveLength(2);
    const matchingFrame = requiredAt(comparison.comparedFrames, 0, "matching comparison frame");
    const mismatchingFrame = requiredAt(
      comparison.comparedFrames,
      1,
      "mismatching comparison frame",
    );
    expect(matchingFrame).toMatchObject({
      timelineSeconds: 1.25,
      previewFrame: previewMatch,
      renderedFrame: renderedMatch,
      diffFrame: null,
      mismatchRatio: 0,
      passed: true,
    });
    expect(mismatchingFrame.timelineSeconds).toBe(2.5);
    expect(mismatchingFrame.mismatchRatio).toBe(1);
    expect(mismatchingFrame.passed).toBe(false);
    expect(mismatchingFrame.diffFrame).toContain("diff-0002.png");
    expect(readFileSync(mismatchingFrame.diffFrame as string).length).toBeGreaterThan(0);
  });

  it("can fail the process after writing previewComparison evidence for mismatches", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-preview-compare-gate-"));
    const previewMismatch = join(dir, "preview-mismatch.png");
    const renderedMismatch = join(dir, "rendered-mismatch.png");
    const outPath = join(dir, "preview-comparison.json");
    const diffDir = join(dir, "diffs");

    writeRgbaPng(previewMismatch, 1, 1, [[20, 40, 60, 255]]);
    writeRgbaPng(renderedMismatch, 1, 1, [[200, 30, 10, 255]]);

    let exitStatus: number | null = null;
    try {
      execFileSync(
        "node",
        [
          "scripts/compare-preview-render-frames.mjs",
          "--threshold",
          "0.01",
          "--fail-on-mismatch",
          "--out",
          outPath,
          "--diff-dir",
          diffDir,
          "--frame",
          `2.5:${previewMismatch}:${renderedMismatch}`,
        ],
        { cwd: repoRoot, encoding: "utf8" },
      );
    } catch (error) {
      exitStatus = (error as { status?: number }).status ?? null;
    }

    const comparison = JSON.parse(readFileSync(outPath, "utf8")) as {
      status: string;
      comparedFrames: Array<{ diffFrame: string | null; passed: boolean }>;
    };
    expect(exitStatus).toBe(1);
    expect(comparison.status).toBe("failed");
    const mismatchingFrame = requiredAt(comparison.comparedFrames, 0, "mismatching comparison frame");
    expect(mismatchingFrame.passed).toBe(false);
    expect(mismatchingFrame.diffFrame).toContain("diff-0001.png");
  });
});

function writeRgbaPng(
  path: string,
  width: number,
  height: number,
  pixels: Array<readonly [number, number, number, number]>,
) {
  const rowBytes = width * 4;
  const raw = Buffer.alloc((rowBytes + 1) * height);
  for (let y = 0; y < height; y += 1) {
    raw[y * (rowBytes + 1)] = 0;
    for (let x = 0; x < width; x += 1) {
      const pixel = requiredAt(pixels, y * width + x, "RGBA pixel");
      const offset = y * (rowBytes + 1) + 1 + x * 4;
      raw[offset] = pixel[0];
      raw[offset + 1] = pixel[1];
      raw[offset + 2] = pixel[2];
      raw[offset + 3] = pixel[3];
    }
  }

  writeFileSync(
    path,
    Buffer.concat([
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
      pngChunk("IHDR", ihdr(width, height)),
      pngChunk("IDAT", deflateSync(raw)),
      pngChunk("IEND", Buffer.alloc(0)),
    ]),
  );
}

function ihdr(width: number, height: number) {
  const data = Buffer.alloc(13);
  data.writeUInt32BE(width, 0);
  data.writeUInt32BE(height, 4);
  data[8] = 8;
  data[9] = 6;
  return data;
}

function pngChunk(type: string, data: Buffer) {
  const typeBuffer = Buffer.from(type, "ascii");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function crc32(buffer: Buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}
