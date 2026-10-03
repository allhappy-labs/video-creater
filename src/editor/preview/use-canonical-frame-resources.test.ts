import { expect, it } from "vitest";
import type { PreparedProjectPreview } from "@/lib/project";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { canonicalFrameResourcePaths } from "./use-canonical-frame-resources";

it("authorizes bounded chunks ahead of playback instead of a new ticket on every frame", () => {
  const result: PreparedProjectPreview = { project: fixtureProject(), reports: [], frameSequences: [{
    itemId: "item-1", preparedMediaId: "media-1", startSeconds: 0, durationSeconds: 60, fps: 30,
    framePaths: Array.from({ length: 1800 }, (_, index) => `cache/${index}.png`),
  }] };
  expect(canonicalFrameResourcePaths(result, 0)).toEqual(canonicalFrameResourcePaths(result, 0.3));
  const seek = canonicalFrameResourcePaths(result, 40);
  expect(seek).toContain("cache/1200.png");
  expect(seek).toContain("cache/1223.png");
  expect(seek.length).toBeLessThanOrEqual(26);
});
