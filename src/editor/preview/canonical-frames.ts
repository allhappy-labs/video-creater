import { previewUrlForMedia } from "@/lib/media/preview-source";
import type { PreparedProjectPreview, VideoProject } from "@/lib/project";
import type { TimelinePreviewFrame, TimelinePreviewLayer } from "@/lib/timeline-preview";
import type { CanonicalPreparation } from "../store/playback-slice";

export interface CanonicalFrameSequence {
  readonly itemId: string;
  readonly preparedMediaId: string;
  readonly startSeconds: number;
  readonly durationSeconds: number;
  readonly fps: number;
  readonly frameUrls: readonly (string | null)[];
}

export interface CanonicalFrameLayer {
  readonly layer: TimelinePreviewLayer;
  readonly frameUrl: string;
}

/** Every source layer stays below preview chrome inside the isolated canvas. */
export function previewLayerStackingOrder(frame: TimelinePreviewFrame): Readonly<Record<string, number>> {
  return Object.fromEntries(frame.layers.map((layer, index) => [layer.itemId, -2 * frame.layers.length + 2 * index + 1]));
}

/** What the compositor needs to know about canonical preparation for the current project. */
export type CompositorCanonicalState =
  | { readonly status: "pending" }
  | { readonly status: "failed"; readonly message: string }
  | { readonly status: "ready" }
  | null;

/** Frames preloaded behind and ahead of the current frame of each active sequence. */
const preloadFramesBehind = 2;
const preloadFramesAhead = 12;
const maximumLoadedFrameUrls = 480;

/**
 * Canonical state for `project`. A preparation only counts for the exact project object it was
 * started for; a stale one reads as pending while the project still needs preparation. A missing
 * backend (`unavailable`) keeps the preview DOM-only.
 */
export function canonicalStateForProject(
  preparation: CanonicalPreparation | null,
  project: VideoProject,
  projectNeedsCanonical: boolean,
): CompositorCanonicalState {
  if (preparation?.sourceProject === project) {
    switch (preparation.status) {
      case "preparing":
        return { status: "pending" };
      case "failed":
        return { status: "failed", message: preparation.message };
      case "ready":
        return { status: "ready" };
      case "unavailable":
        return null;
    }
  }
  return projectNeedsCanonical ? { status: "pending" } : null;
}

/** The prepared result for `project`, only when it was prepared from that exact project object. */
export function preparedResultForProject(
  preparation: CanonicalPreparation | null,
  project: VideoProject,
): PreparedProjectPreview | null {
  return preparation?.sourceProject === project && preparation.status === "ready" ? preparation.result : null;
}

/** Frame sequences with project-relative frame paths turned into preview URLs (unresolvable ones dropped). */
export function canonicalFrameSequences(result: PreparedProjectPreview, projectDir: string): CanonicalFrameSequence[] {
  return result.frameSequences.map((sequence) => ({
    itemId: sequence.itemId,
    preparedMediaId: sequence.preparedMediaId,
    startSeconds: sequence.startSeconds,
    durationSeconds: sequence.durationSeconds,
    fps: sequence.fps,
    frameUrls: sequence.framePaths.map((path) => previewUrlForMedia(projectDir, path)),
  }));
}

/** The prepared frame's audio layers with preview URLs for their prepared media (reversed audio plays these). */
export function preparedAudioLayers(preparedFrame: TimelinePreviewFrame | null, projectDir: string) {
  return (preparedFrame?.audioLayers ?? []).flatMap((layer) => {
    const sourceUrl = previewUrlForMedia(projectDir, layer.relativePath);
    return sourceUrl ? [{ layer, sourceUrl }] : [];
  });
}

function activeFrameIndex(sequence: CanonicalFrameSequence, seconds: number): number | null {
  const localSeconds = seconds - sequence.startSeconds;
  if (localSeconds < 0 || localSeconds >= sequence.durationSeconds || sequence.fps <= 0 || sequence.frameUrls.length === 0) {
    return null;
  }
  return Math.min(sequence.frameUrls.length - 1, Math.max(0, Math.floor(localSeconds * sequence.fps)));
}

/** For each prepared layer, the frame of the sequence covering `seconds` for its prepared media. */
export function canonicalFrameLayers(
  preparedFrame: TimelinePreviewFrame | null,
  sequences: readonly CanonicalFrameSequence[],
  seconds: number,
): CanonicalFrameLayer[] {
  return (preparedFrame?.layers ?? []).flatMap((layer) => {
    for (const sequence of sequences) {
      if (sequence.preparedMediaId !== layer.mediaId) continue;
      const index = activeFrameIndex(sequence, seconds);
      const frameUrl = index === null ? undefined : sequence.frameUrls[index];
      if (frameUrl) return [{ layer, frameUrl }];
    }
    return [];
  });
}

/**
 * Item ids the canonical frames stand in for. A flattened layer ("flatten-…") covers every direct
 * layer that needs preparation but has no prepared layer of its own, the clips beneath it that the
 * composite includes (`flattenedCoverItemIds`), and both clips of every transition baked into it
 * (`flattened`): the composite draws them, and the dip solid, across the whole window it was
 * widened to.
 */
export function canonicalCoverageItemIds(
  frameLayers: readonly CanonicalFrameLayer[],
  preparedFrame: TimelinePreviewFrame | null,
  directFrame: TimelinePreviewFrame,
): ReadonlySet<string> {
  const covered = new Set(frameLayers.map(({ layer }) => layer.itemId));
  if (![...covered].some((itemId) => itemId.startsWith("flatten-"))) return covered;
  const preparedIds = new Set(preparedFrame?.layers.map((layer) => layer.itemId) ?? []);
  for (const layer of directFrame.layers) {
    if (layer.canonicalPreparationRequired && !preparedIds.has(layer.itemId)) covered.add(layer.itemId);
  }
  for (const itemId of directFrame.flattenedCoverItemIds ?? []) covered.add(itemId);
  for (const transition of directFrame.transitions ?? []) {
    if (!transition.flattened) continue;
    covered.add(transition.leftItemId);
    covered.add(transition.rightItemId);
  }
  return covered;
}

/** Frame URLs to warm around `seconds`: two frames behind to twelve ahead in every active sequence. */
export function canonicalPreloadUrls(sequences: readonly CanonicalFrameSequence[], seconds: number): string[] {
  return sequences.flatMap((sequence) => {
    const index = activeFrameIndex(sequence, seconds);
    if (index === null) return [];
    const first = Math.max(0, index - preloadFramesBehind);
    const last = Math.min(sequence.frameUrls.length - 1, index + preloadFramesAhead);
    return sequence.frameUrls.slice(first, last + 1).filter((url): url is string => Boolean(url));
  });
}

/** Decodes frames ahead of display. Loaded URLs and pending images are each capped at 480. */
export class CanonicalFramePreloader {
  private readonly loaded = new Set<string>();
  private readonly pending = new Map<string, HTMLImageElement>();

  constructor(private readonly createImage: () => HTMLImageElement = () => new Image()) {}

  preload(urls: readonly string[]): void {
    for (const url of urls) {
      if (this.loaded.has(url) || this.pending.has(url)) continue;
      if (this.pending.size >= maximumLoadedFrameUrls) break;
      const image = this.createImage();
      image.decoding = "async";
      const markLoaded = () => this.markLoaded(url, image);
      image.onload = markLoaded;
      image.onerror = () => { if (this.pending.get(url) === image) this.pending.delete(url); };
      this.pending.set(url, image);
      image.src = url;
      if (typeof image.decode === "function") void image.decode().then(markLoaded, () => undefined);
    }
  }

  isLoaded(url: string): boolean {
    return this.loaded.has(url);
  }

  clear(): void {
    this.loaded.clear();
    for (const image of this.pending.values()) {
      image.onload = null;
      image.onerror = null;
      image.removeAttribute("src");
    }
    this.pending.clear();
  }

  private markLoaded(url: string, image: HTMLImageElement): void {
    if (this.pending.get(url) !== image) return;
    this.pending.delete(url);
    this.loaded.add(url);
    while (this.loaded.size > maximumLoadedFrameUrls) {
      const oldest = this.loaded.values().next().value;
      if (oldest === undefined) break;
      this.loaded.delete(oldest);
    }
  }
}
