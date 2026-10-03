import { describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { buildTimelinePreviewFrame } from "@/lib/timeline-preview";
import { fixtureItem, fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";
import { crossfade, sourceClip, transitionTestProject } from "../timeline/transition-test-project";
import type { CompositorCanonicalState } from "./canonical-frames";
import { buildCompositorModel, noProjectFolderMessage, type CompositorModelInput } from "./compositor-model";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

const urls = { "media-1": "/p/media/input.mp4", "media-voiceover": "/p/media/voiceover.m4a", "sample-generated-output": "/p/gen.mp4" };

function input(project: VideoProject, overrides: Partial<CompositorModelInput> = {}): CompositorModelInput {
  const playheadSeconds = overrides.playheadSeconds ?? 1;
  return {
    frame: buildTimelinePreviewFrame({ ...project, playheadSeconds }),
    timeline: project.timeline,
    media: project.media,
    playheadSeconds,
    mediaPreviewUrls: urls,
    failedLayerIds: new Set(),
    canonical: null,
    coverageItemIds: new Set(),
    canonicalFrameCount: 0,
    preparedAudioLayers: [],
    ...overrides,
  };
}

// Captions start at 0.65 s, so tests of the empty canvas sit at 0.2 s.

/** The sample video clip with a richer blend mode, which needs canonical preparation. */
function canonicalProject(): VideoProject {
  const project = fixtureProject();
  fixtureItem(project, "video").properties.blendMode = "add";
  return project;
}

describe("buildCompositorModel", () => {
  it("waits for prepared frame tickets without asking to rebuild successful preparation", () => {
    const model = buildCompositorModel(input(canonicalProject(), { canonical: { status: "ready" }, mediaLoading: true, playheadSeconds: 0.2 }));
    expect(model.retryPreparesCanonical).toBe(false);
    expect(model.issues).not.toContain("Prepared frame missing for timeline item item-1. Rebuild the canonical preview.");
    expect(model.emptyStateCopy).toBe("Connecting prepared preview frames");
  });
  it("uses canonical animated frames for templates and exposes preparation failures", () => {
    const project = fixtureProject();
    project.timeline.tracks = [{
      id: "graphics", name: "Graphics", kind: "overlay", enabled: true, locked: false,
      items: [{ id: "template-1", kind: "overlay", startSeconds: 0, durationSeconds: 4,
        source: { type: "generated", artifactId: "orphan" }, label: "Lower third",
        properties: { templateId: "kinetic-lower-third-v1" } }],
    }];
    const fixture = buildCompositorModel(input(project));
    expect(fixture.visibleOverlayLayers).toHaveLength(1);
    const pending = buildCompositorModel(input(project, { canonical: { status: "pending" } }));
    expect(pending).toMatchObject({ visibleOverlayLayers: [], issueTitle: "Preparing preview", emptyStateCopy: "Preparing canonical preview" });
    const failed = buildCompositorModel(input(project, { canonical: { status: "failed", message: "Shader compile failed" } }));
    expect(failed).toMatchObject({ visibleOverlayLayers: [], issues: ["Shader compile failed"], retryPreparesCanonical: true });
    const missing = buildCompositorModel(input(project, { canonical: { status: "ready" } }));
    expect(missing).toMatchObject({ visibleOverlayLayers: [], issueTitle: "Prepared frame missing", retryPreparesCanonical: true });
    const ready = buildCompositorModel(input(project, { canonical: { status: "ready" }, coverageItemIds: new Set(["template-1"]), canonicalFrameCount: 1 }));
    expect(ready).toMatchObject({ visibleOverlayLayers: [], issues: [], emptyStateCopy: null });
  });

  it("draws the video and audio layers at the playhead with no issues", () => {
    const model = buildCompositorModel(input(fixtureProject()));
    expect(model.visibleMediaLayers.map(({ layer }) => layer.itemId)).toEqual(["item-1"]);
    expect(model.visibleAudioLayers.map(({ sourceUrl }) => sourceUrl)).toEqual(["/p/media/voiceover.m4a"]);
    expect(model).toMatchObject({ issues: [], issueState: "clear", emptyStateCopy: null, problemLayer: null });
  });

  it("reports failed layers first with a reload retry and a problem layer", () => {
    const model = buildCompositorModel(input(fixtureProject(), { failedLayerIds: new Set(["item-1", "music-bed"]) }));
    expect(model.visibleMediaLayers).toEqual([]);
    expect(model.issues).toEqual(["Timeline item item-1 failed to load.", "Timeline audio item music-bed failed to load."]);
    expect(model).toMatchObject({
      issueTitle: "Preview failed",
      issueState: "retry",
      retryReloadsLayers: true,
      retryPreparesCanonical: false,
      problemLayer: { itemId: "item-1", mediaId: "media-1" },
      failedLayerCount: 2,
    });
  });

  it("notes layers without preview URLs without offering retry", () => {
    const model = buildCompositorModel(input(fixtureProject(), { mediaPreviewUrls: {}, playheadSeconds: 0.2 }));
    expect(model.issues).toEqual([
      "Timeline item item-1 has no local preview URL.",
      "Timeline audio item music-bed has no local preview URL.",
    ]);
    expect(model).toMatchObject({ issueTitle: "Preview unavailable", issueState: "notice", emptyStateCopy: "No timeline media at playhead" });
  });

  it("hides layers awaiting canonical preparation while pending", () => {
    const model = buildCompositorModel(input(canonicalProject(), { canonical: { status: "pending" }, playheadSeconds: 0.2 }));
    expect(model.visibleMediaLayers).toEqual([]);
    expect(model).toMatchObject({
      issues: ["Preparing canonical Lottie, LUT, reversed-clip, or richer-blend preview frames."],
      issueTitle: "Preparing preview",
      issueState: "notice",
      emptyStateCopy: "Preparing canonical preview",
    });
  });

  it("offers a preparation retry after a backend failure but not without a project folder", () => {
    const failed: CompositorCanonicalState = { status: "failed", message: "Precompose sidecar crashed." };
    const model = buildCompositorModel(input(canonicalProject(), { canonical: failed, playheadSeconds: 0.2 }));
    expect(model).toMatchObject({
      issues: ["Precompose sidecar crashed."],
      issueState: "retry",
      retryPreparesCanonical: true,
      emptyStateCopy: "Canonical preview unavailable",
    });
    const noFolder = buildCompositorModel(input(canonicalProject(), { canonical: { status: "failed", message: noProjectFolderMessage } }));
    expect(noFolder).toMatchObject({ issueState: "notice", retryPreparesCanonical: false });
  });

  it("flags prepared frames missing for uncovered layers once ready", () => {
    const missing = buildCompositorModel(input(canonicalProject(), { canonical: { status: "ready" }, playheadSeconds: 0.2 }));
    expect(missing).toMatchObject({
      issues: ["Prepared frame missing for timeline item item-1. Rebuild the canonical preview."],
      issueTitle: "Prepared frame missing",
      retryPreparesCanonical: true,
      emptyStateCopy: "Canonical preview unavailable",
    });
    const covered = buildCompositorModel(
      input(canonicalProject(), { canonical: { status: "ready" }, coverageItemIds: new Set(["item-1"]), canonicalFrameCount: 1, playheadSeconds: 0.2 }),
    );
    expect(covered).toMatchObject({ issues: [], emptyStateCopy: null });
  });

  it("places a dip solid beneath the first drawn clip of its transition", () => {
    const project = transitionTestProject(undefined, [{ ...crossfade(1), kind: "dipToBlack" }]);
    const model = buildCompositorModel(input(project, { playheadSeconds: 2 }));
    expect(model.visibleMediaLayers.map(({ layer }) => layer.itemId)).toEqual(["a", "b"]);
    expect(model.transitionSolids).toEqual([{ transitionId: "fade", color: "black", beforeItemId: "a" }]);

    const outgoingFailed = buildCompositorModel(input(project, { playheadSeconds: 2, failedLayerIds: new Set(["a"]) }));
    expect(outgoingFailed.transitionSolids).toEqual([{ transitionId: "fade", color: "black", beforeItemId: "b" }]);
    const noUrls = buildCompositorModel(input(project, { playheadSeconds: 2, mediaPreviewUrls: {} }));
    expect(noUrls).toMatchObject({ transitionSolids: [{ beforeItemId: null }], emptyStateCopy: null });
    const crossfadeModel = buildCompositorModel(input(transitionTestProject(undefined, [crossfade(1)]), { playheadSeconds: 2 }));
    expect(crossfadeModel.transitionSolids).toEqual([]);
  });

  it("leaves a flattened transition's clips and dip solid to the prepared frame once ready", () => {
    const blended = sourceClip("a", 0, 2, 0);
    blended.properties.blendMode = "screen";
    const project = transitionTestProject([blended, sourceClip("b", 2, 2, 2)], [{ ...crossfade(1), kind: "dipToBlack" }]);
    const pending = buildCompositorModel(input(project, { playheadSeconds: 2, canonical: { status: "pending" } }));
    expect(pending.visibleMediaLayers.map(({ layer }) => layer.itemId)).toEqual(["b"]);
    expect(pending.transitionSolids).toEqual([{ transitionId: "fade", color: "black", beforeItemId: "b" }]);

    const covered = new Set(["flatten-1", "a", "b"]);
    const ready = buildCompositorModel(input(project, { playheadSeconds: 2, canonical: { status: "ready" }, coverageItemIds: covered, canonicalFrameCount: 1 }));
    expect(ready).toMatchObject({ visibleMediaLayers: [], transitionSolids: [], issues: [], emptyStateCopy: null });

    // A transition the render draws natively keeps its layers and solid beside prepared frames.
    const plain = transitionTestProject(undefined, [{ ...crossfade(1), kind: "dipToBlack" }]);
    const native = buildCompositorModel(input(plain, { playheadSeconds: 2, canonical: { status: "ready" }, coverageItemIds: new Set(["flatten-1"]), canonicalFrameCount: 1 }));
    expect(native.visibleMediaLayers.map(({ layer }) => layer.itemId)).toEqual(["a", "b"]);
    expect(native.transitionSolids).toEqual([{ transitionId: "fade", color: "black", beforeItemId: "a" }]);
  });

  it("keeps reversed audio silent until its prepared audio is ready", () => {
    const project = fixtureProject();
    fixtureItem(project, "audio").properties.reverse = true;
    for (const canonical of [null, { status: "pending" }] satisfies CompositorCanonicalState[]) {
      const model = buildCompositorModel(input(project, { canonical }));
      expect(model.visibleAudioLayers).toEqual([]);
      expect(model.issues).toEqual(["Reversed audio is preparing."]);
      expect(model).toMatchObject({ issueState: "notice", retryPreparesCanonical: false, visibleMediaLayers: [{ layer: { itemId: "item-1" } }] });
    }
    expect(buildCompositorModel(input(project, { canonical: { status: "pending" } })).issueTitle).toBe("Preparing preview");

    const failed = buildCompositorModel(input(project, { canonical: { status: "failed", message: "Reverse decode failed." } }));
    expect(failed).toMatchObject({ visibleAudioLayers: [], issues: ["Reverse decode failed."], issueState: "retry", retryPreparesCanonical: true });

    const missing = buildCompositorModel(input(project, { canonical: { status: "ready" } }));
    expect(missing).toMatchObject({
      visibleAudioLayers: [],
      issues: ["Prepared audio missing for timeline audio item music-bed. Rebuild the canonical preview."],
      retryPreparesCanonical: true,
    });
  });

  it("plays reversed audio from the prepared audio layer once ready", () => {
    const project = fixtureProject();
    fixtureItem(project, "audio").properties.reverse = true;
    const direct = buildTimelinePreviewFrame({ ...project, playheadSeconds: 1 });
    const directLayer = direct.audioLayers[0];
    if (!directLayer) throw new Error("fixture frame has no audio layer");
    const preparedLayer = { ...directLayer, mediaId: "audio-reverse-abc", relativePath: "cache/audio-reverse/v1/output.wav", sourceTimeSeconds: 1, canonicalPreparationRequired: false };
    const sourceUrl = "/p/cache/audio-reverse/v1/output.wav";
    const model = buildCompositorModel(input(project, { canonical: { status: "ready" }, preparedAudioLayers: [{ layer: preparedLayer, sourceUrl }] }));
    expect(model.visibleAudioLayers).toEqual([{ layer: preparedLayer, sourceUrl }]);
    expect(model.issues).toEqual([]);
    // A prepared layer is never used for audio that plays forwards.
    const forward = buildCompositorModel(input(fixtureProject(), { canonical: { status: "ready" }, preparedAudioLayers: [{ layer: preparedLayer, sourceUrl }] }));
    expect(forward.visibleAudioLayers.map(({ sourceUrl: url }) => url)).toEqual(["/p/media/voiceover.m4a"]);
  });

  it("plays reversed audio inside a nested sequence from the layer Rust prepares for it", () => {
    const project = fixtureProject();
    const audioItem = fixtureItem(project, "audio");
    audioItem.properties.reverse = true;
    const nested = project.timeline;
    const videoTrack = fixtureTrack(project, "video");
    project.timelines = [{ id: "nested", name: "Nested", timeline: nested }];
    project.timeline = {
      durationSeconds: nested.durationSeconds,
      tracks: [
        { ...videoTrack, id: "root-empty", items: [] },
        {
          ...videoTrack,
          id: "root-wrapper",
          items: [{ id: "wrapper", kind: "video_clip", startSeconds: 0, durationSeconds: nested.durationSeconds, label: "Nested", source: { type: "timeline", timelineId: "nested" }, properties: {} }],
        },
      ],
    };
    // Rust `expand_timeline_for_render` namespaces nested items by the wrapper's track index.
    const preparedItemId = `root:1:wrapper:${audioItem.id}`;
    const directLayer = buildTimelinePreviewFrame({ ...project, playheadSeconds: 1 }).audioLayers.find((layer) => layer.canonicalPreparationRequired);
    if (!directLayer) throw new Error("nested fixture frame has no reversed audio layer");
    const preparedLayer = { ...directLayer, itemId: preparedItemId, mediaId: "audio-reverse-abc", relativePath: "cache/audio-reverse/v1/output.wav", canonicalPreparationRequired: false };
    const sourceUrl = "/p/cache/audio-reverse/v1/output.wav";
    const model = buildCompositorModel(input(project, { canonical: { status: "ready" }, preparedAudioLayers: [{ layer: preparedLayer, sourceUrl }] }));
    expect(model.visibleAudioLayers).toEqual([{ layer: preparedLayer, sourceUrl }]);
    expect(model.issues).toEqual([]);
    expect(directLayer.itemId).toBe(preparedItemId);
  });

  it("hides a reversed video clip until its prepared frames cover it", () => {
    const project = fixtureProject();
    fixtureItem(project, "video").properties.reverse = true;
    const pending = buildCompositorModel(input(project, { canonical: { status: "pending" }, playheadSeconds: 0.2 }));
    expect(pending).toMatchObject({ visibleMediaLayers: [], issueTitle: "Preparing preview" });
    const covered = buildCompositorModel(
      input(project, { canonical: { status: "ready" }, coverageItemIds: new Set(["item-1"]), canonicalFrameCount: 1, playheadSeconds: 0.2 }),
    );
    expect(covered).toMatchObject({ visibleMediaLayers: [], issues: [], emptyStateCopy: null });
  });

  it("points Open source at an unresolved media item on an enabled track", () => {
    const project = fixtureProject();
    project.media = project.media.filter((asset) => asset.id !== "media-1");
    const model = buildCompositorModel(input(project));
    expect(model.problemLayer).toEqual({ itemId: "item-1", mediaId: "media-1" });
  });
});
