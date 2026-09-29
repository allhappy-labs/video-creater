import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type ProjectAction, type VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { planDetachAudio } from "@/lib/timeline-ops/detach-audio";
import { fixtureProject } from "@/test-utils/editor-fixtures";

// fixtureProject() media "media-1" is a 4 second video; "media-voiceover" is audio.
function clip(id: string, kind: TimelineItem["kind"], startSeconds: number, durationSeconds: number, properties: TimelineItem["properties"] = {}, mediaId = "media-1"): TimelineItem {
  return { id, kind, startSeconds, durationSeconds, source: { type: "media", mediaId }, label: id, properties };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[], locked = false): TimelineTrack {
  return { id, name: id, kind, locked, enabled: true, items };
}

function projectWith(tracks: TimelineTrack[]): VideoProject {
  return { ...fixtureProject(), timeline: { durationSeconds: 8, tracks } };
}

const ids = { audioItemId: "shot-audio", linkGroupId: "link-shot", trackId: "track-audio-9" };

const shot = () =>
  clip("shot", "video_clip", 1, 2, {
    sourceIn: 0.5,
    sourceOut: 2.5,
    volumeDb: -6,
    fadeInSeconds: 0.25,
    keyframes: { volumeDb: [{ atSeconds: 0, value: -12 }], opacity: [{ atSeconds: 0, value: 1 }] },
    effects: [
      { effectType: "audio.denoise", enabled: true },
      { effectType: "blur.gaussian", enabled: true },
    ],
  });

function detach(targetTrackId = "a1", overrides: Partial<Extract<ProjectAction, { type: "detachAudio" }>> = {}): ProjectAction {
  return { type: "detachAudio", itemId: "shot", audioItemId: "shot-audio", targetTrackId, linkGroupId: "link-shot", ...overrides };
}

function actionsOf(result: CommandResult) {
  if ("blocked" in result) throw new Error(`Unexpectedly blocked: ${result.blocked}`);
  return result.actions;
}

function find(project: VideoProject, itemId: string) {
  const item = project.timeline.tracks.flatMap((candidate) => candidate.items).find((candidate) => candidate.id === itemId);
  if (!item) throw new Error(`missing ${itemId}`);
  return item;
}

describe("applyDetachAudio", () => {
  it("adds a linked audio clip with the clip's range and moves its sound properties", () => {
    const project = projectWith([track("v1", "video", [shot()]), track("a1", "audio", [])]);
    const next = applyProjectActionLocally(project, detach());
    expect(find(next, "shot-audio")).toEqual({
      id: "shot-audio",
      kind: "audio_clip",
      startSeconds: 1,
      durationSeconds: 2,
      source: { type: "media", mediaId: "media-1" },
      label: "shot audio",
      properties: {
        linkGroupId: "link-shot",
        sourceClipType: "audio",
        sourceIn: 0.5,
        sourceOut: 2.5,
        volumeDb: -6,
        keyframes: { volumeDb: [{ atSeconds: 0, value: -12 }] },
        effects: [{ effectType: "audio.denoise", enabled: true }],
      },
    });
    expect(find(next, "shot").properties).toEqual({
      sourceIn: 0.5,
      sourceOut: 2.5,
      fadeInSeconds: 0.25,
      keyframes: { opacity: [{ atSeconds: 0, value: 1 }] },
      effects: [{ effectType: "blur.gaussian", enabled: true }],
      linkGroupId: "link-shot",
      audioDetached: true,
    });
  });

  it("copies speed and keeps an existing link group", () => {
    const linked = clip("shot", "video_clip", 0, 2, { speed: 2, linkGroupId: "link-old" });
    const project = projectWith([track("v1", "video", [linked]), track("a1", "audio", [])]);
    const next = applyProjectActionLocally(project, detach("a1", { linkGroupId: "link-old" }));
    expect(find(next, "shot-audio").properties).toMatchObject({ speed: 2, linkGroupId: "link-old" });
    expect(() => applyProjectActionLocally(project, detach("a1", { linkGroupId: "link-new" }))).toThrow(
      "effect parameter is invalid: link group id must be 1 to 128 characters and match the clip's existing link group: link-new",
    );
  });

  it("rejects clips without detachable sound and clips already linked to audio", () => {
    const image = projectWith([track("v1", "video", [clip("shot", "video_clip", 0, 2, {}, "media-still")]), track("a1", "audio", [])]);
    image.media = [...image.media, { id: "media-still", relativePath: "media/still.png", kind: "image", durationSeconds: 0, width: 10, height: 10, fps: null, folderId: null }];
    expect(() => applyProjectActionLocally(image, detach())).toThrow("timeline item has no detachable audio: shot");
    const audioMedia = projectWith([track("v1", "video", [clip("shot", "video_clip", 0, 2, {}, "media-voiceover")]), track("a1", "audio", [])]);
    expect(() => applyProjectActionLocally(audioMedia, detach())).toThrow("timeline item has no detachable audio: shot");
    const linked = projectWith([
      track("v1", "video", [clip("shot", "video_clip", 0, 2, { linkGroupId: "link-shot" })]),
      track("a1", "audio", [clip("voice", "audio_clip", 0, 2, { linkGroupId: "link-shot" })]),
    ]);
    expect(() => applyProjectActionLocally(linked, detach("a1"))).toThrow("timeline item audio is already on a linked audio clip: shot");
  });

  it("requires a link group id of 1 to 128 characters, like Rust", () => {
    const project = projectWith([track("v1", "video", [shot()]), track("a1", "audio", [])]);
    for (const linkGroupId of ["", "   ", "g".repeat(129)]) {
      expect(() => applyProjectActionLocally(project, detach("a1", { linkGroupId }))).toThrow("link group id must be 1 to 128 characters");
    }
    expect(find(applyProjectActionLocally(project, detach("a1", { linkGroupId: "g".repeat(128) })), "shot-audio").properties.linkGroupId).toBe("g".repeat(128));
  });

  it("rejects overlaps, locked tracks and the wrong track kind", () => {
    const overlap = projectWith([track("v1", "video", [shot()]), track("a1", "audio", [clip("music", "audio_clip", 2, 4, {}, "media-voiceover")])]);
    expect(() => applyProjectActionLocally(overlap, detach())).toThrow("timeline items overlap on track a1: shot-audio overlaps music");
    const locked = projectWith([track("v1", "video", [shot()]), track("a1", "audio", [], true)]);
    expect(() => applyProjectActionLocally(locked, detach())).toThrow("track is locked: a1");
    const wrongKind = projectWith([track("v1", "video", [shot()]), track("v2", "video", [])]);
    expect(() => applyProjectActionLocally(wrongKind, detach("v2"))).toThrow("item kind AudioClip is incompatible with track kind Video");
    expect(() => applyProjectActionLocally(wrongKind, detach("missing"))).toThrow("timeline track was not found: missing");
  });
});

describe("planDetachAudio", () => {
  it("detaches onto the first free unlocked audio track", () => {
    const project = projectWith([
      track("v1", "video", [shot()]),
      track("a1", "audio", [], true),
      track("a2", "audio", [clip("music", "audio_clip", 3, 1, {}, "media-voiceover")]),
    ]);
    expect(planDetachAudio(project, "shot", ids)).toEqual({ actions: [detach("a2")] });
    expect(applyProjectActionLocally(project, detach("a2")).timeline.tracks[2]?.items.map((item) => item.id)).toEqual(["shot-audio", "music"]);
  });

  it("creates an audio track first when every audio track collides", () => {
    const project = projectWith([track("v1", "video", [shot()]), track("a1", "audio", [clip("music", "audio_clip", 0, 4, {}, "media-voiceover")])]);
    const actions = actionsOf(planDetachAudio(project, "shot", ids));
    expect(actions.map((action) => action.type)).toEqual(["createTrack", "detachAudio"]);
    expect(actions.at(-1)).toEqual(detach("track-audio-9"));
    const next = actions.reduce(applyProjectActionLocally, project);
    expect(find(next, "shot-audio").properties.linkGroupId).toBe("link-shot");
    expect(next.timeline.tracks.find((candidate) => candidate.id === "track-audio-9")?.items.map((item) => item.id)).toEqual(["shot-audio"]);
  });

  it("explains why a clip can't detach its audio", () => {
    const project = projectWith([
      track("v1", "video", [shot(), clip("still", "image_clip", 4, 1), clip("silent", "video_clip", 6, 1, {}, "media-voiceover")]),
      track("v2", "video", [clip("locked", "video_clip", 0, 1)], true),
      track("a1", "audio", [clip("voice", "audio_clip", 0, 1, {}, "media-voiceover")]),
    ]);
    expect(planDetachAudio(project, "still", ids)).toEqual({ blocked: "Detach audio is available for video clips." });
    expect(planDetachAudio(project, "voice", ids)).toEqual({ blocked: "Detach audio is available for video clips." });
    expect(planDetachAudio(project, "silent", ids)).toEqual({ blocked: "This clip has no sound to detach." });
    expect(planDetachAudio(project, "locked", ids)).toEqual({ blocked: "Unlock the track to detach this clip's audio." });
    const linked = actionsOf(planDetachAudio(project, "shot", ids)).reduce(applyProjectActionLocally, project);
    expect(planDetachAudio(linked, "shot", ids)).toEqual({ blocked: "This clip's sound is already on a linked audio clip." });
  });
});
