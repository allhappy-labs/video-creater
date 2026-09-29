import { afterEach, describe, expect, it, vi } from "vitest";

const { openMock } = vi.hoisted(() => ({ openMock: vi.fn() }));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));
vi.mock("../backend-client", () => ({ backendRequest: vi.fn() }));

import { backendRequest } from "../backend-client";
import { isBackendUnavailableError } from "../backend-transport";
import { installRuntimeMode } from "../runtime-mode";
import { chooseExportDirectory, fixtureExportDirectoryChooserOperation, fixtureMediaChooserOperation, openMediaFiles } from "./tauri-dialog";

const request = {
  title: "Import media",
  filters: [{ name: "Media", extensions: ["mp4", "png"] }],
} as const;

describe("tauri media file dialog", () => {
  afterEach(() => {
    openMock.mockReset();
    delete window.__TAURI_INTERNALS__;
  });

  it("opens a multiple-file chooser with the requested title and filters", async () => {
    window.__TAURI_INTERNALS__ = {};
    openMock.mockResolvedValue(["/tmp/clip.mp4", " ", "/tmp/still.png"]);

    await expect(openMediaFiles(request)).resolves.toEqual(["/tmp/clip.mp4", "/tmp/still.png"]);
    expect(openMock).toHaveBeenCalledWith({
      multiple: true,
      directory: false,
      title: "Import media",
      filters: [{ name: "Media", extensions: ["mp4", "png"] }],
    });
  });

  it("accepts a single selected path", async () => {
    window.__TAURI_INTERNALS__ = {};
    openMock.mockResolvedValue("/tmp/voice.wav");

    await expect(openMediaFiles(request)).resolves.toEqual(["/tmp/voice.wav"]);
  });

  it("returns null when the chooser is cancelled", async () => {
    window.__TAURI_INTERNALS__ = {};
    openMock.mockResolvedValue(null);

    await expect(openMediaFiles(request)).resolves.toBeNull();
  });

  it("reports an unavailable backend outside the desktop app", async () => {
    const result = openMediaFiles(request);

    await expect(result).rejects.toSatisfy(isBackendUnavailableError);
    expect(openMock).not.toHaveBeenCalled();
  });
});

describe("tauri export folder dialog", () => {
  afterEach(() => {
    openMock.mockReset();
    delete window.__TAURI_INTERNALS__;
  });

  it("opens a single folder chooser at the default path", async () => {
    window.__TAURI_INTERNALS__ = {};
    openMock.mockResolvedValue("/Users/me/Movies");

    await expect(chooseExportDirectory("/projects/edison/exports")).resolves.toBe("/Users/me/Movies");
    expect(openMock).toHaveBeenCalledWith({
      multiple: false,
      directory: true,
      title: "Choose export folder",
      defaultPath: "/projects/edison/exports",
    });
  });

  it("returns null when the chooser is cancelled", async () => {
    window.__TAURI_INTERNALS__ = {};
    openMock.mockResolvedValue(null);

    await expect(chooseExportDirectory()).resolves.toBeNull();
    expect(openMock).toHaveBeenCalledWith({ multiple: false, directory: true, title: "Choose export folder" });
  });

  it("reports an unavailable backend outside the desktop app", async () => {
    await expect(chooseExportDirectory()).rejects.toSatisfy(isBackendUnavailableError);
    expect(openMock).not.toHaveBeenCalled();
  });
});

describe("fixture media chooser", () => {
  afterEach(() => {
    openMock.mockReset();
    installRuntimeMode("browser");
    delete window.__EDITOR_FIXTURE_RUNTIME__;
  });

  it("asks the fixture backend instead of the native chooser in the fixture runtime", async () => {
    window.__EDITOR_FIXTURE_RUNTIME__ = { enabled: true };
    installRuntimeMode("fixture");
    vi.mocked(backendRequest).mockResolvedValueOnce(["/fixtures/preview.webm"]);

    await expect(openMediaFiles(request)).resolves.toEqual(["/fixtures/preview.webm"]);
    expect(backendRequest).toHaveBeenCalledWith(fixtureMediaChooserOperation, {
      title: "Import media",
      filters: [{ name: "Media", extensions: ["mp4", "png"] }],
    });
    expect(openMock).not.toHaveBeenCalled();
  });
});

describe("fixture export folder chooser", () => {
  afterEach(() => {
    openMock.mockReset();
    installRuntimeMode("browser");
    delete window.__EDITOR_FIXTURE_RUNTIME__;
  });

  it("asks the fixture backend for an export folder in the fixture runtime", async () => {
    window.__EDITOR_FIXTURE_RUNTIME__ = { enabled: true };
    installRuntimeMode("fixture");
    vi.mocked(backendRequest).mockResolvedValueOnce("/tmp/video-creater-exports");

    await expect(chooseExportDirectory("/p/exports")).resolves.toBe("/tmp/video-creater-exports");
    expect(fixtureExportDirectoryChooserOperation).toBe("open_export_directory_dialog");
    expect(backendRequest).toHaveBeenCalledWith(fixtureExportDirectoryChooserOperation, { defaultPath: "/p/exports" });
    expect(openMock).not.toHaveBeenCalled();
  });
});
