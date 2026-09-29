import { act } from "@testing-library/react";
import { vi } from "vitest";

/**
 * Test doubles for jsdom, which has neither media playback nor animation frames: media elements
 * keep `currentTime` and `paused` in memory and `play()` resolves; animation frames queue until
 * `runFrame`. Call from `beforeEach`, and `vi.unstubAllGlobals()` plus `vi.restoreAllMocks()` after.
 */
export function installMediaAndFrameStubs() {
  let frames: { callback: FrameRequestCallback; id: number }[] = [];
  let nextFrameId = 1;
  const times = new WeakMap<HTMLMediaElement, number>();
  const paused = new WeakMap<HTMLMediaElement, boolean>();
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    const id = nextFrameId++;
    frames.push({ callback, id });
    return id;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => {
    frames = frames.filter((frame) => frame.id !== id);
  });
  const play = vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(function (this: HTMLMediaElement) {
    paused.set(this, false);
    return Promise.resolve();
  });
  const pause = vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(function (this: HTMLMediaElement) {
    paused.set(this, true);
  });
  vi.spyOn(HTMLMediaElement.prototype, "paused", "get").mockImplementation(function (this: HTMLMediaElement) {
    return paused.get(this) ?? true;
  });
  vi.spyOn(HTMLMediaElement.prototype, "currentTime", "get").mockImplementation(function (this: HTMLMediaElement) {
    return times.get(this) ?? 0;
  });
  vi.spyOn(HTMLMediaElement.prototype, "currentTime", "set").mockImplementation(function (this: HTMLMediaElement, value: number) {
    times.set(this, value);
  });
  return {
    play,
    pause,
    pendingFrameCount: () => frames.length,
    /** Runs the queued animation frames at `nowMs`. */
    runFrame(nowMs: number) {
      const pending = frames;
      frames = [];
      act(() => {
        for (const frame of pending) frame.callback(nowMs);
      });
    },
  };
}
