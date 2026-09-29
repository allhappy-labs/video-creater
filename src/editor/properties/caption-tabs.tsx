import { createContext, useContext, useMemo, useState, type ReactNode } from "react";
import type { CaptionMotionPreset, CaptionPlacement, CaptionStylePreset, CaptionWordAnimationPreset } from "@/lib/captions/caption-items";
import { captionWordTokens } from "@/lib/captions/caption-items";
import {
  captionGroupItems,
  captionMotionPreset,
  captionPlacement,
  captionPresetActions,
  captionStyleActions,
  captionStylePreset,
  captionTextAction,
  captionWordAnimationPreset,
  captionWordAnimationPresetActions,
  captionWordStaggerSeconds,
  type CaptionStyleScope,
} from "@/lib/properties/caption-properties";
import { textStyle } from "@/lib/properties/text-properties";
import { getCaptionReadingWarning, getTimelineItemText, type TimelineItem } from "@/lib/timeline";
import { stringProperty } from "@/lib/timeline-ops/item-properties";
import { cn } from "@/lib/utils";
import { useEditorStore } from "../store/editor-store-context";
import { captionEmphasisAction, captionEmphasizedWords, captionRegroup, captionWordsPerCue, captionWordsPerCueRange } from "./caption-edits";
import { CaptionRegroupDialog } from "./caption-regroup-dialog";
import { ColorSwatches } from "./controls/color-swatches";
import { PresetGrid } from "./controls/preset-grid";
import { PropertySection } from "./controls/property-section";
import { SegmentedField } from "./controls/segmented-field";
import { SelectField } from "./controls/select-field";
import { SliderField } from "./controls/slider-field";
import { TextCommitField } from "./controls/text-commit-field";
import { formatNumber, formatSeconds } from "./formatters";
import type { PropertyTab } from "./property-tabs";
import { fontOptions, fontSizeRange, highlightColorOptions } from "./text-style-options";
import { usePropertyCommit } from "./use-property-commit";

interface ScopeState {
  readonly scope: CaptionStyleScope;
  setScope(scope: CaptionStyleScope): void;
}

const CaptionScopeContext = createContext<ScopeState | null>(null);

/** Holds "Applies to" across the Style, Position and Animation tabs; key it by the selection. */
export function CaptionScopeProvider({ children }: { readonly children: ReactNode }) {
  const [scope, setScope] = useState<CaptionStyleScope>("all");
  const value = useMemo(() => ({ scope, setScope }), [scope]);
  return <CaptionScopeContext.Provider value={value}>{children}</CaptionScopeContext.Provider>;
}

const scopeOptions = [
  { value: "all", label: "All captions" },
  { value: "this", label: "Only this" },
] as const;

const stylePresetOptions: readonly { value: CaptionStylePreset; label: string }[] = [
  { value: "boldReadableLower", label: "Bold lower" },
  { value: "kineticFocus", label: "Kinetic focus" },
  { value: "centeredMinimal", label: "Centered minimal" },
];

const placementOptions: readonly { value: CaptionPlacement; label: string }[] = [
  { value: "lower", label: "Lower" },
  { value: "center", label: "Center" },
  { value: "upper", label: "Upper" },
];

const motionOptions: readonly { value: CaptionMotionPreset; label: string }[] = [
  { value: "snap-pop-v1", label: "Snap pop" },
  { value: "pulse-emphasis-v2", label: "Pulse emphasis" },
  { value: "soft-depth-card-v2", label: "Soft depth" },
];

const wordAnimationOptions: readonly { value: CaptionWordAnimationPreset; label: string }[] = [
  { value: "sequentialPop", label: "Sequential pop" },
  { value: "groupPulse", label: "Group pulse" },
  { value: "karaokeFade", label: "Karaoke fade" },
];

/** Legacy word accent color, used until a highlight color is chosen. */
const defaultHighlightColor = "#ffcf5a";
const maximumStaggerSeconds = 0.5;

/** The scope, the project and a committer for scoped style properties. */
function useCaptionEdit(item: TimelineItem) {
  const shared = useContext(CaptionScopeContext);
  const [localScope, setLocalScope] = useState<CaptionStyleScope>("all");
  const { scope, setScope } = shared ?? { scope: localScope, setScope: setLocalScope };
  const project = useEditorStore((state) => state.project);
  const { commit, preview } = usePropertyCommit();
  const style = (properties: Readonly<Record<string, unknown>>) => commit(captionStyleActions(project, item.id, scope, properties));
  return { project, scope, setScope, commit, preview, style };
}

function AppliesTo({ item }: { readonly item: TimelineItem }) {
  const { project, scope, setScope } = useCaptionEdit(item);
  const count = scope === "all" ? captionGroupItems(project, item.id).length : 1;
  return (
    <div className="flex flex-col gap-1.5 border-b border-line px-3 py-3">
      <SegmentedField label="Applies to" labelStyle="heading" value={scope} options={scopeOptions} onChange={setScope} />
      <p className="text-[11px] text-dim">
        {count.toString()} {count === 1 ? "cue receives" : "cues receive"} these changes.
      </p>
    </div>
  );
}

function CaptionTextTab({ item }: { readonly item: TimelineItem }) {
  const project = useEditorStore((state) => state.project);
  const text = getTimelineItemText(item);
  return (
    <PropertySection title="This caption">
      <TextCommitField
        key={item.id}
        label="Caption text"
        multiline
        rows={2}
        value={text}
        hint={getCaptionReadingWarning({ text, durationSeconds: item.durationSeconds })}
        build={(next) => captionTextAction(item, next, project)}
      />
      <p className="text-[11px] text-dim">{item.properties.textEdited === true ? "User edited" : "Generated text"}</p>
    </PropertySection>
  );
}

/** Max words per line regroups the whole caption group, so it asks first. */
function MaxWordsField({ item }: { readonly item: TimelineItem }) {
  const project = useEditorStore((state) => state.project);
  const [pending, setPending] = useState<number | null>(null);
  const current = captionWordsPerCue(project, item.id);
  const availability = captionRegroup(project, item.id, current);
  const unavailable = "blocked" in availability ? availability.blocked : null;
  return (
    <>
      <SliderField
        label="Max words"
        value={current}
        min={captionWordsPerCueRange.min}
        max={captionWordsPerCueRange.max}
        step={1}
        format={formatNumber}
        disabled={unavailable !== null}
        onCommit={(value) => {
          if (value !== current) setPending(value);
        }}
      />
      {unavailable && <p className="text-[11px] text-dim">{unavailable}</p>}
      <CaptionRegroupDialog itemId={item.id} wordsPerCue={pending} onClose={() => setPending(null)} />
    </>
  );
}

function CaptionStyleTab({ item }: { readonly item: TimelineItem }) {
  const { project, scope, commit, preview, style } = useCaptionEdit(item);
  const font = textStyle(item);
  return (
    <>
      <AppliesTo item={item} />
      <PropertySection title="Preset">
        <PresetGrid
          label="Caption style preset"
          value={captionStylePreset(item)}
          options={stylePresetOptions}
          onChange={(preset) => void commit(captionPresetActions(project, item.id, scope, preset))}
        />
      </PropertySection>
      <PropertySection title="Font" onReset={() => void style({ fontName: null, fontSize: null, highlightColor: null })}>
        <SelectField label="Family" value={font.fontName} options={fontOptions(font.fontName)} onChange={(fontName) => void style({ fontName })} />
        <SliderField
          label="Size"
          value={font.fontSize}
          min={fontSizeRange.min}
          max={fontSizeRange.max}
          step={fontSizeRange.step}
          format={formatNumber}
          onPreview={(fontSize) => preview(item.id, { fontSize })}
          onCommit={(fontSize) => style({ fontSize })}
        />
        <ColorSwatches
          label="Highlight"
          value={stringProperty(item, "highlightColor") ?? defaultHighlightColor}
          options={highlightColorOptions}
          onChange={(highlightColor) => void style({ highlightColor })}
        />
        <MaxWordsField item={item} />
      </PropertySection>
    </>
  );
}

function CaptionPositionTab({ item }: { readonly item: TimelineItem }) {
  const { style } = useCaptionEdit(item);
  return (
    <>
      <AppliesTo item={item} />
      <PropertySection title="Position">
        <SegmentedField label="Placement" value={captionPlacement(item)} options={placementOptions} onChange={(placement) => void style({ captionPlacement: placement })} />
      </PropertySection>
    </>
  );
}

/** Word emphasis belongs to this cue's own words, so it ignores "Applies to". */
function WordEmphasisSection({ item }: { readonly item: TimelineItem }) {
  const { commit } = usePropertyCommit();
  const words = captionWordTokens(getTimelineItemText(item));
  const emphasized = captionEmphasizedWords(item);
  return (
    <PropertySection title="Word emphasis">
      <div role="group" aria-label="Emphasized words" className="flex flex-wrap gap-1">
        {words.map((word, index) => {
          const pressed = emphasized.includes(index);
          return (
            <button
              key={`${index.toString()}-${word}`}
              type="button"
              aria-pressed={pressed}
              onClick={() => void commit(captionEmphasisAction(item, index))}
              className={cn(
                "h-7 rounded-md bg-raised px-2 text-[12px] text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                pressed && "bg-accent-soft text-primary ring-1 ring-primary hover:bg-accent-soft",
              )}
            >
              {word}
            </button>
          );
        })}
      </div>
      <p className="text-[11px] text-dim">Choose the words in this caption that receive the accent in preview and export.</p>
    </PropertySection>
  );
}

function CaptionAnimationTab({ item }: { readonly item: TimelineItem }) {
  const { project, scope, commit, style } = useCaptionEdit(item);
  const stagger = captionWordStaggerSeconds(item);
  const storedPreset = stringProperty(item, "captionWordAnimationPreset") === null ? null : captionWordAnimationPreset(item);
  const animateWords = (preset: CaptionWordAnimationPreset, staggerSeconds: number) =>
    commit(captionWordAnimationPresetActions(project, item.id, scope, preset, staggerSeconds));
  return (
    <>
      <AppliesTo item={item} />
      <PropertySection title="Motion">
        <PresetGrid label="Caption motion" value={captionMotionPreset(item)} options={motionOptions} onChange={(motionPresetId) => void style({ motionPresetId })} />
      </PropertySection>
      <PropertySection title="Word animation">
        <PresetGrid label="Word animation preset" value={storedPreset} options={wordAnimationOptions} onChange={(preset) => void animateWords(preset, stagger)} />
        <SliderField
          label="Stagger"
          value={stagger}
          min={0}
          max={maximumStaggerSeconds}
          step={0.01}
          format={formatSeconds}
          onCommit={(value) => animateWords(captionWordAnimationPreset(item), value)}
        />
        {captionEmphasizedWords(item).length === 0 && <p className="text-[11px] text-dim">Emphasize words below to animate them.</p>}
      </PropertySection>
      <WordEmphasisSection item={item} />
    </>
  );
}

/** Text · Style · Position · Animation; the last three follow "Applies to". */
export function CaptionTabBody({ tab, item }: { readonly tab: PropertyTab; readonly item: TimelineItem }) {
  switch (tab.id) {
    case "style":
      return <CaptionStyleTab item={item} />;
    case "position":
      return <CaptionPositionTab item={item} />;
    case "animation":
      return <CaptionAnimationTab item={item} />;
    default:
      return <CaptionTextTab item={item} />;
  }
}
