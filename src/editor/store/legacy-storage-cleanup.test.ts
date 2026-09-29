import { beforeEach, describe, expect, it } from "vitest";
import { cleanLegacyEditorStorage, legacyStorageCleanedKey } from "./legacy-storage-cleanup";

const legacyEntries = [
  "video-creater.workspace-layout.v1",
  "video-creater:responsive-rail-state",
  "video-creater.editorTour.dismissed.v1",
  "video-creater.codexChatTranscript.v1",
  "video-creater.codexChatTranscript.v1:%2Fprojects%2Fdemo",
  "video-creater.timeline-view-state:/projects/demo",
];

const keptEntries = [
  "video-creater.editor.v2.layout",
  "video-creater.editor.v2.timeline-view:/projects/demo",
  "video-creater.editor.v2.agent.autoApplySafe",
  "video-creater.appSettings.v1",
  "video-creater.recentProjects",
  "video-creater.workspace-layout.v1.backup",
  "other-app.timeline-view-state:/projects/demo",
];

function storedKeys(): string[] {
  return Object.keys(window.localStorage).sort();
}

describe("cleanLegacyEditorStorage", () => {
  beforeEach(() => {
    window.localStorage.clear();
    for (const key of [...legacyEntries, ...keptEntries]) window.localStorage.setItem(key, "value");
  });

  it("removes only the listed v1 keys and prefixes and leaves v2 and app keys untouched", () => {
    cleanLegacyEditorStorage();

    expect(storedKeys()).toEqual([...keptEntries, legacyStorageCleanedKey].sort());
    for (const key of keptEntries) expect(window.localStorage.getItem(key)).toBe("value");
  });

  it("runs once per profile", () => {
    cleanLegacyEditorStorage();
    window.localStorage.setItem("video-creater.workspace-layout.v1", "written later");

    cleanLegacyEditorStorage();

    expect(window.localStorage.getItem("video-creater.workspace-layout.v1")).toBe("written later");
  });

  it("does nothing without storage and survives storage that throws", () => {
    expect(() => cleanLegacyEditorStorage(null)).not.toThrow();
    const blocked = {
      get length(): number {
        throw new Error("blocked");
      },
      getItem: () => null,
    } as unknown as Storage;

    expect(() => cleanLegacyEditorStorage(blocked)).not.toThrow();
    expect(storedKeys()).toContain("video-creater.workspace-layout.v1");
  });
});
