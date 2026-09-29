import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { TimelineItem } from "@/lib/timeline";
import { waveformPeaks } from "@/lib/timeline-ops/automation";
import { ClipWaveform, waveformBarCount } from "./clip-waveform";

function audioItem(properties: Record<string, unknown> = {}): TimelineItem {
  return {
    id: "audio-1",
    kind: "audio_clip",
    startSeconds: 0,
    durationSeconds: 4,
    source: { type: "media", mediaId: "media-audio" },
    label: "Voice",
    properties,
  };
}

function bars(): SVGRectElement[] {
  return Array.from(screen.getByTestId("clip-waveform").querySelectorAll("rect"));
}

describe("ClipWaveform", () => {
  it("draws one bar per three pixels, sampling the supplied peaks", () => {
    render(<ClipWaveform item={audioItem({ waveformPeaks: [0.2, 1] })} width={12} />);
    expect(waveformBarCount(12)).toBe(4);
    expect(bars().map((bar) => bar.getAttribute("height"))).toEqual(["20", "20", "100", "100"]);
  });

  it("draws a reversed clip's peaks last to first", () => {
    render(<ClipWaveform item={audioItem({ waveformPeaks: [0.2, 1], reverse: true })} width={12} />);
    expect(bars().map((bar) => bar.getAttribute("height"))).toEqual(["100", "100", "20", "20"]);
  });

  it("centers bars vertically and keeps a visible minimum height", () => {
    render(<ClipWaveform item={audioItem({ waveformPeaks: [0, 0.5] })} width={6} />);
    const [silent, half] = bars();
    expect(silent).toHaveAttribute("height", "6");
    expect(silent).toHaveAttribute("y", "47");
    expect(half).toHaveAttribute("y", "25");
  });

  it("falls back to deterministic peaks when none are supplied", () => {
    const item = audioItem();
    render(<ClipWaveform item={item} width={3 * 24} />);
    expect(bars().map((bar) => Number(bar.getAttribute("height")))).toEqual(
      waveformPeaks(item).map((peak) => Math.max(6, Math.round(peak * 100))),
    );
  });

  it("caps the bar count for very wide clips", () => {
    expect(waveformBarCount(1_000_000)).toBe(1200);
    expect(waveformBarCount(0)).toBe(1);
  });
});
