import { describe, expect, it } from "vitest";
import {
  audioDenoise,
  audioFades,
  audioFadesAction,
  audioPropertyRanges,
  audioVolumeDb,
  denoiseActions,
  denoiseStatusLabel,
  retryDenoiseAction,
  volumeActions,
} from "@/lib/properties/audio-properties";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";

function audio(properties: Record<string, unknown> = {}, kind: TimelineItem["kind"] = "audio_clip"): TimelineItem {
  return {
    id: "music",
    kind,
    startSeconds: 1,
    durationSeconds: 4,
    source: { type: "media", mediaId: "media-voiceover" },
    label: "Music",
    properties,
  };
}

function actionsOf(result: CommandResult) {
  if ("blocked" in result) throw new Error(`Unexpectedly blocked: ${result.blocked}`);
  return result.actions;
}

function blockedOf(result: CommandResult) {
  if (!("blocked" in result)) throw new Error("Expected the edit to be blocked");
  return result.blocked;
}

const queuedPreparation = {
  status: "queued",
  progress: 0,
  retryable: true,
  algorithm: "adaptive-noise-gate-v1",
};

describe("audio readers", () => {
  it("return defaults for missing or invalid values", () => {
    expect(audioVolumeDb(audio())).toBe(0);
    expect(audioVolumeDb(audio({ volumeDb: 40 }))).toBe(0);
    expect(audioFades(audio({ fadeInSeconds: -2, fadeOutSeconds: "1" }))).toEqual({ fadeInSeconds: 0, fadeOutSeconds: 0 });
    expect(audioDenoise(audio())).toEqual({ enabled: false, amount: 0.6, status: null });
  });

  it("read stored values", () => {
    expect(audioVolumeDb(audio({ volumeDb: -6 }))).toBe(-6);
    expect(audioFades(audio({ fadeInSeconds: 0.5, fadeOutSeconds: 0.75 }))).toEqual({
      fadeInSeconds: 0.5,
      fadeOutSeconds: 0.75,
    });
    const denoised = audio({
      effects: [{ effectType: "audio.denoise", enabled: true, params: { amount: 0.3 } }],
      audioDenoisePreparation: { status: "failed" },
    });
    expect(audioDenoise(denoised)).toEqual({ enabled: true, amount: 0.3, status: "failed" });
  });

  it("counts a preparation status as enabled, like the legacy draft", () => {
    expect(audioDenoise(audio({ audioDenoisePreparation: { status: "preparing" } })).enabled).toBe(true);
  });

  it.each([
    ["failed", "Failed"],
    ["completed", "Ready"],
    ["ready", "Ready"],
    ["preparing", "Preparing"],
    ["progress", "Preparing"],
    ["queued", "Queued"],
    [null, "Queued"],
  ])("labels denoise status %s as %s", (status, label) => {
    expect(denoiseStatusLabel(status)).toBe(label);
  });
});

describe("audio ranges", () => {
  it("match the legacy inspector inputs", () => {
    expect(audioPropertyRanges.volumeDb).toEqual({ min: -60, max: 24, step: 0.5 });
    expect(audioPropertyRanges.denoisePercent).toEqual({ min: 0, max: 100, step: 1 });
  });
});

describe("volumeActions", () => {
  it("emits updateAudioVolume, with null clearing the volume", () => {
    expect(actionsOf(volumeActions(audio(), -3, 2))).toEqual([{ type: "updateAudioVolume", itemId: "music", volumeDb: -3 }]);
    expect(actionsOf(volumeActions(audio(), null, 2))).toEqual([{ type: "updateAudioVolume", itemId: "music", volumeDb: null }]);
  });

  it("upserts at the clip-local playhead when volume is keyframed", () => {
    const item = audio({ keyframes: { volumeDb: [{ atSeconds: 0, value: 0 }] } });
    expect(actionsOf(volumeActions(item, -12, 3))).toEqual([
      { type: "upsertItemKeyframe", itemId: "music", property: "volumeDb", keyframe: { atSeconds: 2, value: -12 } },
    ]);
  });

  it("rejects values outside -60 to 24 dB and non-audio items", () => {
    expect(blockedOf(volumeActions(audio(), 30, 0))).toMatch(/volume/i);
    expect(blockedOf(volumeActions(audio({}, "video_clip"), -3, 0))).toMatch(/audio/i);
  });
});

describe("audioFadesAction", () => {
  it("emits updateAudioFades", () => {
    expect(actionsOf(audioFadesAction(audio(), 1, 2))).toEqual([
      { type: "updateAudioFades", itemId: "music", fadeInSeconds: 1, fadeOutSeconds: 2 },
    ]);
  });

  it("rejects negative fades and fades longer than the clip", () => {
    expect(blockedOf(audioFadesAction(audio(), -0.1, 0))).toMatch(/fade/i);
    expect(blockedOf(audioFadesAction(audio(), 3, 1.5))).toMatch(/fade/i);
  });
});

describe("denoiseActions", () => {
  const existing = [
    { effectType: "audio.denoise", enabled: true, params: { amount: 0.9 } },
    { effectType: "audio.eq", enabled: false, params: { lowGain: 2 } },
    { effectType: 7, enabled: true, params: {} },
    { effectInstanceId: "compressor-1", effectType: "audio.compressor", enabled: true, params: {} },
  ];

  it("matches the pre-cut applyAudioDenoise pair when enabling", () => {
    expect(actionsOf(denoiseActions(audio({ effects: existing }), true, 0.45))).toEqual([
      {
        type: "updateItemEffects",
        itemIds: ["music"],
        effects: [
          { effectInstanceId: "legacy:audio.eq:1", effectType: "audio.eq", enabled: false, params: { lowGain: 2 } },
          { effectInstanceId: "compressor-1", effectType: "audio.compressor", enabled: true, params: {} },
          { effectInstanceId: "legacy:audio.denoise:1", effectType: "audio.denoise", enabled: true, params: { amount: 0.45 } },
        ],
      },
      {
        type: "updateItemProperties",
        updates: [{ itemId: "music", set: { audioDenoisePreparation: queuedPreparation }, remove: [] }],
      },
    ]);
  });

  it("matches the pre-cut applyAudioDenoise pair when disabling", () => {
    expect(actionsOf(denoiseActions(audio({ effects: existing }), false, 0.45))).toEqual([
      {
        type: "updateItemEffects",
        itemIds: ["music"],
        effects: [
          { effectInstanceId: "legacy:audio.eq:1", effectType: "audio.eq", enabled: false, params: { lowGain: 2 } },
          { effectInstanceId: "compressor-1", effectType: "audio.compressor", enabled: true, params: {} },
        ],
      },
      {
        type: "updateItemProperties",
        updates: [{ itemId: "music", set: {}, remove: ["audioDenoisePreparation"] }],
      },
    ]);
  });

  it("applies to audio clips only and keeps the amount within 0 to 1", () => {
    expect(blockedOf(denoiseActions(audio({}, "video_clip"), true, 0.5))).toMatch(/audio/i);
    expect(blockedOf(denoiseActions(audio(), true, 1.5))).toMatch(/strength/i);
  });

  it("re-queues preparation on retry", () => {
    expect(retryDenoiseAction("music")).toEqual({
      type: "updateItemProperties",
      updates: [{ itemId: "music", set: { audioDenoisePreparation: queuedPreparation }, remove: [] }],
    });
  });
});
