import { useEffect, useRef, useState } from "react";
import { previewUrlForMedia } from "@/lib/media/preview-source";
import { cacheTimelineFilmstripInSplitProjectFolder, type TimelineFilmstripReport } from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { TimelineItem } from "@/lib/timeline";
import { numberProperty, timelineItemSourceMediaId } from "@/lib/timeline-ops/item-properties";
import { isReversedItem } from "@/lib/timeline-ops/reverse";
import { useEditorStore } from "../store/editor-store-context";

interface FilmstripFrame {
  readonly timeSeconds: number;
  readonly url: string;
}

type FilmstripInput = Parameters<typeof cacheTimelineFilmstripInSplitProjectFolder>[0];

/**
 * Session cache of filmstrip requests keyed per clip and zoom/height bucket, so scrolling a
 * clip out of the render window and back never requests it again. Failed requests resolve to
 * `null` (solid fill); only non-connectivity failures are forgotten so a later mount can retry.
 */
const filmstripRequests = new Map<string, Promise<readonly FilmstripFrame[] | null>>();

/** Frames with a loadable URL, or `null` when none remain. */
function framesFromReport(projectDir: string, report: TimelineFilmstripReport): readonly FilmstripFrame[] | null {
  const frames = report.frames.flatMap((frame) => {
    const url = previewUrlForMedia(projectDir, frame.relativePath);
    return url ? [{ timeSeconds: frame.timeSeconds, url }] : [];
  });
  return frames.length > 0 ? frames : null;
}

async function fetchFilmstrip(key: string, input: FilmstripInput): Promise<readonly FilmstripFrame[] | null> {
  try {
    return framesFromReport(input.projectDir, await cacheTimelineFilmstripInSplitProjectFolder(input));
  } catch (error) {
    if (!isBackendUnavailableError(error)) filmstripRequests.delete(key);
    return null;
  }
}

function loadFilmstrip(key: string, input: FilmstripInput): Promise<readonly FilmstripFrame[] | null> {
  const cached = filmstripRequests.get(key);
  if (cached) return cached;
  const request = fetchFilmstrip(key, input);
  filmstripRequests.set(key, request);
  return request;
}

function usesBackendMedia(projectDir: string): boolean {
  const trimmed = projectDir.trim();
  return trimmed.length > 0 && !trimmed.startsWith("browser://");
}

/** The filmstrip cache lives in split project folders (schema 2+), like `applyActions` persistence. */
const minimumSplitFolderSchemaVersion = 2;

function filmstripRequest(
  projectDir: string,
  schemaVersion: number,
  item: TimelineItem,
  zoomPercent: number,
  height: number,
  width: number,
): { readonly key: string; readonly input: FilmstripInput } | null {
  const mediaId = timelineItemSourceMediaId(item);
  if (item.kind !== "video_clip" || !mediaId || !usesBackendMedia(projectDir)) return null;
  if (schemaVersion < minimumSplitFolderSchemaVersion) return null;
  const speed = Math.max(0.01, numberProperty(item, "speed") ?? 1);
  const sourceIn = numberProperty(item, "sourceIn") ?? 0;
  const sourceOut = numberProperty(item, "sourceOut") ?? sourceIn + item.durationSeconds * speed;
  const zoomBucket = Math.max(25, Math.round(zoomPercent / 25) * 25);
  const heightBucket = Math.round(height);
  const input = { projectDir, mediaId, sourceIn, sourceOut, speed, zoomBucket, heightBucket, clipPixelWidth: Math.round(width) };
  const key = [projectDir, item.id, mediaId, sourceIn, sourceOut, speed, zoomBucket, heightBucket].join("|");
  return { key, input };
}

interface ClipFilmstripProps {
  readonly item: TimelineItem;
  readonly width: number;
  readonly height: number;
}

/**
 * Thumbnails behind a video or image clip (last to first for a reversed clip); renders nothing over
 * the solid clip fill when unavailable.
 */
export function ClipFilmstrip({ item, width, height }: ClipFilmstripProps) {
  const projectDir = useEditorStore((state) => state.projectDir);
  const zoomPercent = useEditorStore((state) => state.zoomPercent);
  const schemaVersion = useEditorStore((state) => state.project.schemaVersion);
  const imagePath = useEditorStore((state) => {
    if (item.kind !== "image_clip") return null;
    const mediaId = timelineItemSourceMediaId(item);
    return state.project.media.find((media) => media.id === mediaId)?.relativePath ?? null;
  });
  const request = filmstripRequest(projectDir, schemaVersion, item, zoomPercent, height, width);
  const requestKey = request?.key ?? null;
  const latestInput = useRef(request?.input);
  latestInput.current = request?.input;
  const [loaded, setLoaded] = useState<{ readonly key: string; readonly frames: readonly FilmstripFrame[] | null } | null>(null);
  const [brokenKey, setBrokenKey] = useState<string | null>(null);

  useEffect(() => {
    const input = latestInput.current;
    if (!requestKey || !input) return undefined;
    let cancelled = false;
    void loadFilmstrip(requestKey, input).then((frames) => {
      if (!cancelled) setLoaded({ key: requestKey, frames });
    });
    return () => {
      cancelled = true;
    };
  }, [requestKey]);

  const frames = loaded && loaded.key === requestKey && brokenKey !== requestKey ? loaded.frames : null;
  if (frames && requestKey) {
    return (
      <div data-testid="clip-filmstrip" data-state="frames" aria-hidden className="pointer-events-none absolute inset-0 flex">
        {(isReversedItem(item) ? [...frames].reverse() : frames).map((frame) => (
          <img
            key={frame.timeSeconds}
            alt=""
            draggable={false}
            src={frame.url}
            onError={() => setBrokenKey(requestKey)}
            className="h-full min-w-0 flex-1 object-cover"
          />
        ))}
      </div>
    );
  }

  const imageUrl = imagePath && usesBackendMedia(projectDir) ? previewUrlForMedia(projectDir, imagePath) : null;
  if (imageUrl) {
    return (
      <div
        data-testid="clip-filmstrip"
        data-state="image"
        aria-hidden
        className="pointer-events-none absolute inset-0 bg-repeat-x"
        style={{ backgroundImage: `url(${JSON.stringify(imageUrl)})`, backgroundSize: "auto 100%" }}
      />
    );
  }

  return <div data-testid="clip-filmstrip" data-state="fallback" aria-hidden className="pointer-events-none absolute inset-0" />;
}
