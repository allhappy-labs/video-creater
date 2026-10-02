import { Film, Image as ImageIcon, Sparkles } from "lucide-react";
import { useState } from "react";
import { mediaContentKind } from "@/lib/media/media-filters";
import { previewUrlForMedia } from "@/lib/media/preview-source";
import { useMediaReadiness } from "@/lib/media/use-media-readiness";
import { refreshRemoteMediaUrl } from "@/lib/runtime/adapters/remote-resource-cache";
import type { MediaAsset } from "@/lib/project";

/** Legacy deterministic waveform: 18 bars seeded from the id and duration. */
function waveformBars(media: MediaAsset): number[] {
  const seed = Array.from(media.id).reduce((total, character) => total + character.charCodeAt(0), Math.round(media.durationSeconds * 100));
  return Array.from({ length: 18 }, (_, index) =>
    Math.round((0.2 + Math.abs(Math.sin((seed + index * 13) * 0.36) * Math.cos((seed + index * 5) * 0.24)) * 0.78) * 100),
  );
}

/**
 * A square media thumbnail: the first video frame or the image itself when the file loads, a
 * waveform for audio, and a kind glyph otherwise (or after a load error).
 */
export function MediaThumbnail({ media, projectDir }: { readonly media: MediaAsset; readonly projectDir: string }) {
  useMediaReadiness(projectDir);
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const kind = mediaContentKind(media);
  const previewUrl = previewUrlForMedia(projectDir, media.relativePath);
  const url = previewUrl === failedUrl ? null : previewUrl;

  if (kind === "audio") {
    return (
      <span aria-hidden className="absolute inset-0 flex items-center gap-0.5 bg-raised px-3">
        {waveformBars(media).map((height, index) => (
          <span key={index} className="min-w-[2px] flex-1 rounded-full bg-clip-audio/80" style={{ height: `${height}%` }} />
        ))}
      </span>
    );
  }
  if (url && kind === "video") {
    return (
      <video
        aria-hidden
        muted
        playsInline
        preload="metadata"
        src={`${url}#t=0.1`}
        onError={() => { setFailedUrl(url); refreshRemoteMediaUrl(url); }}
        className="pointer-events-none absolute inset-0 h-full w-full bg-background object-cover"
      />
    );
  }
  if (url && kind === "image") {
    return (
      <img
        alt=""
        draggable={false}
        src={url}
        onError={() => { setFailedUrl(url); refreshRemoteMediaUrl(url); }}
        className="pointer-events-none absolute inset-0 h-full w-full bg-background object-cover"
      />
    );
  }
  const Icon = kind === "image" ? ImageIcon : kind === "lottie" || media.kind === "generated" ? Sparkles : Film;
  return (
    <span aria-hidden className="absolute inset-0 grid place-items-center bg-raised text-dim">
      <Icon className="h-6 w-6" />
    </span>
  );
}
