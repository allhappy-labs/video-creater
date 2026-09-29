import { describe, expect, it } from "vitest";
import {
  advancePlayhead,
  frameStepSeconds,
  mediaElementNeedsSeek,
  mediaElementVolume,
  playheadForPlayStart,
} from "@/lib/preview/playback-clock";

describe("advancePlayhead", () => {
  it("anchors the first frame without moving the playhead", () => {
    expect(advancePlayhead({ playheadSeconds: 1, durationSeconds: 8, lastFrameMs: null }, 500)).toEqual({
      playheadSeconds: 1,
      lastFrameMs: 500,
      ended: false,
    });
  });

  it("advances by the elapsed wall-clock time at speed 1", () => {
    const tick = advancePlayhead({ playheadSeconds: 1, durationSeconds: 8, lastFrameMs: 1000 }, 1250);
    expect(tick).toEqual({ playheadSeconds: 1.25, lastFrameMs: 1250, ended: false });
  });

  it("never moves backwards when timestamps go back", () => {
    expect(advancePlayhead({ playheadSeconds: 2, durationSeconds: 8, lastFrameMs: 1000 }, 900).playheadSeconds).toBe(2);
  });

  it("stops at the end of the timeline", () => {
    expect(advancePlayhead({ playheadSeconds: 7.9, durationSeconds: 8, lastFrameMs: 0 }, 500)).toEqual({
      playheadSeconds: 8,
      lastFrameMs: 500,
      ended: true,
    });
  });

  it("treats an empty or invalid timeline as ended at 0", () => {
    expect(advancePlayhead({ playheadSeconds: 3, durationSeconds: Number.NaN, lastFrameMs: 0 }, 16)).toEqual({
      playheadSeconds: 0,
      lastFrameMs: 16,
      ended: true,
    });
  });
});

describe("playheadForPlayStart", () => {
  it("restarts from 0 when play is pressed at or past the end", () => {
    expect(playheadForPlayStart(8, 8)).toBe(0);
    expect(playheadForPlayStart(9, 8)).toBe(0);
    expect(playheadForPlayStart(3.5, 8)).toBe(3.5);
  });
});

describe("mediaElementNeedsSeek", () => {
  it("always seeks while paused and when the element time is not finite", () => {
    expect(mediaElementNeedsSeek({ currentTime: 1, targetSeconds: 1.01, playing: false })).toBe(true);
    expect(mediaElementNeedsSeek({ currentTime: Number.NaN, targetSeconds: 1, playing: true })).toBe(true);
  });

  it("only seeks a playing element when it drifts more than 0.1 s", () => {
    expect(mediaElementNeedsSeek({ currentTime: 1, targetSeconds: 1.09, playing: true })).toBe(false);
    expect(mediaElementNeedsSeek({ currentTime: 1, targetSeconds: 1.11, playing: true })).toBe(true);
    expect(mediaElementNeedsSeek({ currentTime: 1.2, targetSeconds: 1, playing: true })).toBe(true);
  });
});

describe("mediaElementVolume", () => {
  it("clamps the layer gain to the media element volume range", () => {
    expect(mediaElementVolume(0.5)).toBe(0.5);
    expect(mediaElementVolume(1.8)).toBe(1);
    expect(mediaElementVolume(-1)).toBe(0);
    expect(mediaElementVolume(Number.NaN)).toBe(0);
  });
});

describe("frameStepSeconds", () => {
  it("steps to the neighbouring frame at 24 fps", () => {
    expect(frameStepSeconds(1, 24, 1)).toBeCloseTo(1 + 1 / 24, 9);
    expect(frameStepSeconds(1, 24, -1)).toBeCloseTo(1 - 1 / 24, 9);
  });

  it("snaps an off-grid playhead to the next frame boundary in the step direction", () => {
    expect(frameStepSeconds(1.01, 24, 1)).toBeCloseTo(25 / 24, 9);
    expect(frameStepSeconds(1.01, 24, -1)).toBeCloseTo(1, 9);
  });

  it("stays on the grid after repeated steps and never goes below 0", () => {
    let seconds = 0;
    for (let index = 0; index < 48; index += 1) seconds = frameStepSeconds(seconds, 24, 1);
    expect(seconds).toBeCloseTo(2, 9);
    expect(frameStepSeconds(0, 24, -1)).toBe(0);
  });

  it("falls back to 24 fps when the render fps is invalid", () => {
    expect(frameStepSeconds(0, 0, 1)).toBeCloseTo(1 / 24, 9);
  });
});
