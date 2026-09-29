import { Trash2 } from "lucide-react";
import { IconButton } from "@/components/ui/icon-button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { TransitionKind } from "@/lib/timeline";
import {
  transitionDurationRange,
  transitionKindLabels,
  transitionKinds,
} from "@/lib/timeline-ops/transition-commands";
import { formatTransitionMaxSeconds, formatTransitionSeconds } from "@/lib/timeline-ops/transitions";
import { useEditorStore } from "../store/editor-store-context";
import { transitionKindIcons } from "../timeline/transition-badge";
import { useSelectedTransition, useTransitionCommands } from "../timeline/transition-commands";
import { PropertySection } from "./controls/property-section";
import { SliderField } from "./controls/slider-field";

/** "1.0s", "0.25s": the same text as the timeline badge. */
const formatDuration = (seconds: number) => `${formatTransitionSeconds(seconds)}s`;

/** Slider values stay on hundredths: the lowest hundredth at or above one frame. */
function sliderBounds(range: { readonly min: number; readonly max: number }) {
  const min = Math.ceil(range.min * 100 - 1e-6) / 100;
  const max = Math.floor(range.max * 100 + 1e-6) / 100;
  return { min, max: Math.max(min, max), disabled: max < min };
}

/** Which sections a mobile clip tool sheet shows; the Properties panel shows both. */
export type TransitionSectionId = "type" | "duration";

/**
 * The Transition tab for the selected transition: a type segmented control and a duration slider
 * with its maximum. Each change commits one `updateTransition` (slider on release).
 */
export function TransitionTabBody({ sections = ["type", "duration"] }: { readonly sections?: readonly TransitionSectionId[] }) {
  const project = useEditorStore((state) => state.project);
  const located = useSelectedTransition();
  const commands = useTransitionCommands();
  if (!located) return null;
  const { track, transition } = located;
  const range = transitionDurationRange(project, track, transition);
  const left = track.items.find((item) => item.id === transition.leftItemId);
  const right = track.items.find((item) => item.id === transition.rightItemId);
  const locked = track.locked;

  return (
    <>
      {sections.includes("type") && (
        <PropertySection
          title="Type"
          action={
            <IconButton label="Delete transition" size="sm" disabled={locked} onClick={() => void commands.remove(transition.id)}>
              <Trash2 className="h-3.5 w-3.5" aria-hidden />
            </IconButton>
          }
        >
          <ToggleGroup
            type="single"
            aria-label="Transition type"
            value={transition.kind}
            disabled={locked}
            onValueChange={(next) => {
              const kind = transitionKinds.find((candidate) => candidate === next);
              if (kind && kind !== transition.kind) void commands.setKind(transition.id, kind);
            }}
            className="grid grid-cols-2"
          >
            {transitionKinds.map((kind: TransitionKind) => {
              const Icon = transitionKindIcons[kind];
              return (
                <ToggleGroupItem key={kind} value={kind} className="justify-start">
                  <Icon className="h-3.5 w-3.5 shrink-0" aria-hidden />
                  {transitionKindLabels[kind]}
                </ToggleGroupItem>
              );
            })}
          </ToggleGroup>
          {left && right && (
            <p className="truncate text-[12px] text-dim" title={`Between ${left.label} and ${right.label}`}>
              Between {left.label} and {right.label}
            </p>
          )}
        </PropertySection>
      )}
      {sections.includes("duration") && range && <DurationSection transitionId={transition.id} seconds={transition.durationSeconds} range={range} locked={locked} />}
    </>
  );
}

interface DurationSectionProps {
  readonly transitionId: string;
  readonly seconds: number;
  readonly range: { readonly min: number; readonly max: number };
  readonly locked: boolean;
}

function DurationSection({ transitionId, seconds, range, locked }: DurationSectionProps) {
  const commands = useTransitionCommands();
  const bounds = sliderBounds(range);
  return (
    <PropertySection title="Duration">
      <SliderField
        label="Duration"
        value={Math.min(Math.max(seconds, bounds.min), bounds.max)}
        min={bounds.min}
        max={bounds.max}
        step={0.01}
        format={formatDuration}
        disabled={locked || bounds.disabled}
        onCommit={(value) => commands.setDuration(transitionId, Math.round(value * 100) / 100)}
      />
      <p className="tabular-time text-right text-[11px] text-dim">Max {formatTransitionMaxSeconds(range.max)}s</p>
    </PropertySection>
  );
}
