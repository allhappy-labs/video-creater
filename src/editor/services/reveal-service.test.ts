import { beforeEach, describe, expect, it, vi } from "vitest";
import { backendArtifactUrl, backendRequest } from "@/lib/runtime/backend-client";
import { installRuntimeMode } from "@/lib/runtime/runtime-mode";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "../store/editor-store";
import { createRevealService } from "./reveal-service";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendArtifactUrl: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function setup(projectDir = "/projects/edison") {
  const store = createEditorStore({ projectDir, project: { ...fixtureProject(), schemaVersion: 2 } });
  return { store, service: createRevealService(store) };
}

describe("reveal service", () => {
  beforeEach(() => {
    installRuntimeMode("desktop");
    vi.mocked(backendRequest).mockReset();
    vi.mocked(backendArtifactUrl).mockReset();
  });

  it("downloads a recorded export from the host in a browser", async () => {
    installRuntimeMode("browser");
    vi.mocked(backendArtifactUrl).mockResolvedValue("/api/v1/artifacts/0123456789abcdef0123456789abcdef");
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    const { store, service } = setup("opaque-project");
    store.setState({
      project: {
        ...store.getState().project,
        exportArtifacts: [{
          schemaVersion: 1,
          id: "export-a",
          kind: "mp4",
          format: "mp4",
          path: "exports/final.mp4",
          mimeType: "video/mp4",
          createdAt: "2026-09-24T00:00:00Z",
        }],
      },
    });

    await expect(service.revealExportArtifact("exports/final.mp4")).resolves.toBe(true);

    expect(backendArtifactUrl).toHaveBeenCalledWith("opaque-project", "export-a");
    expect(click).toHaveBeenCalledTimes(1);
    click.mockRestore();
  });

  it("reveals a recorded export through the restricted command for the open project folder", async () => {
    vi.mocked(backendRequest).mockResolvedValue(undefined);
    const { store, service } = setup();

    await expect(service.revealExportArtifact("renders/export-mp4-1/output.mp4")).resolves.toBe(true);
    expect(backendRequest).toHaveBeenCalledWith("reveal_export_artifact_in_split_project_folder", {
      projectDir: "/projects/edison",
      artifactPath: "renders/export-mp4-1/output.mp4",
    });
    expect(store.getState().toasts).toEqual([]);
  });

  it("toasts the backend's plain message when the file can't be shown", async () => {
    // Tauri command errors arrive as plain strings.
    vi.mocked(backendRequest).mockRejectedValue("This file isn't a recorded export of this project.");
    const { store, service } = setup();

    await expect(service.revealExportArtifact("media/private.mp4")).resolves.toBe(false);
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["This file isn't a recorded export of this project."]);
  });

  it("needs a project folder before calling the backend", async () => {
    const { store, service } = setup("");

    await expect(service.revealExportArtifact("exports/timeline.xml")).resolves.toBe(false);
    expect(backendRequest).not.toHaveBeenCalled();
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Save this project to a folder before showing its files."]);
  });
});
