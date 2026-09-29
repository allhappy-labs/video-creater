import { describe, expect, it } from "vitest";
import type { Timeline, TimelineTrack, TrackKind } from "@/lib/timeline";
import {
  orderedTracksByBand,
  trackBand,
  trackDisplayNames,
} from "@/lib/timeline-ops/track-bands";

function track(id: string, kind: TrackKind): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items: [] };
}

function timeline(...tracks: TimelineTrack[]): Timeline {
  return { durationSeconds: 0, tracks };
}

describe("trackBand", () => {
  it("puts graphics (hyperframe_scene), text (overlay) and caption tracks above, video in the main band and audio below", () => {
    const kinds: TrackKind[] = ["video", "hyperframe_scene", "overlay", "caption", "audio"];
    expect(kinds.map((kind) => [kind, trackBand(kind)])).toEqual([
      ["video", "main"],
      ["hyperframe_scene", "above"],
      ["overlay", "above"],
      ["caption", "above"],
      ["audio", "below"],
    ]);
  });
});

describe("orderedTracksByBand", () => {
  it("orders above, main then below bands and keeps the stored order inside each band", () => {
    const ordered = orderedTracksByBand(
      timeline(
        track("v1", "video"),
        track("a1", "audio"),
        track("scene", "hyperframe_scene"),
        track("v2", "video"),
        track("text", "overlay"),
        track("a2", "audio"),
        track("captions", "caption"),
      ),
    );
    expect(ordered.map((entry) => entry.id)).toEqual([
      "scene",
      "text",
      "captions",
      "v1",
      "v2",
      "a1",
      "a2",
    ]);
  });
});

describe("trackDisplayNames", () => {
  it("numbers Video, Text (overlay), Graphics (hyperframe_scene) and Audio per kind in band order", () => {
    const names = trackDisplayNames(
      timeline(
        track("v1", "video"),
        track("a1", "audio"),
        track("text-1", "overlay"),
        track("v2", "video"),
        track("scene-1", "hyperframe_scene"),
        track("text-2", "overlay"),
        track("a2", "audio"),
      ),
    );
    expect(Object.fromEntries(names)).toEqual({
      "text-1": "Text 1",
      "scene-1": "Graphics 1",
      "text-2": "Text 2",
      v1: "Video 1",
      v2: "Video 2",
      a1: "Audio 1",
      a2: "Audio 2",
    });
  });

  it("names a single caption track Captions and numbers the second one Captions 2", () => {
    expect(Object.fromEntries(trackDisplayNames(timeline(track("c1", "caption"))))).toEqual({
      c1: "Captions",
    });
    expect(
      Object.fromEntries(
        trackDisplayNames(timeline(track("c1", "caption"), track("v1", "video"), track("c2", "caption"))),
      ),
    ).toEqual({ c1: "Captions", c2: "Captions 2", v1: "Video 1" });
  });
});
