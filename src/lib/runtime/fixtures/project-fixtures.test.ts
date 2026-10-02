import { describe, expect, it } from "vitest";
import { mediaIdsFromIndexedSearch } from "@/lib/media/search";
import type {
  ImportMediaResult,
  PreparedProjectPreview,
  ProjectActionWriteResult,
  ProjectMediaSearchResult,
  TimelineFilmstripReport,
  VideoProject,
} from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import type { ShaderBackgroundTemplateDefinition } from "@/lib/shader-background-templates";
import { fixtureMediaChooserOperation } from "../adapters/tauri-dialog";
import { createFixtureProjectStore } from "./fixture-project-store";
import { fixtureImportFile, projectFixtureOperations } from "./project-fixtures";

const projectDir = "/tmp/video-creater-editor-project";

function setup() {
  const store = createFixtureProjectStore();
  const handlers = projectFixtureOperations(store);
  async function request<Result>(operation: string, input: Record<string, unknown> = {}): Promise<Result> {
    const handler = handlers.get(operation);
    if (!handler) throw new Error(`no project fixture handler for ${operation}`);
    return structuredClone(await handler(structuredClone(input))) as Result;
  }
  async function open(): Promise<VideoProject> {
    await request("materialize_sample_project_media", { projectDir });
    return (await request<ProjectActionWriteResult>("save_split_project_to_folder", { projectDir, project: createSampleProject(), expectedRevision: 0 })).project;
  }
  return { store, request, open };
}

describe("project fixture operations", () => {
  it("requires an existing project for a nonactivating restore while default saves can create", async () => {
    const { store, request, open } = setup();
    await expect(request("save_split_project_to_folder", {
      projectDir, project: createSampleProject(), expectedRevision: 0, activateProject: false,
    })).rejects.toBe("This folder has no saved project yet.");
    expect(store.current).toBeNull();

    const opened = await open();
    const snapshot = { ...opened, name: "restored snapshot" };
    const restored = await request<ProjectActionWriteResult>("save_split_project_to_folder", {
      projectDir, project: snapshot, expectedRevision: opened.contentRevision, activateProject: false,
    });
    expect(restored.project).toMatchObject({ name: snapshot.name, contentRevision: 2 });
    await expect(request<VideoProject>("read_project_snapshot_from_split_project_folder", { projectDir })).resolves.toEqual(restored.project);
    await expect(request("save_split_project_to_folder", {
      projectDir, project: { ...restored.project, id: "replacement" }, expectedRevision: 2, activateProject: false,
    })).rejects.toThrow("Project identity does not match");
    expect(store.current).toEqual(restored.project);
  });

  it("opens, reloads and edits the sample as a folder project with increasing revisions", async () => {
    const { request, open } = setup();
    await expect(request("load_split_project_from_folder", { projectDir })).rejects.toBe("This folder has no saved project yet.");
    const opened = await open();
    expect(opened).toMatchObject({ schemaVersion: 2, contentRevision: 1 });

    const renamed = await request<ProjectActionWriteResult>("apply_project_action_to_split_project_folder", { projectDir, action: { type: "renameMedia", mediaId: "media-1", name: "Reel" } });
    expect(renamed.project.contentRevision).toBe(2);
    const batch = await request<ProjectActionWriteResult>("apply_project_actions_to_split_project_folder", {
      projectDir,
      actions: [{ type: "updateVisualClipOpacity", itemId: "item-1", opacity: 0.5 }],
    });
    expect(batch.project.contentRevision).toBe(3);
    await expect(request<VideoProject>("load_split_project_from_folder", { projectDir })).resolves.toEqual(batch.project);

    await expect(request("save_split_project_to_folder", { projectDir, project: opened, expectedRevision: 1 })).rejects.toBe(
      "project revision conflict: expected revision 1, but canonical revision is 3",
    );
  });

  it("chooses the fixture video and imports it (and a fixture PNG) with deterministic ids", async () => {
    const { request, open } = setup();
    await open();
    await expect(request(fixtureMediaChooserOperation, { title: "Import media", filters: [{ name: "Media", extensions: ["webm", "png"] }] })).resolves.toEqual([fixtureImportFile]);
    await expect(request(fixtureMediaChooserOperation, { title: "Import audio", filters: [{ name: "Audio", extensions: ["wav"] }] })).resolves.toBeNull();

    const first = await request<ImportMediaResult>("import_media_to_project", { projectDir, project: createSampleProject(), sourcePaths: [fixtureImportFile, "/fixtures/notes.txt"] });
    expect(first.imported).toEqual([
      { id: "media-fixture-import-1", name: "preview", relativePath: "media/media-fixture-import-1-preview.webm", kind: "video", durationSeconds: 0.999, width: 1920, height: 1080, fps: 30, folderId: null },
    ]);
    expect(first.skipped).toEqual([{ sourcePath: "/fixtures/notes.txt", reason: "unsupported media extension" }]);
    expect(first.project.contentRevision).toBe(2);
    expect(first.project.media.map((media) => media.id)).toContain("media-fixture-import-1");

    const second = await request<ImportMediaResult>("import_media_to_project", { projectDir, project: first.project, sourcePaths: ["/fixtures/preview-frame.png"] });
    expect(second.imported).toMatchObject([{ id: "media-fixture-import-2", kind: "image", width: 320, height: 180 }]);
    await expect(request("import_media_to_project", { projectDir, project: second.project, sourcePaths: ["/fixtures/notes.txt"] })).rejects.toBe("no selected files can be imported");
  });

  it("names imported media from the names map", async () => {
    const { request, open } = setup();
    const project = await open();
    const sourcePath = `${projectDir}/renders/save-range-1/output.webm`;
    const result = await request<ImportMediaResult>("import_media_to_project", { projectDir, project, sourcePaths: [sourcePath], names: { [sourcePath]: "Edison Restoration Demo 00:04–00:09" } });
    expect(result.imported[0]).toMatchObject({ name: "Edison Restoration Demo 00:04–00:09", relativePath: "media/media-fixture-import-1-edison-restoration-demo-00-04-00-09.webm" });
  });

  it("answers preview preparation without frame sequences and filmstrips without frames", async () => {
    const { request, open } = setup();
    const project = await open();
    await expect(request<PreparedProjectPreview>("prepare_project_preview", { projectDir, project })).resolves.toEqual({ project, reports: [], frameSequences: [] });
    const filmstrip = await request<TimelineFilmstripReport>("cache_timeline_filmstrip_in_split_project_folder", {
      projectDir,
      mediaId: "media-1",
      sourceIn: 0,
      sourceOut: 4,
      speed: 1,
      zoomBucket: 100,
      heightBucket: 48,
      clipPixelWidth: 320,
    });
    expect(filmstrip).toMatchObject({ mediaId: "media-1", sourceIn: 0, sourceOut: 4, frames: [] });
    const templates = await request<ShaderBackgroundTemplateDefinition[]>("list_shader_background_templates", { projectDir });
    expect(templates.length).toBeGreaterThan(0);
    expect(templates.every((template) => template.placement.trackKind === "hyperframe_scene")).toBe(true);
  });

  it("searches media, transcript words and generations locally", async () => {
    const { request, open } = setup();
    await open();
    const byFolder = await request<ProjectMediaSearchResult>("search_project_media", { projectDir, query: "source footage", limit: 20, scope: "both" });
    expect([...mediaIdsFromIndexedSearch(byFolder)]).toEqual(["media-1"]);
    const spoken = await request<ProjectMediaSearchResult>("search_project_media", { projectDir, query: "split", limit: 20, scope: "spoken" });
    expect(spoken.groups.spoken).toEqual([expect.objectContaining({ mediaId: "media-1", text: "split" })]);
    expect(spoken.groups.metadata).toEqual([]);
    const generated = await request<ProjectMediaSearchResult>("search_project_media", { projectDir, query: "edison", limit: 20, scope: "generated" });
    expect([...mediaIdsFromIndexedSearch(generated)]).toEqual(["sample-generated-output"]);
    expect(generated).toMatchObject({ visualStatus: "notInstalled", spokenStatus: "ready", indexStatus: { stored: true } });
  });
});
