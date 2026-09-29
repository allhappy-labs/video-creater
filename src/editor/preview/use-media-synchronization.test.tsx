import { render } from "@testing-library/react";
import { useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { installMediaAndFrameStubs } from "./media-element-stubs";
import { useMediaSynchronization } from "./use-media-synchronization";

interface ProbeProps {
  readonly playing?: boolean;
  readonly sourceTimeSeconds?: number;
  readonly playbackRate?: number;
  readonly onElement?: (element: HTMLAudioElement) => void;
}

function Probe({ playing = false, sourceTimeSeconds = 0, playbackRate, onElement }: ProbeProps) {
  const ref = useRef<HTMLAudioElement>(null);
  useMediaSynchronization(ref, { playing, itemId: "music", sourceUrl: "media/music.wav", sourceTimeSeconds, gain: 1, ...(playbackRate === undefined ? {} : { playbackRate }) });
  return (
    <audio
      ref={(element) => {
        ref.current = element;
        if (element) onElement?.(element);
      }}
      data-testid="probe"
    />
  );
}

describe("useMediaSynchronization", () => {
  beforeEach(() => {
    installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("plays at the layer rate with pitch preserved, and resets to normal speed", () => {
    const view = render(<Probe playbackRate={2} />);
    const element = view.getByTestId("probe") as HTMLAudioElement & { preservesPitch?: boolean };
    expect(element.playbackRate).toBe(2);
    expect(element.preservesPitch).toBe(true);

    view.rerender(<Probe playbackRate={1} />);
    expect(element.playbackRate).toBe(1);
    expect(element.preservesPitch).toBe(true);
  });

  it("sets the WebKit pitch flag when the element has one", () => {
    const flagged: { element?: HTMLAudioElement & { webkitPreservesPitch?: boolean } } = {};
    render(
      <Probe
        playbackRate={1.5}
        onElement={(element) => {
          if (!("webkitPreservesPitch" in element)) Object.defineProperty(element, "webkitPreservesPitch", { value: false, writable: true, configurable: true });
          flagged.element = element;
        }}
      />,
    );
    expect(flagged.element?.playbackRate).toBe(1.5);
    expect(flagged.element?.webkitPreservesPitch).toBe(true);
  });

  it("keeps seeking by the drift rule while retimed", () => {
    const view = render(<Probe playing playbackRate={2} sourceTimeSeconds={4} />);
    const element = view.getByTestId("probe") as HTMLAudioElement;
    expect(element.currentTime).toBe(4);
    // Within the drift tolerance a playing element is left alone.
    view.rerender(<Probe playing playbackRate={2} sourceTimeSeconds={4.01} />);
    expect(element.currentTime).toBe(4);
    view.rerender(<Probe playing playbackRate={2} sourceTimeSeconds={6} />);
    expect(element.currentTime).toBe(6);
  });
});
