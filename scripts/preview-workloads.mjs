export const PREVIEW_SCENARIOS = Object.freeze(["plain", "transitions", "nested", "captions", "many-media", "prepared-effects"]);

// Seed-free fixtures: all identities, ranges and effects are derived from the index.
// Assets are descriptive inputs, not files, decoder output or provider requests.
export function createPreviewWorkload(scenario, clipCount) {
  if (!PREVIEW_SCENARIOS.includes(scenario)) throw new Error(`Unknown preview scenario: ${scenario}`);
  if (!Number.isInteger(clipCount) || clipCount < 2 || clipCount > 10000) throw new Error("clip count must be an integer from 2 through 10000");
  const mediaCount = scenario === "many-media" ? clipCount : 1;
  const media = Array.from({ length: mediaCount }, (_, index) => ({ id: `media-${index}`, relativePath: `fixture/media-${index}.mp4`, kind: "video", durationSeconds: 10, width: 1920, height: 1080, fps: 30, folderId: null }));
  const items = Array.from({ length: clipCount }, (_, index) => ({
    id: `clip-${index}`, kind: "video_clip", startSeconds: index * 4, durationSeconds: 4,
    label: `Clip ${index}`, source: { type: "media", mediaId: `media-${index % mediaCount}` },
    properties: { sourceIn: 2, sourceOut: 6, ...(scenario === "prepared-effects" ? { effects: [{ effectType: "stylize.grain", enabled: true, params: { amount: 0.2 } }], keyframes: { opacity: [{ atSeconds: 0, value: 0.8 }, { atSeconds: 4, value: 1 }] } } : {}) },
  }));
  const transitions = scenario === "transitions" || scenario === "nested"
    ? items.slice(1).map((item, index) => ({ id: `transition-${index}`, leftItemId: items[index].id, rightItemId: item.id, kind: "crossfade", durationSeconds: 1 })) : [];
  const videoTrack = { id: "video", name: "Video", kind: "video", enabled: true, locked: false, syncLocked: false, items, transitions };
  const tracks = [videoTrack];
  const durationSeconds = clipCount * 4;
  const transcripts = [];
  if (scenario === "captions") {
    const captions = items.map((item, index) => ({ id: `caption-${index}`, kind: "caption", startSeconds: item.startSeconds, durationSeconds: 4, label: `Caption ${index}`, source: { type: "text", text: `Deterministic caption ${index}` }, properties: { placement: "lower", stylePreset: "boldReadableLower" } }));
    tracks.push({ id: "captions", name: "Captions", kind: "caption", enabled: true, locked: false, syncLocked: false, items: captions });
    transcripts.push({ id: "transcript", segments: captions.map((caption) => ({ startSeconds: caption.startSeconds, endSeconds: caption.startSeconds + caption.durationSeconds, text: caption.source.text })) });
  }
  let timeline = { durationSeconds, tracks };
  const timelines = [];
  if (scenario === "nested") {
    timelines.push({ id: "nested", timeline });
    timeline = { durationSeconds, tracks: [{ ...videoTrack, transitions: [], items: [{ id: "wrapper", kind: "video_clip", startSeconds: 0, durationSeconds, label: "Nested sequence", source: { type: "timeline", timelineId: "nested" }, properties: {} }] }] };
  }
  return {
    scenario,
    dimensions: { videoClips: clipCount, rootItems: timeline.tracks.reduce((sum, track) => sum + track.items.length, 0), tracks: tracks.length, transitions: transitions.length, nestedTimelines: timelines.length, captions: scenario === "captions" ? clipCount : 0, transcriptSegments: scenario === "captions" ? clipCount : 0, mediaAssets: mediaCount, preparedEffectClips: scenario === "prepared-effects" ? clipCount : 0, durationSeconds, width: 1920, height: 1080, fps: 30 },
    input: { timeline, timelines, media, generatedAssets: [], fps: 30 },
    transcripts,
  };
}
