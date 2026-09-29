import { useEffect, useRef, type RefObject } from "react";
import { mediaElementNeedsSeek, mediaElementVolume } from "@/lib/preview/playback-clock";

interface MediaSynchronizationOptions {
  readonly playing: boolean;
  readonly itemId: string;
  readonly sourceUrl: string;
  /** Where the element should be, from the frame built at the store playhead. */
  readonly sourceTimeSeconds: number;
  /** Audio layer gain; video layers are muted and leave volume alone. */
  readonly gain?: number;
  /** The clip speed; pitch is preserved so retimed audio keeps its key. Defaults to 1. */
  readonly playbackRate?: number;
}

type PitchPreservingMediaElement = HTMLMediaElement & { preservesPitch?: boolean; webkitPreservesPitch?: boolean };

/**
 * Keeps one media element following the playback clock: it seeks when paused or drifting, takes
 * its volume from the layer gain, plays at the clip speed with pitch preserved, and plays or
 * pauses with the store. A `play()` still pending for the same item and URL is not repeated, and a
 * play that settles after playback stopped is paused again. Returns a callback for `loadedmetadata`.
 */
export function useMediaSynchronization(elementRef: RefObject<HTMLMediaElement | null>, options: MediaSynchronizationOptions) {
  const { playing, itemId, sourceUrl, sourceTimeSeconds, gain } = options;
  const playbackRate = options.playbackRate ?? 1;
  const identity = `${itemId}\u0000${sourceUrl}`;
  const identityRef = useRef(identity);
  const generationRef = useRef(0);
  const pendingPlayRef = useRef<{ generation: number; promise: Promise<void> } | null>(null);
  const desiredPlayingRef = useRef(playing);
  const latestRef = useRef({ playing, sourceTimeSeconds, gain, playbackRate });
  if (identityRef.current !== identity) {
    identityRef.current = identity;
    generationRef.current += 1;
    pendingPlayRef.current = null;
  }
  desiredPlayingRef.current = playing;
  latestRef.current = { playing, sourceTimeSeconds, gain, playbackRate };

  function synchronizeClock() {
    const element = elementRef.current;
    if (!element) return;
    const latest = latestRef.current;
    if (mediaElementNeedsSeek({ currentTime: element.currentTime, targetSeconds: latest.sourceTimeSeconds, playing: latest.playing })) {
      element.currentTime = latest.sourceTimeSeconds;
    }
    if (latest.gain !== undefined) element.volume = mediaElementVolume(latest.gain);
    applyPlaybackRate(element, latest.playbackRate);
  }

  function synchronizeTransport() {
    const element = elementRef.current;
    if (!element) return;
    if (!desiredPlayingRef.current) {
      if (!element.paused) element.pause();
      return;
    }
    const generation = generationRef.current;
    if (!element.paused || pendingPlayRef.current?.generation === generation) return;
    const result = element.play() as Promise<void> | undefined;
    if (!result) return;
    const pending: Promise<void> = Promise.resolve(result)
      .catch(() => undefined)
      .then(() => {
        if (!desiredPlayingRef.current && !element.paused) element.pause();
      })
      .finally(() => {
        if (pendingPlayRef.current?.promise === pending) pendingPlayRef.current = null;
      });
    pendingPlayRef.current = { generation, promise: pending };
  }

  useEffect(synchronizeClock, [playing, sourceTimeSeconds, gain, playbackRate, elementRef]);
  useEffect(synchronizeTransport, [playing, itemId, sourceUrl, elementRef]);
  useEffect(
    () => () => {
      desiredPlayingRef.current = false;
      const element = elementRef.current;
      if (element && !element.paused) element.pause();
    },
    [elementRef],
  );

  return () => {
    synchronizeClock();
    synchronizeTransport();
  };
}

function applyPlaybackRate(element: PitchPreservingMediaElement, playbackRate: number) {
  const rate = Number.isFinite(playbackRate) && playbackRate > 0 ? playbackRate : 1;
  if (element.playbackRate !== rate) element.playbackRate = rate;
  element.preservesPitch = true;
  // Older WebKit engines only expose the prefixed flag.
  if ("webkitPreservesPitch" in element) element.webkitPreservesPitch = true;
}
