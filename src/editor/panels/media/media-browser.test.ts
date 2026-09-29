import { describe, expect, it } from "vitest";
import type { VideoProject } from "@/lib/project";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import {
  childFolders,
  existingFolderId,
  folderBreadcrumb,
  folderItemCount,
  mediaClips,
  mediaCountLabel,
  pendingGenerations,
  visibleMedia,
  type MediaBrowserQuery,
} from "./media-browser";

function project(): VideoProject {
  const base = fixtureProject();
  return {
    ...base,
    mediaFolders: [...(base.mediaFolders ?? []), { id: "folder-takes", name: "Takes", parentId: "folder-source" }],
    media: [
      ...base.media,
      { id: "media-still", name: "Poster", relativePath: "media/poster.png", kind: "image", durationSeconds: 0, width: 10, height: 10, fps: null, folderId: "folder-takes" },
    ],
  };
}

const query: MediaBrowserQuery = { filter: "all", folderId: null, search: "", sort: "dateAdded", scope: "both", indexedMediaIds: null };

function ids(media: { id: string }[]) {
  return media.map((asset) => asset.id);
}

describe("media browser model", () => {
  it("scopes folders recursively and resets missing folders", () => {
    const base = project();
    expect(ids(visibleMedia(base, { ...query, folderId: "folder-source" }))).toEqual(["media-1", "media-still"]);
    expect(folderItemCount(base, "folder-source")).toBe(2);
    expect(folderItemCount(base, null)).toBe(4);
    expect(childFolders(base, null).map((folder) => folder.name)).toEqual(["Source footage", "Generated selects", "Audio"]);
    expect(childFolders(base, "folder-source").map((folder) => folder.name)).toEqual(["Takes"]);
    expect(folderBreadcrumb(base, "folder-takes").map((folder) => folder.name)).toEqual(["Source footage", "Takes"]);
    expect(existingFolderId(base, "folder-gone")).toBeNull();
  });

  it("combines chips, local search and indexed hits", () => {
    const base = project();
    expect(ids(visibleMedia(base, { ...query, filter: "images" }))).toEqual(["media-still"]);
    expect(ids(visibleMedia(base, { ...query, search: " POSTER " }))).toEqual(["media-still"]);
    expect(ids(visibleMedia(base, { ...query, search: "takes" }))).toEqual(["media-still"]);
    const indexed = new Set(["media-voiceover"]);
    expect(ids(visibleMedia(base, { ...query, search: "poster", indexedMediaIds: indexed }))).toEqual(["media-voiceover", "media-still"]);
    expect(ids(visibleMedia(base, { ...query, search: "poster", scope: "visual", indexedMediaIds: indexed }))).toEqual(["media-voiceover"]);
  });

  it("sorts and labels counts", () => {
    const base = project();
    expect(ids(visibleMedia(base, { ...query, sort: "name" }))[0]).toBe("media-1");
    expect(mediaCountLabel(base, null, "", 4)).toBe("4 items");
    expect(mediaCountLabel(base, "folder-takes", "", 1)).toBe("1 item");
    expect(mediaCountLabel(base, null, " poster ", 1)).toBe('Showing 1 of 4 for "poster"');
  });

  it("lists pending generations and clips for media", () => {
    const base = project();
    const pending = { ...base.generatedAssets[0]!, id: "gen-pending", status: "running" as const, outputs: [], name: "Lab bench" };
    const withPending = { ...base, generatedAssets: [...base.generatedAssets, pending] };
    expect(pendingGenerations(withPending, "all", "", null).map((asset) => asset.id)).toEqual(["gen-pending"]);
    expect(pendingGenerations(withPending, "images", "", null)).toEqual([]);
    expect(pendingGenerations(withPending, "generated", "bench", null)).toHaveLength(1);
    expect(pendingGenerations(withPending, "all", "", "folder-takes")).toEqual([]);
    expect(mediaClips(base, "media-voiceover").length).toBeGreaterThan(0);
  });

  it("keeps failed generations and missing downloads until they are retried", () => {
    const base = project();
    const template = base.generatedAssets[0]!;
    const failed = { ...template, id: "gen-failed", status: "failed" as const, outputs: [] };
    const missingDownload = { ...template, id: "gen-download", outputs: [{ ...template.outputs[0]!, mediaId: "media-missing", sourceUrl: "https://cdn/out.mp4" }] };
    const withFailures = { ...base, generatedAssets: [...base.generatedAssets, failed, missingDownload] };
    expect(pendingGenerations(withFailures, "all", "", null).map((asset) => asset.id)).toEqual(["gen-failed", "gen-download"]);
    const retry = { ...template, id: "gen-retry", status: "queued" as const, outputs: [], retryOfAssetId: "gen-failed" };
    expect(pendingGenerations({ ...withFailures, generatedAssets: [...withFailures.generatedAssets, retry] }, "all", "", null).map((asset) => asset.id)).toEqual(["gen-download", "gen-retry"]);
  });
});
