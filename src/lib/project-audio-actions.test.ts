import { describe, expect, it } from "vitest";
import { applyProjectActionLocally } from "./project";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function musicBed(project: ReturnType<typeof fixtureProject>) {
  const item = project.timeline.tracks.flatMap((track) => track.items).find((candidate) => candidate.id === "music-bed");
  if (!item) throw new Error("fixture music bed");
  return item;
}

describe("audio clip speed action", () => {
  it("sets and clears speed on an audio clip without resizing it", () => {
    const project = fixtureProject();
    const duration = musicBed(project).durationSeconds;
    const retimed = applyProjectActionLocally(project, { type: "updateAudioClipSpeed", itemId: "music-bed", speed: 2 });
    expect(musicBed(retimed).properties.speed).toBe(2);
    expect(musicBed(retimed).durationSeconds).toBe(duration);
    const restored = applyProjectActionLocally(retimed, { type: "updateAudioClipSpeed", itemId: "music-bed", speed: 1 });
    expect(musicBed(restored).properties).not.toHaveProperty("speed");
  });

  it("rejects missing items and clips that are not audio", () => {
    const project = fixtureProject();
    expect(() => applyProjectActionLocally(project, { type: "updateAudioClipSpeed", itemId: "missing", speed: 2 })).toThrow(
      "Timeline item missing was not found.",
    );
    expect(() => applyProjectActionLocally(project, { type: "updateAudioClipSpeed", itemId: "item-1", speed: 2 })).toThrow(
      "Timeline item item-1 is not an audio clip.",
    );
  });

  it("rejects speeds outside 0.1 to 8 and clips on locked tracks, like Rust", () => {
    const project = fixtureProject();
    for (const speed of [12, 0.05, 0, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(() => applyProjectActionLocally(project, { type: "updateAudioClipSpeed", itemId: "music-bed", speed })).toThrow(
        "audio clip speed is invalid: music-bed",
      );
    }
    for (const speed of [0.1, 8]) {
      expect(musicBed(applyProjectActionLocally(project, { type: "updateAudioClipSpeed", itemId: "music-bed", speed })).properties.speed).toBe(speed);
    }
    const trackId = project.timeline.tracks.find((track) => track.items.some((item) => item.id === "music-bed"))?.id ?? "";
    const locked = { ...project, timeline: { ...project.timeline, tracks: project.timeline.tracks.map((track) => (track.id === trackId ? { ...track, locked: true } : track)) } };
    expect(() => applyProjectActionLocally(locked, { type: "updateAudioClipSpeed", itemId: "music-bed", speed: 2 })).toThrow(`track is locked: ${trackId}`);
  });
});
