import type { TimelineItem } from "@/lib/timeline";
import { waveformPeaks } from "@/lib/timeline-ops/automation";
import { isReversedItem } from "@/lib/timeline-ops/reverse";
import { cn } from "@/lib/utils";

const pixelsPerBar = 3;
const maximumBars = 1200;
const minimumBarHeight = 6;

export function waveformBarCount(width: number): number {
  return Math.min(maximumBars, Math.max(1, Math.round(width / pixelsPerBar)));
}

interface ClipWaveformProps {
  readonly item: TimelineItem;
  /** Rendered clip width in pixels; decides how many bars are drawn. */
  readonly width: number;
  readonly className?: string;
}

/** Mirrored bar waveform from `waveformPeaks` (last to first for a reversed clip), stretched to fill its box. */
export function ClipWaveform({ item, width, className }: ClipWaveformProps) {
  const peaks = isReversedItem(item) ? [...waveformPeaks(item)].reverse() : waveformPeaks(item);
  const count = waveformBarCount(width);
  return (
    <svg
      data-testid="clip-waveform"
      aria-hidden
      viewBox={`0 0 ${count} 100`}
      preserveAspectRatio="none"
      className={cn("pointer-events-none fill-current", className)}
    >
      {Array.from({ length: count }, (_, index) => {
        const peak = peaks[Math.floor((index * peaks.length) / count)] ?? 0;
        const height = Math.max(minimumBarHeight, Math.round(peak * 100));
        return <rect key={index} x={index + 0.2} width={0.6} y={(100 - height) / 2} height={height} />;
      })}
    </svg>
  );
}
