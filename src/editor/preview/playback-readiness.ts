/** Only active preview elements participate: thumbnails and future requests cannot stall it. */
export function previewPlaybackReady(root: HTMLElement | null): boolean {
  if (!root || root.matches('[data-preview-buffering="true"]') || root.querySelector('[data-preview-buffering="true"]')) return false;
  for (const element of root.querySelectorAll<HTMLMediaElement>("video, audio")) {
    if ((element.readyState < HTMLMediaElement.HAVE_FUTURE_DATA && !element.ended) || element.seeking) return false;
  }
  for (const element of root.querySelectorAll<HTMLImageElement>("img")) {
    if (!element.complete || element.naturalWidth === 0) return false;
  }
  return true;
}

/** Keep decoded audio/video aligned with a composition waiting on another layer. */
export function pausePreviewMedia(root: HTMLElement | null): void {
  for (const element of root?.querySelectorAll<HTMLMediaElement>("video, audio") ?? []) {
    if (!element.paused) element.pause();
  }
}

/** Resume once the whole composition is ready; late promises must respect current intent. */
export function resumePreviewMedia(root: HTMLElement | null, wantsPlayback: () => boolean): void {
  for (const element of root?.querySelectorAll<HTMLMediaElement>("video, audio") ?? []) {
    if (!element.paused || element.ended) continue;
    const source = element.getAttribute("src");
    const reconcile = () => {
      // A changed URL belongs to the media synchronization hook's next request.
      if (element.getAttribute("src") !== source) return;
      if ((!element.isConnected || !wantsPlayback()) && !element.paused) element.pause();
    };
    void element.play().then(reconcile, reconcile);
  }
}
