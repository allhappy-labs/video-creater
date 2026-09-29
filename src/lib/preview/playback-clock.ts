/** A playing media element further than this from its timeline source time is re-seeked. */
const mediaDriftToleranceSeconds = 0.1;
const fallbackFps = 24;
/** Absorbs float error so a playhead sitting on a frame boundary counts as on the grid. */
const frameGridEpsilon = 1e-6;

interface PlaybackClockState {
  readonly playheadSeconds: number;
  readonly durationSeconds: number;
  /** The previous animation frame timestamp, or null right after start or a visibility change. */
  readonly lastFrameMs: number | null;
}

interface PlaybackClockTick {
  readonly playheadSeconds: number;
  readonly lastFrameMs: number;
  readonly ended: boolean;
}

/**
 * One playback clock step: adds the wall-clock time since the previous frame (never negative) to
 * the playhead and clamps it to the timeline. `ended` means playback must stop.
 */
export function advancePlayhead(state: PlaybackClockState, nowMs: number): PlaybackClockTick {
  const duration = Number.isFinite(state.durationSeconds) ? Math.max(0, state.durationSeconds) : 0;
  const current = Number.isFinite(state.playheadSeconds) ? Math.min(Math.max(0, state.playheadSeconds), duration) : 0;
  const elapsedSeconds = state.lastFrameMs === null ? 0 : Math.max(0, (nowMs - state.lastFrameMs) / 1000);
  const playheadSeconds = Math.min(duration, current + elapsedSeconds);
  return { playheadSeconds, lastFrameMs: nowMs, ended: playheadSeconds >= duration };
}

/** Pressing play at or past the end restarts from the beginning. */
export function playheadForPlayStart(playheadSeconds: number, durationSeconds: number): number {
  return playheadSeconds >= durationSeconds ? 0 : playheadSeconds;
}

/**
 * Media elements follow the clock: a paused element (or one without a finite time) always takes
 * the target time; a playing one is only corrected when it drifts past the tolerance.
 */
export function mediaElementNeedsSeek(input: {
  readonly currentTime: number;
  readonly targetSeconds: number;
  readonly playing: boolean;
}): boolean {
  if (!input.playing || !Number.isFinite(input.currentTime)) return true;
  return Math.abs(input.currentTime - input.targetSeconds) > mediaDriftToleranceSeconds;
}

/** `HTMLMediaElement.volume` for a preview audio layer gain (fades and dB already applied). */
export function mediaElementVolume(gain: number): number {
  return Number.isFinite(gain) ? Math.max(0, Math.min(1, gain)) : 0;
}

/** The previous or next frame boundary at `fps`, snapping an off-grid playhead onto the grid. */
export function frameStepSeconds(playheadSeconds: number, fps: number, direction: -1 | 1): number {
  const rate = Number.isFinite(fps) && fps > 0 ? fps : fallbackFps;
  const position = Math.max(0, Number.isFinite(playheadSeconds) ? playheadSeconds : 0) * rate;
  const frame = direction > 0 ? Math.floor(position + frameGridEpsilon) + 1 : Math.ceil(position - frameGridEpsilon) - 1;
  return Math.max(0, frame) / rate;
}
