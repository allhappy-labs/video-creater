import { describe, expect, it } from "vitest";
import type { VideoProject } from "@/lib/project";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { projectWithTracks, testItem, track } from "../../properties/properties-test-utils";
import {
  cleanupTargetOptions,
  resolveCleanupTarget,
  speechAnalysisItem,
  targetAudioItems,
  targetSilenceRanges,
  type CleanupTarget,
} from "./cleanup-target";

function project(): VideoProject {
  const clip = (id: string, startSeconds: number, properties: Record<string, unknown> = {}) => ({
    ...testItem(id, "video_clip", { sourceIn: startSeconds, sourceOut: startSeconds + 4, ...properties }),
    startSeconds,
  });
  return projectWithTracks(
    [
      track("v1", "video", [clip("first", 0, { linkGroupId: "g1" }), clip("second", 4)]),
      track("a1", "audio", [{ ...testItem("first-audio", "audio_clip", { linkGroupId: "g1", sourceIn: 0, sourceOut: 4 }, "media-1") }]),
    ],
    {
      mediaSilenceRanges: [
        { mediaId: "media-1", sourceIn: 1, sourceOut: 2, confidence: 0.9 },
        { mediaId: "media-1", sourceIn: 5, sourceOut: 6.5, confidence: 0.9 },
        { mediaId: "media-voiceover", sourceIn: 1, sourceOut: 3, confidence: 0.9 },
      ],
    } as Partial<VideoProject>,
  );
}

function target(value: string, base = project()): CleanupTarget {
  const resolved = resolveCleanupTarget(base, value);
  if (!resolved) throw new Error(`no target for ${value}`);
  return resolved;
}

describe("cleanup targets", () => {
  it("offers the selected clip first, then timeline sources before unused ones", () => {
    expect(cleanupTargetOptions(project(), ["second"])).toEqual([
      { value: "item:second", label: "Selected clip · second" },
      { value: "media:media-1", label: "input.mp4" },
      { value: "media:media-voiceover", label: "voiceover.m4a" },
      { value: "media:sample-generated-output", label: "product-reveal.mp4" },
    ]);
    expect(cleanupTargetOptions(project(), [])[0]).toEqual({ value: "media:media-1", label: "input.mp4" });
  });

  it("skips selected clips without speech media", () => {
    const base = fixtureProject();
    expect(cleanupTargetOptions(base, ["caption-1"])[0]?.value).toBe("media:media-1");
  });

  it("resolves a media target to every clip of that source", () => {
    expect(target("media:media-1").items.map((item) => item.id)).toEqual(["first", "second", "first-audio"]);
    expect(resolveCleanupTarget(project(), "item:missing")).toBeNull();
    expect(resolveCleanupTarget(project(), "media:missing")).toBeNull();
    expect(resolveCleanupTarget(project(), null)).toBeNull();
  });

  it("limits silence ranges to the target media and, for a clip, its span", () => {
    const whole = targetSilenceRanges(project(), target("media:media-1"));
    expect(whole.map((range) => [range.startSeconds, range.endSeconds])).toEqual([
      [1.12, 1.88],
      [5.12, 6.38],
    ]);
    const clip = targetSilenceRanges(project(), target("item:second"));
    expect(clip.map((range) => [range.startSeconds, range.endSeconds])).toEqual([[5.12, 6.38]]);
  });

  it("resolves audio clips through links and prefers them for speech analysis", () => {
    const base = project();
    expect(targetAudioItems(base, target("item:first", base)).map((item) => item.id)).toEqual(["first-audio"]);
    expect(targetAudioItems(base, target("item:second", base))).toEqual([]);
    expect(speechAnalysisItem(base, target("item:second", base))?.id).toBe("second");
    expect(speechAnalysisItem(base, target("media:media-1", base))?.id).toBe("first-audio");
  });
});
