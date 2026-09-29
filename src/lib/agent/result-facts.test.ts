import { describe, expect, it } from "vitest";
import { clockLabel, resultFacts, reviewPlacements } from "@/lib/agent/result-facts";
import { generationCostLabel } from "@/lib/generation/clip-actions";
import type { CodexProposalImpact, ProjectAction } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";

type GeneratedAssetAction = Extract<ProjectAction, { type: "recordGeneratedAsset" }>;

const impact: CodexProposalImpact = {
  summary: "Tightened the pacing.",
  beforeDurationSeconds: 130,
  afterDurationSeconds: 45,
  affectedItemIds: ["item-1"],
  affectedRanges: [{ startSeconds: 10, endSeconds: 14 }],
  previewTimestamp: 10,
};

const idPattern = /media-\d|item-\d|caption-\d|track-|sample-generated|asset-|folder-/;

function caption(id: string, startSeconds: number, text = `Line ${id}`): TimelineItem {
  return {
    id,
    kind: "caption",
    startSeconds,
    durationSeconds: 1,
    source: { type: "text", text },
    label: `Caption ${id}`,
    properties: {},
  };
}

function title(id: string, startSeconds: number): TimelineItem {
  return {
    id,
    kind: "overlay",
    startSeconds,
    durationSeconds: 2,
    source: { type: "text", text: "Opening Hook" },
    label: "Opening title",
    properties: { templateId: "kinetic-lower-third-v1" },
  };
}

function generatedAsset(
  id: string,
  overrides: Partial<GeneratedAssetAction["asset"]> = {},
): GeneratedAssetAction {
  return {
    type: "recordGeneratedAsset",
    asset: {
      id,
      kind: "video",
      status: "queued",
      name: "Lab shot",
      targetFolderId: "folder-generated",
      placementIntent: "library",
      prompt: "A wide shot of a restored lab bench",
      model: { provider: "replicate", id: "bytedance/seedance-2.0-fast" },
      references: { mediaIds: [], firstFrameMediaId: null, lastFrameMediaId: null },
      settings: {
        width: 1280,
        height: 720,
        durationSeconds: 6,
        fps: 24,
        aspectRatio: "16:9",
      },
      outputs: [],
      createdAt: "2026-09-15T00:00:00Z",
      parentAssetId: null,
      retryOfAssetId: null,
      ...overrides,
    },
  };
}

const labels = (facts: readonly { label: string }[]) => facts.map((fact) => fact.label);

describe("clockLabel", () => {
  it("formats minutes and seconds, adding hours only when needed", () => {
    expect([0, 4.4, 45, 130, 3725, Number.NaN, -3].map(clockLabel)).toEqual([
      "0:00",
      "0:04",
      "0:45",
      "2:10",
      "1:02:05",
      "0:00",
      "0:00",
    ]);
  });
});

describe("resultFacts", () => {
  it("formats the duration change and notes an unchanged length", () => {
    const project = fixtureProject();
    expect(resultFacts(impact, [], project, project)).toEqual([
      { kind: "duration", label: "2:10 → 0:45" },
      { kind: "replaces", label: "Replaces nothing" },
    ]);
    expect(labels(resultFacts({ ...impact, afterDurationSeconds: 130.2 }, [], project, null))).toEqual([
      "2:10 (unchanged)",
      "Replaces nothing",
    ]);
  });

  it("falls back to project durations when the impact has none", () => {
    const before = fixtureProject();
    const after = fixtureProject();
    after.timeline.durationSeconds = 6;
    const missing = { ...impact, beforeDurationSeconds: Number.NaN, afterDurationSeconds: Number.NaN };
    expect(labels(resultFacts(missing, [], before, after))[0]).toBe("0:08 → 0:06");
  });

  it("counts cuts, captions, and titles per action type", () => {
    const project = fixtureProject();
    const captions = Array.from({ length: 38 }, (_, index) => caption(`new-caption-${index}`, index));
    const actions: ProjectAction[] = [
      {
        type: "splitItems",
        splits: [
          { itemId: "item-1", newItemId: "item-1b", splitSeconds: 1 },
          { itemId: "item-1b", newItemId: "item-1c", splitSeconds: 2 },
        ],
      },
      {
        type: "rippleDeleteRanges",
        ranges: Array.from({ length: 12 }, (_, index) => ({
          startSeconds: index,
          endSeconds: index + 0.5,
          trackIds: ["track-video"],
        })),
      },
      { type: "addItems", targetTrackId: "track-captions", items: captions },
      { type: "insertItems", targetTrackId: "track-overlays", insertSeconds: 0, items: [title("new-title", 0)] },
      { type: "editCaptionText", itemId: "caption-1", text: "Edited" },
    ];
    expect(labels(resultFacts(impact, actions, project, project))).toEqual([
      "2:10 → 0:45",
      "14 cuts",
      "38 captions",
      "1 title",
      "Replaces nothing",
    ]);
    const singles: ProjectAction[] = [
      { type: "splitItems", splits: [{ itemId: "item-1", newItemId: "item-1b", splitSeconds: 1 }] },
      { type: "addItems", targetTrackId: "track-captions", items: [caption("one", 0)] },
    ];
    expect(labels(resultFacts(impact, singles, project, project)).slice(1, 3)).toEqual(["1 cut", "1 caption"]);
  });

  it("summarizes generation count, kind, duration, provider, and cost", () => {
    const project = fixtureProject();
    const actions = [generatedAsset("asset-a"), generatedAsset("asset-b"), generatedAsset("asset-c")];
    const credits = generationCostLabel(actions[0]!.asset, null, 3).replace(/^Est\. (\d+) credits?$/, "$1");
    expect(Number(credits)).toBeGreaterThan(0);
    expect(resultFacts(impact, actions, project, null)).toEqual([
      { kind: "duration", label: "2:10 → 0:45" },
      { kind: "generation", label: "3 × video · 6s" },
      { kind: "provider", label: "Replicate" },
      { kind: "cost", label: `≈ ${credits} credits` },
      { kind: "replaces", label: "Replaces nothing" },
    ]);
  });

  it("groups mixed generations and reports a varying cost", () => {
    const project = fixtureProject();
    const actions: ProjectAction[] = [
      generatedAsset("asset-a"),
      generatedAsset("asset-b", {
        kind: "image",
        model: { provider: "openai", id: "gpt-image-2" },
        settings: { width: 1024, height: 1024, durationSeconds: null, fps: null, aspectRatio: "1:1" },
      }),
      generatedAsset("asset-c", { model: { provider: "fal.ai", id: "fal-ai/video-upscaler" } }),
    ];
    expect(labels(resultFacts(impact, actions, project, null))).toEqual([
      "2:10 → 0:45",
      "2 × video · 6s",
      "1 × image",
      "Replicate, OpenAI, fal.ai",
      "Cost varies",
      "Replaces nothing",
    ]);
  });

  it("names what gets replaced by human name, never by id", () => {
    const project = fixtureProject();
    const replaceOne: ProjectAction[] = [
      { type: "replaceTimelineItemWithGeneratedOutput", replacement: { itemId: "item-1", mediaId: "media-9" } },
    ];
    expect(labels(resultFacts(impact, replaceOne, project, null)).pop()).toBe("Replaces “Opening clip”");

    const removeMany: ProjectAction[] = [
      { type: "removeItems", itemIds: ["item-1", "caption-1", "missing-item"] },
      generatedAsset("asset-a", { placementIntent: "replace:item-1" }),
    ];
    expect(labels(resultFacts(impact, removeMany, project, null)).pop()).toBe("Replaces 3 items");
    expect(labels(resultFacts(impact, [{ type: "removeItems", itemIds: ["missing-item"] }], project, null)).pop()).toBe(
      "Replaces 1 item",
    );
    for (const action of [replaceOne, removeMany]) {
      expect(labels(resultFacts(impact, action, project, null)).join(" ")).not.toMatch(idPattern);
    }
  });
});

describe("resultFacts for clip playback edits", () => {
  it("counts reversed clips, speed changes, and the audio clip Detach audio adds", () => {
    const project = fixtureProject();
    const actions: ProjectAction[] = [
      { type: "updateClipReverse", itemId: "item-1", reverse: true },
      { type: "updateClipReverse", itemId: "music-bed", reverse: false },
      { type: "updateAudioClipSpeed", itemId: "music-bed", speed: 1.5 },
      { type: "updateVisualClipSpeed", itemId: "item-1", speed: 2 },
      { type: "detachAudio", itemId: "item-1", audioItemId: "item-1-audio", targetTrackId: "track-audio-2", linkGroupId: "link-1" },
    ];
    const facts = resultFacts({ ...impact, afterDurationSeconds: 130 }, actions, project, null);
    expect(facts).toEqual([
      { kind: "duration", label: "2:10 (unchanged)" },
      { kind: "reverse", label: "1 reversed clip" },
      { kind: "reverse", label: "1 clip played forward" },
      { kind: "speed", label: "2 speed changes" },
      { kind: "detachedAudio", label: "1 detached audio clip" },
      { kind: "replaces", label: "Replaces nothing" },
    ]);
    expect(labels(facts).join(" ")).not.toMatch(idPattern);
  });
});

describe("reviewPlacements", () => {
  it("describes reverse, speed, and detach audio edits by clip name", () => {
    const project = fixtureProject();
    const lines = reviewPlacements(
      [
        { type: "updateClipReverse", itemId: "item-1", reverse: true },
        { type: "updateClipReverse", itemId: "music-bed", reverse: false },
        { type: "updateClipReverse", itemId: "missing-item", reverse: true },
        { type: "updateAudioClipSpeed", itemId: "music-bed", speed: 1.5 },
        { type: "updateVisualClipSpeed", itemId: "item-1", speed: 0.333333 },
        { type: "createTrack", track: { id: "track-audio-2", name: "Detached", kind: "audio", locked: false, items: [] } },
        { type: "detachAudio", itemId: "item-1", audioItemId: "item-1-audio", targetTrackId: "track-audio-2", linkGroupId: "link-1" },
      ],
      project,
    );
    expect(lines).toEqual([
      "Reverse “Opening clip”",
      "Play “Music bed” forward",
      "Reverse a clip",
      "Set “Music bed” to 1.5× speed",
      "Set “Opening clip” to 0.33× speed",
      "Add an audio track “Audio 2”",
      "Detach audio from “Opening clip”",
      "Add “Opening clip audio” on Audio 2 at 0:00",
    ]);
    expect(lines.join("\n")).not.toMatch(idPattern);
  });

  it("describes generation placements with human names and timecodes", () => {
    const project = fixtureProject();
    const lines = reviewPlacements(
      [
        generatedAsset("asset-a", { placementIntent: "timeline", settings: { ...generatedAsset("x").asset.settings, timelineStartSeconds: 72 } }),
        generatedAsset("asset-b", { name: "  ", prompt: "A slow push-in on the restored phonograph horn in warm light" }),
        generatedAsset("asset-c", { placementIntent: "replace:sample-generated-clip", kind: "audio", targetFolderId: null }),
        { type: "replaceTimelineItemWithGeneratedOutput", replacement: { itemId: "item-1", mediaId: "sample-generated-output" } },
      ],
      project,
    );
    expect(lines).toEqual([
      "Generate video “Lab shot” and place it on the timeline at 1:12",
      "Generate video “A slow push-in on the restored phonograph…” into the media library (Generated selects)",
      "Generate audio “Lab shot” to replace “Restored Edison alternate” at 0:04",
      "Replace “Opening clip” at 0:00 with “Bundled Edison restoration”",
    ]);
  });

  it("describes timeline placements, cuts, and removals", () => {
    const project = fixtureProject();
    const lines = reviewPlacements(
      [
        { type: "createTrack", track: { id: "track-new", name: "B-roll", kind: "video", locked: false, items: [] } },
        { type: "addItems", targetTrackId: "track-captions", items: [caption("c1", 5, "Welcome back to the lab")] },
        { type: "addItems", targetTrackId: "track-captions", items: Array.from({ length: 4 }, (_, i) => caption(`m${i}`, 60 + i)) },
        {
          type: "insertItems",
          targetTrackId: "track-new",
          insertSeconds: 3,
          items: [{ ...title("t1", 3), kind: "video_clip", label: "", source: { type: "media", mediaId: "media-1" }, properties: {} }],
        },
        { type: "addItems", targetTrackId: "track-overlays", items: [title("t2", 1)] },
        { type: "moveItems", moves: [{ itemId: "caption-2", targetTrackId: "track-captions", startSeconds: 6 }] },
        { type: "splitItems", splits: [{ itemId: "item-1", newItemId: "item-1b", splitSeconds: 2 }] },
        { type: "rippleDeleteRanges", ranges: [{ startSeconds: 10, endSeconds: 14.2, trackIds: ["track-video", "track-captions"] }] },
        { type: "rippleDeleteRanges", ranges: [{ startSeconds: 20, endSeconds: 21, trackIds: [] }, { startSeconds: 30, endSeconds: 31, trackIds: [] }, { startSeconds: 40, endSeconds: 41, trackIds: [] }, { startSeconds: 50, endSeconds: 52, trackIds: [] }] },
        { type: "removeItems", itemIds: ["music-bed", "unknown-item"] },
        { type: "deleteMedia", mediaIds: ["media-voiceover"] },
        { type: "removeTracks", trackIds: ["track-scenes"] },
        { type: "createTimeline", timelineId: "timeline-2", name: "Short cut", duplicateActive: true },
        { type: "editCaptionText", itemId: "caption-1", text: "Not a placement" },
      ],
      project,
    );
    expect(lines).toEqual([
      "Add a video track “Video 2”",
      "Add caption “Welcome back to the lab” on Captions at 0:05",
      "Add 4 captions on Captions from 1:00 to 1:04",
      "Insert “input.mp4” on Video 2 at 0:03",
      "Add title “Opening Hook” on Text 1 at 0:01",
      "Move caption “Second clean split” to Captions at 0:06",
      "Split “Opening clip” at 0:02",
      "Cut 0:10–0:14 on Video 1, Captions and close the gap",
      "Cut 4 ranges (5s total) and close the gaps",
      "Remove “Music bed” from Audio 1 at 0:00",
      "Remove 1 item",
      "Delete “voiceover.m4a” and its clips from the project",
      "Remove the Graphics 1 track",
      "Create timeline “Short cut”",
    ]);
    expect(lines.join("\n")).not.toMatch(idPattern);
  });

  // Same names as the timeline track headers (`trackDisplayNames`), including tracks the bundle creates.
  it("names tracks the way the timeline track headers do", () => {
    const project = fixtureProject();
    const lines = reviewPlacements(
      [
        { type: "createTrack", track: { id: "track-audio-2", name: "Music", kind: "audio", locked: false, items: [] } },
        { type: "createTrack", track: { id: "track-captions-2", name: "Subtitles", kind: "caption", locked: false, items: [] } },
        { type: "addItems", targetTrackId: "track-captions-2", items: [caption("c9", 2, "Second language")] },
        { type: "rippleDeleteRanges", ranges: [{ startSeconds: 1, endSeconds: 2, trackIds: ["track-video", "track-captions", "track-audio"] }] },
      ],
      project,
    );
    expect(lines).toEqual([
      "Add an audio track “Audio 2”",
      "Add a caption track “Captions 2”",
      "Add caption “Second language” on Captions 2 at 0:02",
      "Cut 0:01–0:02 on Video 1, Captions, Audio 1 and close the gap",
    ]);
  });

  it("describes transition changes between named clips", () => {
    const base = fixtureProject();
    const fade = { id: "fade-1", leftItemId: "item-1", rightItemId: "sample-generated-clip", kind: "crossfade" as const, durationSeconds: 1 };
    const project = {
      ...base,
      timeline: {
        ...base.timeline,
        tracks: base.timeline.tracks.map((track) => (track.id === "track-video" ? { ...track, transitions: [fade] } : track)),
      },
    };
    const lines = reviewPlacements(
      [
        { type: "addTransition", trackId: "track-video", transition: { ...fade, id: "dip", kind: "dipToBlack" } },
        { type: "addTransition", trackId: "track-video", transition: { ...fade, id: "wipe", kind: "wipe", rightItemId: "unknown" } },
        { type: "updateTransition", trackId: "track-video", transitionId: "fade-1", kind: "wipe", durationSeconds: 0.5 },
        { type: "updateTransition", trackId: "track-video", transitionId: "fade-1", durationSeconds: 1.25 },
        { type: "updateTransition", trackId: "track-video", transitionId: "missing", kind: "dipToWhite" },
        { type: "removeTransition", trackId: "track-video", transitionId: "fade-1" },
      ],
      project,
    );
    expect(lines).toEqual([
      "Add a dip to black between “Opening clip” and “Restored Edison alternate”",
      "Add a wipe between two clips",
      "Change the transition between “Opening clip” and “Restored Edison alternate” to a 0.5s wipe",
      "Set the transition between “Opening clip” and “Restored Edison alternate” to 1.25s",
      "Change the transition to a dip to white",
      "Remove the crossfade between “Opening clip” and “Restored Edison alternate”",
    ]);
    expect(lines.join("\n")).not.toMatch(idPattern);
  });
});
