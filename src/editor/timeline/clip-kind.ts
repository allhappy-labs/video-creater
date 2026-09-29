import { Captions, Film, Music, Sparkles, Type, type LucideIcon } from "lucide-react";
import { isTemplateTimelineItem, type TimelineItem, type TrackKind } from "@/lib/timeline";

/** Visual family of a clip; each maps to one `--clip-*` token. */
export type ClipTone = "video" | "text" | "caption" | "audio" | "graphics";

export function clipTone(item: TimelineItem): ClipTone {
  switch (item.kind) {
    case "video_clip":
    case "image_clip":
    case "lottie_clip":
    case "generated_clip":
      return "video";
    case "caption":
      return "caption";
    case "audio_clip":
      return "audio";
    case "hyperframe_scene":
      return "graphics";
    case "overlay":
      return isTemplateTimelineItem(item) ? "graphics" : "text";
  }
}

/** Literal class names so Tailwind picks them up. */
export const clipFillClass: Readonly<Record<ClipTone, string>> = {
  video: "bg-clip-video",
  text: "bg-clip-text",
  caption: "bg-clip-caption",
  audio: "bg-clip-audio",
  graphics: "bg-clip-graphics",
};

export const clipToneIcon: Readonly<Record<ClipTone, LucideIcon>> = {
  video: Film,
  text: Type,
  caption: Captions,
  audio: Music,
  graphics: Sparkles,
};

export const trackKindIcon: Readonly<Record<TrackKind, LucideIcon>> = {
  video: Film,
  overlay: Type,
  caption: Captions,
  hyperframe_scene: Sparkles,
  audio: Music,
};
