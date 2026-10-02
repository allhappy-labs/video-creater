import { describe, expect, it, vi } from "vitest";
import type { PreparedProjectPreview } from "@/lib/project";
import { buildTimelinePreviewFrame, type TimelinePreviewFrame, type TimelinePreviewLayer } from "@/lib/timeline-preview";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { crossfade, sourceClip, transitionTestProject } from "../timeline/transition-test-project";
import {
  CanonicalFramePreloader,
  canonicalCoverageItemIds,
  canonicalFrameLayers,
  canonicalFrameSequences,
  canonicalPreloadUrls,
  canonicalStateForProject,
  preparedAudioLayers,
  preparedResultForProject,
  type CanonicalFrameSequence,
} from "./canonical-frames";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function layer(itemId: string, mediaId: string, canonicalPreparationRequired = false): TimelinePreviewLayer {
  const frame = buildTimelinePreviewFrame({ ...fixtureProject(), playheadSeconds: 1 });
  const base = frame.layers[0];
  if (!base) throw new Error("fixture frame has no visual layer");
  return { ...base, itemId, mediaId, canonicalPreparationRequired };
}

function frameWith(layers: TimelinePreviewLayer[]): TimelinePreviewFrame {
  return { status: "ready", playheadSeconds: 0, layers, audioLayers: [], overlayLayers: [], issues: [] };
}

const sequence: CanonicalFrameSequence = {
  itemId: "flatten-1",
  preparedMediaId: "prepared-1",
  startSeconds: 1,
  durationSeconds: 2,
  fps: 10,
  frameUrls: Array.from({ length: 20 }, (_, index) => `/frames/${index}.png`),
};

describe("canonical state", () => {
  it("only honours a preparation started for the same project object", () => {
    const project = fixtureProject();
    const copy = structuredClone(project);
    const failed = { sourceProject: project, status: "failed", message: "decoder crashed" } as const;
    expect(canonicalStateForProject(failed, project, true)).toEqual({ status: "failed", message: "decoder crashed" });
    expect(canonicalStateForProject(failed, copy, true)).toEqual({ status: "pending" });
    expect(canonicalStateForProject(failed, copy, false)).toBeNull();
    expect(canonicalStateForProject({ sourceProject: project, status: "unavailable" }, project, true)).toBeNull();

    const result = { project, reports: [], frameSequences: [] } satisfies PreparedProjectPreview;
    const ready = { sourceProject: project, status: "ready", result } as const;
    expect(canonicalStateForProject(ready, project, true)).toEqual({ status: "ready" });
    expect(preparedResultForProject(ready, project)).toBe(result);
    expect(preparedResultForProject(ready, copy)).toBeNull();
  });
});

describe("canonical frame selection", () => {
  it("preserves frame positions when media URLs are temporarily unresolved", () => {
    const project = fixtureProject();
    const result: PreparedProjectPreview = {
      project,
      reports: [],
      frameSequences: [{ ...sequence, startSeconds: 0, fps: 1, durationSeconds: 3, framePaths: ["cache/0.png", "../escape.png", "cache/2.png"] }],
    };
    const sequences = canonicalFrameSequences(result, "/p");
    expect(sequences[0]?.frameUrls).toEqual(["/p/cache/0.png", null, "/p/cache/2.png"]);
    expect(canonicalFrameLayers(frameWith([layer("flatten-1", "prepared-1")]), sequences, 1)).toEqual([]);
    expect(canonicalFrameLayers(frameWith([layer("flatten-1", "prepared-1")]), sequences, 2)[0]?.frameUrl).toBe("/p/cache/2.png");
  });

  it("picks floor((t - start) * fps) clamped to the sequence", () => {
    const prepared = frameWith([layer("flatten-1", "prepared-1")]);
    expect(canonicalFrameLayers(prepared, [sequence], 1.25)[0]?.frameUrl).toBe("/frames/2.png");
    expect(canonicalFrameLayers(prepared, [sequence], 2.99)[0]?.frameUrl).toBe("/frames/19.png");
    expect(canonicalFrameLayers(prepared, [sequence], 3)).toEqual([]);
    expect(canonicalFrameLayers(prepared, [{ ...sequence, fps: 0 }], 1.25)).toEqual([]);
    expect(canonicalFrameLayers(frameWith([layer("other", "prepared-2")]), [sequence], 1.25)).toEqual([]);
  });

  it("extends flattened coverage to direct layers that need preparation", () => {
    const prepared = frameWith([layer("flatten-1", "prepared-1")]);
    const direct = frameWith([layer("item-a", "media-1", true), layer("item-b", "media-1", false)]);
    const covered = canonicalCoverageItemIds(canonicalFrameLayers(prepared, [sequence], 1.5), prepared, direct);
    expect([...covered]).toEqual(["flatten-1", "item-a"]);

    const plain = frameWith([layer("item-a", "prepared-1")]);
    expect([...canonicalCoverageItemIds(canonicalFrameLayers(plain, [sequence], 1.5), plain, direct)]).toEqual(["item-a"]);
  });

  it("covers both clips of a transition baked into a flattened composite", () => {
    const blended = sourceClip("a", 0, 2, 0);
    blended.properties.blendMode = "screen";
    const dip = { ...crossfade(1), kind: "dipToBlack" as const };
    const direct = buildTimelinePreviewFrame({ ...transitionTestProject([blended, sourceClip("b", 2, 2, 2)], [dip]), playheadSeconds: 2.25 });
    expect(direct.transitions).toMatchObject([{ transitionId: "fade", flattened: true }]);
    const prepared = frameWith([layer("flatten-1", "prepared-1")]);
    const frameLayers = canonicalFrameLayers(prepared, [sequence], 2.25);
    expect([...canonicalCoverageItemIds(frameLayers, prepared, direct)]).toEqual(["flatten-1", "a", "b"]);
    // Nothing extra without an active flattened frame, or for a pair the render draws natively.
    expect([...canonicalCoverageItemIds([], null, direct)]).toEqual([]);
    const plain = buildTimelinePreviewFrame({ ...transitionTestProject(undefined, [dip]), playheadSeconds: 2.25 });
    expect([...canonicalCoverageItemIds(frameLayers, prepared, plain)]).toEqual(["flatten-1"]);
  });

  it("covers plain lower-track clips inside a flattened group", () => {
    const base = sourceClip("a", 0, 4, 0);
    const top = sourceClip("b", 1, 2, 0);
    top.properties.blendMode = "screen";
    const project = transitionTestProject([base]);
    const [video, audio] = project.timeline.tracks;
    if (!video || !audio) throw new Error("fixture tracks");
    project.timeline.tracks = [video, { ...video, id: "v2", name: "v2", items: [top] }, audio];
    const direct = buildTimelinePreviewFrame({ ...project, playheadSeconds: 1.5 });
    expect(direct.flattenedCoverItemIds).toEqual(["a", "b"]);
    const prepared = frameWith([layer("flatten-1", "prepared-1")]);
    const frameLayers = canonicalFrameLayers(prepared, [sequence], 1.5);
    expect([...canonicalCoverageItemIds(frameLayers, prepared, direct)]).toEqual(["flatten-1", "b", "a"]);
    // Without an active flattened frame the lower clip still draws directly.
    expect([...canonicalCoverageItemIds([], null, direct)]).toEqual([]);
  });

  it("draws a reversed video clip from its prepared frames and resolves its prepared reversed audio", () => {
    const project = fixtureProject();
    for (const track of project.timeline.tracks) {
      for (const item of track.items) if (item.id === "item-1" || item.id === "music-bed") item.properties.reverse = true;
    }
    const prepared = structuredClone(project);
    prepared.media.push(
      { id: "precompose-rev", relativePath: "cache/precompose/v1/sha256/ab/rev/intermediate.mov", kind: "video", durationSeconds: 4, width: 1920, height: 1080, fps: 24 },
      { id: "audio-reverse-abc", relativePath: "cache/audio-reverse/v1/sha256/ab/abc/output.wav", kind: "audio", durationSeconds: 4, width: null, height: null, fps: null },
    );
    for (const track of prepared.timeline.tracks) {
      for (const item of track.items) {
        if (item.id !== "item-1" && item.id !== "music-bed") continue;
        delete item.properties.reverse;
        item.source = { type: "media", mediaId: item.id === "item-1" ? "precompose-rev" : "audio-reverse-abc" };
      }
    }
    const direct = buildTimelinePreviewFrame({ ...project, playheadSeconds: 1 });
    expect(direct.layers.find((entry) => entry.itemId === "item-1")?.canonicalPreparationRequired).toBe(true);
    const preparedFrame = buildTimelinePreviewFrame({ ...prepared, playheadSeconds: 1 });
    const reversedFrames = { ...sequence, itemId: "item-1", preparedMediaId: "precompose-rev", startSeconds: 0, durationSeconds: 4 };
    const frameLayers = canonicalFrameLayers(preparedFrame, [reversedFrames], 1);
    expect(frameLayers.map(({ layer: entry, frameUrl }) => [entry.itemId, frameUrl])).toEqual([["item-1", "/frames/10.png"]]);
    expect([...canonicalCoverageItemIds(frameLayers, preparedFrame, direct)]).toEqual(["item-1"]);

    expect(preparedAudioLayers(preparedFrame, "/p")).toEqual([
      {
        layer: expect.objectContaining({ itemId: "music-bed", mediaId: "audio-reverse-abc", sourceTimeSeconds: 1, canonicalPreparationRequired: false }),
        sourceUrl: "/p/cache/audio-reverse/v1/sha256/ab/abc/output.wav",
      },
    ]);
    expect(preparedAudioLayers(null, "/p")).toEqual([]);
  });

  it("preloads two frames behind and twelve ahead", () => {
    expect(canonicalPreloadUrls([sequence], 1.5)).toEqual(
      Array.from({ length: 15 }, (_, index) => `/frames/${index + 3}.png`),
    );
    expect(canonicalPreloadUrls([sequence], 0.5)).toEqual([]);
  });
});

describe("CanonicalFramePreloader", () => {
  it("bounds stalled image admissions and ignores callbacks from cleared loads", () => {
    const images: HTMLImageElement[] = [];
    const preloader = new CanonicalFramePreloader(() => {
      const image = document.createElement("img");
      images.push(image);
      return image;
    });
    preloader.preload(Array.from({ length: 1200 }, (_, index) => `/pending-${index}.png`));
    expect(images).toHaveLength(480);
    const oldLoaded = images[0]?.onload;
    preloader.clear();
    preloader.preload(["/pending-0.png"]);
    oldLoaded?.call(images[0]!, new Event("load"));
    expect(preloader.isLoaded("/pending-0.png")).toBe(false);
    images.at(-1)?.onload?.(new Event("load"));
    expect(preloader.isLoaded("/pending-0.png")).toBe(true);
  });
  it("loads a URL once and evicts the oldest past 480 loaded URLs", () => {
    const images: HTMLImageElement[] = [];
    const preloader = new CanonicalFramePreloader(() => {
      const image = document.createElement("img");
      images.push(image);
      return image;
    });
    preloader.preload(["/a.png", "/a.png"]);
    expect(images).toHaveLength(1);
    images[0]?.onload?.(new Event("load"));
    preloader.preload(["/a.png"]);
    expect(images).toHaveLength(1);

    const urls = Array.from({ length: 480 }, (_, index) => `/f${index}.png`);
    preloader.preload(urls);
    for (const image of images.slice(1)) image.onload?.(new Event("load"));
    expect(preloader.isLoaded("/a.png")).toBe(false);
    expect(preloader.isLoaded("/f479.png")).toBe(true);

    preloader.preload(["/broken.png"]);
    images.at(-1)?.onerror?.(new Event("error"));
    preloader.preload(["/broken.png"]);
    expect(images.at(-1)?.src).toContain("/broken.png");
    expect(images).toHaveLength(483);
  });
});
