import { useEffect, type RefObject } from "react";
import { advancePlayhead } from "@/lib/preview/playback-clock";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { pausePreviewMedia, previewPlaybackReady, resumePreviewMedia } from "./playback-readiness";

/**
 * The single playback clock. While timeline playback runs, each animation frame adds the elapsed
 * wall-clock time to the store playhead (media elements follow the playhead) and stops at the end.
 * A visibility change re-anchors the clock so time spent in a hidden tab is not skipped over.
 */
export function usePlaybackClock(panelRef: RefObject<HTMLElement | null>): void {
  const store = useEditorStoreApi();
  const running = useEditorStore((state) => state.playing && state.previewSource.kind === "timeline");

  useEffect(() => {
    if (!running) return;
    let lastFrameMs: number | null = null;
    let frameId: number | null = null;
    let buffering = false;
    const reanchor = () => {
      lastFrameMs = null;
    };
    const advance = (nowMs: number) => {
      if (!previewPlaybackReady(panelRef.current)) {
        buffering = true;
        pausePreviewMedia(panelRef.current);
        lastFrameMs = null;
        frameId = window.requestAnimationFrame(advance);
        return;
      }
      if (buffering) {
        buffering = false;
        resumePreviewMedia(panelRef.current, () => {
          const latest = store.getState();
          return latest.playing && latest.previewSource.kind === "timeline" && previewPlaybackReady(panelRef.current);
        });
      }
      const state = store.getState();
      const tick = advancePlayhead(
        { playheadSeconds: state.playheadSeconds, durationSeconds: state.project.timeline.durationSeconds, lastFrameMs },
        nowMs,
      );
      lastFrameMs = tick.lastFrameMs;
      if (tick.playheadSeconds !== state.playheadSeconds) state.seek(tick.playheadSeconds);
      if (tick.ended) {
        frameId = null;
        state.setPlaying(false);
        return;
      }
      frameId = window.requestAnimationFrame(advance);
    };
    frameId = window.requestAnimationFrame(advance);
    document.addEventListener("visibilitychange", reanchor);
    return () => {
      document.removeEventListener("visibilitychange", reanchor);
      if (frameId !== null) window.cancelAnimationFrame(frameId);
    };
  }, [running, store, panelRef]);
}
