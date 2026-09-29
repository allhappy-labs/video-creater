import { describe, expect, it } from "vitest";

import type { CaptionBuildOptions } from "@/lib/captions/caption-items";
import { applyProjectActionLocally, type ProjectAction, type Transcript, type VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack, TrackKind } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";

import { captionBuildActions } from "./caption-track";

const transcript: Transcript = {
  id: "transcript-build",
  mediaId: "media-1",
  repairs: [],
  segments: [],
  words: [
    { text: "Make", startSeconds: 1, endSeconds: 1.2 },
    { text: "this", startSeconds: 1.2, endSeconds: 1.4 },
    { text: "feel", startSeconds: 1.4, endSeconds: 1.6 },
    { text: "cinematic", startSeconds: 1.6, endSeconds: 2 },
    { text: "today", startSeconds: 2, endSeconds: 2.3 },
  ],
};

const options: CaptionBuildOptions = {
  transcript,
  range: { startSeconds: 0, endSeconds: 3 },
  wordsPerCue: 2,
  stylePreset: "boldReadableLower",
  groupId: "caption-group-test",
};

function caption(id: string, startSeconds: number, durationSeconds: number): TimelineItem {
  return {
    id,
    kind: "caption",
    startSeconds,
    durationSeconds,
    source: { type: "text", text: id },
    label: id,
    properties: {},
  };
}

function track(id: string, kind: TrackKind, items: TimelineItem[] = [], locked = false): TimelineTrack {
  return { id, name: id, kind, locked, enabled: true, items };
}

function projectWithTracks(tracks: TimelineTrack[]): VideoProject {
  const project = fixtureProject();
  const video = project.timeline.tracks.find((candidate) => candidate.kind === "video");
  project.timeline.tracks = [...(video ? [video] : []), ...tracks];
  project.transcripts = [transcript];
  return project;
}

function applied(project: VideoProject, actions: readonly ProjectAction[]) {
  return actions.reduce(applyProjectActionLocally, project);
}

describe("captionBuildActions", () => {
  it("creates a caption track and adds every cue in one action list when none exists", () => {
    const project = projectWithTracks([track("a1", "audio")]);

    const result = captionBuildActions(project, options, "track-caption-9");

    if ("blocked" in result) throw new Error(result.blocked);
    expect(result.actions.map((action) => action.type)).toEqual(["createTrack", "addItems"]);
    expect(result.trackId).toBe("track-caption-9");
    expect(result.itemIds).toEqual([
      "caption-caption-group-test-1",
      "caption-caption-group-test-2",
      "caption-caption-group-test-3",
    ]);
    const captions = applied(project, result.actions).timeline.tracks.find((candidate) => candidate.id === "track-caption-9");
    expect(captions?.kind).toBe("caption");
    expect(captions?.items.map((item) => item.source.type === "text" ? item.source.text : null)).toEqual([
      "Make this",
      "feel cinematic",
      "today",
    ]);
  });

  it("reuses an empty unlocked caption track", () => {
    const project = projectWithTracks([track("captions", "caption"), track("a1", "audio")]);

    const result = captionBuildActions(project, options, "track-caption-9");

    if ("blocked" in result) throw new Error(result.blocked);
    expect(result.trackId).toBe("captions");
    expect(result.actions).toHaveLength(1);
    expect(result.actions[0]).toMatchObject({ type: "addItems", targetTrackId: "captions" });
  });

  it("reuses a caption track whose cues do not overlap the new captions", () => {
    const project = projectWithTracks([track("captions", "caption", [caption("later", 5, 1)])]);

    const result = captionBuildActions(project, options, "track-caption-9");

    expect(result).toMatchObject({ trackId: "captions", actions: [{ type: "addItems" }] });
  });

  it("creates a new caption track when existing cues overlap or the track is locked", () => {
    const project = projectWithTracks([
      track("locked", "caption", [], true),
      track("busy", "caption", [caption("existing", 1.5, 1)]),
    ]);

    const result = captionBuildActions(project, options, "track-caption-9");

    if ("blocked" in result) throw new Error(result.blocked);
    expect(result.trackId).toBe("track-caption-9");
    expect(result.actions[0]).toMatchObject({ type: "createTrack", track: { id: "track-caption-9", kind: "caption" } });
    expect(result.actions.at(-1)).toMatchObject({ type: "addItems", targetTrackId: "track-caption-9" });
    const after = applied(project, result.actions);
    expect(after.timeline.tracks.find((candidate) => candidate.id === "busy")?.items).toHaveLength(1);
  });

  it("blocks when the range has no transcript words", () => {
    const project = projectWithTracks([]);

    expect(
      captionBuildActions(project, { ...options, range: { startSeconds: 10, endSeconds: 12 } }, "track-caption-9"),
    ).toEqual({ blocked: "There are no transcript words in this range." });
  });

  it("blocks when the new track id is already taken", () => {
    const project = projectWithTracks([track("track-caption-9", "caption", [caption("existing", 1, 2)])]);

    expect(captionBuildActions(project, options, "track-caption-9")).toEqual({
      blocked: "Track track-caption-9 already exists",
    });
  });
});
