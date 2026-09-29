import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { openMock } = vi.hoisted(() => ({ openMock: vi.fn() }));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));

import {
  dataTransferHasFiles,
  externalFilePathsFromDataTransfer,
  formatSkippedMediaImports,
  openMediaFilePaths,
} from "./media-import";

describe("media import helpers", () => {
  beforeEach(() => {
    openMock.mockReset();
    window.__TAURI_INTERNALS__ = {};
  });
  afterEach(() => {
    delete window.__TAURI_INTERNALS__;
  });

  it("opens a multiple-file native media chooser through the dialog adapter", async () => {
    openMock.mockResolvedValue(["/tmp/clip.mp4", "/tmp/voice.wav"]);

    await expect(openMediaFilePaths()).resolves.toEqual([
      "/tmp/clip.mp4",
      "/tmp/voice.wav",
    ]);
    expect(openMock).toHaveBeenCalledWith(
      expect.objectContaining({
        multiple: true,
        directory: false,
        title: "Import media",
        filters: [
          expect.objectContaining({
            extensions: expect.arrayContaining(["mp4", "wav", "png", "lottie"]),
          }),
        ],
      }),
    );
  });

  it("yields no paths when the chooser is cancelled", async () => {
    openMock.mockResolvedValue(null);

    await expect(openMediaFilePaths()).resolves.toEqual([]);
  });

  it("detects drags that carry external files", () => {
    expect(dataTransferHasFiles({ types: ["Files"] } as unknown as DataTransfer)).toBe(true);
    expect(dataTransferHasFiles({ types: ["text/plain"] } as unknown as DataTransfer)).toBe(false);
    expect(dataTransferHasFiles(null)).toBe(false);
  });

  it("falls back to file names only when asked", () => {
    const transfer = {
      files: [{ name: "fixture-clip.mp4" }, { name: " " }],
      getData: vi.fn(() => ""),
    } as unknown as DataTransfer;

    expect(externalFilePathsFromDataTransfer(transfer)).toEqual([]);
    expect(
      externalFilePathsFromDataTransfer(transfer, { fileNameFallback: true }),
    ).toEqual(["fixture-clip.mp4"]);
  });

  it("extracts Tauri file paths from browser drop files", () => {
    const transfer = {
      files: [{ path: "/tmp/drop/image.png" }],
      getData: vi.fn(() => ""),
    } as unknown as DataTransfer;

    expect(externalFilePathsFromDataTransfer(transfer)).toEqual([
      "/tmp/drop/image.png",
    ]);
  });

  it("formats each skipped filename with its actual reason", () => {
    expect(
      formatSkippedMediaImports([
        { sourcePath: "/tmp/notes.txt", reason: "unsupported media extension" },
        { sourcePath: "/tmp/folder.mov", reason: "source path is not a file" },
      ]),
    ).toBe(
      "notes.txt: unsupported media extension · folder.mov: source path is not a file",
    );
  });
});
