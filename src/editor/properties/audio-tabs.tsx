import { Scissors } from "lucide-react";
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
import { useEditorStoreApi } from "../store/editor-store-context";
import { PropertySection } from "./controls/property-section";
import { SliderField } from "./controls/slider-field";
import { SwitchField } from "./controls/switch-field";
import { formatDecibels, formatPercent, formatSeconds } from "./formatters";
import type { PropertyTab } from "./property-tabs";
import { SpeakersSection } from "./speakers-section";
import { ClipSpeedTab } from "./speed-tab";
import { useAnimatedProperty } from "./use-animated-property";
import { usePropertyCommit } from "./use-property-commit";

function VolumeSection({ item }: { readonly item: TimelineItem }) {
  const { commit, preview } = usePropertyCommit();
  const animated = useAnimatedProperty(item, "volumeDb", audioVolumeDb(item));
  const range = audioPropertyRanges.volumeDb;
  const clear = () => commit(volumeActions(item, null, animated.playheadSeconds));
  return (
    <PropertySection title="Volume" onReset={clear}>
      <SliderField
        label="Volume"
        value={animated.value}
        min={range.min}
        max={range.max}
        step={range.step}
        format={formatDecibels}
        keyframe={animated.keyframe}
        onBlank={clear}
        onPreview={(value) => preview(item.id, { volumeDb: value })}
        onCommit={(value) => commit(volumeActions(item, value, animated.playheadSeconds))}
      />
    </PropertySection>
  );
}

function AudioFadeSection({ item }: { readonly item: TimelineItem }) {
  const { commit, preview } = usePropertyCommit();
  const fades = audioFades(item);
  const max = Math.max(item.durationSeconds, 0);
  const step = audioPropertyRanges.fade.step;
  return (
    <PropertySection title="Fade" onReset={() => commit(audioFadesAction(item, 0, 0))}>
      <SliderField
        label="Fade in"
        value={Math.min(fades.fadeInSeconds, max)}
        min={0}
        max={max}
        step={step}
        format={formatSeconds}
        onPreview={(value) => preview(item.id, { fadeInSeconds: value })}
        onCommit={(value) => commit(audioFadesAction(item, value, fades.fadeOutSeconds))}
      />
      <SliderField
        label="Fade out"
        value={Math.min(fades.fadeOutSeconds, max)}
        min={0}
        max={max}
        step={step}
        format={formatSeconds}
        onPreview={(value) => preview(item.id, { fadeOutSeconds: value })}
        onCommit={(value) => commit(audioFadesAction(item, fades.fadeInSeconds, value))}
      />
    </PropertySection>
  );
}

/** Denoise switch, strength, preparation status and Retry. */
export function DenoiseSection({ item }: { readonly item: TimelineItem }) {
  const { commit } = usePropertyCommit();
  const denoise = audioDenoise(item);
  const percent = Math.round(denoise.amount * 100);
  const range = audioPropertyRanges.denoisePercent;
  return (
    <PropertySection title="Denoise">
      <SwitchField
        label="Denoise"
        checked={denoise.enabled}
        {...(denoise.enabled ? { description: denoiseStatusLabel(denoise.status) } : {})}
        onChange={(enabled) => void commit(denoiseActions(item, enabled, denoise.amount))}
      />
      <SliderField
        label="Strength"
        value={percent}
        min={range.min}
        max={range.max}
        step={range.step}
        format={formatPercent}
        disabled={!denoise.enabled}
        onCommit={(value) => commit(denoiseActions(item, true, value / 100))}
      />
      {denoise.enabled && denoise.status === "failed" && (
        <button
          type="button"
          onClick={() => void commit([retryDenoiseAction(item.id)])}
          className="h-7 self-start rounded-control bg-raised px-2.5 text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          Retry denoise
        </button>
      )}
    </PropertySection>
  );
}

/** Volume and fades; the visual clip Audio tab reuses these for its linked audio clip. */
export function AudioBasicSections({ item }: { readonly item: TimelineItem }) {
  return (
    <>
      <VolumeSection item={item} />
      <AudioFadeSection item={item} />
    </>
  );
}

/** Silence removal needs the range review, so this hands off to the Audio tab's Remove silences card. */
function SilencesSection() {
  const store = useEditorStoreApi();
  function reviewInAudioTab() {
    const state = store.getState();
    state.setPendingCleanupFocus("removeSilences");
    state.setActiveTab("audio");
    // On mobile Properties is itself a sheet, so the Audio tab opens as the sheet that replaces it.
    if (state.openSheetId !== null) state.openSheet("audio");
  }
  return (
    <PropertySection title="Silences">
      <button
        type="button"
        onClick={reviewInAudioTab}
        className="flex h-7 items-center gap-1.5 self-start rounded-control bg-raised px-2.5 text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none"
      >
        <Scissors className="h-3.5 w-3.5" aria-hidden />
        Remove silences…
      </button>
    </PropertySection>
  );
}

/**
 * Basic (volume, fades), Voice (denoise, silences, speakers) and Speed. Speed retimes the clip with
 * `updateAudioClipSpeed`; renders and the preview keep its pitch.
 */
export function AudioTabBody({ tab, item }: { readonly tab: PropertyTab; readonly item: TimelineItem }) {
  if (tab.id === "speed") return <ClipSpeedTab item={item} />;
  if (tab.id === "voice") {
    return (
      <>
        <DenoiseSection item={item} />
        <SilencesSection />
        <SpeakersSection />
      </>
    );
  }
  return <AudioBasicSections item={item} />;
}
