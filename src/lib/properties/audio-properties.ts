import { legacyEffectInstanceId, type ProjectAction } from "@/lib/project";
import { keyframedEditAction } from "@/lib/properties/keyframe-actions";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import {
  audioDenoiseAmount,
  audioDenoisePreparationStatus,
  hasEffect,
  numberProperty,
  projectActionEffectsForItem,
} from "@/lib/timeline-ops/item-properties";

export interface AudioFades {
  readonly fadeInSeconds: number;
  readonly fadeOutSeconds: number;
}

export interface AudioDenoiseState {
  readonly enabled: boolean;
  /** Strength 0–1; the slider shows `round(amount * 100)%`. */
  readonly amount: number;
  readonly status: string | null;
}

/** Legacy inspector input ranges. */
export const audioPropertyRanges = {
  volumeDb: { min: -60, max: 24, step: 0.5 },
  fade: { min: 0, max: Number.POSITIVE_INFINITY, step: 0.1 },
  denoisePercent: { min: 0, max: 100, step: 1 },
} as const;

const notAudioMessage = "Select an audio clip to change this property.";
const denoiseEffectType = "audio.denoise";

function queuedDenoisePreparation() {
  return { status: "queued", progress: 0, retryable: true, algorithm: "adaptive-noise-gate-v1" };
}

export function audioVolumeDb(item: TimelineItem): number {
  const value = numberProperty(item, "volumeDb");
  const range = audioPropertyRanges.volumeDb;
  return value !== null && value >= range.min && value <= range.max ? value : 0;
}

export function audioFades(item: TimelineItem): AudioFades {
  const read = (key: string) => {
    const value = numberProperty(item, key);
    return value !== null && value >= 0 ? value : 0;
  };
  return { fadeInSeconds: read("fadeInSeconds"), fadeOutSeconds: read("fadeOutSeconds") };
}

/** Legacy draft: enabled when the effect exists or a preparation status is stored. */
export function audioDenoise(item: TimelineItem): AudioDenoiseState {
  const status = audioDenoisePreparationStatus(item);
  return {
    enabled: hasEffect(item, denoiseEffectType) || status !== null,
    amount: audioDenoiseAmount(item),
    status,
  };
}

export function denoiseStatusLabel(status: string | null): "Failed" | "Ready" | "Preparing" | "Queued" {
  if (status === "failed") return "Failed";
  if (status === "completed" || status === "ready") return "Ready";
  if (status === "preparing" || status === "progress") return "Preparing";
  return "Queued";
}

/**
 * `null` clears the stored volume (0 dB). When volume is keyframed the edit upserts at the
 * clip-local playhead instead, with `null` keyed as 0 dB.
 */
export function volumeActions(item: TimelineItem, volumeDb: number | null, playheadSeconds: number): CommandResult {
  if (item.kind !== "audio_clip") return { blocked: notAudioMessage };
  const range = audioPropertyRanges.volumeDb;
  if (volumeDb !== null && !(Number.isFinite(volumeDb) && volumeDb >= range.min && volumeDb <= range.max)) {
    return { blocked: "Volume must be between -60 and 24 dB." };
  }
  const keyframed = keyframedEditAction(item, "volumeDb", playheadSeconds, volumeDb ?? 0);
  return { actions: [keyframed ?? { type: "updateAudioVolume", itemId: item.id, volumeDb }] };
}

export function audioFadesAction(item: TimelineItem, fadeInSeconds: number, fadeOutSeconds: number): CommandResult {
  if (item.kind !== "audio_clip") return { blocked: notAudioMessage };
  if (!Number.isFinite(fadeInSeconds) || !Number.isFinite(fadeOutSeconds) || fadeInSeconds < 0 || fadeOutSeconds < 0) {
    return { blocked: "Fades must be 0 seconds or longer." };
  }
  if (fadeInSeconds + fadeOutSeconds > item.durationSeconds) {
    return { blocked: "Fade in and fade out together can't be longer than the clip." };
  }
  return { actions: [{ type: "updateAudioFades", itemId: item.id, fadeInSeconds, fadeOutSeconds }] };
}

/**
 * Pre-cut `applyAudioDenoise`: rebuild the effect list without any `audio.denoise` (malformed
 * entries dropped, missing instance ids filled), append the denoise effect when enabling, and
 * queue or clear `audioDenoisePreparation` in the same batch.
 */
export function denoiseActions(item: TimelineItem, enabled: boolean, amount: number): CommandResult {
  if (item.kind !== "audio_clip") return { blocked: notAudioMessage };
  if (enabled && !(Number.isFinite(amount) && amount >= 0 && amount <= 1)) {
    return { blocked: "Denoise strength must be between 0 and 100%." };
  }
  const effects = projectActionEffectsForItem(item).filter((effect) => effect.effectType !== denoiseEffectType);
  if (enabled) {
    effects.push({
      effectInstanceId: legacyEffectInstanceId(denoiseEffectType, 0),
      effectType: denoiseEffectType,
      enabled: true,
      params: { amount },
    });
  }
  return {
    actions: [
      { type: "updateItemEffects", itemIds: [item.id], effects },
      {
        type: "updateItemProperties",
        updates: [
          {
            itemId: item.id,
            set: enabled ? { audioDenoisePreparation: queuedDenoisePreparation() } : {},
            remove: enabled ? [] : ["audioDenoisePreparation"],
          },
        ],
      },
    ],
  };
}

/** Pre-cut `retryAudioDenoise`: re-queue preparation. */
export function retryDenoiseAction(itemId: string): ProjectAction {
  return {
    type: "updateItemProperties",
    updates: [{ itemId, set: { audioDenoisePreparation: queuedDenoisePreparation() }, remove: [] }],
  };
}
