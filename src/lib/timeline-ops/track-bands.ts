import type { Timeline, TimelineTrack, TrackKind } from "@/lib/timeline";

export type TrackBand = "above" | "main" | "below";

/** Bands from top to bottom. */
export const trackBandOrder: readonly TrackBand[] = ["above", "main", "below"];

/** Graphics, text and caption tracks sit above the main video band; audio sits below it. */
export function trackBand(kind: TrackKind): TrackBand {
  switch (kind) {
    case "video":
      return "main";
    case "audio":
      return "below";
    case "hyperframe_scene":
    case "overlay":
    case "caption":
      return "above";
  }
}

/** Top-to-bottom display order: bands in order, stored order inside each band. */
export function orderedTracksByBand(timeline: Timeline): TimelineTrack[] {
  return trackBandOrder.flatMap((band) =>
    timeline.tracks.filter((track) => trackBand(track.kind) === band),
  );
}

const displayNameBase: Record<TrackKind, string> = {
  video: "Video",
  hyperframe_scene: "Graphics",
  overlay: "Text",
  caption: "Captions",
  audio: "Audio",
};

/** Base visible name for a track kind, used for new tracks and numbered display names. */
export function trackKindDisplayName(kind: TrackKind): string {
  return displayNameBase[kind];
}

/**
 * Visible names numbered per kind in band order: "Video 1", "Text 1", "Graphics 1",
 * "Audio 1". Caption tracks read "Captions" for the first one, then "Captions 2".
 */
export function trackDisplayNames(timeline: Timeline): Map<string, string> {
  const counts = new Map<TrackKind, number>();
  return new Map(
    orderedTracksByBand(timeline).map((track) => {
      const count = (counts.get(track.kind) ?? 0) + 1;
      counts.set(track.kind, count);
      const base = trackKindDisplayName(track.kind);
      const name = track.kind === "caption" && count === 1 ? base : `${base} ${count}`;
      return [track.id, name] as const;
    }),
  );
}
