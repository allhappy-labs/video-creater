import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type GeneratedAsset, type ProjectAction, type VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import {
  AUDIO_TRACK,
  clip,
  cutProject,
  projectWithCrossfade,
  trackOf,
  transition,
  transitionSampleProject,
  VIDEO_TRACK,
  withSourceRange,
  withTracks,
  withVideoItem,
} from "@/lib/timeline-ops/transition-fixtures";
import {
  applyTransitionAction,
  cutsOnTrack,
  nearestCut,
  transitionBounds,
  transitionFrameSeconds,
  transitionMaxDuration,
  transitionWindow,
  validateTransition,
  type TransitionAction,
  type TransitionError,
} from "@/lib/timeline-ops/transitions";

function errorOf(project: VideoProject, action: TransitionAction): TransitionError | null {
  const result = applyTransitionAction(project, action);
  return "error" in result ? result.error : null;
}

/** Rust `assert_action_error_leaves_project_unchanged`: the error, and a no-op local apply. */
function expectRejected(project: VideoProject, action: TransitionAction, error: TransitionError) {
  expect(errorOf(project, action)).toEqual(error);
  expect(applyProjectActionLocally(project, action)).toBe(project);
}

function add(project: VideoProject, transitionValue = transition("fade-1", "clip-a", "clip-b", 1), trackId = VIDEO_TRACK) {
  return { type: "addTransition", trackId, transition: transitionValue } satisfies ProjectAction;
}

function invalid(message: string): TransitionError {
  return { code: "invalidTransition", message };
}

function expectAddRejected(project: VideoProject, transitionValue: ReturnType<typeof transition>, message: string) {
  expectRejected(project, add(project, transitionValue), invalid(message));
}

function applied(project: VideoProject, action: ProjectAction): VideoProject {
  const next = applyProjectActionLocally(project, action);
  expect(next).not.toBe(project);
  return next;
}

describe("transition validation (Rust project_action/transitions.rs)", () => {
  it("adds a transition between adjacent video clips with enough handles", () => {
    const project = projectWithCrossfade();
    expect(trackOf(project, VIDEO_TRACK).transitions).toEqual([transition("fade-1", "clip-a", "clip-b", 1)]);
    const active = project.timelines?.find((entry) => entry.id === project.activeTimelineId);
    expect(active?.timeline).toEqual(project.timeline);
    expect(validateTransition(project, trackOf(project, VIDEO_TRACK), transition("fade-1", "clip-a", "clip-b", 1))).toBeNull();
  });

  it("accepts up to one frame of gap and rejects more", () => {
    const oneFrame = withVideoItem(cutProject(), 1, (item) => ({ ...item, startSeconds: 4 + 1 / 24 }));
    expect(errorOf(oneFrame, add(oneFrame))).toBeNull();

    const gap = withVideoItem(cutProject(), 1, (item) => ({ ...item, startSeconds: 4.1 }));
    expectAddRejected(gap, transition("fade-1", "clip-a", "clip-b", 1), "Opening shot must end where Closing shot starts to add a transition.");

    expectAddRejected(cutProject(), transition("fade-1", "clip-b", "clip-a", 1), "Closing shot must end where Opening shot starts to add a transition.");
  });

  it("rejects clips on different tracks and unknown clips", () => {
    const project = withTracks(cutProject(), (tracks) => {
      const [video, ...rest] = tracks;
      const right = video!.items[1]!;
      return [
        { ...video!, items: [video!.items[0]!] },
        ...rest,
        { id: "track-video-2", name: "Video 2", kind: "video", locked: false, enabled: true, items: [right] },
      ];
    });
    expectAddRejected(project, transition("fade-1", "clip-a", "clip-b", 1), "Opening shot and Closing shot must be on the same track to add a transition.");
    expectRejected(project, add(project, transition("fade-1", "clip-a", "missing", 1)), {
      code: "itemNotFound",
      message: "timeline item was not found: missing",
    });
  });

  it("rejects mixed audio and visual pairs", () => {
    const project = withVideoItem(cutProject(), 1, (item) => ({ ...item, kind: "audio_clip" }));
    expectAddRejected(project, transition("fade-1", "clip-a", "clip-b", 1), "Transitions need two visual clips or two audio clips, not a mix.");
  });

  it("rejects items that cannot transition", () => {
    const lottie = withVideoItem(cutProject(), 1, (item) => ({ ...item, kind: "lottie_clip" }));
    expectAddRejected(lottie, transition("fade-1", "clip-a", "clip-b", 1), "Transitions only work between video, image, generated, or audio clips.");

    const nested = withVideoItem(cutProject(), 1, (item) => ({ ...item, source: { type: "timeline", timelineId: "main" } }));
    expectAddRejected(nested, transition("fade-1", "clip-a", "clip-b", 1), "Transitions don't support nested sequences yet.");

    const text = withVideoItem(cutProject(), 0, (item) => ({ ...item, source: { type: "text", text: "Hi" } }));
    expectAddRejected(text, transition("fade-1", "clip-a", "clip-b", 1), "Transitions only work between video, image, generated, or audio clips.");

    expectAddRejected(cutProject(), transition("fade-1", "clip-a", "clip-a", 1), "A transition needs two different clips.");
  });

  it("rejects a duplicate on the same cut or id, and an empty id", () => {
    let project = projectWithCrossfade();
    expectAddRejected(project, { ...transition("wipe-1", "clip-a", "clip-b", 0.5), kind: "wipe" }, "Opening shot and Closing shot already have a transition.");

    project = withTracks(project, (tracks) =>
      tracks.map((track) => (track.id === VIDEO_TRACK ? { ...track, items: [...track.items, clip("clip-c", "Outro", 8, 2, 10)] } : track)),
    );
    expectAddRejected(project, transition("fade-1", "clip-b", "clip-c", 0.5), "A transition with id fade-1 already exists.");
    expectAddRejected(project, transition(" ", "clip-b", "clip-c", 0.5), "Transition id cannot be empty.");
  });

  it("rejects short handles and reports the maximum", () => {
    const shortHead = withVideoItem(cutProject(), 1, (item) => withSourceRange(item, 0.2, 4.2));
    expectAddRejected(shortHead, transition("fade-1", "clip-a", "clip-b", 1), "Not enough unused media before Closing shot for a 1.0s transition. Maximum is 0.4s.");

    const shortTail = withVideoItem(cutProject(), 0, (item) => withSourceRange(item, 7.8, 11.8));
    expectAddRejected(shortTail, transition("fade-1", "clip-a", "clip-b", 1), "Not enough unused media after Opening shot for a 1.0s transition. Maximum is 0.4s.");
    expect(errorOf(shortTail, add(shortTail, transition("fade-1", "clip-a", "clip-b", 0.4)))).toBeNull();
  });

  it("rejects clips shorter than the transition", () => {
    const project = transitionSampleProject([
      clip("clip-a", "Opening shot", 0, 0.5, 2),
      clip("clip-b", "Closing shot", 0.5, 0.75, 6),
    ]);
    expectAddRejected(project, transition("fade-1", "clip-a", "clip-b", 1), "Opening shot is too short for a 1.0s transition. Maximum is 0.5s.");

    const rightShort = transitionSampleProject([
      clip("clip-a", "Opening shot", 0, 2, 2),
      clip("clip-b", "", 2, 0.25, 6),
    ]);
    expectAddRejected(rightShort, transition("fade-1", "clip-a", "clip-b", 1), "this clip is too short for a 1.0s transition. Maximum is 0.25s.");
  });

  it("measures handles in timeline seconds at clip speed", () => {
    const project = withVideoItem(cutProject(), 1, (item) =>
      withSourceRange({ ...item, properties: { ...item.properties, speed: 2 } }, 1, 9),
    );
    expectAddRejected(project, transition("fade-1", "clip-a", "clip-b", 1.2), "Not enough unused media before Closing shot for a 1.2s transition. Maximum is 1.0s.");
    expect(errorOf(project, add(project))).toBeNull();
  });

  it("treats still images as unlimited handles", () => {
    const still = (id: string, startSeconds: number): TimelineItem => ({
      id,
      kind: "image_clip",
      startSeconds,
      durationSeconds: 6,
      source: { type: "media", mediaId: "still-1" },
      label: "Still",
      properties: {},
    });
    const base = transitionSampleProject([still("still-a", 0), still("still-b", 6)]);
    const project: VideoProject = {
      ...base,
      media: [
        ...base.media,
        { id: "still-1", relativePath: "media/still.png", kind: "image", durationSeconds: 0, width: 1920, height: 1080, fps: null },
      ],
    };
    expect(errorOf(project, add(project, transition("fade-1", "still-a", "still-b", 5)))).toBeNull();
  });

  it("accepts adjacent audio clips", () => {
    const base = transitionSampleProject();
    const audio = (id: string, startSeconds: number, sourceIn: number): TimelineItem => ({
      ...clip(id, "Music", startSeconds, 5, sourceIn),
      kind: "audio_clip",
      source: { type: "media", mediaId: "music" },
    });
    const project = withTracks(
      {
        ...base,
        media: [...base.media, { id: "music", relativePath: "media/music.wav", kind: "audio", durationSeconds: 30, width: null, height: null, fps: null }],
      },
      (tracks) => tracks.map((track) => (track.id === AUDIO_TRACK ? { ...track, items: [audio("music-a", 0, 0), audio("music-b", 5, 10)] } : track)),
    );
    const next = applied(project, add(project, transition("audio-fade", "music-a", "music-b", 2), AUDIO_TRACK));
    expect(trackOf(next, AUDIO_TRACK).transitions).toHaveLength(1);
  });

  it("rejects zero, NaN and over five second durations", () => {
    const project = cutProject();
    expectAddRejected(project, transition("fade-1", "clip-a", "clip-b", 0), "Transition duration must be at least one frame (0.04s).");
    expectAddRejected(project, transition("fade-1", "clip-a", "clip-b", Number.NaN), "Transition duration must be at least one frame (0.04s).");
    expectAddRejected(project, transition("fade-1", "clip-a", "clip-b", 5.5), "Transition duration cannot be longer than 5.0s.");
  });

  it("rejects locked and unknown tracks", () => {
    const locked = withTracks(cutProject(), (tracks) => tracks.map((track) => ({ ...track, locked: track.id === VIDEO_TRACK })));
    expectRejected(locked, add(locked), { code: "trackLocked", message: "track is locked: track-video" });
    const project = cutProject();
    expectRejected(project, add(project, undefined, "nope"), { code: "trackNotFound", message: "timeline track was not found: nope" });
  });

  it("updates kind and duration without clamping and rejects invalid updates", () => {
    let project = applied(projectWithCrossfade(), {
      type: "updateTransition",
      trackId: VIDEO_TRACK,
      transitionId: "fade-1",
      kind: "dipToBlack",
    });
    expect(trackOf(project, VIDEO_TRACK).transitions).toEqual([{ ...transition("fade-1", "clip-a", "clip-b", 1), kind: "dipToBlack" }]);

    project = applied(project, { type: "updateTransition", trackId: VIDEO_TRACK, transitionId: "fade-1", durationSeconds: 4 });
    expect(trackOf(project, VIDEO_TRACK).transitions?.[0]?.durationSeconds).toBe(4);

    expectRejected(
      project,
      { type: "updateTransition", trackId: VIDEO_TRACK, transitionId: "fade-1", durationSeconds: 4.5 },
      invalid("Opening shot is too short for a 4.5s transition. Maximum is 4.0s."),
    );
    expectRejected(
      project,
      { type: "updateTransition", trackId: VIDEO_TRACK, transitionId: "missing", kind: "wipe" },
      { code: "transitionNotFound", message: "transition was not found: missing" },
    );
  });

  it("removes a transition and omits the empty transitions field", () => {
    const project = applied(projectWithCrossfade(), { type: "removeTransition", trackId: VIDEO_TRACK, transitionId: "fade-1" });
    expect(trackOf(project, VIDEO_TRACK)).not.toHaveProperty("transitions");
    expect(project.timelines?.[0]?.timeline.tracks[0]).not.toHaveProperty("transitions");
    expectRejected(
      project,
      { type: "removeTransition", trackId: VIDEO_TRACK, transitionId: "fade-1" },
      { code: "transitionNotFound", message: "transition was not found: fade-1" },
    );
  });
});

describe("transition bounds", () => {
  it("uses one frame at the project rate, or 24 fps when invalid", () => {
    expect(transitionFrameSeconds(cutProject())).toBeCloseTo(1 / 24);
    expect(transitionFrameSeconds({ renderSettings: { ...cutProject().renderSettings, fps: 0 } })).toBeCloseTo(1 / 24);
    expect(transitionFrameSeconds({ renderSettings: { ...cutProject().renderSettings, fps: 60 } })).toBeCloseTo(1 / 60);
  });

  it("reports the first limiting constraint on ties", () => {
    const project = cutProject();
    const [left, right] = trackOf(project, VIDEO_TRACK).items as [TimelineItem, TimelineItem];
    expect(transitionBounds(project, left, right)).toEqual({ maxSeconds: 4, limit: "leftDuration" });

    // 2 s tail handle and 2 s head handle both double to 4 s, equal to both durations.
    const tied = transitionSampleProject([clip("a", "A", 0, 4, 6), clip("b", "B", 4, 4, 2)]);
    const [tiedLeft, tiedRight] = trackOf(tied, VIDEO_TRACK).items as [TimelineItem, TimelineItem];
    expect(transitionBounds(tied, tiedLeft, tiedRight)).toEqual({ maxSeconds: 4, limit: "leftHandle" });

    const long = transitionSampleProject([clip("a", "A", 0, 6, 0), clip("b", "B", 6, 6, 0)]);
    const media = { ...long, media: [{ ...long.media[0]!, durationSeconds: 60 }] };
    const [longLeft, longRight] = trackOf(media, VIDEO_TRACK).items as [TimelineItem, TimelineItem];
    // sourceIn 0 leaves no head handle on the right clip.
    expect(transitionBounds(media, longLeft, longRight)).toEqual({ maxSeconds: 0, limit: "rightHandle" });
    expect(transitionBounds(media, longLeft, { ...longRight, properties: { sourceIn: 10, sourceOut: 16 } })).toEqual({
      maxSeconds: 5,
      limit: "cap",
    });
  });

  it("treats zero-duration, missing, timeline and generated sources like Rust", () => {
    const project = cutProject();
    const [left, right] = trackOf(project, VIDEO_TRACK).items as [TimelineItem, TimelineItem];
    const unlimited = { ...right, properties: { sourceIn: 0, sourceOut: 4 } };
    const missing = { ...unlimited, source: { type: "media", mediaId: "gone" } } satisfies TimelineItem;
    expect(transitionMaxDuration(project, left, missing)).toBe(4);
    const zero = { ...project, media: [{ ...project.media[0]!, durationSeconds: 0 }] };
    expect(transitionMaxDuration(zero, left, unlimited)).toBe(4);

    const generated = { ...unlimited, kind: "generated_clip", source: { type: "generated", artifactId: "gen-1" } } satisfies TimelineItem;
    // No completed asset: unlimited.
    expect(transitionMaxDuration(project, left, generated)).toBe(4);
    const output = { mediaId: "gen-out", relativePath: "gen.mp4", width: 1, height: 1, durationSeconds: 8, fps: 24 };
    const asset: GeneratedAsset = { ...sampleGeneratedAsset, id: "gen-1", outputs: [output] };
    // Output media missing from the library: falls back to the output duration, and sourceIn 0 has no head handle.
    expect(transitionMaxDuration({ ...project, generatedAssets: [asset] }, left, generated)).toBe(0);
    expect(transitionMaxDuration({ ...project, generatedAssets: [{ ...asset, status: "running" as const }] }, left, generated)).toBe(4);
  });
});

const sampleGeneratedAsset: GeneratedAsset = {
  schemaVersion: 1,
  id: "sample",
  kind: "generated",
  status: "completed",
  prompt: "",
  model: { provider: "local", id: "sample" },
  references: { mediaIds: [], firstFrameMediaId: null, lastFrameMediaId: null },
  settings: { width: 1, height: 1, durationSeconds: 8, fps: 24, aspectRatio: "16:9" },
  outputs: [],
  createdAt: "2026-01-01T00:00:00Z",
  parentAssetId: null,
  retryOfAssetId: null,
};

describe("cuts and transition windows", () => {
  it("lists adjacent eligible pairs on a track, with any transition on the pair", () => {
    const project = withTracks(projectWithCrossfade(), (tracks) =>
      tracks.map((track) =>
        track.id === VIDEO_TRACK
          ? {
              ...track,
              items: [
                ...track.items,
                clip("clip-c", "Gap", 9, 1, 0),
                { ...clip("clip-d", "Nested", 10, 1, 0), source: { type: "timeline", timelineId: "x" } },
                clip("clip-e", "Last", 11 + 1 / 48, 1, 0),
              ],
            }
          : track,
      ),
    );
    const track = trackOf(project, VIDEO_TRACK);
    expect(cutsOnTrack(track)).toEqual([
      { trackId: VIDEO_TRACK, leftItemId: "clip-a", rightItemId: "clip-b", seconds: 4, transitionId: "fade-1" },
    ]);
    const withoutNested = { ...track, items: track.items.filter((item) => item.id !== "clip-d") };
    expect(cutsOnTrack({ ...withoutNested, items: [...withoutNested.items, clip("clip-f", "F", 10, 1, 0)] }).map((cut) => cut.rightItemId)).toEqual([
      "clip-b",
      "clip-f",
      "clip-e",
    ]);
    // A one-frame gap at 24 fps is not adjacent at 60 fps.
    expect(cutsOnTrack({ ...withoutNested, items: [...withoutNested.items, clip("clip-f", "F", 10, 1, 0)] }, 1 / 60).map((cut) => cut.rightItemId)).toEqual([
      "clip-b",
      "clip-f",
    ]);
  });

  it("finds the nearest cut across tracks or on one track", () => {
    const base = cutProject();
    const project = withTracks(base, (tracks) => [
      ...tracks,
      { id: "track-video-2", name: "Video 2", kind: "video", locked: false, enabled: true, items: [clip("x", "X", 0, 6, 0), clip("y", "Y", 6, 2, 6)] },
    ]);
    expect(nearestCut(project.timeline, 3)?.rightItemId).toBe("clip-b");
    expect(nearestCut(project.timeline, 5.5)?.rightItemId).toBe("y");
    // Ties go to the earliest track.
    expect(nearestCut(project.timeline, 5)?.rightItemId).toBe("clip-b");
    expect(nearestCut(project.timeline, 3, "track-video-2")?.rightItemId).toBe("y");
    expect(nearestCut(project.timeline, 3, AUDIO_TRACK)).toBeNull();
  });

  it("centers the transition window on the right clip's start", () => {
    const project = projectWithCrossfade();
    const track = trackOf(project, VIDEO_TRACK);
    expect(transitionWindow(transition("fade-1", "clip-a", "clip-b", 1), track)).toEqual({ start: 3.5, end: 4.5 });
    expect(transitionWindow(transition("fade-1", "clip-a", "missing", 1), track)).toBeNull();
  });
});
